use std::collections::HashSet;

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use super::CODE_PREVIEW_LINES;
use super::HighlightCache;
use super::ast::{Block, Inline};
use crate::ui::theme;

fn replace_emoji_shortcodes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(':') {
        result.push_str(&rest[..start]);
        let after_colon = &rest[start + 1..];
        // Scan bytes to avoid slicing through a UTF-8 character while looking for the closing colon.
        if let Some(end) = after_colon.bytes().take(40).position(|b| b == b':') {
            let code = &after_colon[..end];
            if !code.is_empty()
                && code
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'+')
                && let Some(emoji) = emojis::get_by_shortcode(code)
            {
                result.push_str(emoji.as_str());
                rest = &after_colon[end + 1..];
                continue;
            }
            // This colon is literal, but a later one may still start a shortcode.
            result.push(':');
            rest = after_colon;
        } else {
            result.push(':');
            rest = after_colon;
        }
    }
    result.push_str(rest);
    result
}

/// Styled and plain rows must stay aligned: the reader searches and selects the plain text.
#[derive(Debug)]
pub struct RenderedDocument {
    pub lines: Vec<Line<'static>>,
    pub plain_lines: Vec<String>,
    pub interactive: Vec<InteractiveElement>,
}

#[derive(Debug)]
pub struct InteractiveElement {
    pub kind: InteractiveKind,
    pub line_start: usize,
    // Inclusive, unlike the character range below.
    pub line_end: usize,
    // Character columns, end exclusive; None highlights the whole element.
    pub col_range: Option<(usize, usize)>,
}

#[derive(Debug)]
pub enum InteractiveKind {
    CodeBlock {
        id: usize,
        line_count: usize,
        header_col: usize,
    },
    Link {
        url: String,
    },
    Heading,
}

pub fn render(
    blocks: &[Block],
    expanded_blocks: &HashSet<usize>,
    highlights: &HighlightCache,
) -> RenderedDocument {
    let mut ctx = RenderContext {
        lines: Vec::new(),
        plain: Vec::new(),
        interactive: Vec::new(),
        expanded: expanded_blocks,
        highlights,
    };
    for block in blocks {
        ctx.render_block(block, 0);
    }
    RenderedDocument {
        lines: ctx.lines,
        plain_lines: ctx.plain,
        interactive: ctx.interactive,
    }
}

struct RenderContext<'a> {
    lines: Vec<Line<'static>>,
    plain: Vec<String>,
    interactive: Vec<InteractiveElement>,
    expanded: &'a HashSet<usize>,
    highlights: &'a HighlightCache,
}

