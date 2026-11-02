use crate::keys::is_plain_key;
use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, PartialEq)]
pub enum TextInputResult {
    /// Also returned for Backspace on an empty buffer.
    Changed,
    Submit,
    Cancel,
    Ignored,
}

pub struct TextInput<'a> {
    pub text: &'a mut String,
}

impl<'a> TextInput<'a> {
    pub fn new(text: &'a mut String) -> Self {
        Self { text }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> TextInputResult {
        match key.code {
            KeyCode::Esc => TextInputResult::Cancel,
            KeyCode::Enter => TextInputResult::Submit,
            KeyCode::Char(c) if is_plain_key(key) => {
                self.text.push(c);
                TextInputResult::Changed
            }
            KeyCode::Backspace => {
                self.text.pop();
                TextInputResult::Changed
            }
            _ => TextInputResult::Ignored,
        }
    }
}
