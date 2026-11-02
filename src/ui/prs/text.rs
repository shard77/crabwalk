use crate::app::reader::SearchMatch;
use crate::app::{ReaderMode, ReaderState};
use crate::markdown::{InteractiveElement, InteractiveKind};
use crate::ui::theme;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

#[derive(Clone, Copy)]
pub(super) struct FocusedElement {
    pub line_start: usize,
    // Inclusive rendered-line index.
    pub line_end: usize,
    // Character columns, end exclusive; None highlights whole lines.
    pub col_range: Option<(usize, usize)>,
    pub code_header_col: Option<usize>,
}

impl From<&InteractiveElement> for FocusedElement {
    fn from(element: &InteractiveElement) -> Self {
        Self {
            line_start: element.line_start,
            line_end: element.line_end,
            col_range: element.col_range,
            code_header_col: match element.kind {
                InteractiveKind::CodeBlock { header_col, .. } => Some(header_col),
                _ => None,
            },
        }
    }
}

pub(super) struct Overlay<'a> {
    pub cursor: (usize, usize),
    pub visual_start: Option<(usize, usize)>,
    pub mode: &'a ReaderMode,
    pub search_matches: &'a [SearchMatch],
    pub focused_element: Option<FocusedElement>,
}

pub(super) fn style_conversation_line(
    line: &mut Line<'static>,
    width: u16,
    background: Style,
    active: bool,
) {
    line.style = background.patch(line.style);
    if active && line.style.bg == background.bg {
        line.style = line.style.patch(theme::FOCUS_BLOCK_BG);
    }
    let padding = usize::from(width).saturating_sub(line.width());
    line.spans.push(Span::raw(" ".repeat(padding)));
}

// Overlay before wrapping so links and selections keep the reader's character coordinates.
pub(super) fn append_inline_focus(
    lines: &mut Vec<Line<'static>>,
    focus: &ReaderState,
    width: u16,
    start: usize,
) -> Option<usize> {
    let element = focus
        .focused_element
        .and_then(|i| focus.rendered.interactive.get(i));
    let overlay = Overlay {
        cursor: focus.cursor,
        visual_start: focus.visual_start,
        mode: &focus.mode,
        search_matches: &focus.search_matches,
        focused_element: element.map(FocusedElement::from),
    };
    let mut focused_row = None;
    let mut started = false;
    for (index, line) in focus.rendered.lines.iter().enumerate().skip(start) {
        if !started && line.spans.iter().all(|span| span.content.trim().is_empty()) {
            continue;
        }
        started = true;
        let cursor_target = focus.mode != ReaderMode::Normal || !focus.search_matches.is_empty();
        let target = if cursor_target {
            (focus.cursor.0 == index).then_some(focus.cursor.1)
        } else {
            element
                .filter(|element| element.line_start == index)
                .map(|element| element.col_range.map_or(0, |(start, _)| start))
        };
        let highlighted = render_line_with_overlay(index, line, &overlay);
        let (wrapped, target_row) = wrap_conversation_line_with_target(highlighted, width, target);
        if let Some(row) = target_row {
            focused_row = Some(lines.len() + row);
        }
        lines.extend(wrapped);
    }
    focused_row
}

pub(super) fn conversation_header(
    content: Vec<Span<'static>>,
    metadata: Vec<Span<'static>>,
    width: u16,
) -> Vec<Line<'static>> {
    let mut line = Line::from(content);
    if metadata.is_empty() {
        return wrap_header_spans(line, width);
    }
    let metadata = Line::from(metadata);
    if line.width() + 2 + metadata.width() <= usize::from(width) {
        let gap = usize::from(width) - line.width() - metadata.width();
        line.spans.push(Span::raw(" ".repeat(gap)));
        line.spans.extend(metadata.spans);
        return vec![line];
    }
    let mut lines = wrap_header_spans(line, width);
    for mut row in wrap_header_spans(metadata, width) {
        let gap = usize::from(width).saturating_sub(row.width());
        row.spans.insert(0, Span::raw(" ".repeat(gap)));
        lines.push(row);
    }
    lines
}

