use crate::claude_code::ClaudeCodeMain;
use crate::codex_main::CodexMain;
use arx_engine::StoreCall;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    pub n: u32,
    pub path: PathBuf,
    pub media_type: String,
    pub bytes: u64,
}

pub struct HandCall {
    pub verb: String,
    pub args: serde_json::Value,
}

pub struct FileCall {
    pub tool: String,
    pub args: serde_json::Value,
}

pub struct SystemPrompt {
    pub static_block: String,
    pub dynamic: String,
}

pub struct SpawnOpts {
    pub model: String,
    pub system_prompt: SystemPrompt,
    pub cwd: PathBuf,
}

#[derive(Debug)]
pub struct TurnOutcome {
    pub text: String,
    pub usage: Option<serde_json::Value>,
    pub models: Option<Vec<String>>,
    pub stopped: bool,
}

#[derive(Debug)]
pub enum MainError {
    EntryMissing { path: PathBuf },
    SpawnIo { source: std::io::Error },
    ReadyTimeout,
    ProtocolMalformed { line: String },
    ContextWindowExceeded { message: String },
    UsageLimitReached { message: String },
    ChildDiedMidTurn,
    BodyFatal { message: String },
    VendorCompaction { text: String },
    ModelSwitchFailed { message: String },
}

const CONTEXT_WINDOW_NEEDLES: [&str; 5] = [
    "prompt is too long",
    "tokens > ",
    "context window",
    "context_length_exceeded",
    "too many tokens",
];

pub fn is_context_window_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    CONTEXT_WINDOW_NEEDLES
        .iter()
        .any(|needle| lower.contains(needle))
}

impl fmt::Display for MainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MainError::EntryMissing { path } => {
                write!(f, "main entry not found: {}", path.display())
            }
            MainError::SpawnIo { source } => write!(f, "main process io error: {source}"),
            MainError::ReadyTimeout => {
                write!(f, "main did not report ready within the deadline")
            }
            MainError::ProtocolMalformed { line } => {
                write!(f, "malformed main protocol line: {line}")
            }
            MainError::ContextWindowExceeded { message } => {
                write!(f, "the body rejected the turn: window full — {message}")
            }
            MainError::UsageLimitReached { message } => write!(f, "{message}"),
            MainError::ChildDiedMidTurn => write!(f, "the main child died mid-turn"),
            MainError::ModelSwitchFailed { message } | MainError::BodyFatal { message } => {
                write!(f, "{message}")
            }
            MainError::VendorCompaction { .. } => {
                write!(f, "Codex compacted its context; restarting from arx state")
            }
        }
    }
}

impl std::error::Error for MainError {}

#[derive(Clone)]
pub struct MainBody(Arc<Mutex<Body>>);

#[derive(Clone)]
enum Body {
    Claude(ClaudeCodeMain),
    Codex(CodexMain),
}

impl MainBody {
    pub fn spawn(sidecar: &Path, opts: SpawnOpts) -> Result<Self, MainError> {
        Ok(Self(Arc::new(Mutex::new(Body::Claude(
            ClaudeCodeMain::spawn(sidecar, opts)?,
        )))))
    }

    pub fn spawn_for(kind: &str, sidecar: &Path, opts: SpawnOpts) -> Result<Self, MainError> {
        match kind {
            "claude-code" => Self::spawn(sidecar, opts),
            "codex" => Ok(Self(Arc::new(Mutex::new(Body::Codex(CodexMain::spawn(
                &opts,
            )?))))),
            _ => Err(MainError::BodyFatal {
                message: format!("unsupported main body: {kind}"),
            }),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self.active() {
            Body::Claude(_) => "claude-code",
            Body::Codex(_) => "codex",
        }
    }

    pub fn replace(&mut self, next: MainBody) {
        self.kill();
        *self.0.lock().expect("main body mutex poisoned") = next.active();
    }

    fn active(&self) -> Body {
        self.0.lock().expect("main body mutex poisoned").clone()
    }

    pub fn respawn(&self, sidecar: &Path, opts: &SpawnOpts) -> Result<(), MainError> {
        match self.active() {
            Body::Claude(body) => body.respawn(sidecar, opts),
            Body::Codex(mut body) => {
                body.kill();
                let next = CodexMain::spawn(opts)?;
                *self.0.lock().expect("main body mutex poisoned") = Body::Codex(next);
                Ok(())
            }
        }
    }

    pub fn take_pending_model_fallback(&self) -> Option<(String, String)> {
        match self.active() {
            Body::Claude(body) => body.take_pending_model_fallback(),
            Body::Codex(_) => None,
        }
    }

    pub fn take_model_selection_result(&self) -> Option<Result<String, String>> {
        match self.active() {
            Body::Claude(_) => None,
            Body::Codex(body) => body.take_model_selection_result(),
        }
    }

    pub fn set_model(&mut self, model: &str) -> Result<(), MainError> {
        match self.active() {
            Body::Claude(mut body) => body.set_model(model),
            Body::Codex(mut body) => body.set_model(model),
        }
    }

    pub fn kill(&mut self) {
        match self.active() {
            Body::Claude(mut body) => body.kill(),
            Body::Codex(mut body) => body.kill(),
        }
    }
    pub fn shutdown(self) {
        match self.active() {
            Body::Claude(body) => body.shutdown(),
            Body::Codex(body) => body.shutdown(),
        }
    }

    pub fn run_turn(
        &mut self,
        prompt: &str,
        on_delta: impl FnMut(&str),
        on_model_fallback: impl FnMut(&str, &str),
        on_store: impl FnMut(&StoreCall) -> serde_json::Value,
        on_hand: impl FnMut(&HandCall) -> serde_json::Value,
        on_file: impl FnMut(&FileCall) -> serde_json::Value,
    ) -> Result<TurnOutcome, MainError> {
        self.run_turn_controlled(
            prompt,
            &[],
            on_delta,
            on_model_fallback,
            on_store,
            on_hand,
            on_file,
            &mut || None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_turn_controlled(
        &mut self,
        prompt: &str,
        images: &[Attachment],
        on_delta: impl FnMut(&str),
        on_model_fallback: impl FnMut(&str, &str),
        on_store: impl FnMut(&StoreCall) -> serde_json::Value,
        on_hand: impl FnMut(&HandCall) -> serde_json::Value,
        on_file: impl FnMut(&FileCall) -> serde_json::Value,
        on_control: &mut dyn FnMut() -> Option<Control>,
    ) -> Result<TurnOutcome, MainError> {
        match self.active() {
            Body::Claude(mut body) => body.run_turn_controlled(
                prompt,
                images,
                on_delta,
                on_model_fallback,
                on_store,
                on_hand,
                on_file,
                on_control,
            ),
            Body::Codex(mut body) => body.run_turn_controlled(
                prompt, images, on_delta, on_store, on_hand, on_file, on_control,
            ),
        }
    }
}
