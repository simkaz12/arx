//! The Anthropic adapter of L2-10c: `POST /v1/messages`, its SSE stream folded into `Chunk`s
//! at the seam `72bfc6e9` draws.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, Read};
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::openai_compat::{parse_overshoot, RedactingStream};
use crate::provider::{
    Cancel, Chunk, ChunkStream, ContentBlock, Message, Provider, ProviderError, Role, Termination, ToolDef,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const ERROR_BODY_CAP: u64 = 2000;
const MAX_TOKENS: u64 = 8192;
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub(crate) struct Anthropic {
    base_url: String,
    api_key: String,
    model: String,
    cancel: Cancel,
    client: reqwest::blocking::Client,
}

impl Anthropic {
    pub(crate) fn new(base_url: &str, api_key: String, model: &str, cancel: Cancel) -> Anthropic {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("a client with only timeouts configured always builds");
        Anthropic {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model: model.to_string(),
            cancel,
            client,
        }
    }
}

fn redact_key(text: &str, key: &str) -> String {
    if key.is_empty() {
        text.to_string()
    } else {
        text.replace(key, "[redacted]")
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

pub(crate) fn build_request_body(model: &str, system: &str, messages: &[Message], tools: &[ToolDef]) -> serde_json::Value {
    let mut wire_messages = Vec::new();

    for message in messages {
        match message.role {
            Role::User => {
                wire_messages.push(serde_json::json!({
                    "role": "user",
                    "content": [{"type": "text", "text": join_text(&message.content)}],
                }));
            }
            Role::Assistant => {
                let blocks: Vec<serde_json::Value> = message
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text(text) => Some(serde_json::json!({"type": "text", "text": text})),
                        ContentBlock::ToolCall { id, name, arguments } => Some(serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": arguments,
                        })),
                        ContentBlock::ToolResult { .. } => None,
                    })
                    .collect();
                wire_messages.push(serde_json::json!({"role": "assistant", "content": blocks}));
            }
            Role::Tool => {
                for block in &message.content {
                    if let ContentBlock::ToolResult { call_id, content } = block {
                        wire_messages.push(serde_json::json!({
                            "role": "user",
                            "content": [{"type": "tool_result", "tool_use_id": call_id, "content": content}],
                        }));
                    }
                }
            }
        }
    }

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": system,
        "messages": wire_messages,
    });

    if !tools.is_empty() {
        let wire_tools: Vec<serde_json::Value> = tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.parameters,
                })
            })
            .collect();
        body["tools"] = serde_json::Value::Array(wire_tools);
    }

    body
}

fn classify_error(status: u16, retry_after: Option<Duration>, body: &str) -> ProviderError {
    if status == 429 {
        return ProviderError::Transient {
            kind: "429".to_string(),
            retry_after,
        };
    }
    if status == 529 {
        return ProviderError::Transient {
            kind: "529".to_string(),
            retry_after,
        };
    }
    if (500..=599).contains(&status) {
        return ProviderError::Transient {
            kind: status.to_string(),
            retry_after,
        };
    }
    if status == 400 && body.contains("prompt is too long") {
        return ProviderError::ContextWindowExceeded {
            overshoot: parse_overshoot(body),
        };
    }
    ProviderError::Fatal {
        kind: format!("http {status}: {body}"),
    }
}

impl Provider for Anthropic {
    fn complete(&self, system: &str, messages: &[Message], tools: &[ToolDef]) -> Result<ChunkStream, ProviderError> {
        let url = format!("{}/v1/messages", self.base_url);
        let body = build_request_body(&self.model, system, messages, tools);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
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
            return Err(classify_error(status.as_u16(), retry_after, &tail_text));
        }

        let reader = std::io::BufReader::new(response);
        let stream = chunks_from_reader(reader, self.cancel.clone());
        Ok(Box::new(RedactingStream::new(stream, self.api_key.clone())))
    }
}

enum OpenBlock {
    Text,
    Tool { id: String, name: String, json: String },
    Dropped,
}

struct AnthropicChunks<R> {
    reader: R,
    queue: VecDeque<Result<Chunk, ProviderError>>,
    finished: bool,
    closed: bool,
    data_lines: Vec<String>,
    blocks: BTreeMap<u64, OpenBlock>,
    stop_reason: Option<String>,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_input_tokens: u64,
    cache_read_input_tokens: u64,
    cancel: Cancel,
}

fn as_u64(value: Option<&serde_json::Value>) -> u64 {
    value.and_then(|v| v.as_u64()).unwrap_or(0)
}

