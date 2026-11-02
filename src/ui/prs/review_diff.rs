use super::{reviews, text::conversation_header};
use crate::types::{Comment, CommentKind, DiffSide, ReviewCommentContext, review_hunk};
use crate::ui::{theme, widgets::clip};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::StatefulWidget,
};
use ratatui_diff::{Diff, DiffDocument, DiffState, DiffTheme, Side};

const PREVIEW_ROWS: usize = 14;
const CONTEXT_LINES: usize = 3;

fn parse_diff(hunk: &str) -> Option<DiffDocument> {
    DiffDocument::parse(&review_hunk::normalized_patch(hunk)?).ok()
}

fn palette() -> DiffTheme {
    DiffTheme {
        context: theme::DIFF_CONTEXT,
        insert: theme::DIFF_INSERT,
        delete: theme::DIFF_DELETE,
        header: theme::DIFF_HEADER,
        gutter: theme::DIFF_GUTTER,
        ..DiffTheme::monochrome()
    }
}

fn notice(text: &str, width: u16) -> Line<'static> {
    Line::styled(clip(text, width.into(), false), palette().header)
}

#[derive(Clone, Copy)]
struct CommentLineRange {
    start: usize,
    end: usize,
    start_side: Side,
    end_side: Side,
}

fn diff_side(side: DiffSide) -> Side {
    match side {
        DiffSide::Left => Side::Old,
        DiffSide::Right => Side::New,
    }
}

// Use the review-time line numbers so later pushes do not shift the snippet.
fn comment_line_range(context: &ReviewCommentContext, outdated: bool) -> Option<CommentLineRange> {
    let original = outdated || context.original_line.is_some();
    let (start, end) = if original {
        (context.original_start_line, context.original_line?)
    } else {
        (context.start_line, context.line?)
    };
    let end_side = diff_side(context.side?);
    let start_side = if start.is_some() {
        diff_side(context.start_side?)
    } else {
        end_side
    };
    Some(CommentLineRange {
        start: usize::try_from(start.unwrap_or(end)).ok()?,
        end: usize::try_from(end).ok()?,
        start_side,
        end_side,
    })
}

fn comment_rows(state: &DiffState, line_range: CommentLineRange) -> Option<(usize, usize)> {
    let first = (0..state.row_count()).find(|&row| {
        state
            .source_at(row, line_range.start_side)
            .is_some_and(|position| position.line == line_range.start)
    })?;
    let last = (0..state.row_count()).rfind(|&row| {
        state
            .source_at(row, line_range.end_side)
            .is_some_and(|position| position.line == line_range.end)
    })?;
    (first <= last).then_some((first, last))
}

fn snippet_rows(
    state: &DiffState,
    hunk_start: usize,
    (first, last): (usize, usize),
) -> (usize, usize) {
    let mut start = first;
    let mut previous = None;
    let mut remaining = CONTEXT_LINES;
    for row in (hunk_start..first).rev() {
        let source = (
            state.source_at(row, Side::Old),
            state.source_at(row, Side::New),
        );
        if source != (None, None) && previous != Some(source) {
            if remaining == 0 {
                break;
            }
            remaining -= 1;
            previous = Some(source);
        }
        start = row;
    }
    (start, last + 1)
}

fn gutter(
    state: &DiffState,
    row: usize,
    preferred_side: Side,
    digits: usize,
    style: Style,
) -> Option<Span<'static>> {
    let other_side = match preferred_side {
        Side::Old => Side::New,
        Side::New => Side::Old,
    };
    let position = state
        .source_at(row, preferred_side)
        .or_else(|| state.source_at(row, other_side))?;
    let continuation = row > 0 && state.source_at(row - 1, position.side) == Some(position);
    let style = style.patch(palette().gutter);
    let label = if continuation {
        format!("{:>digits$} ", "↪")
    } else {
        format!("{:>digits$} ", position.line)
    };
    Some(Span::styled(label, style))
}

fn buffer_line(buffer: &Buffer, y: u16) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut x = 0;
    while x < buffer.area.width {
        let cell = &buffer[(x, y)];
        let style = cell.style();
        let symbol = cell.symbol();
        let advance = Span::raw(symbol).width().max(1) as u16;
        if let Some(last) = spans.last_mut().filter(|span| span.style == style) {
            last.content.to_mut().push_str(symbol);
        } else {
            spans.push(Span::styled(symbol.to_owned(), style));
        }
        x += advance;
    }
    // Keep the row background during wrapping, including its padding.
    Line::from(spans).style(theme::CODE_BODY.bg(buffer[(0, y)].bg))
}

