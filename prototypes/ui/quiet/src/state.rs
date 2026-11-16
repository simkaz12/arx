pub const SCOPE: &str = "arx";
pub const STORE_ROOT: &str = "~/.local/share/arx/store";
pub const WINDOW: u64 = 200_000;
pub const RESERVE: u64 = 40_000;
pub const NUDGE_THRESHOLD: u64 = 120_000;
pub const MODELS: [&str; 3] = ["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"];
pub const INITIAL_MODEL: &str = "claude-opus-5";
pub const INITIAL_CHECKPOINT: u32 = 2;
pub const INITIAL_OCCUPANCY: u64 = 61_412;
pub const GROUNDING: &str = "hot 4 claims · 2 embers";

pub struct SessionState {
    pub model: String,
    pub occupancy: u64,
    pub checkpoint: u32,
    pub hands_in_flight: i64,
    pub last_tokens: Option<u64>,
    pub last_elapsed_ms: Option<u64>,
}

impl SessionState {
    pub fn new() -> Self {
        SessionState {
            model: INITIAL_MODEL.to_string(),
            occupancy: INITIAL_OCCUPANCY,
            checkpoint: INITIAL_CHECKPOINT,
            hands_in_flight: 0,
            last_tokens: None,
            last_elapsed_ms: None,
        }
    }

    pub fn status_line(&self) -> String {
        let usage = match (self.last_tokens, self.last_elapsed_ms) {
            (Some(t), Some(e)) => format!("{:.1}k tok · {:.1}s", t as f64 / 1000.0, e as f64 / 1000.0),
            _ => "no turns yet".to_string(),
        };
        format!(
            "{} · {} · {} of {} (reserve {}) · checkpoint {} · {} hand{} in flight · {}",
            self.model,
            SCOPE,
            self.occupancy,
            WINDOW,
            RESERVE,
            self.checkpoint,
            self.hands_in_flight,
            if self.hands_in_flight == 1 { "" } else { "s" },
            usage,
        )
    }
}
