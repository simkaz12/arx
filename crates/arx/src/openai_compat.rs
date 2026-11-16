//! The OpenAI-compatible adapter of L1-10: one `chat/completions` request, its SSE stream
//! folded into `Chunk`s at the seam `72bfc6e9` draws.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, Read};
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::provider::{Cancel, ChunkStream, Chunk, ContentBlock, Message, Overshoot, Provider, ProviderError, Role, Termination, ToolDef};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const ERROR_BODY_CAP: u64 = 2000;

pub(crate) struct OpenAiCompatible {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
    cancel: Cancel,
}

impl OpenAiCompatible {
    pub(crate) fn new(base_url: &str, api_key: String, model: &str, cancel: Cancel) -> OpenAiCompatible {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("a client with only timeouts configured always builds");
        OpenAiCompatible {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model: model.to_string(),
            client,
            cancel,
        }
    }
}

fn join_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn has_text(content: &[ContentBlock]) -> bool {
    content.iter().any(|block| matches!(block, ContentBlock::Text(_)))
}

pub(crate) fn build_request_body(model: &str, system: &str, messages: &[Message], tools: &[ToolDef]) -> serde_json::Value {
    let mut wire_messages = vec![serde_json::json!({"role": "system", "content": system})];

    for message in messages {
        match message.role {
            Role::User => {
                wire_messages.push(serde_json::json!({
                    "role": "user",
                    "content": join_text(&message.content),
                }));
            }
            Role::Assistant => {
                let content = if has_text(&message.content) {
                    serde_json::Value::String(join_text(&message.content))
                } else {
                    serde_json::Value::Null
                };
                let tool_calls: Vec<serde_json::Value> = message
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::ToolCall { id, name, arguments } => Some(serde_json::json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": arguments.to_string(),
                            }
                        })),
                        _ => None,
                    })
                    .collect();
                let mut entry = serde_json::json!({"role": "assistant", "content": content});
                if !tool_calls.is_empty() {
                    entry["tool_calls"] = serde_json::Value::Array(tool_calls);
                }
                wire_messages.push(entry);
            }
            Role::Tool => {
                for block in &message.content {
                    if let ContentBlock::ToolResult { call_id, content } = block {
                        wire_messages.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": call_id,
                            "content": content,
                        }));
                    }
                }
            }
        }
    }

    let mut body = serde_json::json!({
        "model": model,
        "messages": wire_messages,
        "stream": true,
        "stream_options": {"include_usage": true},
    });

    if !tools.is_empty() {
        let wire_tools: Vec<serde_json::Value> = tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                    }
                })
            })
            .collect();
        body["tools"] = serde_json::Value::Array(wire_tools);
    }

    body
}

fn redact_key(text: &str, key: &str) -> String {
    if key.is_empty() {
        text.to_string()
    } else {
        text.replace(key, "[redacted]")
    }
}

fn first_integer_after(body: &str, marker: &str) -> Option<u64> {
    let start = body.find(marker)? + marker.len();
    let rest = &body[start..];
    let digits: String = rest.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<u64>().ok()
    }
}

fn parse_overshoot_openai(body: &str) -> Option<Overshoot> {
    let maximum = first_integer_after(body, "maximum context length is")?;
    let requested = first_integer_after(body, "requested").or_else(|| first_integer_after(body, "resulted in"))?;
    Some(Overshoot { requested, maximum })
}

fn last_integer_in(text: &str) -> Option<u64> {
    let mut result = None;
    let mut current = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            current.push(c);
        } else if !current.is_empty() {
            result = current.parse::<u64>().ok();
            current.clear();
        }
    }
    if !current.is_empty() {
        result = current.parse::<u64>().ok();
    }
    result
}

fn first_integer_in(text: &str) -> Option<u64> {
    let digits: String = text.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<u64>().ok()
    }
}

