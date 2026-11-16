#[derive(Debug, Clone)]
pub enum Admission {
    Inline { bytes: u64 },
    Externalized { bytes: u64, trace: String },
}

#[derive(Debug, Clone)]
pub struct DispatchInfo {
    pub hand_id: String,
    pub agent: String,
    pub role: &'static str,
    pub body: &'static str,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct Delivery {
    pub hand_id: String,
    pub outcome: &'static str,
    pub text: String,
    pub trace: String,
    pub bytes: u64,
    pub elapsed_ms: u64,
    pub lag_ms: u64,
    pub schema_ok: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct Usage {
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub model: String,
    pub model_source: &'static str,
    pub elapsed_ms: u64,
    pub occupancy_after: u64,
}

#[derive(Debug, Clone)]
pub enum Event {
    TurnStart,
    Chunk(String),
    TurnEnd(String),
    ArxLine(String),
    ToolCall { tool: String, subject: String },
    GateVerdict { tool: String, subject: String, allowed: bool, rule: String },
    GateAsk { tool: String, subject: String },
    ToolResult { admission: Admission, preview: String },
    Spawn(DispatchInfo),
    Delivered(Delivery),
    Notice(String),
    Amber(String),
    Error(String),
    Usage(Usage),
    CheckpointWritten { number: u32, path: String, delta: i64, since: u32 },
    ResetConfirm { body: String },
    Orientation { text: String, verdict: &'static str },
    ModelSwitched { from: String, to: String },
    ModelAlready(String),
    ModelUnlisted { requested: String, models: Vec<String> },
    ProviderError(String),
    RetryOk,
    Aborted,
    OccupancyReset(u64),
}

#[derive(Debug, Clone)]
pub struct AskAnswer {
    pub key: char,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    Chat(String),
    AskAnswer(AskAnswer),
    ModelPick(String),
    Checkpoint,
    Reset,
    ConfirmLine(String),
    Abort,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Tab,
    Up,
    Down,
    Esc,
    CtrlC,
}
