use super::ast::{Block, Inline};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub fn parse(md: &str) -> Vec<Block> {
    let opts = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES;
    // Peek so nested parsers can leave the parent's closing tag untouched.
    let mut iter = Parser::new_ext(md, opts).peekable();
    let mut next_id = 0;
    parse_blocks(&mut iter, &mut next_id)
}

type PeekIter<'a> = std::iter::Peekable<Parser<'a>>;

fn parse_blocks(iter: &mut PeekIter<'_>, next_id: &mut usize) -> Vec<Block> {
    let mut out = Vec::new();
    loop {
        let ev = match iter.peek().cloned() {
            None => break,
            Some(e) => e,
        };
        match ev {
            Event::End(_) => break,

            Event::Start(Tag::Paragraph) => {
                iter.next();
                let inlines = parse_inlines(iter, Some(TagEnd::Paragraph));
                if !inlines.is_empty() {
                    out.push(Block::Paragraph(inlines));
                }
            }

            Event::Start(Tag::Heading { level, .. }) => {
                let level_u8 = heading_level(level);
                iter.next();
                let inlines = parse_inlines(iter, Some(TagEnd::Heading(level)));
                out.push(Block::Heading {
                    level: level_u8,
                    inlines,
                });
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = code_block_lang(kind);
                iter.next();
                let mut content = String::new();
                loop {
                    match iter.peek().cloned() {
                        Some(Event::Text(t)) => {
                            content.push_str(&t);
                            iter.next();
                        }
                        Some(Event::End(TagEnd::CodeBlock)) => {
                            iter.next();
                            break;
                        }
                        None => break,
                        _ => {
                            iter.next();
                        }
                    }
                }
                if content.ends_with('\n') {
                    content.pop();
                }
                let id = *next_id;
                // Share the counter so nested blocks cannot reuse code-block IDs.
                *next_id += 1;
                out.push(Block::Code { id, lang, content });
            }

            Event::Start(Tag::BlockQuote(_)) => {
                iter.next();
                let inner = parse_blocks(iter, next_id);
                if let Some(Event::End(TagEnd::BlockQuote(_))) = iter.peek().cloned() {
                    iter.next();
                }
                out.push(Block::Quote(inner));
            }

            Event::Start(Tag::List(start_opt)) => {
                let (ordered, start) = match start_opt {
                    Some(n) => (true, n),
                    None => (false, 1),
                };
                iter.next();
                let items = parse_list_items(iter, next_id);
                out.push(Block::List {
                    ordered,
                    start,
                    items,
                });
            }

            Event::Rule => {
                iter.next();
                out.push(Block::Rule);
            }

            Event::Start(Tag::HtmlBlock) => {
                iter.next();
                for event in iter.by_ref() {
                    // Leaving this closing tag would end the surrounding block early.
                    if matches!(event, Event::End(TagEnd::HtmlBlock)) {
                        break;
                    }
                }
            }

            Event::Start(Tag::Table(_)) => {
                iter.next();
                skip_until_end(iter, TagEnd::Table);
            }

            // Tight list items have no paragraph tags; wrap their inline runs without swallowing nested blocks.
            Event::Text(_)
            | Event::Code(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::TaskListMarker(_)
            | Event::InlineHtml(_)
            | Event::Start(
                Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Link { .. }
                | Tag::Image { .. },
            ) => {
                out.push(Block::Paragraph(parse_inlines(iter, None)));
            }

            _ => {
                iter.next();
            }
        }
    }
    out
}

fn parse_list_items(iter: &mut PeekIter<'_>, next_id: &mut usize) -> Vec<Vec<Block>> {
    let mut items = Vec::new();
    loop {
        match iter.peek().cloned() {
            Some(Event::Start(Tag::Item)) => {
                iter.next();
                let item_blocks = parse_blocks(iter, next_id);
                if let Some(Event::End(TagEnd::Item)) = iter.peek().cloned() {
                    iter.next();
                }
                items.push(item_blocks);
            }
            Some(Event::End(TagEnd::List(_))) => {
                iter.next();
                break;
            }
            None => break,
            _ => {
                iter.next();
            }
        }
    }
    items
}

fn parse_inlines(iter: &mut PeekIter<'_>, end: Option<TagEnd>) -> Vec<Inline> {
    let mut out = Vec::new();
    loop {
        let ev = match iter.peek().cloned() {
            None => break,
            Some(e) => e,
        };
        match ev {
            Event::End(tag) => {
                if Some(tag) == end {
                    iter.next();
                }
                break;
            }
            Event::Text(t) => {
                out.push(Inline::Text(t.to_string()));
                iter.next();
            }
            Event::Code(t) => {
                out.push(Inline::Code(t.to_string()));
                iter.next();
            }
            Event::SoftBreak => {
                out.push(Inline::SoftBreak);
                iter.next();
            }
            Event::HardBreak => {
                out.push(Inline::HardBreak);
                iter.next();
            }
            Event::Start(Tag::Emphasis) => {
                iter.next();
                let children = parse_inlines(iter, Some(TagEnd::Emphasis));
                out.push(Inline::Italic(children));
            }
            Event::Start(Tag::Strong) => {
                iter.next();
                let children = parse_inlines(iter, Some(TagEnd::Strong));
                out.push(Inline::Bold(children));
            }
            Event::Start(Tag::Strikethrough) => {
                iter.next();
                let children = parse_inlines(iter, Some(TagEnd::Strikethrough));
                out.push(Inline::Strikethrough(children));
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let url = dest_url.to_string();
                iter.next();
                let children = parse_inlines(iter, Some(TagEnd::Link));
                out.push(Inline::Link { url, children });
            }
            Event::Start(Tag::Image { .. }) => {
                iter.next();
                let alt = collect_text(iter);
                out.push(Inline::Image { alt });
            }
            Event::TaskListMarker(checked) => {
                out.push(Inline::TaskListMarker(checked));
                iter.next();
            }
            Event::InlineHtml(s) => {
                out.push(Inline::Text(s.to_string()));
                iter.next();
            }
            _ if end.is_none() => break,
            Event::Start(_) => {
                iter.next();
                let inner = collect_text(iter);
                if !inner.is_empty() {
                    out.push(Inline::Text(inner));
                }
            }
            _ => {
                iter.next();
            }
        }
    }
    out
}

fn collect_text(iter: &mut PeekIter<'_>) -> String {
    let mut s = String::new();
    loop {
        let ev = match iter.peek().cloned() {
            None => break,
            Some(e) => e,
        };
        match ev {
            Event::End(_) => {
                iter.next();
                break;
            }
            Event::Text(t) => {
                s.push_str(&t);
                iter.next();
            }
            Event::Start(_) => {
                iter.next();
                let inner = collect_text(iter);
                s.push_str(&inner);
            }
            _ => {
                iter.next();
            }
        }
    }
    s
}

fn skip_until_end(iter: &mut PeekIter<'_>, end: TagEnd) {
    loop {
        match iter.peek().cloned() {
            None => break,
            Some(Event::End(e)) if e == end => {
                iter.next();
                break;
            }
            _ => {
                iter.next();
            }
        }
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn code_block_lang(kind: CodeBlockKind) -> Option<String> {
    match kind {
        CodeBlockKind::Fenced(lang) => {
            let s = lang.trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        }
        CodeBlockKind::Indented => None,
    }
}