fn parse_overshoot_anthropic(body: &str) -> Option<Overshoot> {
    let split = body.find('>')?;
    let (left, right) = body.split_at(split);
    let requested = last_integer_in(left)?;
    let maximum = first_integer_in(&right[1..])?;
    Some(Overshoot { requested, maximum })
}

pub(crate) fn parse_overshoot(body: &str) -> Option<Overshoot> {
    parse_overshoot_openai(body).or_else(|| parse_overshoot_anthropic(body))
}

fn classify_error(status: u16, retry_after: Option<Duration>, body: &str, url: &str) -> ProviderError {
    if status == 429 {
        return ProviderError::Transient {
            kind: "429".to_string(),
            retry_after,
        };
    }
    if (500..=599).contains(&status) {
        return ProviderError::Transient {
            kind: status.to_string(),
            retry_after,
        };
    }
    if status == 400 && (body.contains("context_length_exceeded") || body.contains("maximum context length")) {
        return ProviderError::ContextWindowExceeded {
            overshoot: parse_overshoot(body),
        };
    }
    ProviderError::Fatal {
        kind: format!("HTTP {status} from {url}: {body}"),
    }
}

impl Provider for OpenAiCompatible {
    fn complete(&self, system: &str, messages: &[Message], tools: &[ToolDef]) -> Result<ChunkStream, ProviderError> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = build_request_body(&self.model, system, messages, tools);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&body)
            .send()
            .map_err(|_| ProviderError::Transient {
                kind: "transport".to_string(),
                retry_after: None,
            })?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            let mut tail = Vec::new();
            let _ = response.take(ERROR_BODY_CAP).read_to_end(&mut tail);
            let tail_text = redact_key(&String::from_utf8_lossy(&tail), &self.api_key);
            return Err(classify_error(status.as_u16(), retry_after, &tail_text, &url));
        }

        let reader = std::io::BufReader::new(response);
        let stream = chunks_from_reader(reader, self.cancel.clone());
        Ok(Box::new(RedactingStream {
            inner: stream,
            key: self.api_key.clone(),
        }))
    }
}

pub(crate) struct RedactingStream<I> {
    inner: I,
    key: String,
}

impl<I> RedactingStream<I> {
    pub(crate) fn new(inner: I, key: String) -> RedactingStream<I> {
        RedactingStream { inner, key }
    }
}

impl<I: Iterator<Item = Result<Chunk, ProviderError>>> Iterator for RedactingStream<I> {
    type Item = Result<Chunk, ProviderError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| match item {
            Ok(chunk) => Ok(chunk),
            Err(ProviderError::Fatal { kind }) => Err(ProviderError::Fatal {
                kind: redact_key(&kind, &self.key),
            }),
            Err(other) => Err(other),
        })
    }
}

struct PendingToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

struct SseChunks<R> {
    reader: R,
    queue: VecDeque<Result<Chunk, ProviderError>>,
    finished: bool,
    closed: bool,
    data_lines: Vec<String>,
    pending_tool_calls: BTreeMap<u64, PendingToolCall>,
    any_tool_calls: bool,
    finish_reason: Option<String>,
    usage: Option<serde_json::Value>,
    cancel: Cancel,
}

impl<R: BufRead> SseChunks<R> {
    fn new(reader: R, cancel: Cancel) -> SseChunks<R> {
        SseChunks {
            reader,
            queue: VecDeque::new(),
            finished: false,
            closed: false,
            data_lines: Vec::new(),
            pending_tool_calls: BTreeMap::new(),
            any_tool_calls: false,
            finish_reason: None,
            usage: None,
            cancel,
        }
    }

