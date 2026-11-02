//! Text navigation, selection, search, and link lookup for rendered reader content.
//!
//! Cursor positions use character columns; terminal-cell widths and UTF-8 byte offsets differ.

use super::ReaderMode;
use crate::markdown::{InteractiveElement, InteractiveKind};

pub fn nearest_element_forward(
    interactive: &[InteractiveElement],
    from_line: usize,
) -> Option<usize> {
    interactive
        .iter()
        .enumerate()
        .find(|(_, element)| element.line_start >= from_line)
        .map(|(index, _)| index)
}

pub fn next_code_block(
    interactive: &[InteractiveElement],
    cursor_line: usize,
) -> Option<(usize, usize)> {
    interactive
        .iter()
        .enumerate()
        .find(|(_, element)| {
            element.line_start > cursor_line
                && matches!(element.kind, InteractiveKind::CodeBlock { .. })
        })
        .map(|(index, element)| (index, element.line_start))
}

pub fn previous_code_block(
    interactive: &[InteractiveElement],
    cursor_line: usize,
) -> Option<(usize, usize)> {
    interactive
        .iter()
        .enumerate()
        .rev()
        .find(|(_, element)| {
            element.line_start < cursor_line
                && matches!(element.kind, InteractiveKind::CodeBlock { .. })
        })
        .map(|(index, element)| (index, element.line_start))
}

pub fn interactive_element_at_line(
    interactive: &[InteractiveElement],
    line: usize,
) -> Option<usize> {
    interactive
        .iter()
        .position(|element| line >= element.line_start && line <= element.line_end)
}

pub fn line_char_count(lines: &[String], line: usize) -> usize {
    lines.get(line).map(|s| s.chars().count()).unwrap_or(0)
}

pub fn clamp_column(lines: &[String], line: usize, col: usize) -> usize {
    let len = line_char_count(lines, line);
    if len == 0 { 0 } else { col.min(len - 1) }
}

pub fn move_left(cursor: (usize, usize)) -> (usize, usize) {
    let (line, column) = cursor;
    if column > 0 {
        (line, column - 1)
    } else {
        cursor
    }
}

pub fn move_right(lines: &[String], cursor: (usize, usize)) -> (usize, usize) {
    let (line, column) = cursor;
    let len = line_char_count(lines, line);
    if len > 0 && column + 1 < len {
        (line, column + 1)
    } else {
        cursor
    }
}

pub fn move_down(lines: &[String], cursor: (usize, usize), line_count: usize) -> (usize, usize) {
    let line = cursor
        .0
        .saturating_add(line_count)
        .min(lines.len().saturating_sub(1));
    (line, clamp_column(lines, line, cursor.1))
}

pub fn move_up(lines: &[String], cursor: (usize, usize), line_count: usize) -> (usize, usize) {
    let line = cursor.0.saturating_sub(line_count);
    (line, clamp_column(lines, line, cursor.1))
}

/// Punctuation is skipped rather than treated as a Vim word.
pub fn move_word_forward(lines: &[String], cursor: (usize, usize)) -> (usize, usize) {
    let (mut line, mut column) = cursor;
    if lines.is_empty() {
        return cursor;
    }
    loop {
        let chars: Vec<char> = lines[line].chars().collect();
        while column < chars.len() && chars[column].is_alphanumeric() {
            column += 1;
        }
        while column < chars.len() && !chars[column].is_alphanumeric() {
            column += 1;
        }
        if column < chars.len() {
            return (line, column);
        }
        if line + 1 >= lines.len() {
            return (line, chars.len().saturating_sub(1));
        }
        line += 1;
        column = 0;
    }
}

pub fn move_word_backward(lines: &[String], cursor: (usize, usize)) -> (usize, usize) {
    let (mut line, mut column) = cursor;
    if column == 0 {
        if line == 0 {
            return cursor;
        }
        line -= 1;
        column = line_char_count(lines, line);
    }
    column = column.saturating_sub(1);
    let chars: Vec<char> = lines[line].chars().collect();
    while column > 0 && !chars[column].is_alphanumeric() {
        column -= 1;
    }
    while column > 0 && chars[column - 1].is_alphanumeric() {
        column -= 1;
    }
    (line, column)
}