impl RenderContext<'_> {
    fn push(&mut self, line: Line<'static>, plain: String) {
        self.lines.push(line);
        self.plain.push(plain);
    }

    fn push_blank(&mut self) {
        self.push(Line::from(""), String::new());
    }

    fn push_inlines(&mut self, inlines: &[Inline], mut prefix: Option<Span<'static>>) {
        let continuation = prefix
            .as_ref()
            .map(|span| Span::raw(" ".repeat(span.content.chars().count())));
        for line in inlines.split(|inline| matches!(inline, Inline::HardBreak)) {
            self.push_inline_line(line, prefix.take().or_else(|| continuation.clone()));
        }
    }

    fn push_inline_line(&mut self, inlines: &[Inline], prefix: Option<Span<'static>>) {
        let RenderedInlines {
            mut spans,
            mut plain,
            links,
        } = render_inlines(inlines, Style::default());
        let prefix_len = prefix
            .as_ref()
            .map_or(0, |span| span.content.chars().count());
        if let Some(prefix) = prefix {
            plain.insert_str(0, &prefix.content);
            spans.insert(0, prefix);
        }
        let line_index = self.lines.len();
        self.push(Line::from(spans), plain);
        for InlineLink {
            col_start,
            col_end,
            url,
        } in links
        {
            self.interactive.push(InteractiveElement {
                kind: InteractiveKind::Link { url },
                line_start: line_index,
                line_end: line_index,
                col_range: Some((col_start + prefix_len, col_end + prefix_len)),
            });
        }
    }

    fn render_block(&mut self, block: &Block, indent: usize) {
        let indent_str: String = "  ".repeat(indent);

        match block {
            Block::Heading { level, inlines } => {
                let style = heading_style(*level);
                let prefix = heading_prefix(*level);
                let RenderedInlines { spans, plain, .. } = render_inlines(inlines, style);
                let line_index = self.lines.len();
                let mut line_spans: Vec<Span<'static>> =
                    vec![Span::styled(prefix.to_string(), style)];
                line_spans.extend(spans);
                self.push(Line::from(line_spans), format!("{prefix}{plain}"));
                self.interactive.push(InteractiveElement {
                    kind: InteractiveKind::Heading,
                    line_start: line_index,
                    line_end: line_index,
                    col_range: None,
                });
            }

            Block::Paragraph(inlines) => {
                let prefix = (indent > 0).then(|| Span::raw(indent_str));
                self.push_inlines(inlines, prefix);
                self.push_blank();
            }

            Block::Code { id, lang, content } => {
                let line_count = content.lines().count();
                let is_expanded = self.expanded.contains(id);

                let line_start = self.lines.len();

                self.render_code_block(lang.as_deref(), content, line_count, is_expanded);

                let line_end = self.lines.len().saturating_sub(1);
                self.interactive.push(InteractiveElement {
                    kind: InteractiveKind::CodeBlock {
                        id: *id,
                        line_count,
                        header_col: 0,
                    },
                    line_start,
                    line_end,
                    col_range: None,
                });
            }

            Block::Quote(inner) => {
                // Add quote gutters after rendering so every nested block does not need prefix state.
                let start_index = self.lines.len();
                let interactive_start = self.interactive.len();
                for b in inner {
                    self.render_block(b, 0);
                }
                // Leave trailing paragraph padding outside the quote to avoid a dangling gutter.
                let end_index = (start_index..self.lines.len())
                    .rfind(|&i| !self.plain[i].trim().is_empty())
                    .map_or(start_index, |i| i + 1);
                for i in start_index..end_index {
                    let old_line = std::mem::take(&mut self.lines[i]);
                    let text_style = theme::BLOCKQUOTE_TEXT.patch(old_line.style);
                    let mut new_spans: Vec<Span<'static>> =
                        vec![Span::styled("│ ", theme::BLOCKQUOTE_BAR)];
                    new_spans.extend(
                        old_line.spans.into_iter().map(|s| {
                            Span::styled(s.content.into_owned(), text_style.patch(s.style))
                        }),
                    );
                    self.lines[i] = Line::from(new_spans).style(old_line.style);

                    let old_plain = std::mem::take(&mut self.plain[i]);
                    self.plain[i] = format!("│ {old_plain}");
                }
                for el in &mut self.interactive[interactive_start..] {
                    if let InteractiveKind::CodeBlock { header_col, .. } = &mut el.kind {
                        *header_col += 2;
                    }
                    if let Some((ref mut cs, ref mut ce)) = el.col_range {
                        *cs += 2;
                        *ce += 2;
                    }
                }
            }

            Block::List {
                ordered,
                start,
                items,
            } => {
                for (i, item_blocks) in items.iter().enumerate() {
                    let bullet: String = if *ordered {
                        format!("{:>2}. ", i as u64 + start)
                    } else {
                        "  • ".to_string()
                    };
                    let continuation_indent = " ".repeat(bullet.len());

                    let mut first = true;
                    for block in item_blocks {
                        if first && let Block::Paragraph(inlines) = block {
                            let prefix = Span::styled(bullet.clone(), theme::LIST_BULLET);
                            self.push_inlines(inlines, Some(prefix));
                            first = false;
                            continue;
                        }
                        let sub_indent = if first {
                            0
                        } else {
                            continuation_indent.len() / 2
                        };
                        self.render_block(block, sub_indent.max(indent + 1));
                        first = false;
                    }
                }
                self.push_blank();
            }

            Block::Rule => {
                let rule = "─".repeat(72);
                self.push(
                    Line::from(Span::styled(rule.clone(), theme::RULE_LINE)),
                    rule,
                );
                self.push_blank();
            }
        }
    }

    fn render_code_block(
        &mut self,
        lang: Option<&str>,
        content: &str,
        line_count: usize,
        expanded: bool,
    ) {
        let lang_str = lang.unwrap_or("text");
        let truncated = !expanded && line_count > CODE_PREVIEW_LINES;
        let header = if truncated {
            format!("▶ {lang_str}  [{CODE_PREVIEW_LINES}/{line_count} lines; Enter: expand]")
        } else if expanded && line_count > CODE_PREVIEW_LINES {
            format!("▼ {lang_str}  [Enter: collapse]")
        } else {
            format!("▼ {lang_str}")
        };
        let header_style = theme::CODE_BODY.patch(theme::CODE_BORDER);
        self.push(Line::styled(header.clone(), header_style), header);
        self.push(Line::styled("  ", theme::CODE_BODY), "  ".into());

        // Highlight from the start even for previews so multiline syntax has the right state.
        let content = if truncated {
            let end = content
                .split_inclusive('\n')
                .take(CODE_PREVIEW_LINES)
                .map(str::len)
                .sum();
            &content[..end]
        } else {
            content
        };
        let rows = self.highlights.highlight(content, lang);
        self.render_code_rows(
            content,
            rows.into_iter().map(|row| {
                row.into_iter()
                    .map(|(text, style)| Span::styled(text, style))
            }),
        );

        self.push(Line::styled("  ", theme::CODE_BODY), "  ".into());
    }

    fn render_code_rows(
        &mut self,
        content: &str,
        rows: impl IntoIterator<Item = impl IntoIterator<Item = Span<'static>>>,
    ) {
        let mut plain_lines = content.lines();
        for row_spans in rows {
            let plain_line = plain_lines.next().unwrap_or("");
            let padding = Span::styled("    ", theme::CODE_BODY);
            let mut line_spans: Vec<Span<'static>> = vec![padding];
            line_spans.extend(row_spans);
            if line_spans.len() == 1 {
                line_spans.push(Span::raw(plain_line.to_string()));
            }
            self.push(
                Line::from(line_spans).style(theme::CODE_BODY),
                format!("    {plain_line}"),
            );
        }
    }
}

