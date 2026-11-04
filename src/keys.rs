pub mod detail;
pub mod dropdown;
pub mod filter;
pub mod main_view;
mod reader;
pub mod report;
pub mod settings;
pub mod text_input;
pub mod token;

use crate::app::{App, ContextAction, DeletionTarget, Modal, View};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub fn handle_event(app: &mut App, event: Event) -> bool {
    match event {
        Event::Key(key) => handle_key(app, key),
        Event::Resize(_, _) => {}
        _ => return false,
    }
    true
}

pub fn is_help_available(app: &App) -> bool {
    matches!(app.modal, Modal::None)
        && app.context.dropdown.is_none()
        && !app.is_settings_text_input_focused()
        && !(app.navigation.view == View::Main
            && app.main.detail.fullscreen
            && app.is_reader_editing())
}

// Shift is already reflected in characters such as G and ?, so it does not disqualify them.
pub fn is_plain_key(key: KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

pub fn vim_alias(key: KeyEvent) -> KeyEvent {
    let code = match key.code {
        KeyCode::Char('h') if is_plain_key(key) => KeyCode::Left,
        KeyCode::Char('j') if is_plain_key(key) => KeyCode::Down,
        KeyCode::Char('k') if is_plain_key(key) => KeyCode::Up,
        KeyCode::Char('l') if is_plain_key(key) => KeyCode::Right,
        _ => return key,
    };
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        ..key
    }
}

fn with_code(key: KeyEvent, code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        ..key
    }
}

/// Text fields must bypass translation so Vim letters can be typed normally.
pub fn navigation_key(app: &mut App, key: KeyEvent) -> Option<KeyEvent> {
    let pending = std::mem::take(&mut app.navigation.pending_prefix);
    match key.code {
        KeyCode::Char('g') if is_plain_key(key) && pending => Some(with_code(key, KeyCode::Home)),
        KeyCode::Char('g') if is_plain_key(key) => {
            app.navigation.pending_prefix = true;
            None
        }
        KeyCode::Char('G') if is_plain_key(key) => Some(with_code(key, KeyCode::End)),
        _ => Some(vim_alias(key)),
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.kind == KeyEventKind::Release
        || (key.kind == KeyEventKind::Repeat && !is_repeat_allowed(app, key))
        || (!is_plain_key(key)
            && matches!(
                key.code,
                KeyCode::Enter
                    | KeyCode::Esc
                    | KeyCode::Tab
                    | KeyCode::BackTab
                    | KeyCode::Char(' ')
            ))
    {
        return;
    }
    let before = std::mem::discriminant(&app.modal);
    handle_key_inner(app, key);
    app.resume_filter_menu();
    app.focus_settings_text_field();
    if before != std::mem::discriminant(&app.modal) {
        app.navigation.pending_prefix = false;
    }
}

fn is_repeat_allowed(app: &App, key: KeyEvent) -> bool {
    if app.is_text_entry_focused() && matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace) {
        return true;
    }
    match key.code {
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Home
        | KeyCode::End
        | KeyCode::PageUp
        | KeyCode::PageDown => true,
        KeyCode::Tab | KeyCode::BackTab => {
            app.navigation.view == View::Main
                && app.main.detail.fullscreen
                && !app.is_reader_editing()
                && matches!(app.modal, Modal::None)
                && app.context.dropdown.is_none()
                && app.context.focused.is_none()
        }
        KeyCode::Char('h' | 'j' | 'k' | 'l') => is_plain_key(key),
        KeyCode::Char('u' | 'd') => key.modifiers == KeyModifiers::CONTROL,
        _ => false,
    }
}

