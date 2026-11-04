use super::navigation::*;
use super::{App, ReaderMode};
use crate::markdown::InteractiveKind;

/// Reader operations independent of the input device or key binding.
#[derive(Clone, Copy)]
pub(crate) enum ReaderAction {
    StartSearch,
    CancelSearch,
    SubmitSearch,
    SearchMatch { forward: bool },
    Copy,
    CancelVisual,
    Left,
    Right,
    Down,
    Up,
    WordForward,
    WordBackward,
    WordEnd,
    LineStart,
    FirstNonWhitespace,
    LineEnd,
    DocumentStart,
    DocumentEnd,
    PageDown,
    PageUp,
    HalfPageDown,
    HalfPageUp,
    StartVisual { linewise: bool },
    Activate { open_links: bool },
    NextCodeBlock,
    PreviousCodeBlock,
}

impl App {
    pub fn apply_reader_action(&mut self, action: ReaderAction) {
        let before = self.main.detail.reader.as_ref().map(|reader| {
            (
                reader.focused_element,
                reader.cursor,
                reader.expanded_code_blocks.len(),
                reader.mode,
            )
        });
        self.apply_reader_action_inner(action);
        let after = self.main.detail.reader.as_ref().map(|reader| {
            (
                reader.focused_element,
                reader.cursor,
                reader.expanded_code_blocks.len(),
                reader.mode,
            )
        });
        if before.map(|state| state.2) != after.map(|state| state.2) {
            self.remember_detail_expansion();
        }
        if before != after
            || before
                .is_some_and(|state| matches!(state.3, ReaderMode::Visual | ReaderMode::VisualLine))
        {
            self.main.detail.fullscreen_cache = None;
        }
    }

    fn apply_reader_action_inner(&mut self, action: ReaderAction) {
        let Some(reader) = self.main.detail.reader.as_mut() else {
            return;
        };
        match action {
            ReaderAction::StartSearch => {
                reader.start_search();
                return;
            }
            ReaderAction::CancelSearch => {
                reader.cancel_search();
                return;
            }
            ReaderAction::SubmitSearch => {
                reader.submit_search();
                return;
            }
            _ => {}
        }
        let line_count = reader.rendered.plain_lines.len();
        // Finish the reader borrow before applying clipboard and browser effects through the app.
        let mut effects = ReaderEffects::default();

        let page_rows = self.navigation.page_rows;

        let is_visual = matches!(reader.mode, ReaderMode::Visual | ReaderMode::VisualLine);
        if matches!(action, ReaderAction::Copy) {
            effects.text_to_copy = if is_visual {
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
            effects.apply(self);
            return;
        }
        if let ReaderAction::SearchMatch { forward } = action {
            reader.advance_search_match(forward, !is_visual);
            return;
        }

        if is_visual {
            match action {
                ReaderAction::CancelVisual => {
                    reader.visual_start = None;
                    reader.mode = ReaderMode::Normal;
                }

                ReaderAction::Left => reader.cursor = move_left(reader.cursor),
                ReaderAction::Right => {
                    reader.cursor = move_right(&reader.rendered.plain_lines, reader.cursor)
                }
                ReaderAction::Down => {
                    reader.cursor = move_down(&reader.rendered.plain_lines, reader.cursor, 1)
                }
                ReaderAction::Up => {
                    reader.cursor = move_up(&reader.rendered.plain_lines, reader.cursor, 1)
                }

                ReaderAction::WordForward => {
                    reader.cursor = move_word_forward(&reader.rendered.plain_lines, reader.cursor);
                }
                ReaderAction::WordBackward => {
                    reader.cursor = move_word_backward(&reader.rendered.plain_lines, reader.cursor);
                }
                ReaderAction::WordEnd => {
                    reader.cursor = move_to_word_end(&reader.rendered.plain_lines, reader.cursor);
                }

                ReaderAction::LineStart => {
                    reader.cursor.1 = 0;
                }
                ReaderAction::FirstNonWhitespace => {
                    if let Some(line) = reader.rendered.plain_lines.get(reader.cursor.0) {
                        reader.cursor.1 =
                            line.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
                    }
                }
                ReaderAction::LineEnd => {
                    let len = reader
                        .rendered
                        .plain_lines
                        .get(reader.cursor.0)
                        .map(|l| l.chars().count())
                        .unwrap_or(0);
                    reader.cursor.1 = len.saturating_sub(1);
                }

                ReaderAction::DocumentStart => {
                    reader.cursor = (
                        0,
                        clamp_column(&reader.rendered.plain_lines, 0, reader.cursor.1),
                    );
                }

                ReaderAction::DocumentEnd => {
                    let last = line_count.saturating_sub(1);
                    reader.cursor = (
                        last,
                        clamp_column(&reader.rendered.plain_lines, last, reader.cursor.1),
                    );
                }

                ReaderAction::PageDown => {
                    reader.cursor =
                        move_down(&reader.rendered.plain_lines, reader.cursor, page_rows);
                }
                ReaderAction::PageUp => {
                    reader.cursor = move_up(&reader.rendered.plain_lines, reader.cursor, page_rows);
                }
                ReaderAction::HalfPageDown => {
                    reader.cursor = move_down(
                        &reader.rendered.plain_lines,
                        reader.cursor,
                        (page_rows / 2).max(1),
                    );
                }
                ReaderAction::HalfPageUp => {
                    reader.cursor = move_up(
                        &reader.rendered.plain_lines,
                        reader.cursor,
                        (page_rows / 2).max(1),
                    );
                }

                _ => {}
            }
        } else {
            match action {
                ReaderAction::Right | ReaderAction::Down | ReaderAction::Up => {
                    let forward = matches!(action, ReaderAction::Right | ReaderAction::Down);
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

                ReaderAction::StartVisual { linewise } => {
                    if let Some(index) = reader.focused_element {
                        reader.cursor = (reader.rendered.interactive[index].line_start, 0);
                    }
                    reader.mode = if linewise {
                        ReaderMode::VisualLine
                    } else {
                        ReaderMode::Visual
                    };
                    reader.visual_start = Some(reader.cursor);
                }

                ReaderAction::Activate { open_links } => {
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
                                    .render(&reader.expanded_code_blocks, &self.highlights);
                                let new_line = rendered
                                    .interactive
                                    .get(index)
                                    .map(|element| element.line_start)
                                    .unwrap_or(old_line);
                                reader.cursor = (new_line, 0);
                                reader.rendered = rendered;
                            }
                            InteractiveKind::Link { url, .. } if open_links => {
                                effects.url_to_open = Some(url.clone());
                            }
                            _ => {}
                        }
                    } else if open_links
                        && let Some(url) = link_at(&reader.rendered.plain_lines, reader.cursor)
                    {
                        effects.url_to_open = Some(url);
                    }
                }

                ReaderAction::NextCodeBlock => {
                    if let Some((new_index, line_start)) =
                        next_code_block(&reader.rendered.interactive, reader.cursor.0)
                    {
                        reader.focused_element = Some(new_index);
                        reader.cursor = (line_start, 0);
                    }
                }
                ReaderAction::PreviousCodeBlock => {
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

        effects.apply(self);
    }
}

#[derive(Default)]
struct ReaderEffects {
    text_to_copy: Option<String>,
    url_to_open: Option<String>,
}

impl ReaderEffects {
    fn apply(self, app: &mut App) {
        if let Some(text) = self.text_to_copy {
            app.copy_to_clipboard(&text, "Copied!");
        }
        if let Some(url) = self.url_to_open {
            let _ = open::that(url);
        }
    }
}
