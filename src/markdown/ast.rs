#[derive(Debug)]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    Code {
        id: usize,
        lang: Option<String>,
        content: String,
    },
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: u64,
        items: Vec<Vec<Block>>,
    },
    Rule,
}

#[derive(Debug)]
pub enum Inline {
    Text(String),
    SoftBreak,
    HardBreak,
    Code(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Link { url: String, children: Vec<Inline> },
    Image { alt: String },
    TaskListMarker(bool),
}