// Modal handlers must put the modal back into app.modal to keep it open.
fn handle_key_inner(app: &mut App, key: KeyEvent) {
    if matches!(app.modal, Modal::Help) {
        let Some(key) = navigation_key(app, key) else {
            return;
        };
        if let Some(action) = navigation_action(key) {
            let page = usize::from(app.help.viewport_height);
            let scroll = action.apply(usize::from(app.help.scroll), u16::MAX.into(), page);
            app.help.scroll = scroll as u16;
        } else if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1)) {
            app.modal = Modal::None;
        }
        return;
    }
    if app.context.dropdown.is_some() {
        dropdown::handle_dropdown_key(app, key);
        return;
    }

    let modal = std::mem::replace(&mut app.modal, Modal::None);
    match modal {
        Modal::None => {}
        Modal::Help => unreachable!(),
        Modal::Error(msg) => {
            if !matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                app.modal = Modal::Error(msg);
            }
            return;
        }
        Modal::ConfirmDelete { target } => {
            match key.code {
                KeyCode::Enter => match target {
                    DeletionTarget::ReportEntry(number) => app.delete_report_entry(number),
                    DeletionTarget::QuickFilter(index) => app.delete_quick_filter(index),
                },
                KeyCode::Esc => {}
                _ => app.modal = Modal::ConfirmDelete { target },
            }
            return;
        }
        Modal::FilterMenu { selected } => {
            let Some(key) = navigation_key(app, key) else {
                app.modal = Modal::FilterMenu { selected };
                return;
            };
            filter::handle_filter_menu_key(app, key, selected);
            return;
        }
        Modal::NoteEditor { pr_number, text } => {
            report::handle_note_editor_key(app, key, pr_number, text);
            return;
        }
        Modal::SaveQuickFilter { name_text } => {
            filter::handle_save_quick_filter_key(app, key, name_text);
            return;
        }
    }

    if app.is_settings_text_input_focused() {
        app.navigation.pending_prefix = false;
        settings::handle_settings_key(app, key);
        return;
    }
    if app.navigation.view == View::Main && app.main.detail.fullscreen && app.is_reader_editing() {
        app.navigation.pending_prefix = false;

        if let Some(key) = reader::translate_key(app, key) {
            reader::handle_reader_key(app, key);
        }
        return;
    }

    let reader_focused = app.navigation.view == View::Main
        && app.main.detail.fullscreen
        && app.main.detail.reader.is_some();
    let key = if reader_focused {
        reader::translate_key(app, key)
    } else {
        navigation_key(app, key)
    };
    let Some(key) = key else {
        return;
    };
    if is_plain_key(key) {
        match key.code {
            KeyCode::Char('q') => {
                app.quit();
                return;
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                app.show_help();
                return;
            }
            KeyCode::Char('1' | '2' | '3') => {
                app.switch_view(match key.code {
                    KeyCode::Char('1') => View::Main,
                    KeyCode::Char('2') => View::Report,
                    _ => View::Settings,
                });
                return;
            }
            _ => {}
        }
    }
    if app.context.focused.is_some() && handle_context_bar_key(app, key) {
        return;
    }
    match app.navigation.view {
        View::Main => main_view::handle_main_key(app, key),
        View::Report => report::handle_report_key(app, key),
        View::Settings => settings::handle_settings_key(app, key),
    }
}

pub fn cycle(index: usize, len: usize, backwards: bool) -> usize {
    match len {
        0 => index,
        _ if backwards => (index + len - 1) % len,
        _ => (index + 1) % len,
    }
}

// Direction payloads use 1 for down and -1 for up.
pub(super) enum NavigationAction {
    Line(i8),
    Page(i8),
    HalfPage(i8),
    Home,
    End,
}

impl NavigationAction {
    /// Home and End return MIN/MAX sentinels; callers must clamp the resulting movement.
    pub(super) fn delta(self, page: usize) -> isize {
        let page = page.max(1) as isize;
        match self {
            Self::Line(direction) => isize::from(direction),
            Self::Page(direction) => page * isize::from(direction),
            Self::HalfPage(direction) => (page / 2).max(1) * isize::from(direction),
            Self::Home => isize::MIN,
            Self::End => isize::MAX,
        }
    }

    pub(super) fn apply(self, current: usize, max: usize, page: usize) -> usize {
        current.saturating_add_signed(self.delta(page)).min(max)
    }
}

pub(super) fn navigation_action(key: KeyEvent) -> Option<NavigationAction> {
    match key.code {
        KeyCode::Down => Some(NavigationAction::Line(1)),
        KeyCode::Up => Some(NavigationAction::Line(-1)),
        KeyCode::Home => Some(NavigationAction::Home),
        KeyCode::End => Some(NavigationAction::End),
        KeyCode::PageDown => Some(NavigationAction::Page(1)),
        KeyCode::PageUp => Some(NavigationAction::Page(-1)),
        KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
            Some(NavigationAction::HalfPage(1))
        }
        KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
            Some(NavigationAction::HalfPage(-1))
        }
        _ => None,
    }
}

pub fn list_navigation_target(
    key: KeyEvent,
    current: usize,
    len: usize,
    page: usize,
) -> Option<usize> {
    let action = navigation_action(key)?;
    Some(action.apply(current, len.saturating_sub(1), page))
}

fn handle_context_bar_key(app: &mut App, key: KeyEvent) -> bool {
    let Some(index) = app.context.focused else {
        return false;
    };
    let count = app.context.regions.len();
    if index >= count {
        app.context.focused = None;
        return false;
    }
    match key.code {
        KeyCode::Left | KeyCode::BackTab => {
            app.context.focused = index.checked_sub(1);
        }
        KeyCode::Right | KeyCode::Tab => {
            app.context.focused = (index + 1 < count).then_some(index + 1);
        }
        KeyCode::Home => app.context.focused = Some(0),
        KeyCode::End => app.context.focused = Some(count - 1),
        KeyCode::Enter | KeyCode::Char(' ') => {
            let action = app.context.regions[index].2;
            if matches!(action, ContextAction::ToggleDrafts) {
                app.toggle_drafts();
            } else if key.code == KeyCode::Enter {
                app.open_dropdown_for(action);
            }
        }
        KeyCode::Down | KeyCode::Esc => app.context.focused = None,
        KeyCode::Up | KeyCode::PageUp | KeyCode::PageDown => {}
        KeyCode::Char('u' | 'd') if key.modifiers == KeyModifiers::CONTROL => {}
        _ => return false,
    }
    true
}