fn header(comment: &Comment, width: u16, nerd_font: bool) -> Vec<Line<'static>> {
    let mut content = vec![Span::styled(
        reviews::location(comment).unwrap_or_else(|| "Code discussion".into()),
        theme::REVIEW_PATH,
    )];
    if reviews::status_badges(comment).contains(&reviews::ReviewBadge::Outdated) {
        content.push(Span::raw(" "));
        content.push(reviews::ReviewBadge::Outdated.into_span(nerd_font));
    }
    if reviews::is_resolved(comment) {
        content.push(Span::raw(" "));
        content.push(reviews::ReviewBadge::Resolved.into_span(nerd_font));
    }
    conversation_header(content, Vec::new(), width)
        .into_iter()
        .map(|line| line.style(palette().header))
        .collect()
}

pub(super) fn render(
    comment: &Comment,
    width: u16,
    expanded: bool,
    nerd_font: bool,
) -> Vec<Line<'static>> {
    let mut lines = header(comment, width, nerd_font);
    lines.extend(render_with_hint(
        comment,
        width,
        expanded,
        "Enter to expand",
    ));
    lines
}

pub(super) fn render_compact(comment: &Comment, width: u16, nerd_font: bool) -> Vec<Line<'static>> {
    let mut lines = header(comment, width, nerd_font);
    lines.extend(render_with_hint(comment, width, false, "more code"));
    lines
}

fn render_with_hint(
    comment: &Comment,
    width: u16,
    expanded: bool,
    hint: &str,
) -> Vec<Line<'static>> {
    let CommentKind::ReviewComment(context) = &comment.kind else {
        return Vec::new();
    };
    let Some(document) = parse_diff(&context.diff_hunk).and_then(|document| {
        let mut files = document.files().to_vec();
        if let Some(path) = &comment.path {
            for file in &mut files {
                file.old_path = Some(path.clone());
                file.new_path = Some(path.clone());
            }
        }
        DiffDocument::new(files).ok()
    }) else {
        return vec![notice(
            "Code unavailable: missing or malformed diff hunk",
            width,
        )];
    };
    let digits = document
        .files()
        .iter()
        .flat_map(|file| &file.hunks)
        // Size from visible line numbers; the exclusive range end may need an extra digit.
        .flat_map(|hunk| &hunk.lines)
        .flat_map(|line| [line.old, line.new])
        .flatten()
        .max()
        .unwrap_or(0)
        .to_string()
        .len();
    let gutter_width = digits + 1;
    if usize::from(width) <= gutter_width + 2 {
        return vec![notice("Code unavailable: viewport too narrow", width)];
    }
    let content_width = width - gutter_width as u16;
    let widget = Diff::new(&document)
        .theme(palette())
        .line_numbers(false)
        .wrap(true)
        .word_highlights(false)
        .tab_width(4);
    let mut state = DiffState::new();
    // Measure with one row so a large diff does not need a full-document buffer.
    let measure = Rect::new(0, 0, content_width, 1);
    (&widget).render(measure, &mut Buffer::empty(measure), &mut state);
    // The enclosing discussion already names the file; skip the library's duplicate header.
    state.next_hunk();
    let hunk_start = state.offset();
    let line_range = comment_line_range(context, comment.outdated);
    let preferred_side = context.side.map_or(Side::New, diff_side);
    let range = line_range.and_then(|range| comment_rows(&state, range));
    let total = state.row_count();
    let (snippet_start, snippet_end) = range
        .map(|range| snippet_rows(&state, hunk_start, range))
        .unwrap_or((hunk_start, total));
    let from = if expanded {
        snippet_start
    } else if let (Some((first, last)), Some(line_range)) = (range, line_range) {
        // Keep the start and line number if a single commented line exceeds the preview budget.
        let end_line_start = (first..=last)
            .find(|&row| {
                state
                    .source_at(row, line_range.end_side)
                    .is_some_and(|position| position.line == line_range.end)
            })
            .unwrap_or(first);
        snippet_end
            .saturating_sub(PREVIEW_ROWS)
            .max(snippet_start)
            .min(end_line_start)
    } else {
        snippet_start
    };
    let to = if expanded {
        snippet_end
    } else {
        (from + PREVIEW_ROWS).min(snippet_end)
    };
    let mut output = Vec::new();
    // Render in chunks to bound temporary memory and stay within u16 buffer heights.
    let mut next = from;
    while next < to {
        let height = (to - next).min(128) as u16;
        let area = Rect::new(0, 0, content_width, height);
        let mut buffer = Buffer::empty(area);
        (&widget).render(area, &mut buffer, &mut state);
        state.start();
        state.scroll_lines(next as isize);
        (&widget).render(area, &mut buffer, &mut state);
        let offset = state.offset();
        let end = (offset + usize::from(height)).min(to);
        for row in next..end {
            let y = (row - offset) as u16;
            let mut line = buffer_line(&buffer, y);
            if let Some(number) =
                gutter(&state, row, preferred_side, digits, buffer[(0, y)].style())
            {
                line.spans.insert(0, number);
            }
            output.push(line);
        }
        next = end;
    }
    if to < snippet_end || from > snippet_start {
        output.push(notice(&format!("… {hint}"), width));
    }
    output
}
