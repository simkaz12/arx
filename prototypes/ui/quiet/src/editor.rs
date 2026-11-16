use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const COMMANDS: [&str; 3] = ["/reset", "/model", "/checkpoint"];

pub enum EditorOutcome {
    Continue,
    Submit(String),
    Quit,
    Cleared,
}

pub struct Editor {
    pub buffer: Vec<char>,
    pub cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    saved: Vec<char>,
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            buffer: Vec::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            saved: Vec::new(),
        }
    }

    pub fn text(&self) -> String {
        self.buffer.iter().collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditorOutcome {
        match (key.code, key.modifiers) {
            (KeyCode::Enter, m) if m.contains(KeyModifiers::ALT) => {
                self.insert('\n');
                EditorOutcome::Continue
            }
            (KeyCode::Enter, _) => {
                let text = self.text();
                if !text.is_empty() {
                    self.history.push(text.clone());
                }
                self.buffer.clear();
                self.cursor = 0;
                self.history_index = None;
                EditorOutcome::Submit(text)
            }
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                if self.buffer.is_empty() {
                    EditorOutcome::Quit
                } else {
                    self.buffer.clear();
                    self.cursor = 0;
                    EditorOutcome::Cleared
                }
            }
            (KeyCode::Char('w'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.delete_word_back();
                EditorOutcome::Continue
            }
            (KeyCode::Left, _) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                EditorOutcome::Continue
            }
            (KeyCode::Right, _) => {
                if self.cursor < self.buffer.len() {
                    self.cursor += 1;
                }
                EditorOutcome::Continue
            }
            (KeyCode::Home, _) => {
                self.cursor = 0;
                EditorOutcome::Continue
            }
            (KeyCode::End, _) => {
                self.cursor = self.buffer.len();
                EditorOutcome::Continue
            }
            (KeyCode::Backspace, m) if m.contains(KeyModifiers::CONTROL) || m.contains(KeyModifiers::ALT) => {
                self.delete_word_back();
                EditorOutcome::Continue
            }
            (KeyCode::Backspace, _) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.buffer.remove(self.cursor);
                }
                EditorOutcome::Continue
            }
            (KeyCode::Delete, _) => {
                if self.cursor < self.buffer.len() {
                    self.buffer.remove(self.cursor);
                }
                EditorOutcome::Continue
            }
            (KeyCode::Up, _) => {
                self.history_prev();
                EditorOutcome::Continue
            }
            (KeyCode::Down, _) => {
                self.history_next();
                EditorOutcome::Continue
            }
            (KeyCode::Tab, _) => {
                self.complete();
                EditorOutcome::Continue
            }
            (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
                self.insert(c);
                EditorOutcome::Continue
            }
            _ => EditorOutcome::Continue,
        }
    }

    fn insert(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn delete_word_back(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut i = self.cursor;
        while i > 0 && self.buffer[i - 1] == ' ' {
            i -= 1;
        }
        while i > 0 && self.buffer[i - 1] != ' ' {
            i -= 1;
        }
        self.buffer.drain(i..self.cursor);
        self.cursor = i;
    }

    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            None => {
                self.saved = self.buffer.clone();
                self.history.len() - 1
            }
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.history_index = Some(idx);
        self.buffer = self.history[idx].chars().collect();
        self.cursor = self.buffer.len();
    }

    fn history_next(&mut self) {
        match self.history_index {
            None => {}
            Some(i) if i + 1 < self.history.len() => {
                self.history_index = Some(i + 1);
                self.buffer = self.history[i + 1].chars().collect();
                self.cursor = self.buffer.len();
            }
            Some(_) => {
                self.history_index = None;
                self.buffer = self.saved.clone();
                self.cursor = self.buffer.len();
            }
        }
    }

    fn complete(&mut self) {
        let text = self.text();
        if !text.starts_with('/') || text.contains(' ') {
            return;
        }
        let matches: Vec<&str> = COMMANDS.iter().copied().filter(|c| c.starts_with(text.as_str())).collect();
        if matches.len() == 1 && matches[0] != text {
            self.buffer = format!("{} ", matches[0]).chars().collect();
            self.cursor = self.buffer.len();
        }
    }
}
