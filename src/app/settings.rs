use super::{App, DeletionTarget, Modal, View};
use crate::config::{self, Preferences};
use crate::{github::GitHubClient, types::QuickFilter};
use ratatui::layout::Rect;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsSection {
    #[default]
    GitHub,
    QuickFilters,
    Display,
}

impl SettingsSection {
    pub const ALL: [Self; 3] = [Self::GitHub, Self::QuickFilters, Self::Display];

    pub fn name(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::QuickFilters => "Quick Filters",
            Self::Display => "Display",
        }
    }

    pub(super) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|section| *section == self)
            .expect("section in ALL")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenField {
    Token,
    SaveToKeyring,
    Connect,
    Cancel,
}

impl TokenField {
    const ALL: [Self; 4] = [
        Self::Token,
        Self::SaveToKeyring,
        Self::Connect,
        Self::Cancel,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickFilterField {
    Name,
    Label,
    Author,
    InactiveDays,
    InactivitySort,
    Drafts,
    Save,
    Cancel,
    Delete,
}

impl QuickFilterField {
    // Delete comes last because new presets do not have it.
    const ALL: [Self; 9] = [
        Self::Name,
        Self::Label,
        Self::Author,
        Self::InactiveDays,
        Self::InactivitySort,
        Self::Drafts,
        Self::Save,
        Self::Cancel,
        Self::Delete,
    ];

    fn form(saved: bool) -> &'static [Self] {
        if saved {
            &Self::ALL
        } else {
            &Self::ALL[..Self::ALL.len() - 1]
        }
    }

    pub fn is_text(self) -> bool {
        matches!(
            self,
            Self::Name | Self::Label | Self::Author | Self::InactiveDays
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum SettingsSelection {
    GitHubOverview(usize),
    Token(TokenField),
    QuickFilterList(usize),
    QuickFilter(QuickFilterField),
    Display,
}

impl Default for SettingsSelection {
    fn default() -> Self {
        Self::GitHubOverview(0)
    }
}

pub struct TokenDraft {
    pub text: String,
    pub save_to_keyring: bool,
    pub error: Option<String>,
}

impl Default for TokenDraft {
    fn default() -> Self {
        Self {
            text: String::new(),
            save_to_keyring: true,
            error: None,
        }
    }
}

pub struct QuickFilterEditor {
    pub draft: QuickFilter,
    pub source_index: Option<usize>,
    pub input: Option<String>,
}

#[derive(Default)]
enum SettingsEditor {
    #[default]
    None,
    Token(TokenDraft),
    QuickFilter(QuickFilterEditor),
}

#[derive(Default)]
pub struct SettingsState {
    pub selected_section: SettingsSection,
    selection: SettingsSelection,
    editor: SettingsEditor,
    // Recorded while drawing so the label dropdown can anchor to its field.
    pub label_input_rect: Option<Rect>,
}

impl SettingsState {
    pub fn new(needs_token: bool) -> Self {
        Self {
            editor: if needs_token {
                SettingsEditor::Token(TokenDraft::default())
            } else {
                SettingsEditor::None
            },
            selection: if needs_token {
                SettingsSelection::Token(TokenField::Token)
            } else {
                SettingsSelection::default()
            },
            ..Self::default()
        }
    }

    pub fn token(&self) -> Option<&TokenDraft> {
        match &self.editor {
            SettingsEditor::Token(draft) => Some(draft),
            _ => None,
        }
    }

    pub fn token_mut(&mut self) -> Option<&mut TokenDraft> {
        match &mut self.editor {
            SettingsEditor::Token(draft) => Some(draft),
            _ => None,
        }
    }

    pub fn quick_filter(&self) -> Option<&QuickFilterEditor> {
        match &self.editor {
            SettingsEditor::QuickFilter(editor) => Some(editor),
            _ => None,
        }
    }

    pub fn quick_filter_mut(&mut self) -> Option<&mut QuickFilterEditor> {
        match &mut self.editor {
            SettingsEditor::QuickFilter(editor) => Some(editor),
            _ => None,
        }
    }

    pub fn text_input(&self) -> Option<&str> {
        self.quick_filter()
            .and_then(|editor| editor.input.as_deref())
    }

    pub fn selected_index(&self) -> usize {
        match self.selection {
            SettingsSelection::GitHubOverview(index)
            | SettingsSelection::QuickFilterList(index) => index,
            SettingsSelection::Token(field) => TokenField::ALL
                .iter()
                .position(|candidate| *candidate == field)
                .unwrap_or_default(),
            SettingsSelection::QuickFilter(field) => QuickFilterField::ALL
                .iter()
                .position(|candidate| *candidate == field)
                .unwrap_or_default(),
            SettingsSelection::Display => 0,
        }
    }

    pub fn select_index(&mut self, index: usize) {
        self.selection = match (&self.editor, self.selected_section) {
            (SettingsEditor::Token(_), SettingsSection::GitHub) => SettingsSelection::Token(
                TokenField::ALL
                    .get(index)
                    .copied()
                    .unwrap_or(TokenField::Token),
            ),
            (SettingsEditor::QuickFilter(editor), SettingsSection::QuickFilters) => {
                let fields = QuickFilterField::form(editor.source_index.is_some());
                SettingsSelection::QuickFilter(
                    fields.get(index).copied().unwrap_or(QuickFilterField::Name),
                )
            }
            (_, SettingsSection::GitHub) => SettingsSelection::GitHubOverview(index.min(1)),
            (_, SettingsSection::QuickFilters) => SettingsSelection::QuickFilterList(index),
            (_, SettingsSection::Display) => SettingsSelection::Display,
        };
    }

    pub fn token_field(&self) -> Option<TokenField> {
        let SettingsSelection::Token(field) = self.selection else {
            return None;
        };
        Some(field)
    }

    pub fn quick_filter_field(&self) -> Option<QuickFilterField> {
        let SettingsSelection::QuickFilter(field) = self.selection else {
            return None;
        };
        Some(field)
    }
}

impl App {
    pub fn is_settings_text_input_focused(&self) -> bool {
        self.navigation.view == View::Settings
            && self.context.focused.is_none()
            && self.context.dropdown.is_none()
            && (self.settings.text_input().is_some()
                || (self.settings.selected_section == SettingsSection::GitHub
                    && self.settings.token().is_some()
                    && self.settings.token_field() == Some(TokenField::Token))
                || (self.settings.selected_section == SettingsSection::QuickFilters
                    && self.settings.quick_filter().is_some()
                    && self
                        .settings
                        .quick_filter_field()
                        .is_some_and(QuickFilterField::is_text)))
    }

    pub fn focus_settings_text_field(&mut self) {
        if self.settings.text_input().is_some()
            || !self.is_settings_text_input_focused()
            || !matches!(self.modal, Modal::None)
        {
            return;
        }
        let Some(field) = self.settings.quick_filter_field() else {
            return;
        };
        let Some(editor) = self.settings.quick_filter_mut() else {
            return;
        };
        editor.input = Some(match field {
            QuickFilterField::Name => editor.draft.name.clone(),
            QuickFilterField::Label => editor.draft.filter.label.clone(),
            QuickFilterField::Author => editor.draft.filter.author.clone(),
            QuickFilterField::InactiveDays => editor
                .draft
                .filter
                .min_inactive_days
                .map(|days| days.to_string())
                .unwrap_or_default(),
            _ => return,
        });
    }

    pub fn settings_field_count(&self) -> usize {
        match self.settings.selected_section {
            SettingsSection::GitHub if self.settings.token().is_some() => 4,
            SettingsSection::GitHub => 2,
            SettingsSection::QuickFilters => self
                .settings
                .quick_filter()
                .map_or(self.main.quick_filters.len() + 1, |editor| {
                    QuickFilterField::form(editor.source_index.is_some()).len()
                }),
            SettingsSection::Display => 1,
        }
    }

    pub fn select_settings_section(&mut self, section: SettingsSection) {
        if self.settings.selected_section != section {
            let selected = self.settings.selected_index();
            self.cancel_settings_quick_filter();
            self.cancel_settings_token();
            self.settings.selected_section = section;
            self.settings
                .select_index(selected.min(self.settings_field_count().saturating_sub(1)));
        }
        self.context.focused = None;
    }

    pub fn cancel_settings_token(&mut self) {
        if self.settings.token().is_some() {
            self.settings.editor = SettingsEditor::None;
            self.settings.select_index(0);
        }
    }

    pub fn cancel_settings_quick_filter(&mut self) {
        if let Some(editor) = self.settings.quick_filter() {
            let selected = editor.source_index.unwrap_or(self.main.quick_filters.len());
            self.settings.editor = SettingsEditor::None;
            self.settings.select_index(selected);
        }
    }

    pub fn apply_settings_label_selection(&mut self, label: &str) {
        if self.navigation.view == View::Settings
            && self.settings.selected_section == SettingsSection::QuickFilters
            && self.settings.quick_filter_field() == Some(QuickFilterField::Label)
            && let Some(editor) = self.settings.quick_filter_mut()
        {
            editor.draft.filter.label = label.to_string();
            editor.input = Some(label.to_string());
        }
    }

    pub fn activate_settings_field(&mut self) {
        match self.settings.selection {
            SettingsSelection::GitHubOverview(0) => {
                self.settings.editor = SettingsEditor::Token(TokenDraft::default());
                self.settings.selection = SettingsSelection::Token(TokenField::Token);
            }
            SettingsSelection::QuickFilter(field) => {
                if let Some(editor) = self.settings.quick_filter_mut() {
                    match field {
                        QuickFilterField::InactivitySort => {
                            editor.draft.filter.inactivity_sort =
                                editor.draft.filter.inactivity_sort.toggled()
                        }
                        QuickFilterField::Drafts => {
                            editor.draft.filter.show_drafts = !editor.draft.filter.show_drafts
                        }
                        QuickFilterField::Save => self.save_quick_filter(),
                        QuickFilterField::Cancel => self.cancel_settings_quick_filter(),
                        QuickFilterField::Delete => {
                            if let Some(index) = editor.source_index {
                                self.modal = Modal::ConfirmDelete {
                                    target: DeletionTarget::QuickFilter(index),
                                };
                            }
                        }
                        _ => {}
                    }
                }
            }
            SettingsSelection::QuickFilterList(field) => {
                let source_index = (field < self.main.quick_filters.len()).then_some(field);
                let draft = self
                    .main
                    .quick_filters
                    .get(field)
                    .cloned()
                    .unwrap_or(QuickFilter {
                        name: String::new(),
                        filter: Default::default(),
                    });
                self.settings.editor = SettingsEditor::QuickFilter(QuickFilterEditor {
                    draft,
                    source_index,
                    input: None,
                });
                self.settings.selection = SettingsSelection::QuickFilter(QuickFilterField::Name);
            }
            SettingsSelection::Display => {
                self.main.nerd_font = !self.main.nerd_font;
                self.main.detail.fullscreen_cache = None;
                let state = if self.main.nerd_font {
                    "enabled"
                } else {
                    "disabled"
                };
                self.save_settings(format!("Nerd Font icons {state}."));
            }
            _ => {}
        }
    }

    pub fn apply_settings_edit(&mut self) -> bool {
        let Some(field) = self.settings.quick_filter_field() else {
            return true;
        };
        let Some(editor) = self.settings.quick_filter_mut() else {
            return true;
        };
        let Some(input) = &editor.input else {
            return true;
        };
        let value = input.trim().to_string();
        match field {
            QuickFilterField::Name => editor.draft.name = value,
            QuickFilterField::Label => editor.draft.filter.label = value,
            QuickFilterField::Author => editor.draft.filter.author = value,
            QuickFilterField::InactiveDays => {
                editor.draft.filter.min_inactive_days = if value.is_empty() {
                    None
                } else if let Ok(days) = value.parse::<u64>() {
                    Some(days)
                } else {
                    self.set_status_message(
                        "Inactive days must be a non-negative whole number, or empty for off.",
                    );
                    return false;
                };
            }
            _ => {}
        }
        editor.input = None;
        true
    }

    fn save_quick_filter(&mut self) {
        let Some(editor) = self.settings.quick_filter() else {
            return;
        };
        if editor.draft.name.trim().is_empty() {
            self.set_status_message("Enter a name for the quick filter.");
            self.settings.selection = SettingsSelection::QuickFilter(QuickFilterField::Name);
            return;
        }
        let mut filters = self.main.quick_filters.clone();
        let index = editor.source_index.unwrap_or(filters.len());
        if let Some(filter) = filters.get_mut(index) {
            *filter = editor.draft.clone();
        } else {
            filters.push(editor.draft.clone());
        }
        match self.save_preferences(|prefs| prefs.quick_filters = filters) {
            Ok(()) => {
                self.main.set_custom_filter();
                self.settings.editor = SettingsEditor::None;
                self.settings.select_index(index);
                self.set_status_message("Quick filter saved.");
            }
            Err(error) => self.set_status_message(format!("Save failed: {error}")),
        }
    }

    fn save_settings(&mut self, success_message: String) {
        match self.save_preferences(|_| {}) {
            Ok(()) => self.set_status_message(success_message),
            Err(error) => self.set_status_message(format!("Save failed: {error}")),
        }
    }

    pub fn save_preferences(
        &mut self,
        update: impl FnOnce(&mut Preferences),
    ) -> anyhow::Result<()> {
        let mut prefs = Preferences {
            quick_filters: self.main.quick_filters.clone(),
            default_filter: self.main.default_filter.clone(),
            nerd_font: self.main.nerd_font,
        };
        update(&mut prefs);
        config::save_config_file(&prefs)?;
        self.main.quick_filters = prefs.quick_filters;
        self.main.default_filter = prefs.default_filter;
        self.main.nerd_font = prefs.nerd_font;
        Ok(())
    }

    pub fn connect_settings_token(&mut self) {
        let Some(draft) = self.settings.token_mut() else {
            return;
        };
        let token = draft.text.trim().to_string();
        if token.is_empty() {
            draft.error = Some("Enter a personal access token before connecting.".into());
            self.settings.selection = SettingsSelection::Token(TokenField::Token);
            return;
        }
        let client = match GitHubClient::new(&token) {
            Ok(client) => client,
            Err(_) => {
                draft.error = Some("Could not use this token. Check it and try again.".into());
                self.settings.selection = SettingsSelection::Token(TokenField::Token);
                return;
            }
        };
        let save_to_keyring = draft.save_to_keyring;
        let first_run = self.needs_token();
        self.set_github_client(Arc::new(client));
        self.cancel_settings_token();
        if !save_to_keyring {
            self.set_status_message("Token accepted (session only, not saved).");
        } else if let Err(error) = config::save_to_keyring(&token) {
            self.modal = Modal::Error(format!(
                "Could not save the token to the system keyring: {error:#}\n\n\
                 The token is used for this session only."
            ));
        } else {
            self.set_status_message("Token saved to system keyring.");
        }
        if first_run {
            self.navigation.view = View::Main;
        }
    }
}
