mod ast;
mod document;
mod highlight;
mod parser;
mod renderer;

pub use document::MarkdownDocument;
pub use highlight::HighlightCache;
pub use renderer::{InteractiveElement, InteractiveKind, RenderedDocument};

pub const CODE_PREVIEW_LINES: usize = 30;

use std::collections::HashSet;

pub fn render_once(
    md: &str,
    expanded_blocks: &HashSet<usize>,
    highlights: &HighlightCache,
) -> RenderedDocument {
    renderer::render(&parser::parse(md), expanded_blocks, highlights)
}