pub(super) fn wrap_header_spans(header: Line<'static>, width: u16) -> Vec<Line<'static>> {
    let mut rows = Vec::new();
    let mut row = Line::default().style(header.style);
    let mut separator = Vec::new();
    for span in header.spans {
        if span.style.bg.is_none() && (span.content.trim().is_empty() || span.content.trim() == "·")
        {
            separator.push(span);
            continue;
        }
        let separator_width: usize = separator.iter().map(Span::width).sum();
        if !row.spans.is_empty()
            && row.width() + separator_width + span.width() > usize::from(width)
        {
            rows.push(row);
            row = Line::default().style(header.style);
        }
        if !row.spans.is_empty() {
            row.spans.append(&mut separator);
        }
        separator.clear();
        if span.width() > usize::from(width) {
            rows.extend(wrap_conversation_line(
                Line::from(span).style(header.style),
                width,
            ));
        } else {
            row.spans.push(span);
        }
    }
    if !row.spans.is_empty() {
        rows.push(row);
    }
    rows
}

pub(super) fn finish_conversation_body(lines: &mut Vec<Line<'static>>, start: usize, width: u16) {
    while lines.len() > start
        && lines
            .last()
            .is_some_and(|line| line.spans.iter().all(|span| span.content.trim().is_empty()))
    {
        lines.pop();
    }
    let body = lines.split_off(start);
    for line in body
        .into_iter()
        .skip_while(|line| line.spans.iter().all(|span| span.content.trim().is_empty()))
    {
        lines.extend(wrap_conversation_line(line, width));
    }
}

pub(super) fn wrap_conversation_line(line: Line<'static>, width: u16) -> Vec<Line<'static>> {
    wrap_conversation_line_with_target(line, width, None).0
}

fn wrap_conversation_line_with_target(
    mut line: Line<'static>,
    width: u16,
    target: Option<usize>,
) -> (Vec<Line<'static>>, Option<usize>) {
    if width > 0
        && line.width() <= usize::from(width)
        && line
            .spans
            .iter()
            .all(|span| !span.content.chars().any(char::is_control))
    {
        // Match the wrapped path's effective styles, including later background styling.
        for span in &mut line.spans {
            span.style = line.style.patch(span.style);
        }
        let mut row = Line::from(line.spans).style(line.style);
        if row.style.bg.is_some() {
            row.spans
                .push(Span::raw(" ".repeat(usize::from(width) - row.width())));
        }
        return (vec![row], target.map(|_| 0));
    }
    wrap_overflowing_line(line, width, target)
}

fn wrap_overflowing_line(
    line: Line<'static>,
    width: u16,
    target: Option<usize>,
) -> (Vec<Line<'static>>, Option<usize>) {
    let padded_row = |spans: Vec<Span<'static>>| {
        let mut row = Line::from(spans).style(line.style);
        if row.style.bg.is_some() {
            let padding = usize::from(width).saturating_sub(row.width());
            row.spans.push(Span::raw(" ".repeat(padding)));
        }
        row
    };
    if width == 0 {
        return (vec![padded_row(Vec::new())], target.map(|_| 0));
    }
    let glyphs: Vec<_> = line
        .spans
        .iter()
        .flat_map(|span| {
            span.styled_graphemes(line.style).map(|g| {
                let span = Span::styled(g.symbol.to_string(), g.style);
                let width = span.width();
                let whitespace = g.symbol.chars().all(char::is_whitespace);
                (span, width, whitespace)
            })
        })
        .scan(0, |column, (span, width, whitespace)| {
            *column += span.content.chars().count();
            Some((span, width, whitespace, *column))
        })
        .collect();
    if glyphs.is_empty() {
        return (vec![padded_row(Vec::new())], target.map(|_| 0));
    }
    // Use the gutter style to distinguish quote bars from literal bars in prose or code.
    let mut quote_end = 0;
    while glyphs.get(quote_end).is_some_and(|(span, _, _, _)| {
        span.content == "│" && span.style.fg == theme::BLOCKQUOTE_BAR.fg
    }) && glyphs.get(quote_end + 1).is_some_and(|(span, _, _, _)| {
        span.content == " " && span.style.fg == theme::BLOCKQUOTE_BAR.fg
    }) {
        quote_end += 2;
    }
    let (prefix, glyphs) = glyphs.split_at(quote_end);
    let prefix_width = prefix.len().min(if glyphs.is_empty() {
        usize::from(width)
    } else {
        usize::from(width.saturating_sub(1))
    });
    let gutter: Vec<_> = prefix[..prefix_width]
        .iter()
        .map(|(span, _, _, _)| span.clone())
        .collect();
    if glyphs.is_empty() {
        return (vec![padded_row(gutter)], target.map(|_| 0));
    }
    let content_width = usize::from(width) - prefix_width;
    let mut rows = Vec::new();
    let mut target_row = None;
    let mut start = 0;
    while start < glyphs.len() {
        let mut end = start;
        let mut used = 0;
        let mut last_space = None;
        let mut has_text = false;
        while end < glyphs.len() && used + glyphs[end].1 <= content_width {
            used += glyphs[end].1;
            if glyphs[end].2 && has_text {
                last_space = Some(end);
            }
            has_text |= !glyphs[end].2;
            end += 1;
        }
        if end == start {
            // Replace glyphs wider than the viewport so wrapping can still make progress.
            if target_row.is_none() && target.is_some_and(|column| column < glyphs[start].3) {
                target_row = Some(rows.len());
            }
            let mut spans = gutter.clone();
            spans.push(Span::styled("�", glyphs[start].0.style));
            rows.push(padded_row(spans));
            start += 1;
            continue;
        }
        if end < glyphs.len()
            && !glyphs[end].2
            && let Some(space) = last_space
        {
            end = space;
        }
        if target_row.is_none() && target.is_some_and(|column| column < glyphs[end - 1].3) {
            target_row = Some(rows.len());
        }
        let mut spans = gutter.clone();
        for (glyph, _, _, _) in &glyphs[start..end] {
            if let Some(last) = spans.last_mut().filter(|last| last.style == glyph.style) {
                last.content.to_mut().push_str(&glyph.content);
            } else {
                spans.push(glyph.clone());
            }
        }
        rows.push(padded_row(spans));
        start = end;
        while start < glyphs.len() && glyphs[start].2 {
            start += 1;
        }
    }
    let target_row = target_row.or_else(|| target.map(|_| rows.len().saturating_sub(1)));
    (rows, target_row)
}

pub(super) fn render_line_with_overlay(
    line_index: usize,
    rendered: &Line<'_>,
    overlay: &Overlay<'_>,
) -> Line<'static> {
    let line_style = rendered.style;
    let row_style = if line_style.bg.is_some()
        && *overlay.mode == ReaderMode::Normal
        && overlay.focused_element.is_some_and(|element| {
            element.col_range.is_none()
                && (element.line_start..=element.line_end).contains(&line_index)
        }) {
        line_style.patch(theme::FOCUS_BLOCK_BG)
    } else {
        line_style
    };
    let flat: Vec<(char, Style)> = rendered
        .spans
        .iter()
        .flat_map(|span| {
            let style = line_style.patch(span.style);
            span.content.chars().map(move |ch| (ch, style))
        })
        .collect();
    let show_cursor = matches!(
        overlay.mode,
        ReaderMode::Visual | ReaderMode::VisualLine | ReaderMode::Search
    );
    let is_cursor_line = overlay.cursor.0 == line_index && show_cursor;
    if flat.is_empty() {
        if is_cursor_line {
            return Line::from(vec![Span::styled(" ", theme::CURSOR)]).style(row_style);
        }
        if let Some(element) = overlay.focused_element
            && element.col_range.is_none()
            && line_index == element.line_start
            && line_index <= element.line_end
        {
            return Line::from(vec![Span::styled("›", theme::FOCUS_GUTTER)]).style(row_style);
        }
        return Line::from("").style(row_style);
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style: Option<Style> = None;
    // Change only the reserved marker so source text and search columns do not shift.
    let code_header_col = overlay.focused_element.and_then(|element| {
        (*overlay.mode == ReaderMode::Normal && line_index == element.line_start)
            .then_some(element.code_header_col)
            .flatten()
    });
    let first = overlay
        .search_matches
        .partition_point(|hit| hit.start.0 < line_index);
    let mut hits = overlay.search_matches[first..]
        .iter()
        .take_while(|hit| hit.start.0 == line_index)
        .peekable();
    for (col, (ch, base_style)) in flat.iter().enumerate() {
        while hits.peek().is_some_and(|hit| hit.end_col <= col) {
            hits.next();
        }
        let is_search_match = hits.peek().is_some_and(|hit| hit.start.1 <= col);
        let base_style = if code_header_col.is_some_and(|start| col >= start) {
            base_style.patch(theme::CODE_HEADER_FOCUSED)
        } else {
            *base_style
        };
        let ch = if code_header_col == Some(col) {
            '►'
        } else {
            *ch
        };
        let style = overlay_style(base_style, line_index, col, overlay, is_search_match);
        match run_style {
            Some(s) if s == style => run.push(ch),
            _ => {
                if let Some(s) = run_style
                    && !run.is_empty()
                {
                    spans.push(Span::styled(run.clone(), s));
                    run.clear();
                }
                run.push(ch);
                run_style = Some(style);
            }
        }
    }
    if let Some(s) = run_style
        && !run.is_empty()
    {
        spans.push(Span::styled(run, s));
    }
    if is_cursor_line && overlay.cursor.1 >= flat.len() {
        spans.push(Span::styled(" ", theme::CURSOR));
    }
    Line::from(spans).style(row_style)
}

// Cursor overrides selection, then search, then element focus.
fn overlay_style(
    base: Style,
    line: usize,
    col: usize,
    overlay: &Overlay<'_>,
    is_search_match: bool,
) -> Style {
    let Overlay {
        cursor,
        visual_start,
        mode,
        focused_element,
        ..
    } = *overlay;
    let show_cursor = matches!(
        mode,
        ReaderMode::Visual | ReaderMode::VisualLine | ReaderMode::Search
    );
    if show_cursor && (line, col) == cursor {
        return theme::CURSOR;
    }
    if let Some(selection_start) = visual_start
        && matches!(mode, ReaderMode::Visual | ReaderMode::VisualLine)
        && is_in_selection(line, col, cursor, selection_start, mode)
    {
        return base.patch(theme::VISUAL_SELECTION);
    }
    if is_search_match {
        return base.patch(theme::SEARCH_MATCH);
    }
    if *mode == ReaderMode::Normal
        && let Some(element) = focused_element
        && line >= element.line_start
        && line <= element.line_end
    {
        match element.col_range {
            Some((start_column, end_column)) => {
                if col >= start_column && col < end_column {
                    return base.add_modifier(Modifier::REVERSED);
                }
            }
            None => return base.patch(theme::FOCUS_BLOCK_BG),
        }
    }
    base
}

fn is_in_selection(
    line: usize,
    col: usize,
    cursor: (usize, usize),
    visual_start: (usize, usize),
    mode: &ReaderMode,
) -> bool {
    let (start, end) = if cursor <= visual_start {
        (cursor, visual_start)
    } else {
        (visual_start, cursor)
    };
    if *mode == ReaderMode::VisualLine {
        return line >= start.0 && line <= end.0;
    }
    if line < start.0 || line > end.0 {
        return false;
    }
    if start.0 == end.0 {
        col >= start.1 && col <= end.1
    } else if line == start.0 {
        col >= start.1
    } else if line == end.0 {
        col <= end.1
    } else {
        true
    }
}