impl<R: BufRead> AnthropicChunks<R> {
    fn new(reader: R, cancel: Cancel) -> AnthropicChunks<R> {
        AnthropicChunks {
            reader,
            queue: VecDeque::new(),
            finished: false,
            closed: false,
            data_lines: Vec::new(),
            blocks: BTreeMap::new(),
            stop_reason: None,
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            cancel,
        }
    }

    fn on_message_start(&mut self, value: &serde_json::Value) {
        let usage = value.get("message").and_then(|m| m.get("usage"));
        self.input_tokens = as_u64(usage.and_then(|u| u.get("input_tokens")));
        self.cache_creation_input_tokens = as_u64(usage.and_then(|u| u.get("cache_creation_input_tokens")));
        self.cache_read_input_tokens = as_u64(usage.and_then(|u| u.get("cache_read_input_tokens")));
    }

    fn on_content_block_start(&mut self, value: &serde_json::Value) {
        let index = value.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
        let Some(content_block) = value.get("content_block") else {
            return;
        };
        let block = match content_block.get("type").and_then(|v| v.as_str()) {
            Some("text") => OpenBlock::Text,
            Some("tool_use") => OpenBlock::Tool {
                id: content_block.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                name: content_block.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                json: String::new(),
            },
            Some("thinking") | Some("redacted_thinking") => OpenBlock::Dropped,
            _ => OpenBlock::Dropped,
        };
        self.blocks.insert(index, block);
    }

    fn on_content_block_delta(&mut self, value: &serde_json::Value) {
        let index = value.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
        let Some(delta) = value.get("delta") else {
            return;
        };
        match delta.get("type").and_then(|v| v.as_str()) {
            Some("text_delta") => {
                let text = delta.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if !text.is_empty() {
                    self.queue.push_back(Ok(Chunk::Content(ContentBlock::Text(text.to_string()))));
                }
            }
            Some("input_json_delta") => {
                let partial = delta.get("partial_json").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(OpenBlock::Tool { json, .. }) = self.blocks.get_mut(&index) {
                    json.push_str(partial);
                }
            }
            _ => {}
        }
    }

    fn on_content_block_stop(&mut self, value: &serde_json::Value) {
        let index = value.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
        let Some(block) = self.blocks.remove(&index) else {
            return;
        };
        if let OpenBlock::Tool { id, name, json } = block {
            let text = if json.is_empty() { "{}" } else { json.as_str() };
            let arguments =
                serde_json::from_str::<serde_json::Value>(text).unwrap_or_else(|_| serde_json::Value::String(json.clone()));
            self.queue.push_back(Ok(Chunk::Content(ContentBlock::ToolCall { id, name, arguments })));
        }
    }

    fn on_message_delta(&mut self, value: &serde_json::Value) {
        if let Some(reason) = value.get("delta").and_then(|d| d.get("stop_reason")).and_then(|v| v.as_str()) {
            self.stop_reason = Some(reason.to_string());
        }
        if let Some(usage) = value.get("usage") {
            self.output_tokens = as_u64(usage.get("output_tokens"));
        }
    }

