use std::collections::HashSet;

use super::{HighlightCache, RenderedDocument, ast::Block, parser, renderer};

#[derive(Debug)]
pub struct MarkdownDocument {
    source: String,
    blocks: Vec<Block>,
}

impl MarkdownDocument {
    pub fn new(source: String) -> Self {
        let blocks = parser::parse(&source);
        Self { source, blocks }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn render(
        &self,
        expanded: &HashSet<usize>,
        highlights: &HighlightCache,
    ) -> RenderedDocument {
        renderer::render(&self.blocks, expanded, highlights)
    }
}
