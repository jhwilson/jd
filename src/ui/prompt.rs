use ratatui::crossterm::event::{KeyCode, KeyModifiers};
#[derive(Clone, Debug, Default)]
pub struct LineEditor {
    pub buffer: String,
    pub cursor: usize,
}
impl LineEditor {
    pub fn new(s: &str) -> Self {
        Self {
            buffer: s.into(),
            cursor: s.chars().count(),
        }
    }
    pub fn key(&mut self, c: KeyCode, m: KeyModifiers) {
        match (c, m) {
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => self.cursor = 0,
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                self.cursor = self.buffer.chars().count()
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.buffer.clear();
                self.cursor = 0
            }
            (KeyCode::Left, _) => self.cursor = self.cursor.saturating_sub(1),
            (KeyCode::Right, _) => self.cursor = (self.cursor + 1).min(self.buffer.chars().count()),
            (KeyCode::Backspace, _) if self.cursor > 0 => {
                let mut v: Vec<_> = self.buffer.chars().collect();
                v.remove(self.cursor - 1);
                self.cursor -= 1;
                self.buffer = v.into_iter().collect()
            }
            (KeyCode::Delete, _) => {
                let mut v: Vec<_> = self.buffer.chars().collect();
                if self.cursor < v.len() {
                    v.remove(self.cursor);
                    self.buffer = v.into_iter().collect()
                }
            }
            (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                let mut v: Vec<_> = self.buffer.chars().collect();
                v.insert(self.cursor, ch);
                self.cursor += 1;
                self.buffer = v.into_iter().collect()
            }
            _ => {}
        }
    }
}