    fn on_message_stop(&mut self) {
        match self.stop_reason.take() {
            Some(reason) => {
                let usage = serde_json::json!({
                    "input_tokens": self.input_tokens,
                    "output_tokens": self.output_tokens,
                    "cache_creation_input_tokens": self.cache_creation_input_tokens,
                    "cache_read_input_tokens": self.cache_read_input_tokens,
                });
                self.queue.push_back(Ok(Chunk::Terminal {
                    termination: map_termination(&reason),
                    usage: Some(usage),
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

    fn on_error(&mut self, value: &serde_json::Value) {
        let error = value.get("error");
        let kind = error.and_then(|e| e.get("type")).and_then(|v| v.as_str()).unwrap_or("error");
        let message = error.and_then(|e| e.get("message")).and_then(|v| v.as_str()).unwrap_or("");
        self.queue.push_back(Err(ProviderError::Fatal {
            kind: format!("{kind}: {message}"),
        }));
        self.closed = true;
    }

    fn apply_event(&mut self, value: &serde_json::Value) {
        match value.get("type").and_then(|v| v.as_str()) {
            Some("message_start") => self.on_message_start(value),
            Some("content_block_start") => self.on_content_block_start(value),
            Some("content_block_delta") => self.on_content_block_delta(value),
            Some("content_block_stop") => self.on_content_block_stop(value),
            Some("message_delta") => self.on_message_delta(value),
            Some("message_stop") => self.on_message_stop(),
            Some("ping") => {}
            Some("error") => self.on_error(value),
            _ => {}
        }
    }

    fn dispatch_event(&mut self) {
        if self.data_lines.is_empty() {
            return;
        }
        let payload = self.data_lines.join("\n");
        self.data_lines.clear();

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
            self.queue.push_back(Err(ProviderError::Fatal {
                kind: "stream ended without finish_reason".to_string(),
            }));
            self.closed = true;
        }
    }
}

fn map_termination(stop_reason: &str) -> Termination {
    match stop_reason {
        "end_turn" => Termination::EndTurn,
        "tool_use" => Termination::ToolUse,
        "max_tokens" => Termination::MaxTokens,
        "refusal" => Termination::Refusal {
            category: "refusal".to_string(),
        },
        "stop_sequence" => Termination::EndTurn,
        "pause_turn" => Termination::EndTurn,
        other => Termination::Refusal {
            category: other.to_string(),
        },
    }
}

impl<R: BufRead> Iterator for AnthropicChunks<R> {
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

fn chunks_from_reader(reader: impl BufRead + Send + 'static, cancel: Cancel) -> ChunkStream {
    Box::new(AnthropicChunks::new(reader, cancel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn unset_cancel() -> Cancel {
        Arc::new(AtomicBool::new(false))
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

    fn read_file_tool() -> ToolDef {
        ToolDef {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            parameters: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        }
    }

    #[test]
    fn request_body_carries_system_tools_and_merged_tool_results() {
        let messages = vec![
            Message::user("go"),
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolCall {
                    id: "call_0".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "a.txt"}),
                }],
            },
            Message {
                role: Role::Tool,
                content: vec![ContentBlock::ToolResult {
                    call_id: "call_0".to_string(),
                    content: "a contents".to_string(),
                }],
            },
            Message {
                role: Role::Tool,
                content: vec![ContentBlock::ToolResult {
                    call_id: "call_1".to_string(),
                    content: "b contents".to_string(),
                }],
            },
        ];
        let body = build_request_body("model-x", "system prompt", &messages, &[read_file_tool()]);

        assert_eq!(body["model"], "model-x");
        assert_eq!(body["max_tokens"], 8192);
        assert_eq!(body["stream"], true);
        assert_eq!(body["system"], "system prompt");

        let wire_messages = body["messages"].as_array().unwrap();
        assert_eq!(wire_messages.len(), 4);
        assert_eq!(
            wire_messages[0],
            serde_json::json!({"role": "user", "content": [{"type": "text", "text": "go"}]})
        );
        assert_eq!(wire_messages[1]["role"], "assistant");
        assert_eq!(wire_messages[1]["content"][0]["type"], "tool_use");
        assert_eq!(wire_messages[1]["content"][0]["id"], "call_0");
        assert_eq!(wire_messages[1]["content"][0]["name"], "read_file");
        assert_eq!(wire_messages[1]["content"][0]["input"], serde_json::json!({"path": "a.txt"}));
        assert_eq!(
            wire_messages[2],
            serde_json::json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "call_0", "content": "a contents"}]})
        );
        assert_eq!(
            wire_messages[3],
            serde_json::json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": "call_1", "content": "b contents"}]})
        );

        assert_eq!(
            body["tools"][0],
            serde_json::json!({
                "name": "read_file",
                "description": "Read a file",
                "input_schema": {"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}
            })
        );
    }

    #[test]
    fn text_deltas_stream_in_order_and_end_turn_terminates() {
        let chunks = collect(sse(&[
            r#"{"type":"message_start","message":{"usage":{"input_tokens":10}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":2}}"#,
            r#"{"type":"message_stop"}"#,
        ]));
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].as_ref().unwrap(), &Chunk::Content(ContentBlock::Text("Hello".to_string())));
        assert_eq!(chunks[1].as_ref().unwrap(), &Chunk::Content(ContentBlock::Text(" world".to_string())));
        match chunks[2].as_ref().unwrap() {
            Chunk::Terminal { termination, usage } => {
                assert_eq!(termination, &Termination::EndTurn);
                assert_eq!(
                    usage,
                    &Some(serde_json::json!({"input_tokens": 10, "output_tokens": 2, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}))
                );
            }
            other => panic!("expected Terminal, got {other:?}"),
        }
    }

    #[test]
    fn a_tool_use_block_assembles_from_json_deltas_into_one_tool_call() {
        let chunks = collect(sse(&[
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":"read_file","input":{}}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"pa"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"th\": \"a.txt\"}"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":1}}"#,
            r#"{"type":"message_stop"}"#,
        ]));
        assert_eq!(chunks.len(), 2);
        assert_eq!(
            chunks[0].as_ref().unwrap(),
            &Chunk::Content(ContentBlock::ToolCall {
                id: "toolu_1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "a.txt"}),
            })
        );
        assert!(matches!(chunks[1].as_ref().unwrap(), Chunk::Terminal { termination: Termination::ToolUse, .. }));
    }

