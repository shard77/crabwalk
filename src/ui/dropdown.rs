use crate::app::{App, ContextAction};
use crate::ui::{theme, widgets::Canvas};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

pub(super) fn draw(f: &mut Frame, app: &mut App, size: Rect) {
    let Some(dropdown) = app.context.dropdown.as_ref() else {
        return;
    };
    let anchor = if dropdown.action == ContextAction::EditQuickFilterLabel {
        app.settings.label_input_rect
    } else {
        app.context.rect
    };
    let Some(anchor) = anchor else {
        return;
    };
    let labels_loading = matches!(
        dropdown.action,
        ContextAction::FilterLabel | ContextAction::EditQuickFilterLabel
    ) && app.background.is_loading_labels();
    let items = if labels_loading {
        Vec::new()
    } else {
        app.dropdown_filtered_items()
    };
    let placeholder = if labels_loading {
        Some("Loading labels…")
    } else if dropdown.action == ContextAction::FilterAuthor {
        Some("(type author name, Enter to apply)")
    } else {
        dropdown.action.has_search().then_some("(no matches)")
    };
    let dropdown = app
        .context
        .dropdown
        .as_mut()
        .expect("dropdown checked above");
    dropdown.viewport_height = Dropdown {
        anchor,
        anchor_col: dropdown.anchor_col,
        anchor_width: dropdown.anchor_width,
        search: dropdown
            .action
            .has_search()
            .then_some(dropdown.search_query.as_str()),
        items: &items,
        selected: dropdown.selected,
        placeholder,
    }
    .draw(f, size);
}

struct Dropdown<'a> {
    anchor: Rect,
    anchor_col: u16,
    anchor_width: u16,
    search: Option<&'a str>,
    items: &'a [String],
    selected: usize,
    placeholder: Option<&'a str>,
}

const EMPTY_ITEM_LABEL: &str = "(any label)";

impl Dropdown<'_> {
    fn draw(self, f: &mut Frame, terminal_size: Rect) -> u16 {
        let display_items: Vec<String> = if self.items.is_empty() {
            self.placeholder.into_iter().map(str::to_owned).collect()
        } else {
            self.items
                .iter()
                .map(|item| {
                    if item.is_empty() {
                        EMPTY_ITEM_LABEL.to_string()
                    } else {
                        item.clone()
                    }
                })
                .collect()
        };
        let search_rows = u16::from(self.search.is_some());
        let visible_count = (display_items.len() as u16).min(10);
        let height = visible_count + 2 + search_rows;
        let longest_item = display_items
            .iter()
            .map(|s| Span::raw(s).width())
            .max()
            .unwrap_or(10);
        let search_width = self.search.map_or(0, |query| Span::raw(query).width() + 4);
        let min_width = self
            .anchor_width
            .max(longest_item as u16 + 4)
            .max(search_width as u16 + 2);
        let width = min_width.min(terminal_size.width.saturating_sub(2));
        let x = (self.anchor.x + self.anchor_col).min(terminal_size.width.saturating_sub(width));
        let y = self.anchor.bottom();
        let height = height.min(terminal_size.height.saturating_sub(y));
        if height < 3 {
            return 0;
        }

        let popup = Rect::new(x, y, width, height);
        f.render_widget(Canvas, popup);
        let inner = popup.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        f.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::DROPDOWN_BORDER),
            popup,
        );
        let items_area = if let Some(query) = self.search {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);
            let cursor = if query.is_empty() { "…" } else { "_" };
            f.render_widget(
                Paragraph::new(format!("{query}{cursor}")).style(theme::INPUT_TEXT),
                chunks[0],
            );
            chunks[1]
        } else {
            inner
        };

        let items: Vec<ListItem> = display_items
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let style = if i == self.selected && !self.items.is_empty() {
                    theme::CHOICE_SELECTED
                } else if self.items.is_empty() {
                    theme::PLACEHOLDER
                } else {
                    theme::DROPDOWN_ITEM
                };
                ListItem::new(Line::from(Span::styled(format!(" {label} "), style)))
            })
            .collect();
        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(
                self.selected.min(display_items.len().saturating_sub(1)),
            ));
        }
        f.render_stateful_widget(List::new(items), items_area, &mut state);
        items_area.height.min(self.items.len() as u16)
    }
}
