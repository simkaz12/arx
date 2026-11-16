use std::collections::HashMap;
use std::path::Path;

const EMBEDDED_DEFAULTS: &str = include_str!("keys.toml");

pub const NORMAL_ACTIONS: [(&str, &str); 20] = [
    ("expand", "toggle a fold or open a call's trace"),
    ("insert", "enter insert mode"),
    ("panel_1", "focus panel 1"),
    ("panel_2", "focus panel 2"),
    ("panel_3", "focus panel 3"),
    ("panel_4", "focus panel 4"),
    ("next_panel", "next panel"),
    ("prev_panel", "previous panel"),
    ("down", "move down"),
    ("up", "move up"),
    ("top", "jump to top"),
    ("bottom", "jump to bottom"),
    ("page_down", "page down"),
    ("page_up", "page up"),
    ("open", "focus the big view"),
    ("back", "back to the panel"),
    ("close", "kill (with confirm) or remove the selected row"),
    ("stop", "stop main's turn, or kill the selected hand/reviewer"),
    ("help", "show keys"),
    ("quit", "quit"),
];

pub const INSERT_ACTIONS: [(&str, &str); 12] = [
    ("submit", "send the line"),
    ("newline", "insert a newline"),
    ("normal", "back to normal mode"),
    ("left", "move left"),
    ("right", "move right"),
    ("home", "start of the row"),
    ("end", "end of the row"),
    ("delete", "delete forward"),
    ("history_up", "previous in history"),
    ("history_down", "next in history"),
    ("complete", "complete a slash command"),
    ("paste", "paste text or an image from the clipboard"),
];

pub const MODAL_ACTIONS: [(&str, &str); 6] = [
    ("confirm", "confirm"),
    ("cancel", "cancel"),
    ("down", "move down"),
    ("up", "move up"),
    ("prev_tab", "previous tab"),
    ("next_tab", "next tab"),
];

pub const SPLIT_ACTIONS: [(&str, &str); 1] = [("swap_pane", "swap brief/turns focus")];

#[derive(Debug, Clone, Default)]
pub struct KeyTable(HashMap<String, Vec<String>>);

impl KeyTable {
    pub fn bound(&self, action: &str) -> &[String] {
        self.0.get(action).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn matches(&self, action: &str, token: &str) -> bool {
        self.bound(action).iter().any(|k| k == token)
    }

    pub fn primary(&self, action: &str) -> Option<&str> {
        self.bound(action).first().map(|s| s.as_str())
    }

    fn apply_overrides(&mut self, overrides: KeyTable) {
        for (action, keys) in overrides.0 {
            self.0.insert(action, keys);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct KeyMap {
    pub normal: KeyTable,
    pub insert: KeyTable,
    pub modal: KeyTable,
    pub split: KeyTable,
}

impl KeyMap {
    pub fn defaults() -> Self {
        parse(EMBEDDED_DEFAULTS).unwrap_or_default()
    }

    pub fn load(path: Option<&Path>) -> Self {
        let mut map = Self::defaults();
        if let Some(path) = path {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(overrides) = parse(&text) {
                    map.normal.apply_overrides(overrides.normal);
                    map.insert.apply_overrides(overrides.insert);
                    map.modal.apply_overrides(overrides.modal);
                    map.split.apply_overrides(overrides.split);
                }
            }
        }
        map
    }
}

fn strip_comment(line: &str) -> &str {
    let mut in_quote = false;
    for (idx, c) in line.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            '#' if !in_quote => return &line[..idx],
            _ => {}
        }
    }
    line
}

fn parse_entry(line: &str) -> Option<(String, Vec<String>)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim().to_string();
    let value = value.trim();
    let inner = value.strip_prefix('[')?.strip_suffix(']')?;
    let items = inner
        .split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| item.trim_matches('"').to_string())
        .collect();
    Some((key, items))
}

fn parse(text: &str) -> Result<KeyMap, String> {
    let mut normal = HashMap::new();
    let mut insert = HashMap::new();
    let mut modal = HashMap::new();
    let mut split = HashMap::new();
    let mut section: Option<&mut HashMap<String, Vec<String>>> = None;

    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = match name {
                "normal" => Some(&mut normal),
                "insert" => Some(&mut insert),
                "modal" => Some(&mut modal),
                "split" => Some(&mut split),
                _ => None,
            };
            continue;
        }
        if let Some(target) = section.as_deref_mut() {
            if let Some((action, keys)) = parse_entry(line) {
                target.insert(action, keys);
            }
        }
    }

    Ok(KeyMap {
        normal: KeyTable(normal),
        insert: KeyTable(insert),
        modal: KeyTable(modal),
        split: KeyTable(split),
    })
}

pub fn key_token(code: crossterm::event::KeyCode, modifiers: crossterm::event::KeyModifiers) -> Option<String> {
    use crossterm::event::{KeyCode, KeyModifiers};
    let base = match code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "BackTab".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        _ => return None,
    };
    let mut out = String::new();
    if modifiers.contains(KeyModifiers::CONTROL) {
        out.push_str("Ctrl-");
    }
    if modifiers.contains(KeyModifiers::ALT) {
        out.push_str("Alt-");
    }
    out.push_str(&base);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_carry_no_abort_binding() {
        let keys = KeyMap::defaults();
        assert!(keys.normal.bound("abort").is_empty());
        assert!(keys.normal.matches("back", "Esc"));
    }

    #[test]
    fn a_partial_override_file_replaces_only_the_action_it_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.toml");
        std::fs::write(&path, "[normal]\nquit = [\"Q\"]\n").unwrap();

        let keys = KeyMap::load(Some(&path));
        assert!(keys.normal.matches("quit", "Q"));
        assert!(!keys.normal.matches("quit", "q"));
        assert!(keys.normal.matches("insert", "i"));
    }

    #[test]
    fn a_missing_override_path_falls_back_to_defaults_only() {
        let keys = KeyMap::load(Some(Path::new("/does/not/exist/keys.toml")));
        assert!(keys.normal.matches("quit", "Ctrl-c"));
        assert!(keys.normal.matches("stop", "q"));
    }
}