    #[test]
    fn thinking_blocks_are_dropped() {
        let chunks = collect(sse(&[
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"pondering"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"answer"}}"#,
            r#"{"type":"content_block_stop","index":1}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":1}}"#,
            r#"{"type":"message_stop"}"#,
        ]));
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].as_ref().unwrap(), &Chunk::Content(ContentBlock::Text("answer".to_string())));
        assert!(matches!(chunks[1].as_ref().unwrap(), Chunk::Terminal { .. }));
    }

    #[test]
    fn every_stop_reason_maps() {
        let cases = [
            ("end_turn", Termination::EndTurn),
            ("tool_use", Termination::ToolUse),
            ("max_tokens", Termination::MaxTokens),
            ("refusal", Termination::Refusal { category: "refusal".to_string() }),
            ("stop_sequence", Termination::EndTurn),
            ("pause_turn", Termination::EndTurn),
            ("something_else", Termination::Refusal { category: "something_else".to_string() }),
        ];
        for (reason, expected) in cases {
            assert_eq!(map_termination(reason), expected, "reason: {reason}");
        }
    }

    #[test]
    fn usage_is_normalised_to_the_reader_s_field_names() {
        let chunks = collect(sse(&[
            r#"{"type":"message_start","message":{"usage":{"input_tokens":100,"cache_creation_input_tokens":20,"cache_read_input_tokens":5}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":10}}"#,
            r#"{"type":"message_stop"}"#,
        ]));
        let last = chunks.last().unwrap().as_ref().unwrap();
        match last {
            Chunk::Terminal { usage, .. } => {
                assert_eq!(
                    usage,
                    &Some(serde_json::json!({
                        "input_tokens": 100,
                        "output_tokens": 10,
                        "cache_creation_input_tokens": 20,
                        "cache_read_input_tokens": 5
                    }))
                );
            }
            other => panic!("expected Terminal, got {other:?}"),
        }
    }

    #[test]
    fn a_prompt_too_long_400_is_a_miss_with_the_overshoot_parsed() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"prompt is too long: 205000 tokens > 200000 maximum"}}"#;
        let err = classify_error(400, None, body);
        match err {
            ProviderError::ContextWindowExceeded { overshoot: Some(overshoot) } => {
                assert_eq!(overshoot.requested, 205000);
                assert_eq!(overshoot.maximum, 200000);
            }
            other => panic!("expected ContextWindowExceeded with an overshoot, got {other:?}"),
        }
    }

    #[test]
    fn a_529_is_transient() {
        let err = classify_error(529, Some(Duration::from_secs(2)), "{}");
        match err {
            ProviderError::Transient { kind, retry_after } => {
                assert_eq!(kind, "529");
                assert_eq!(retry_after, Some(Duration::from_secs(2)));
            }
            other => panic!("expected Transient, got {other:?}"),
        }
    }

    #[test]
    fn an_error_event_mid_stream_is_fatal_with_the_vendor_text() {
        let mut stream = chunks_from_reader(
            sse(&[
                r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
                r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"partial"}}"#,
                r#"{"type":"error","error":{"type":"overloaded_error","message":"the server is overloaded"}}"#,
            ]),
            unset_cancel(),
        );
        assert_eq!(stream.next().unwrap().unwrap(), Chunk::Content(ContentBlock::Text("partial".to_string())));
        match stream.next().unwrap().unwrap_err() {
            ProviderError::Fatal { kind } => {
                assert!(kind.contains("overloaded_error"), "kind: {kind}");
                assert!(kind.contains("the server is overloaded"), "kind: {kind}");
            }
            other => panic!("expected Fatal, got {other:?}"),
        }
        assert!(stream.next().is_none());
    }

    #[test]
    fn the_key_is_redacted_from_error_texts() {
        let raw = "data: {\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"bad key: Bearer sk-secret-123\"}}\n\n";
        let inner = chunks_from_reader(Cursor::new(raw.as_bytes().to_vec()), unset_cancel());
        let mut stream = RedactingStream::new(inner, "sk-secret-123".to_string());
        match stream.next().unwrap() {
            Err(ProviderError::Fatal { kind: text }) => {
                assert!(!text.contains("sk-secret-123"), "text: {text}");
                assert!(text.contains("[redacted]"), "text: {text}");
            }
            other => panic!("expected a redacted Fatal error, got {other:?}"),
        }
    }
}
