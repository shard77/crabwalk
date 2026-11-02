use super::reader::{handle_reader_key, is_copy_key, is_search_key};
use crate::app::{App, ReaderMode};
use crate::keys::{NavigationAction, is_plain_key, navigation_action};
use crate::markdown::{CODE_PREVIEW_LINES, InteractiveKind};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_detail_fullscreen_key(app: &mut App, key: KeyEvent) {
    if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
        let delta = if key.code == KeyCode::Tab { 1 } else { -1 };
        app.main.detail.viewport.scroll_by(delta);
        if let Some(reader) = &mut app.main.detail.reader {
            reader.pending_prefix = None;
        }
        return;
    }
    if navigate_detail_code_block(app, key.code) {
        return;
    }
    if let Some(action) = navigation_action(key)
        && !matches!(action, NavigationAction::Line(_))
    {
        let sections = detail_sections(app);
        let index = sections
            .iter()
            .position(|&section| section == app.main.detail.section)
            .unwrap_or(0);
        let target = action.apply(index, sections.len() - 1, app.main.detail.viewport.height);
        app.unfocus_detail();
        app.set_detail_section(sections[target]);
        return;
    }
    if is_search_key(key) {
        if app.main.detail.reader.is_none() {
            app.enter_reader();
        }
        if let Some(reader) = &mut app.main.detail.reader {
            reader.start_search();
        }
        app.main.detail.fullscreen_cache = None;
        return;
    }
    if !is_plain_key(key) {
        if is_copy_key(key) {
            handle_focused_key(app, key);
        }
        return;
    }
    match key.code {
        KeyCode::Esc if app.main.detail.reader.is_some() => app.unfocus_detail(),
        KeyCode::Esc => app.main.detail.fullscreen = false,
        KeyCode::Left => app.collapse_detail_section(),
        KeyCode::Right if app.main.detail.reader.is_none() => app.enter_reader(),
        KeyCode::Down | KeyCode::Up => {
            let backwards = key.code == KeyCode::Up;
            let at_boundary = app.main.detail.reader.as_ref().is_some_and(|reader| {
                if backwards {
                    reader.focused_element.is_none_or(|index| index == 0)
                } else {
                    reader
                        .focused_element
                        .is_none_or(|index| index + 1 >= reader.rendered.interactive.len())
                }
            });
            if at_boundary {
                app.unfocus_detail();
                if !backwards {
                    focus_adjacent_section(app, false);
                }
            } else if app.main.detail.reader.is_some() {
                handle_focused_key(app, key);
            } else {
                focus_adjacent_section(app, backwards);
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') if app.main.detail.reader.is_none() => {
            if key.code == KeyCode::Char(' ') && is_section_expanded(app) {
                app.collapse_detail_section();
            } else {
                app.enter_reader();
            }
        }
        _ if app.main.detail.reader.is_some() => handle_focused_key(app, key),
        _ => {}
    }
}

fn is_section_expanded(app: &App) -> bool {
    app.main
        .detail
        .expansion
        .is_section_expanded(app.main.detail.section, app.main.detail.snapshot())
}

fn focus_adjacent_section(app: &mut App, backwards: bool) {
    let sections = detail_sections(app);
    let index = sections
        .iter()
        .position(|&section| section == app.main.detail.section)
        .unwrap_or(0);
    let target = if backwards {
        index.saturating_sub(1)
    } else {
        (index + 1).min(sections.len() - 1)
    };
    if target != index {
        app.set_detail_section(sections[target]);
    }
}

fn handle_focused_key(app: &mut App, key: KeyEvent) {
    let before = app.main.detail.reader.as_ref().map(|reader| {
        (
            reader.focused_element,
            reader.cursor,
            reader.expanded_code_blocks.len(),
            reader.mode,
        )
    });
    handle_reader_key(app, key);
    let after = app.main.detail.reader.as_ref().map(|reader| {
        (
            reader.focused_element,
            reader.cursor,
            reader.expanded_code_blocks.len(),
            reader.mode,
        )
    });
    if before.map(|state| state.2) != after.map(|state| state.2) {
        app.remember_detail_expansion();
    }
    if before != after {
        app.main.detail.fullscreen_cache = None;
    }
}

fn navigate_detail_code_block(app: &mut App, code: KeyCode) -> bool {
    let forward = match code {
        KeyCode::Down => true,
        KeyCode::Up => false,
        _ => return false,
    };
    let Some(reader) = app.main.detail.reader.as_ref() else {
        return forward && app.focus_detail_code_block();
    };
    if reader.mode != ReaderMode::Normal || !reader.inline_code {
        return false;
    }
    let Some(index) = reader.focused_element else {
        return false;
    };
    let InteractiveKind::CodeBlock { id, line_count, .. } = reader.rendered.interactive[index].kind
    else {
        return false;
    };
    if line_count > CODE_PREVIEW_LINES && reader.expanded_code_blocks.contains(&id) {
        return false;
    }
    let blocks = &reader.rendered.interactive;
    let target = if forward {
        (index + 1..blocks.len())
            .find(|&i| matches!(blocks[i].kind, InteractiveKind::CodeBlock { .. }))
    } else {
        (0..index)
            .rev()
            .find(|&i| matches!(blocks[i].kind, InteractiveKind::CodeBlock { .. }))
    };
    if let Some(target) = target {
        let reader = app.main.detail.reader.as_mut().unwrap();
        reader.focused_element = Some(target);
        reader.cursor = (reader.rendered.interactive[target].line_start, 0);
        reader.pending_prefix = None;
        app.main.detail.fullscreen_cache = None;
    } else {
        let next = if forward {
            let Some(next) = detail_sections(app)
                .into_iter()
                .find(|&section| section > app.main.detail.section)
            else {
                return true;
            };
            next
        } else {
            app.main.detail.section
        };
        app.unfocus_detail();
        app.set_detail_section(next);
    }
    true
}

fn detail_sections(app: &App) -> Vec<usize> {
    app.main
        .detail
        .snapshot()
        .as_ref()
        .map(|detail| {
            let visibility = app
                .main
                .detail
                .review_threads()
                .expect("detail present")
                .visibility_for(&app.main.detail.expansion.expanded_comments(detail));
            std::iter::once(0)
                .chain(
                    (0..detail.comments.len())
                        .filter(|&index| visibility.representative(index) == index)
                        .map(|index| index + 1),
                )
                .collect()
        })
        .unwrap_or_else(|| vec![0])
}
