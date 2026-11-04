use super::{is_plain_key, vim_alias, with_code};
use crate::app::reader::ReaderAction;
use crate::app::{App, ReaderMode};
use crate::keys::text_input::{TextInput, TextInputResult};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) fn translate_key(app: &mut App, key: KeyEvent) -> Option<KeyEvent> {
    let Some(reader) = app.main.detail.reader.as_mut() else {
        return Some(key);
    };
    if reader.mode == ReaderMode::Search {
        reader.pending_prefix = None;
        return Some(key);
    }
    let pending = reader.pending_prefix.take();
    match key.code {
        KeyCode::Char('g') if is_plain_key(key) && pending == Some('g') => {
            // Visual selection handles gg separately from Home (start of line).
            Some(if reader.mode == ReaderMode::Normal {
                with_code(key, KeyCode::Home)
            } else {
                key
            })
        }
        KeyCode::Char('g') if is_plain_key(key) => {
            reader.pending_prefix = Some('g');
            None
        }
        KeyCode::Char('G') if is_plain_key(key) && reader.mode == ReaderMode::Normal => {
            Some(with_code(key, KeyCode::End))
        }
        _ => Some(vim_alias(key)),
    }
}

pub(super) fn is_search_key(key: KeyEvent) -> bool {
    (key.code == KeyCode::Char('/') && is_plain_key(key))
        || (key.code == KeyCode::Char('f') && key.modifiers == KeyModifiers::CONTROL)
}

pub(super) fn is_copy_key(key: KeyEvent) -> bool {
    (matches!(key.code, KeyCode::Char('c' | 'y')) && is_plain_key(key))
        || (key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL)
}

pub fn handle_reader_key(app: &mut App, key: KeyEvent) {
    let Some(reader) = app.main.detail.reader.as_mut() else {
        return;
    };
    let action = if reader.mode == ReaderMode::Search {
        match TextInput::new(&mut reader.search_query).handle_key(key) {
            TextInputResult::Cancel => Some(ReaderAction::CancelSearch),
            TextInputResult::Submit => Some(ReaderAction::SubmitSearch),
            _ => None,
        }
    } else {
        reader_action(key)
    };
    if let Some(action) = action {
        app.apply_reader_action(action);
    }
}

fn reader_action(key: KeyEvent) -> Option<ReaderAction> {
    if is_search_key(key) {
        return Some(ReaderAction::StartSearch);
    }
    if is_copy_key(key) {
        return Some(ReaderAction::Copy);
    }
    if key.code == KeyCode::F(3) {
        return Some(ReaderAction::SearchMatch {
            forward: !key.modifiers.contains(KeyModifiers::SHIFT),
        });
    }
    if !is_plain_key(key)
        && !(matches!(key.code, KeyCode::Char('u' | 'd')) && key.modifiers == KeyModifiers::CONTROL)
    {
        return None;
    }
    Some(match key.code {
        KeyCode::Esc => ReaderAction::CancelVisual,
        KeyCode::Left => ReaderAction::Left,
        KeyCode::Right => ReaderAction::Right,
        KeyCode::Down => ReaderAction::Down,
        KeyCode::Up => ReaderAction::Up,
        KeyCode::Char('w') => ReaderAction::WordForward,
        KeyCode::Char('b') => ReaderAction::WordBackward,
        KeyCode::Char('e') => ReaderAction::WordEnd,
        KeyCode::Char('0') | KeyCode::Home => ReaderAction::LineStart,
        KeyCode::Char('^') => ReaderAction::FirstNonWhitespace,
        KeyCode::Char('$') | KeyCode::End => ReaderAction::LineEnd,
        KeyCode::Char('g') => ReaderAction::DocumentStart,
        KeyCode::Char('G') => ReaderAction::DocumentEnd,
        KeyCode::PageDown => ReaderAction::PageDown,
        KeyCode::PageUp => ReaderAction::PageUp,
        KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => ReaderAction::HalfPageDown,
        KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => ReaderAction::HalfPageUp,
        KeyCode::Char('n') => ReaderAction::SearchMatch { forward: true },
        KeyCode::Char('N') => ReaderAction::SearchMatch { forward: false },
        KeyCode::Char('v' | 'V') => ReaderAction::StartVisual {
            linewise: key.code == KeyCode::Char('V'),
        },
        KeyCode::Enter | KeyCode::Char(' ') => ReaderAction::Activate {
            open_links: key.code == KeyCode::Enter,
        },
        KeyCode::Char(']') => ReaderAction::NextCodeBlock,
        KeyCode::Char('[') => ReaderAction::PreviousCodeBlock,
        _ => return None,
    })
}
