use ratatui::style::{Color, Modifier, Style};
use std::sync::OnceLock;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    mem::size_of,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::ui::theme as ui_theme;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

type HighlightedLines = Vec<Vec<(String, Style)>>;

struct CachedHighlight {
    rows: HighlightedLines,
    bytes: usize,
}

#[derive(Default)]
struct CacheState {
    languages: HashMap<String, HashMap<String, CachedHighlight>>,
    order: VecDeque<(String, String)>,
    bytes: usize,
}

/// Shared by the app's rendering paths; interior mutability keeps view borrows read-only.
pub struct HighlightCache {
    state: RefCell<CacheState>,
    max_bytes: usize,
    max_entries: usize,
}

impl Default for HighlightCache {
    fn default() -> Self {
        Self {
            state: RefCell::default(),
            max_bytes: 16 * 1024 * 1024,
            max_entries: 1024,
        }
    }
}

impl HighlightCache {
    pub(super) fn highlight(&self, code: &str, lang: Option<&str>) -> HighlightedLines {
        let language = lang.unwrap_or("");
        let mut state = self.state.borrow_mut();
        if let Some(entry) = state
            .languages
            .get(language)
            .and_then(|entries| entries.get(code))
        {
            return entry.rows.clone();
        }
        let rows = highlight(code, lang);
        // Include both lookup/order keys, row allocations, and basic entry overhead.
        let bytes = 2 * (code.len() + language.len())
            + size_of::<CachedHighlight>()
            + size_of::<(String, String)>()
            + size_of::<String>()
            + rows.capacity() * size_of::<Vec<(String, Style)>>()
            + rows
                .iter()
                .map(|row| {
                    row.capacity() * size_of::<(String, Style)>()
                        + row.iter().map(|(text, _)| text.capacity()).sum::<usize>()
                })
                .sum::<usize>();
        if bytes > self.max_bytes || self.max_entries == 0 {
            return rows;
        }
        while state.order.len() >= self.max_entries || state.bytes > self.max_bytes - bytes {
            let (language, code) = state.order.pop_front().unwrap();
            let entries = state.languages.get_mut(&language).unwrap();
            let old = entries.remove(&code).unwrap();
            if entries.is_empty() {
                state.languages.remove(&language);
            }
            state.bytes -= old.bytes;
        }
        state
            .languages
            .entry(language.to_owned())
            .or_default()
            .insert(
                code.to_owned(),
                CachedHighlight {
                    rows: rows.clone(),
                    bytes,
                },
            );
        state
            .order
            .push_back((language.to_owned(), code.to_owned()));
        state.bytes += bytes;
        rows
    }
}

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| {
        let themes = ThemeSet::load_defaults();
        let mut selected = themes
            .themes
            .get("base16-ocean.dark")
            .or_else(|| themes.themes.values().next())
            .expect("at least one built-in theme")
            .clone();
        // The theme's comment colours are too dim on our background; brighten them once.
        let luminance = |r: u8, g: u8, b: u8| {
            let linear = |channel: u8| {
                let value = f64::from(channel) / 255.0;
                if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        };
        let Color::Rgb(r, g, b) = ui_theme::CODE_BORDER.fg.unwrap() else {
            unreachable!("code metadata uses an explicit RGB foreground");
        };
        let minimum = luminance(r, g, b);
        for foreground in std::iter::once(&mut selected.settings.foreground)
            .chain(
                selected
                    .scopes
                    .iter_mut()
                    .map(|item| &mut item.style.foreground),
            )
            .flatten()
        {
            while luminance(foreground.r, foreground.g, foreground.b) < minimum {
                foreground.r = foreground.r.saturating_add(1);
                foreground.g = foreground.g.saturating_add(1);
                foreground.b = foreground.b.saturating_add(1);
            }
        }
        selected
    })
}

fn highlight(code: &str, lang: Option<&str>) -> HighlightedLines {
    let syntaxes = syntax_set();
    let theme = theme();

    let syntax = lang
        .and_then(|language| syntaxes.find_syntax_by_token(language))
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    // Preserve newlines while highlighting so multiline syntax keeps its state.
    for line in LinesWithEndings::from(code) {
        let spans = match highlighter.highlight_line(line, syntaxes) {
            Ok(spans) => spans,
            Err(_) => {
                lines.push(vec![(
                    line.trim_end_matches('\n').to_string(),
                    Style::default(),
                )]);
                continue;
            }
        };

        let row: Vec<(String, Style)> = spans
            .into_iter()
            .filter_map(|(style, text)| {
                let text = text.trim_end_matches('\n');
                if text.is_empty() {
                    return None;
                }
                Some((text.to_string(), syntect_style(style)))
            })
            .collect();

        lines.push(row);
    }

    lines
}

fn syntect_style(s: syntect::highlighting::Style) -> Style {
    let fg = s.foreground;
    let mut r = Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b));
    if s.font_style.contains(FontStyle::BOLD) {
        r = r.add_modifier(Modifier::BOLD);
    }
    if s.font_style.contains(FontStyle::ITALIC) {
        r = r.add_modifier(Modifier::ITALIC);
    }
    if s.font_style.contains(FontStyle::UNDERLINE) {
        r = r.add_modifier(Modifier::UNDERLINED);
    }
    r
}