pub fn move_to_word_end(lines: &[String], cursor: (usize, usize)) -> (usize, usize) {
    let (mut line, mut column) = cursor;
    if lines.is_empty() {
        return cursor;
    }
    column += 1;
    loop {
        let chars: Vec<char> = lines[line].chars().collect();
        while column < chars.len() && !chars[column].is_alphanumeric() {
            column += 1;
        }
        if column < chars.len() {
            while column + 1 < chars.len() && chars[column + 1].is_alphanumeric() {
                column += 1;
            }
            return (line, column);
        }
        if line + 1 >= lines.len() {
            return (line, chars.len().saturating_sub(1));
        }
        line += 1;
        column = 0;
    }
}

pub fn selected_text(
    lines: &[String],
    cursor: (usize, usize),
    visual_start: Option<(usize, usize)>,
    mode: &ReaderMode,
) -> String {
    let Some(selection_start) = visual_start else {
        return String::new();
    };
    if lines.is_empty() {
        return String::new();
    }
    let (start, end) = if cursor <= selection_start {
        (cursor, selection_start)
    } else {
        (selection_start, cursor)
    };

    if *mode == ReaderMode::VisualLine {
        // Content may have changed since selection began; clamp before slicing.
        lines[start.0..=end.0.min(lines.len().saturating_sub(1))].join("\n")
    } else if start.0 == end.0 {
        char_range(&lines[start.0], start.1, Some(end.1))
    } else {
        lines
            .iter()
            .enumerate()
            .take(end.0.saturating_add(1))
            .skip(start.0)
            .map(|(line_index, line)| match line_index {
                index if index == start.0 => char_range(line, start.1, None),
                index if index == end.0 => char_range(line, 0, Some(end.1)),
                _ => line.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn char_range(line: &str, start: usize, inclusive_end: Option<usize>) -> String {
    let count = inclusive_end.map(|end| end.saturating_sub(start).saturating_add(1));
    line.chars()
        .skip(start)
        .take(count.unwrap_or(usize::MAX))
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub struct SearchMatch {
    pub start: (usize, usize),
    // Exclusive character column.
    pub end_col: usize,
}

pub fn find_search_matches(
    lines: &[String],
    query: &str,
    cursor: (usize, usize),
) -> (Vec<SearchMatch>, usize) {
    let mut matches = Vec::new();
    if query.is_empty() {
        return (matches, 0);
    }
    let lowercase_query = query.to_lowercase();
    for (line_index, line) in lines.iter().enumerate() {
        let lowercase_line = line.to_lowercase();
        // Lowercasing can expand a character (İ) or depend on its neighbours (Greek sigma).
        // Lowercase the whole line and map matches back to the original character columns.
        let source_columns = line
            .chars()
            .enumerate()
            .flat_map(|(col, ch)| ch.to_lowercase().map(move |_| col));
        let mut positions = lowercase_line.char_indices().zip(source_columns).peekable();
        for (byte_start, hit) in lowercase_line.match_indices(&lowercase_query) {
            let byte_end = byte_start + hit.len();
            let mut start_col = 0;
            let mut end_col = 0;
            while let Some(&((byte, _), col)) = positions.peek() {
                if byte >= byte_end {
                    break;
                }
                if byte <= byte_start {
                    start_col = col;
                }
                end_col = col + 1;
                positions.next();
            }
            matches.push(SearchMatch {
                start: (line_index, start_col),
                end_col,
            });
        }
    }
    let index = matches.iter().position(|m| m.start >= cursor).unwrap_or(0);
    (matches, index)
}

pub fn link_at(lines: &[String], cursor: (usize, usize)) -> Option<String> {
    let line = lines.get(cursor.0)?;
    let col = cursor.1;
    for prefix in &["https://", "http://"] {
        let mut search_from = 0usize;
        while let Some(byte_offset) = line[search_from..].find(prefix) {
            let byte_start = search_from + byte_offset;
            let url_len = line[byte_start..]
                .bytes()
                .position(|b| matches!(b, b' ' | b'\t' | b')' | b'>' | b'"' | b'\''))
                .unwrap_or(line.len() - byte_start);
            let byte_end = byte_start + url_len;
            let char_start = line[..byte_start].chars().count();
            let char_end = line[..byte_end].chars().count();
            if col >= char_start && col < char_end {
                return Some(line[byte_start..byte_end].to_string());
            }
            search_from = byte_end.max(byte_start + 1);
        }
    }
    None
}
