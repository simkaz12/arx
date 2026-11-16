use serde::{Deserialize, Serialize};

/// arx's event vocabulary: one shape per kind, feeding the four policy constants
/// named in D09 — the admission threshold, the checkpoint token-delta, the reset
/// reserve, and the occupancy nudge. `Event::kind` returns the tag string and
/// `KINDS` lists all twenty-one in declaration order. Every kind has a live emitter:
/// `gate`, `spawn`, `steer`, `tap`, `kill`, `delivered`, `nudge`, `wall`,
/// `admission`, `pull`, `echo`, `rewind`, `checkpoint`, `miss`, `schema`,
/// `family` and `drain` landed in their own cards; `admission`, `pull` and `echo` from main's
/// file and hand closures and the native loop; `reset`, `floor` and
/// `orientation` from the reset path landed in L3-02; `model` from the operator's
/// `/model` command or an automatic SDK refusal fallback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Gate {
        tool: String,
        verdict: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Spawn {
        hand: String,
        agent: String,
        role: String,
    },
    Steer {
        hand: String,
    },
    Tap {
        hand: String,
    },
    Kill {
        hand: String,
    },
    Delivered {
        hand: String,
        outcome: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trace: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lag_ms: Option<u64>,
    },
    Nudge {
        seat: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hand: Option<String>,
        occupancy: u64,
        threshold: u64,
    },
    Wall {
        hand: String,
        tool: String,
    },
    Admission {
        tool: String,
        outcome: String,
        bytes: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handle: Option<String>,
    },
    Pull {
        handle: String,
        path: String,
    },
    Echo {
        handle: String,
    },
    Rewind {
        tokens_cut: u64,
    },
    Checkpoint {
        check: String,
        delta: u64,
    },
    Reset {
        phase: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        check: Option<String>,
    },
    Miss {
        size: u64,
    },
    Floor {
        overshoot: u64,
    },
    Orientation {
        verdict: String,
    },
    Schema {
        hand: String,
        outcome: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Family {
        agent: String,
        family: String,
    },
    Drain {
        hand: String,
        promoted: usize,
        dropped: usize,
        overrides: usize,
    },
    Model {
        #[serde(default = "default_model_position")]
        position: String,
        from: String,
        to: String,
    },
}

/// A model event committed before this field existed named no position;
/// every one of those predates the roster and was always about main.
fn default_model_position() -> String {
    "main".to_string()
}

pub const KINDS: [&str; 21] = [
    "gate",
    "spawn",
    "steer",
    "tap",
    "kill",
    "delivered",
    "nudge",
    "wall",
    "admission",
    "pull",
    "echo",
    "rewind",
    "checkpoint",
    "reset",
    "miss",
    "floor",
    "orientation",
    "schema",
    "family",
    "drain",
    "model",
];

impl Event {
    pub fn kind(&self) -> &'static str {
        match self {
            Event::Gate { .. } => "gate",
            Event::Spawn { .. } => "spawn",
            Event::Steer { .. } => "steer",
            Event::Tap { .. } => "tap",
            Event::Kill { .. } => "kill",
            Event::Delivered { .. } => "delivered",
            Event::Nudge { .. } => "nudge",
            Event::Wall { .. } => "wall",
            Event::Admission { .. } => "admission",
            Event::Pull { .. } => "pull",
            Event::Echo { .. } => "echo",
            Event::Rewind { .. } => "rewind",
            Event::Checkpoint { .. } => "checkpoint",
            Event::Reset { .. } => "reset",
            Event::Miss { .. } => "miss",
            Event::Floor { .. } => "floor",
            Event::Orientation { .. } => "orientation",
            Event::Schema { .. } => "schema",
            Event::Family { .. } => "family",
            Event::Drain { .. } => "drain",
            Event::Model { .. } => "model",
        }
    }
}

#[doc(hidden)]
pub fn every_kind() -> Vec<Event> {
    vec![
        Event::Gate {
            tool: "save".to_string(),
            verdict: "allow".to_string(),
            by: None,
            reason: None,
        },
        Event::Spawn {
            hand: "h1".to_string(),
            agent: "hand-ex".to_string(),
            role: "hand".to_string(),
        },
        Event::Steer { hand: "h1".to_string() },
        Event::Tap { hand: "h1".to_string() },
        Event::Kill { hand: "h1".to_string() },
        Event::Delivered {
            hand: "h1".to_string(),
            outcome: "verdict".to_string(),
            trace: None,
            bytes: None,
            elapsed_ms: None,
            lag_ms: None,
        },
        Event::Nudge {
            seat: "main".to_string(),
            hand: None,
            occupancy: 130_000,
            threshold: 120_000,
        },
        Event::Wall {
            hand: "h1".to_string(),
            tool: "Bash".to_string(),
        },
        Event::Admission {
            tool: "Read".to_string(),
            outcome: "externalized".to_string(),
            bytes: 4096,
            handle: Some("hdl1".to_string()),
        },
        Event::Pull {
            handle: "hdl1".to_string(),
            path: "trace/hdl1.txt".to_string(),
        },
        Event::Echo { handle: "hdl1".to_string() },
        Event::Rewind { tokens_cut: 2048 },
        Event::Checkpoint {
            check: "pass".to_string(),
            delta: 512,
        },
        Event::Reset {
            phase: "armed".to_string(),
            check: None,
        },
        Event::Miss { size: 8192 },
        Event::Floor { overshoot: 256 },
        Event::Orientation {
            verdict: "fine".to_string(),
        },
        Event::Schema {
            hand: "h1".to_string(),
            outcome: "ok".to_string(),
            reason: None,
        },
        Event::Family {
            agent: "reviewer".to_string(),
            family: "claude".to_string(),
        },
        Event::Drain {
            hand: "h1".to_string(),
            promoted: 1,
            dropped: 1,
            overrides: 1,
        },
        Event::Model {
            position: "main".to_string(),
            from: "claude-sonnet-5".to_string(),
            to: "claude-opus-5".to_string(),
        },
    ]
}
