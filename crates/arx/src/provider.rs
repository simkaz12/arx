//! The provider trait of L1-10 (`975f65cd`, `48af6ac7`): stateless, one turn, no session state.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum Role {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) enum ContentBlock {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        call_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub(crate) fn user(text: impl Into<String>) -> Message {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text(text.into())],
        }
    }
}

pub(crate) struct Window {
    messages: Vec<Message>,
}

impl Window {
    pub(crate) fn new(brief: &str) -> Window {
        Window {
            messages: vec![Message::user(brief)],
        }
    }

    pub(crate) fn append(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub(crate) fn rewind(&mut self, keep: usize) -> Vec<Message> {
        if keep >= self.messages.len() {
            return Vec::new();
        }
        self.messages.split_off(keep)
    }

    pub(crate) fn as_slice(&self) -> &[Message] {
        &self.messages
    }

    pub(crate) fn len(&self) -> usize {
        self.messages.len()
    }
}

pub(crate) const RESERVE_TOKENS: u64 = 2048;

pub(crate) fn estimate_tokens(messages: &[Message]) -> u64 {
    let mut bytes: u64 = 0;
    for message in messages {
        for block in &message.content {
            bytes += match block {
                ContentBlock::Text(text) => text.len() as u64,
                ContentBlock::ToolCall { arguments, .. } => serde_json::to_string(arguments).map(|s| s.len()).unwrap_or(0) as u64,
                ContentBlock::ToolResult { content, .. } => content.len() as u64,
            };
        }
    }
    bytes.div_ceil(4)
}

pub(crate) fn rewind_to_fit(window: &mut Window, target: u64) -> Vec<Message> {
    let messages = window.as_slice();
    let mut keep = messages.len();
    let mut cut_tokens = 0u64;
    while keep > 1 && cut_tokens < target {
        keep -= 1;
        cut_tokens += estimate_tokens(&messages[keep..keep + 1]);
    }
    window.rewind(keep)
}

pub(crate) type Cancel = Arc<AtomicBool>;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) enum Termination {
    EndTurn,
    ToolUse,
    MaxTokens,
    Refusal { category: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Chunk {
    Content(ContentBlock),
    Terminal {
        termination: Termination,
        usage: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overshoot {
    pub requested: u64,
    pub maximum: u64,
}

#[non_exhaustive]
#[derive(Debug)]
pub(crate) enum ProviderError {
    Transient { kind: String, retry_after: Option<Duration> },
    Fatal { kind: String },
    ContextWindowExceeded { overshoot: Option<Overshoot> },
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::Transient { kind, .. } => write!(f, "transient ({kind})"),
            ProviderError::Fatal { kind } => write!(f, "fatal: {kind}"),
            ProviderError::ContextWindowExceeded { overshoot: Some(overshoot) } => {
                write!(f, "context window exceeded (requested {}, maximum {})", overshoot.requested, overshoot.maximum)
            }
            ProviderError::ContextWindowExceeded { overshoot: None } => {
                write!(f, "context window exceeded (size unknown)")
            }
        }
    }
}

impl std::error::Error for ProviderError {}

pub(crate) type ChunkStream = Box<dyn Iterator<Item = Result<Chunk, ProviderError>> + Send>;

pub(crate) trait Provider: Send + Sync {
    fn complete(&self, system: &str, messages: &[Message], tools: &[ToolDef]) -> Result<ChunkStream, ProviderError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ProviderKind {
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
    #[serde(rename = "anthropic")]
    Anthropic,
}

const RETRY_ATTEMPTS: u32 = 3;
const CANCEL_POLL: Duration = Duration::from_millis(50);

pub(crate) struct Retrying<P: Provider> {
    inner: P,
    cancel: Cancel,
    attempts: u32,
}

impl<P: Provider> Retrying<P> {
    pub(crate) fn new(inner: P, cancel: Cancel) -> Retrying<P> {
        Retrying {
            inner,
            cancel,
            attempts: RETRY_ATTEMPTS,
        }
    }

    fn wait_or_killed(&self, duration: Duration) -> bool {
        let mut waited = Duration::ZERO;
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return true;
            }
            if waited >= duration {
                return false;
            }
            let step = CANCEL_POLL.min(duration - waited);
            std::thread::sleep(step);
            waited += step;
        }
    }
}

impl<P: Provider> Provider for Retrying<P> {
    fn complete(&self, system: &str, messages: &[Message], tools: &[ToolDef]) -> Result<ChunkStream, ProviderError> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match self.inner.complete(system, messages, tools) {
                Ok(stream) => return Ok(stream),
                Err(ProviderError::Transient { kind, retry_after }) => {
                    if attempt >= self.attempts {
                        return Err(ProviderError::Transient { kind, retry_after });
                    }
                    let wait = retry_after.unwrap_or_else(|| Duration::from_millis(500 * 2u64.pow(attempt - 1)));
                    if self.wait_or_killed(wait) {
                        return Err(ProviderError::Fatal { kind: "killed".to_string() });
                    }
                }
                Err(other) => return Err(other),
            }
        }
    }
}