struct InlineLink {
    col_start: usize,
    col_end: usize,
    url: String,
}

struct RenderedInlines {
    spans: Vec<Span<'static>>,
    plain: String,
    links: Vec<InlineLink>,
}

fn render_inlines(inlines: &[Inline], base_style: Style) -> RenderedInlines {
    let mut spans = Vec::new();
    let mut plain = String::new();
    let mut link_positions = Vec::new();
    render_inlines_inner(
        inlines,
        &mut spans,
        &mut plain,
        base_style,
        &mut link_positions,
    );
    RenderedInlines {
        spans,
        plain,
        links: link_positions,
    }
}

fn render_inlines_inner(
    inlines: &[Inline],
    spans: &mut Vec<Span<'static>>,
    plain: &mut String,
    base: Style,
    link_positions: &mut Vec<InlineLink>,
) {
    for inline in inlines {
        match inline {
            Inline::Text(t) => {
                let t = replace_emoji_shortcodes(t);
                plain.push_str(&t);
                spans.push(Span::styled(t, base));
            }
            // Only top-level hard breaks start new rows; nested ones become spaces.
            Inline::SoftBreak | Inline::HardBreak => {
                plain.push(' ');
                spans.push(Span::styled(" ", base));
            }
            Inline::Code(t) => {
                let padded = format!(" {t} ");
                plain.push_str(&padded);
                let code_style = base.patch(theme::INLINE_CODE);
                spans.push(Span::styled(padded, code_style));
            }
            Inline::Bold(children) => {
                render_inlines_inner(
                    children,
                    spans,
                    plain,
                    base.add_modifier(Modifier::BOLD),
                    link_positions,
                );
            }
            Inline::Italic(children) => {
                render_inlines_inner(
                    children,
                    spans,
                    plain,
                    base.add_modifier(Modifier::ITALIC),
                    link_positions,
                );
            }
            Inline::Strikethrough(children) => {
                render_inlines_inner(
                    children,
                    spans,
                    plain,
                    base.add_modifier(Modifier::CROSSED_OUT),
                    link_positions,
                );
            }
            Inline::Link { url, children } => {
                let col_start = plain.chars().count();

                let link_style = theme::LINK;

                let mut display_text = String::new();
                let mut link_spans: Vec<Span<'static>> = Vec::new();
                render_inlines_inner(
                    children,
                    &mut link_spans,
                    &mut display_text,
                    link_style,
                    link_positions,
                );

                spans.extend(link_spans);
                plain.push_str(&display_text);

                let col_end = plain.chars().count();
                link_positions.push(InlineLink {
                    col_start,
                    col_end,
                    url: url.clone(),
                });
            }
            Inline::Image { alt } => {
                let text = format!("[image: {alt}]");
                plain.push_str(&text);
                spans.push(Span::styled(text, theme::IMAGE_ALT));
            }
            Inline::TaskListMarker(checked) => {
                let marker = if *checked { "[x] " } else { "[ ] " };
                plain.push_str(marker);
                spans.push(Span::styled(
                    marker.to_string(),
                    base.patch(theme::TASK_MARKER),
                ));
            }
        }
    }
}

fn heading_style(level: u8) -> Style {
    if level == 1 {
        theme::HEADING_1
    } else {
        theme::HEADING
    }
}

fn heading_prefix(level: u8) -> &'static str {
    match level {
        1 => "█ ",
        2 => "▌ ",
        3 => "▍ ",
        _ => "  ",
    }
}