    fn apply_tool_call_fragments(&mut self, fragments: &[serde_json::Value]) {
        for fragment in fragments {
            let index = fragment.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
            let entry = self.pending_tool_calls.entry(index).or_insert_with(|| PendingToolCall {
                id: None,
                name: None,
                arguments: String::new(),
            });
            if let Some(id) = fragment.get("id").and_then(|v| v.as_str()) {
                entry.id = Some(id.to_string());
            }
            if let Some(name) = fragment.get("function").and_then(|f| f.get("name")).and_then(|v| v.as_str()) {
                entry.name = Some(name.to_string());
            }
            if let Some(args) = fragment.get("function").and_then(|f| f.get("arguments")).and_then(|v| v.as_str()) {
                entry.arguments.push_str(args);
            }
        }
    }

    fn flush_tool_calls(&mut self) {
        let pending = std::mem::take(&mut self.pending_tool_calls);
        for (_, call) in pending {
            let arguments = serde_json::from_str::<serde_json::Value>(&call.arguments)
                .unwrap_or_else(|_| serde_json::Value::String(call.arguments.clone()));
            self.queue.push_back(Ok(Chunk::Content(ContentBlock::ToolCall {
                id: call.id.unwrap_or_default(),
                name: call.name.unwrap_or_default(),
                arguments,
            })));
            self.any_tool_calls = true;
        }
    }

    fn apply_event(&mut self, value: &serde_json::Value) {
        if let Some(usage) = value.get("usage") {
            if !usage.is_null() {
                self.usage = Some(usage.clone());
            }
        }

        let Some(choice) = value.get("choices").and_then(|c| c.get(0)) else {
            return;
        };

        if let Some(delta) = choice.get("delta") {
            if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                if !content.is_empty() {
                    self.queue.push_back(Ok(Chunk::Content(ContentBlock::Text(content.to_string()))));
                }
            }
            if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                self.apply_tool_call_fragments(tool_calls);
            }
        }

        if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
            self.finish_reason = Some(reason.to_string());
            if !self.pending_tool_calls.is_empty() {
                self.flush_tool_calls();
            }
        }
    }

    fn map_termination(reason: &str, any_tool_calls: bool) -> Termination {
        match reason {
            "stop" if any_tool_calls => Termination::ToolUse,
            "stop" => Termination::EndTurn,
            "tool_calls" => Termination::ToolUse,
            "length" => Termination::MaxTokens,
            other => Termination::Refusal {
                category: other.to_string(),
            },
        }
    }

    fn finalize_stream(&mut self) {
        if !self.pending_tool_calls.is_empty() {
            self.flush_tool_calls();
        }
        match self.finish_reason.take() {
            Some(reason) => {
                let termination = Self::map_termination(&reason, self.any_tool_calls);
                self.queue.push_back(Ok(Chunk::Terminal {
                    termination,
                    usage: self.usage.clone(),
                }));
            }
            None => {
                self.queue.push_back(Err(ProviderError::Fatal {
                    kind: "stream ended without finish_reason".to_string(),
                }));
            }
        }
        self.closed = true;
    }

    fn dispatch_event(&mut self) {
        if self.data_lines.is_empty() {
            return;
        }
        let payload = self.data_lines.join("\n");
        self.data_lines.clear();

        if payload == "[DONE]" {
            self.finalize_stream();
            return;
        }

        match serde_json::from_str::<serde_json::Value>(&payload) {
            Ok(value) => self.apply_event(&value),
            Err(_) => {
                let chars: Vec<char> = payload.chars().collect();
                let start = chars.len().saturating_sub(200);
                let tail: String = chars[start..].iter().collect();
                self.queue.push_back(Err(ProviderError::Fatal {
                    kind: format!("malformed SSE data: {tail}"),
                }));
                self.closed = true;
            }
        }
    }

    fn handle_line(&mut self, line: &str) {
        if line.is_empty() {
            self.dispatch_event();
            return;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let value = rest.strip_prefix(' ').unwrap_or(rest);
            self.data_lines.push(value.to_string());
        }
    }

    fn handle_eof(&mut self) {
        if !self.data_lines.is_empty() {
            self.dispatch_event();
        }
        if !self.closed {
            self.finalize_stream();
        }
    }
}

