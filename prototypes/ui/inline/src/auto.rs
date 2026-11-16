pub enum Action {
    Line(&'static str),
    LineWithAbort(&'static str),
    AskAnswer(char),
    ModelPick(&'static str),
    Confirm(&'static str),
}

pub const SCRIPT: &[Action] = &[
    Action::Line("What actually changed since the store root moved out of the repo?"),
    Action::Line("Can you check what the terminal tests actually pin down before we touch it?"),
    Action::AskAnswer('A'),
    Action::Line("Go ahead and dispatch the cleanup, then have the codex judge review it."),
    Action::Line("Anything back yet?"),
    Action::ModelPick("claude-sonnet-5"),
    Action::Line("We're close to the ceiling; walk me through it."),
    Action::Line("/checkpoint"),
    Action::Line("/reset"),
    Action::Confirm("ok"),
    Action::LineWithAbort("Give me the long version of the polishing status."),
    Action::Line("Try that again."),
    Action::Line("/reste"),
];

pub const SCENE_BOUNDARY_AFTER: [usize; 7] = [0, 2, 4, 5, 9, 11, 12];