pub(crate) fn build_provider(kind: ProviderKind, base_url: &str, api_key: String, model: &str, cancel: Cancel) -> Box<dyn Provider> {
    match kind {
        ProviderKind::OpenAiCompatible => Box::new(Retrying::new(
            crate::openai_compat::OpenAiCompatible::new(base_url, api_key, model, cancel.clone()),
            cancel,
        )),
        ProviderKind::Anthropic => Box::new(Retrying::new(
            crate::anthropic::Anthropic::new(base_url, api_key, model, cancel.clone()),
            cancel,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    fn unset_cancel() -> Cancel {
        Arc::new(AtomicBool::new(false))
    }

    struct ScriptedInner {
        calls: Mutex<u32>,
        scripts: Mutex<VecDeque<Result<Vec<Result<Chunk, ProviderError>>, ProviderError>>>,
    }

    impl ScriptedInner {
        fn new(scripts: Vec<Result<Vec<Result<Chunk, ProviderError>>, ProviderError>>) -> ScriptedInner {
            ScriptedInner {
                calls: Mutex::new(0),
                scripts: Mutex::new(scripts.into_iter().collect()),
            }
        }
    }

    impl Provider for ScriptedInner {
        fn complete(&self, _system: &str, _messages: &[Message], _tools: &[ToolDef]) -> Result<ChunkStream, ProviderError> {
            *self.calls.lock().unwrap() += 1;
            match self.scripts.lock().unwrap().pop_front().expect("scripted provider exhausted") {
                Ok(items) => Ok(Box::new(items.into_iter())),
                Err(err) => Err(err),
            }
        }
    }

    #[test]
    fn retrying_retries_a_transient_before_the_first_item_and_stops_at_the_bound() {
        let inner = ScriptedInner::new(vec![
            Err(ProviderError::Transient {
                kind: "429".to_string(),
                retry_after: Some(Duration::from_millis(1)),
            }),
            Err(ProviderError::Transient {
                kind: "429".to_string(),
                retry_after: Some(Duration::from_millis(1)),
            }),
            Ok(vec![Ok(Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None,
            })]),
        ]);
        let retrying = Retrying::new(inner, unset_cancel());
        let result = retrying.complete("system", &[], &[]);
        assert!(result.is_ok());
        assert_eq!(*retrying.inner.calls.lock().unwrap(), 3);

        let inner_exhausted = ScriptedInner::new(vec![
            Err(ProviderError::Transient {
                kind: "429".to_string(),
                retry_after: Some(Duration::from_millis(1)),
            }),
            Err(ProviderError::Transient {
                kind: "503".to_string(),
                retry_after: Some(Duration::from_millis(1)),
            }),
            Err(ProviderError::Transient {
                kind: "429".to_string(),
                retry_after: Some(Duration::from_millis(1)),
            }),
        ]);
        let retrying_exhausted = Retrying::new(inner_exhausted, unset_cancel());
        match retrying_exhausted.complete("system", &[], &[]) {
            Err(ProviderError::Transient { kind, .. }) => assert_eq!(kind, "429"),
            Err(other) => panic!("expected the third Transient, got {other:?}"),
            Ok(_) => panic!("expected the third Transient, got Ok"),
        }
        assert_eq!(*retrying_exhausted.inner.calls.lock().unwrap(), 3);
    }

    #[test]
    fn retrying_honours_retry_after_and_the_cancel_flag() {
        let inner = ScriptedInner::new(vec![
            Err(ProviderError::Transient {
                kind: "429".to_string(),
                retry_after: Some(Duration::from_millis(200)),
            }),
            Ok(vec![Ok(Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None,
            })]),
        ]);
        let retrying = Retrying::new(inner, unset_cancel());
        let start = std::time::Instant::now();
        let result = retrying.complete("system", &[], &[]);
        let elapsed = start.elapsed();
        assert!(result.is_ok());
        assert!(elapsed >= Duration::from_millis(200), "elapsed: {elapsed:?}");
        assert_eq!(*retrying.inner.calls.lock().unwrap(), 2);

        let cancel = Arc::new(AtomicBool::new(true));
        let inner_killed = ScriptedInner::new(vec![Err(ProviderError::Transient {
            kind: "429".to_string(),
            retry_after: Some(Duration::from_millis(200)),
        })]);
        let retrying_killed = Retrying::new(inner_killed, Arc::clone(&cancel));
        let start = std::time::Instant::now();
        let result = retrying_killed.complete("system", &[], &[]);
        let elapsed = start.elapsed();
        match result {
            Err(ProviderError::Fatal { kind }) => assert_eq!(kind, "killed"),
            Err(other) => panic!("expected Fatal(killed), got {other:?}"),
            Ok(_) => panic!("expected Fatal(killed), got Ok"),
        }
        assert_eq!(*retrying_killed.inner.calls.lock().unwrap(), 1);
        assert!(elapsed < Duration::from_millis(200), "elapsed: {elapsed:?}");
    }

    #[test]
    fn a_stream_error_after_the_first_item_is_not_retried() {
        let inner = ScriptedInner::new(vec![Ok(vec![
            Ok(Chunk::Content(ContentBlock::Text("hi".to_string()))),
            Err(ProviderError::Transient {
                kind: "503".to_string(),
                retry_after: None,
            }),
        ])]);
        let retrying = Retrying::new(inner, unset_cancel());
        let stream = retrying.complete("system", &[], &[]).expect("first call succeeds");
        let items: Vec<_> = stream.collect();
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0], Ok(Chunk::Content(ContentBlock::Text(_)))));
        assert!(matches!(items[1], Err(ProviderError::Transient { .. })));
        assert_eq!(*retrying.inner.calls.lock().unwrap(), 1);
    }

    #[test]
    fn window_appends_in_order_and_rewind_returns_the_tail() {
        let mut window = Window::new("brief");
        window.append(Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text("first".to_string())],
        });
        window.append(Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text("second".to_string())],
        });
        assert_eq!(window.len(), 3);
        assert_eq!(window.as_slice()[0], Message::user("brief"));

        let cut = window.rewind(1);
        assert_eq!(window.len(), 1);
        assert_eq!(cut.len(), 2);
        assert_eq!(cut[0].content, vec![ContentBlock::Text("first".to_string())]);
        assert_eq!(cut[1].content, vec![ContentBlock::Text("second".to_string())]);
    }

    #[test]
    fn rewind_past_the_end_cuts_nothing() {
        let mut window = Window::new("brief");
        window.append(Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text("only".to_string())],
        });
        let cut = window.rewind(10);
        assert!(cut.is_empty());
        assert_eq!(window.len(), 2);
    }

    #[test]
    fn estimate_counts_text_tool_calls_and_results() {
        let messages = vec![
            Message {
                role: Role::User,
                content: vec![ContentBlock::Text("1234567890".to_string())],
            },
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "a.txt"}),
                }],
            },
            Message {
                role: Role::Tool,
                content: vec![ContentBlock::ToolResult {
                    call_id: "call_1".to_string(),
                    content: "1234567".to_string(),
                }],
            },
        ];
        let arguments_bytes = serde_json::to_string(&serde_json::json!({"path": "a.txt"})).unwrap().len() as u64;
        let expected = (10 + arguments_bytes + 7).div_ceil(4);
        assert_eq!(estimate_tokens(&messages), expected);
    }

    #[test]
    fn rewind_to_fit_never_cuts_the_brief() {
        let mut window = Window::new("brief");
        window.append(Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text("a".repeat(100))],
        });
        window.append(Message {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                call_id: "call_1".to_string(),
                content: "b".repeat(100),
            }],
        });

        let cut = rewind_to_fit(&mut window, 1_000_000);
        assert_eq!(window.len(), 1);
        assert_eq!(window.as_slice()[0], Message::user("brief"));
        assert_eq!(cut.len(), 2);
    }
}
