use crate::app::{
    App, ContextAction, DeletionTarget, Modal, QuickFilterField, SettingsSection, View,
};
use crate::keys::cycle;
use crate::keys::is_plain_key;
use crate::keys::text_input::TextInput;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_settings_key(app: &mut App, key: KeyEvent) {
    if app.settings.selected_section == SettingsSection::GitHub && app.settings.token().is_some() {
        super::token::handle_key(app, key);
        return;
    }
    app.focus_settings_text_field();
    if app.settings.text_input().is_some() {
        handle_editing_key(app, key);
    } else {
        handle_navigation_key(app, key);
    }
}

fn handle_navigation_key(app: &mut App, key: KeyEvent) {
    let selected = app.settings.selected_index();
    if key.code == KeyCode::Up && selected == 0 {
        app.context.focus(false);
        return;
    }
    if let Some(index) = super::list_navigation_target(
        key,
        selected,
        app.settings_field_count(),
        app.navigation.page_rows,
    ) {
        app.settings.select_index(index);
        return;
    }
    if !is_plain_key(key) {
        return;
    }
    match key.code {
        KeyCode::Delete | KeyCode::Char('x')
            if app.context.focused.is_none()
                && app.settings.selected_section == SettingsSection::QuickFilters
                && app.settings.quick_filter().is_none()
                && selected < app.main.quick_filters.len() =>
        {
            app.modal = Modal::ConfirmDelete {
                target: DeletionTarget::QuickFilter(selected),
            };
        }
        KeyCode::Left => app.context.focus(false),
        KeyCode::Tab | KeyCode::BackTab if app.settings.quick_filter().is_some() => {
            move_quick_filter_field(app, key.code == KeyCode::BackTab);
        }
        KeyCode::Tab | KeyCode::BackTab => app.context.focus(key.code == KeyCode::BackTab),
        KeyCode::Esc if app.settings.quick_filter().is_some() => app.cancel_settings_quick_filter(),
        KeyCode::Esc => {
            app.context.focused = None;
            app.navigation.view = View::Main;
        }
        KeyCode::Enter => app.activate_settings_field(),
        KeyCode::Char(' ')
            if (app.settings.selected_section == SettingsSection::Display && selected == 0)
                || (app.settings.selected_section == SettingsSection::QuickFilters
                    && app.settings.quick_filter().is_some()
                    && matches!(
                        app.settings.quick_filter_field(),
                        Some(QuickFilterField::InactivitySort | QuickFilterField::Drafts)
                    )) =>
        {
            app.activate_settings_field();
        }
        _ => {}
    }
}

fn handle_editing_key(app: &mut App, key: KeyEvent) {
    if !is_plain_key(key) {
        return;
    }
    if key.code == KeyCode::Esc {
        app.cancel_settings_quick_filter();
        return;
    }
    if app.settings.quick_filter_field() == Some(QuickFilterField::Label)
        && matches!(
            key.code,
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Enter
        )
    {
        let mut query = app.settings.text_input().unwrap_or_default().to_string();
        TextInput::new(&mut query).handle_key(key);
        app.open_context_dropdown(ContextAction::EditQuickFilterLabel, 0, 8);
        app.context.dropdown.as_mut().unwrap().search_query = query;
        return;
    }
    match key.code {
        KeyCode::Tab | KeyCode::BackTab | KeyCode::Enter => {
            if app.apply_settings_edit() {
                move_quick_filter_field(app, key.code == KeyCode::BackTab);
            }
            return;
        }
        KeyCode::Left => {
            if app.apply_settings_edit() {
                app.context.focus(false);
            }
            return;
        }
        _ => {}
    }
    if let Some(index) = super::list_navigation_target(
        key,
        app.settings.selected_index(),
        app.settings_field_count(),
        app.navigation.page_rows,
    ) {
        if app.apply_settings_edit() {
            if key.code == KeyCode::Up && app.settings.selected_index() == 0 {
                app.context.focus(false);
            } else {
                app.settings.select_index(index);
            }
        }
        return;
    }
    if let Some(input) = app
        .settings
        .quick_filter_mut()
        .and_then(|editor| editor.input.as_mut())
    {
        TextInput::new(input).handle_key(key);
    }
}

fn move_quick_filter_field(app: &mut App, backwards: bool) {
    let len = app.settings_field_count();
    let selected = app.settings.selected_index();
    app.settings.select_index(cycle(selected, len, backwards));
}
