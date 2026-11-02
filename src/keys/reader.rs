use super::{is_plain_key, vim_alias, with_code};
use crate::app::reader::navigation::*;
use crate::app::{App, ReaderMode, ReaderState};
use crate::keys::text_input::{TextInput, TextInputResult};
use crate::markdown::InteractiveKind;
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
    if reader.mode == ReaderMode::Search {
        handle_search_mode(reader, key);
        return;
    }
    if is_search_key(key) {
        reader.start_search();
        return;
    }

    let line_count = reader.rendered.plain_lines.len();
    // Finish the reader borrow before applying clipboard and browser actions through app.
    let mut actions = ReaderActions::default();

    let page_rows = app.navigation.page_rows;

    let is_visual = matches!(reader.mode, ReaderMode::Visual | ReaderMode::VisualLine);
    if is_copy_key(key) {
        actions.text_to_copy = if is_visual {
            Some(selected_text(
                &reader.rendered.plain_lines,
                reader.cursor,
                reader.visual_start,
                &reader.mode,
            ))
        } else {
            reader.cursor_line_text()
        };
        reader.visual_start = None;
        reader.mode = ReaderMode::Normal;
        actions.apply(app);
        return;
    }
    if key.code == KeyCode::F(3) {
        reader.advance_search_match(!key.modifiers.contains(KeyModifiers::SHIFT), !is_visual);
        return;
    }
    if !is_plain_key(key)
        && !(matches!(key.code, KeyCode::Char('u' | 'd')) && key.modifiers == KeyModifiers::CONTROL)
    {
        return;
    }

    if is_visual {
        match key.code {
            KeyCode::Esc => {
                reader.visual_start = None;
                reader.mode = ReaderMode::Normal;
            }

            KeyCode::Left => reader.cursor = move_left(reader.cursor),
            KeyCode::Right => {
                reader.cursor = move_right(&reader.rendered.plain_lines, reader.cursor)
            }
            KeyCode::Down => {
                reader.cursor = move_down(&reader.rendered.plain_lines, reader.cursor, 1)
            }
            KeyCode::Up => reader.cursor = move_up(&reader.rendered.plain_lines, reader.cursor, 1),

            KeyCode::Char('w') => {
                reader.cursor = move_word_forward(&reader.rendered.plain_lines, reader.cursor);
            }
            KeyCode::Char('b') => {
                reader.cursor = move_word_backward(&reader.rendered.plain_lines, reader.cursor);
            }
            KeyCode::Char('e') => {
                reader.cursor = move_to_word_end(&reader.rendered.plain_lines, reader.cursor);
            }

            KeyCode::Char('0') | KeyCode::Home => {
                reader.cursor.1 = 0;
            }
            KeyCode::Char('^') => {
                if let Some(line) = reader.rendered.plain_lines.get(reader.cursor.0) {
                    reader.cursor.1 = line.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
                }
            }
            KeyCode::Char('$') | KeyCode::End => {
                let len = reader
                    .rendered
                    .plain_lines
                    .get(reader.cursor.0)
                    .map(|l| l.chars().count())
                    .unwrap_or(0);
                reader.cursor.1 = len.saturating_sub(1);
            }

            KeyCode::Char('g') => {
                reader.cursor = (
                    0,
                    clamp_column(&reader.rendered.plain_lines, 0, reader.cursor.1),
                );
            }

            KeyCode::Char('G') => {
                let last = line_count.saturating_sub(1);
                reader.cursor = (
                    last,
                    clamp_column(&reader.rendered.plain_lines, last, reader.cursor.1),
                );
            }

            KeyCode::PageDown => {
                reader.cursor = move_down(&reader.rendered.plain_lines, reader.cursor, page_rows);
            }
            KeyCode::PageUp => {
                reader.cursor = move_up(&reader.rendered.plain_lines, reader.cursor, page_rows);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                reader.cursor = move_down(
                    &reader.rendered.plain_lines,
                    reader.cursor,
                    (page_rows / 2).max(1),
                );
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                reader.cursor = move_up(
                    &reader.rendered.plain_lines,
                    reader.cursor,
                    (page_rows / 2).max(1),
                );
            }

            KeyCode::Char('n') if !reader.search_matches.is_empty() => {
                reader.advance_search_match(true, false);
            }
            KeyCode::Char('N') if !reader.search_matches.is_empty() => {
                reader.advance_search_match(false, false);
            }

            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Right | KeyCode::Down | KeyCode::Up => {
                let forward = matches!(key.code, KeyCode::Right | KeyCode::Down);
                let elements = &reader.rendered.interactive;
                let target = match reader.focused_element {
                    Some(index) if forward => (index + 1 < elements.len()).then_some(index + 1),
                    Some(index) => index.checked_sub(1),
                    None if forward => nearest_element_forward(elements, reader.cursor.0),
                    None => None,
                };
                if let Some(index) = target {
                    reader.focused_element = Some(index);
                    reader.cursor = (elements[index].line_start, 0);
                }
            }

            KeyCode::Char(c @ ('v' | 'V')) => {
                if let Some(index) = reader.focused_element {
                    reader.cursor = (reader.rendered.interactive[index].line_start, 0);
                }
                reader.mode = if c == 'v' {
                    ReaderMode::Visual
                } else {
                    ReaderMode::VisualLine
                };
                reader.visual_start = Some(reader.cursor);
            }

            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(index) = reader.focused_element {
                    match &reader.rendered.interactive[index].kind {
                        InteractiveKind::CodeBlock { id, .. } => {
                            // Save the position before re-rendering replaces the lines.
                            let code_block_id = *id;
                            let old_line = reader.rendered.interactive[index].line_start;
                            if reader.expanded_code_blocks.contains(&code_block_id) {
                                reader.expanded_code_blocks.remove(&code_block_id);
                            } else {
                                reader.expanded_code_blocks.insert(code_block_id);
                            }
                            let rendered = reader
                                .markdown
                                .render(&reader.expanded_code_blocks, &app.highlights);
                            let new_line = rendered
                                .interactive
                                .get(index)
                                .map(|element| element.line_start)
                                .unwrap_or(old_line);
                            reader.cursor = (new_line, 0);
                            reader.rendered = rendered;
                        }
                        InteractiveKind::Link { url, .. } if key.code == KeyCode::Enter => {
                            actions.url_to_open = Some(url.clone());
                        }
                        _ => {}
                    }
                } else if key.code == KeyCode::Enter
                    && let Some(url) = link_at(&reader.rendered.plain_lines, reader.cursor)
                {
                    actions.url_to_open = Some(url);
                }
            }

            KeyCode::Char('n') if !reader.search_matches.is_empty() => {
                reader.advance_search_match(true, true);
            }
            KeyCode::Char('N') if !reader.search_matches.is_empty() => {
                reader.advance_search_match(false, true);
            }

            KeyCode::Char(']') => {
                if let Some((new_index, line_start)) =
                    next_code_block(&reader.rendered.interactive, reader.cursor.0)
                {
                    reader.focused_element = Some(new_index);
                    reader.cursor = (line_start, 0);
                }
            }
            KeyCode::Char('[') => {
                if let Some((new_index, line_start)) =
                    previous_code_block(&reader.rendered.interactive, reader.cursor.0)
                {
                    reader.focused_element = Some(new_index);
                    reader.cursor = (line_start, 0);
                }
            }

            _ => {}
        }
    }

    actions.apply(app);
}