impl<R: BufRead> Iterator for SseChunks<R> {
    type Item = Result<Chunk, ProviderError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.finished {
                return None;
            }
            if self.cancel.load(Ordering::SeqCst) {
                self.finished = true;
                return Some(Err(ProviderError::Fatal { kind: "killed".to_string() }));
            }
            if let Some(item) = self.queue.pop_front() {
                if matches!(item, Err(_)) || matches!(item, Ok(Chunk::Terminal { .. })) {
                    self.finished = true;
                }
                return Some(item);
            }

            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => {
                    self.handle_eof();
                }
                Ok(_) => {
                    let trimmed = line.trim_end_matches(['\n', '\r']).to_string();
                    self.handle_line(&trimmed);
                }
                Err(err) => {
                    self.queue.push_back(Err(ProviderError::Fatal {
                        kind: format!("stream read error: {err}"),
                    }));
                    self.closed = true;
                }
            }

            if self.cancel.load(Ordering::SeqCst) {
                self.finished = true;
                return Some(Err(ProviderError::Fatal { kind: "killed".to_string() }));
            }
        }
    }
}

pub(crate) fn chunks_from_reader(reader: impl BufRead + Send + 'static, cancel: Cancel) -> ChunkStream {
    Box::new(SseChunks::new(reader, cancel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::atomic::AtomicBool;

    fn unset_cancel() -> Cancel {
        std::sync::Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn redact_key_strips_every_occurrence_of_the_configured_secret() {
        let text = "HTTP 401 from https://api.example.com/chat/completions: bad Authorization: Bearer sk-secret-123";
        assert_eq!(
            redact_key(text, "sk-secret-123"),
            "HTTP 401 from https://api.example.com/chat/completions: bad Authorization: Bearer [redacted]"
        );
    }

    #[test]
    fn redact_key_is_a_no_op_on_an_empty_key() {
        let text = "some error text";
        assert_eq!(redact_key(text, ""), text);
    }

    #[test]
    fn redacting_stream_scrubs_the_key_from_a_stream_originated_fatal_error() {
        let raw = "data: not json and it echoes Bearer sk-secret-123 back at you\n\n";
        let inner = chunks_from_reader(Cursor::new(raw.as_bytes().to_vec()), unset_cancel());
        let mut stream = RedactingStream {
            inner,
            key: "sk-secret-123".to_string(),
        };
        match stream.next().unwrap() {
            Err(ProviderError::Fatal { kind: text }) => {
                assert!(!text.contains("sk-secret-123"), "text: {text}");
                assert!(text.contains("[redacted]"), "text: {text}");
            }
            other => panic!("expected a redacted Fatal error, got {other:?}"),
        }
    }

    fn sse(events: &[&str]) -> Cursor<Vec<u8>> {
        let mut buf = String::new();
        for event in events {
            buf.push_str("data: ");
            buf.push_str(event);
            buf.push_str("\n\n");
        }
        Cursor::new(buf.into_bytes())
    }

    fn collect(reader: Cursor<Vec<u8>>) -> Vec<Result<Chunk, ProviderError>> {
        chunks_from_reader(reader, unset_cancel()).collect()
    }

    #[test]
    fn text_fragments_yield_in_order() {
        let chunks = collect(sse(&[
            r#"{"choices":[{"delta":{"content":"Hello"}}]}"#,
            r#"{"choices":[{"delta":{"content":" world"}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ]));

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].as_ref().unwrap(), &Chunk::Content(ContentBlock::Text("Hello".to_string())));
        assert_eq!(chunks[1].as_ref().unwrap(), &Chunk::Content(ContentBlock::Text(" world".to_string())));
        assert_eq!(
            chunks[2].as_ref().unwrap(),
            &Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None
            }
        );
    }

    #[test]
    fn two_tool_calls_interleaved_by_index_are_assembled_and_parsed() {
        let chunks = collect(sse(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_0","type":"function","function":{"name":"read_file","arguments":""}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_1","type":"function","function":{"name":"read_file","arguments":""}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"pa"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"function":{"arguments":"{\"pa"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\": \"a"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"function":{"arguments":"th\": \"b"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":".txt\"}"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"function":{"arguments":".txt\"}"}}]}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
            "[DONE]",
        ]));

        assert_eq!(chunks.len(), 3);
        assert_eq!(
            chunks[0].as_ref().unwrap(),
            &Chunk::Content(ContentBlock::ToolCall {
                id: "call_0".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "a.txt"}),
            })
        );
        assert_eq!(
            chunks[1].as_ref().unwrap(),
            &Chunk::Content(ContentBlock::ToolCall {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "b.txt"}),
            })
        );
        assert_eq!(
            chunks[2].as_ref().unwrap(),
            &Chunk::Terminal {
                termination: Termination::ToolUse,
                usage: None
            }
        );
    }

    #[test]
    fn stop_maps_to_end_turn() {
        let chunks = collect(sse(&[r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#, "[DONE]"]));
        assert_eq!(
            chunks[0].as_ref().unwrap(),
            &Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None
            }
        );
    }

    #[test]
    fn tool_calls_reason_maps_to_tool_use() {
        let chunks = collect(sse(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c","function":{"name":"read_file","arguments":"{}"}}]}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
            "[DONE]",
        ]));
        let last = chunks.last().unwrap().as_ref().unwrap();
        assert_eq!(
            last,
            &Chunk::Terminal {
                termination: Termination::ToolUse,
                usage: None
            }
        );
    }

    #[test]
    fn stop_with_assembled_tool_calls_is_tool_use() {
        let chunks = collect(sse(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c","function":{"name":"read_file","arguments":"{}"}}]}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ]));
        let last = chunks.last().unwrap().as_ref().unwrap();
        assert_eq!(
            last,
            &Chunk::Terminal {
                termination: Termination::ToolUse,
                usage: None
            }
        );
    }

    #[test]
    fn length_maps_to_max_tokens() {
        let chunks = collect(sse(&[r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#, "[DONE]"]));
        assert_eq!(
            chunks[0].as_ref().unwrap(),
            &Chunk::Terminal {
                termination: Termination::MaxTokens,
                usage: None
            }
        );
    }

    #[test]
    fn content_filter_maps_to_refusal() {
        let chunks = collect(sse(&[r#"{"choices":[{"delta":{},"finish_reason":"content_filter"}]}"#, "[DONE]"]));
        assert_eq!(
            chunks[0].as_ref().unwrap(),
            &Chunk::Terminal {
                termination: Termination::Refusal {
                    category: "content_filter".to_string()
                },
                usage: None
            }
        );
    }

    #[test]
    fn unrecognized_finish_reason_is_refusal_with_the_raw_value() {
        let chunks = collect(sse(&[
            r#"{"choices":[{"delta":{},"finish_reason":"insufficient_system_resource"}]}"#,
            "[DONE]",
        ]));
        assert_eq!(
            chunks[0].as_ref().unwrap(),
            &Chunk::Terminal {
                termination: Termination::Refusal {
                    category: "insufficient_system_resource".to_string()
                },
                usage: None
            }
        );
    }

    #[test]
    fn usage_is_captured_from_the_final_chunk_and_null_usage_is_ignored() {
        let chunks = collect(sse(&[
            r#"{"choices":[{"delta":{"content":"hi"}}],"usage":null}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}],"usage":null}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}"#,
            "[DONE]",
        ]));
        let last = chunks.last().unwrap().as_ref().unwrap();
        assert_eq!(
            last,
            &Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: Some(serde_json::json!({"prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3}))
            }
        );
    }

    #[test]
    fn reasoning_content_and_role_deltas_are_ignored() {
        let chunks = collect(sse(&[
            r#"{"choices":[{"delta":{"role":"assistant"}}]}"#,
            r#"{"choices":[{"delta":{"reasoning_content":"thinking..."}}]}"#,
            r#"{"choices":[{"delta":{"content":"hi"}}]}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ]));
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].as_ref().unwrap(), &Chunk::Content(ContentBlock::Text("hi".to_string())));
    }

    #[test]
    fn keep_alive_comment_lines_are_ignored() {
        let raw = ": keep-alive\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
        let chunks: Vec<_> = chunks_from_reader(Cursor::new(raw.as_bytes().to_vec()), unset_cancel()).collect();
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0].as_ref().unwrap(),
            &Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None
            }
        );
    }

    #[test]
    fn malformed_data_line_is_fatal() {
        let chunks = collect(sse(&["not json at all"]));
        assert_eq!(chunks.len(), 1);
        match chunks[0].as_ref().unwrap_err() {
            ProviderError::Fatal { kind: text } => assert!(text.contains("malformed SSE data"), "text: {text}"),
            other => panic!("expected a malformed-SSE Fatal error, got {other:?}"),
        }
    }

    #[test]
    fn eof_without_finish_reason_is_fatal() {
        let raw = "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n";
        let mut stream = chunks_from_reader(Cursor::new(raw.as_bytes().to_vec()), unset_cancel());
        assert_eq!(stream.next().unwrap().unwrap(), Chunk::Content(ContentBlock::Text("hi".to_string())));
        match stream.next().unwrap().unwrap_err() {
            ProviderError::Fatal { kind: text } => assert!(text.contains("stream ended without finish_reason"), "text: {text}"),
            other => panic!("expected a finish_reason Fatal error, got {other:?}"),
        }
    }

    #[test]
    fn nothing_is_yielded_after_terminal() {
        let mut stream = chunks_from_reader(sse(&[r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#, "[DONE]"]), unset_cancel());
        assert!(matches!(stream.next(), Some(Ok(Chunk::Terminal { .. }))));
        assert!(stream.next().is_none());
        assert!(stream.next().is_none());
    }

    #[test]
    fn nothing_is_yielded_after_a_fatal_error() {
        let mut stream = chunks_from_reader(sse(&["not json"]), unset_cancel());
        assert!(matches!(stream.next(), Some(Err(ProviderError::Fatal { .. }))));
        assert!(stream.next().is_none());
    }

    #[test]
    fn a_set_cancel_flag_ends_the_stream_on_the_next_line() {
        let mut raw = ": keep-alive\n\n".repeat(10);
        raw.push_str("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n");
        let cancel = unset_cancel();
        let mut stream = chunks_from_reader(Cursor::new(raw.into_bytes()), std::sync::Arc::clone(&cancel));
        cancel.store(true, Ordering::SeqCst);

        match stream.next().unwrap().unwrap_err() {
            ProviderError::Fatal { kind: text } => assert_eq!(text, "killed"),
            other => panic!("expected a killed Fatal error, got {other:?}"),
        }
        assert!(stream.next().is_none());
    }

    fn user_message(text: &str) -> Message {
        Message::user(text)
    }

    fn assistant_with_tool_call() -> Message {
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolCall {
                id: "call_0".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "a.txt"}),
            }],
        }
    }

    fn tool_result_message() -> Message {
        Message {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                call_id: "call_0".to_string(),
                content: "file contents".to_string(),
            }],
        }
    }

    fn read_file_tool() -> ToolDef {
        ToolDef {
            name: "read_file".to_string(),
            description: "Read a UTF-8 text file relative to the working directory and return its whole content.".to_string(),
            parameters: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        }
    }

    #[test]
    fn system_is_first_and_user_brief_is_second() {
        let body = build_request_body("model-x", "system prompt", &[user_message("say the word")], &[]);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0], serde_json::json!({"role": "system", "content": "system prompt"}));
        assert_eq!(messages[1], serde_json::json!({"role": "user", "content": "say the word"}));
    }

    #[test]
    fn assistant_tool_calls_and_tool_results_translate() {
        let body = build_request_body(
            "model-x",
            "system",
            &[user_message("go"), assistant_with_tool_call(), tool_result_message()],
            &[],
        );
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(messages[2]["content"], serde_json::Value::Null);
        assert_eq!(messages[2]["tool_calls"][0]["id"], "call_0");
        assert_eq!(messages[2]["tool_calls"][0]["type"], "function");
        assert_eq!(messages[2]["tool_calls"][0]["function"]["name"], "read_file");
        assert_eq!(messages[2]["tool_calls"][0]["function"]["arguments"], serde_json::json!({"path":"a.txt"}).to_string());

        assert_eq!(
            messages[3],
            serde_json::json!({"role": "tool", "tool_call_id": "call_0", "content": "file contents"})
        );
    }

    #[test]
    fn tools_present_when_nonempty_and_absent_when_empty() {
        let with_tools = build_request_body("model-x", "system", &[], &[read_file_tool()]);
        assert_eq!(with_tools["tools"][0]["function"]["name"], "read_file");

        let without_tools = build_request_body("model-x", "system", &[], &[]);
        assert!(without_tools.get("tools").is_none());
    }

    #[test]
    fn stream_and_stream_options_are_set_with_no_max_tokens() {
        let body = build_request_body("model-x", "system", &[], &[]);
        assert_eq!(body["stream"], true);
        assert_eq!(body["stream_options"], serde_json::json!({"include_usage": true}));
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn a_429_is_transient_with_retry_after() {
        let err = classify_error(429, Some(Duration::from_secs(3)), "", "http://x");
        match err {
            ProviderError::Transient { kind, retry_after } => {
                assert_eq!(kind, "429");
                assert_eq!(retry_after, Some(Duration::from_secs(3)));
            }
            other => panic!("expected Transient, got {other:?}"),
        }
    }

    #[test]
    fn a_503_is_transient() {
        let err = classify_error(503, None, "", "http://x");
        match err {
            ProviderError::Transient { kind, retry_after } => {
                assert_eq!(kind, "503");
                assert_eq!(retry_after, None);
            }
            other => panic!("expected Transient, got {other:?}"),
        }
    }

    #[test]
    fn a_400_naming_the_context_length_is_a_miss_with_the_overshoot_parsed() {
        let body = "This model's maximum context length is 8192 tokens. However, you requested 10000 tokens";
        let err = classify_error(400, None, body, "http://x");
        match err {
            ProviderError::ContextWindowExceeded {
                overshoot: Some(overshoot),
            } => {
                assert_eq!(overshoot.requested, 10000);
                assert_eq!(overshoot.maximum, 8192);
            }
            other => panic!("expected ContextWindowExceeded with an overshoot, got {other:?}"),
        }
    }

    #[test]
    fn a_400_without_numbers_is_a_miss_with_no_overshoot() {
        let err = classify_error(400, None, "maximum context length exceeded", "http://x");
        assert!(matches!(err, ProviderError::ContextWindowExceeded { overshoot: None }), "err: {err:?}");
    }

    #[test]
    fn a_403_is_fatal() {
        let err = classify_error(403, None, "forbidden", "http://x");
        match err {
            ProviderError::Fatal { kind } => assert!(kind.contains("HTTP 403"), "kind: {kind}"),
            other => panic!("expected Fatal, got {other:?}"),
        }
    }

    #[test]
    fn parse_overshoot_reads_resulted_in_too() {
        let body = "maximum context length is 4096 tokens; this request resulted in 5000 tokens";
        let overshoot = parse_overshoot(body).expect("overshoot parses");
        assert_eq!(overshoot.maximum, 4096);
        assert_eq!(overshoot.requested, 5000);
    }
}