fn handle_search_mode(reader: &mut ReaderState, key: KeyEvent) {
    let mut query = std::mem::take(&mut reader.search_query);
    let result = TextInput::new(&mut query).handle_key(key);
    reader.search_query = query;
    match result {
        TextInputResult::Cancel => {
            reader.mode = ReaderMode::Normal;
            reader.search_query.clear();
            reader.search_matches.clear();
        }
        TextInputResult::Submit => {
            let (matches, index) = find_search_matches(
                &reader.rendered.plain_lines,
                &reader.search_query,
                reader.cursor,
            );
            if let Some(found) = matches.get(index) {
                reader.cursor = found.start;
                reader.focused_element =
                    interactive_element_at_line(&reader.rendered.interactive, reader.cursor.0);
            }
            reader.search_matches = matches;
            reader.search_match_index = index;
            reader.mode = ReaderMode::Normal;
        }
        _ => {}
    }
}

#[derive(Default)]
struct ReaderActions {
    text_to_copy: Option<String>,
    url_to_open: Option<String>,
}

impl ReaderActions {
    fn apply(self, app: &mut App) {
        if let Some(text) = self.text_to_copy {
            app.copy_to_clipboard(&text, "Copied!");
        }
        if let Some(url) = self.url_to_open {
            let _ = open::that(url);
        }
    }
}
