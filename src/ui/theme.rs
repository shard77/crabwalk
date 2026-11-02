use ratatui::style::{Color, Modifier, Style};

const MUTED: Color = Color::Indexed(245); // #8a8a8a
const MUTED_DIM: Color = Color::Indexed(240); // #585858
const SUBTLE_BG: Color = Color::Indexed(236); // #303030

const CANVAS: Color = Color::Rgb(17, 17, 17); // #111111
const SURFACE_RECESSED: Color = Color::Rgb(25, 25, 25); // #191919
const SURFACE: Color = Color::Rgb(34, 34, 34); // #222222
const SURFACE_RAISED: Color = Color::Rgb(42, 42, 42); // #2a2a2a
const SURFACE_FOCUSED: Color = Color::Rgb(49, 49, 49); // #313131
const SURFACE_HIGHLIGHT: Color = Color::Rgb(58, 58, 58); // #3a3a3a
const TEXT_STRONG: Color = Color::Rgb(238, 238, 238); // #eeeeee
const TEXT_BODY: Color = Color::Rgb(212, 212, 212); // #d4d4d4
const TEXT_SECONDARY: Color = Color::Rgb(188, 188, 188); // #bcbcbc
const TEXT_MUTED: Color = Color::Rgb(168, 168, 168); // #a8a8a8
const INFORMATION: Color = Color::Rgb(121, 192, 255); // #79c0ff
const SUCCESS: Color = Color::Rgb(141, 219, 154); // #8ddb9a
const ATTENTION: Color = Color::Rgb(227, 179, 65); // #e3b341
const REMOVAL: Color = Color::Rgb(255, 161, 152); // #ffa198
const SUCCESS_BG: Color = Color::Rgb(28, 52, 39); // #1c3427
const REMOVAL_BG: Color = Color::Rgb(60, 36, 40); // #3c2428

pub const BORDER: Style = Style::new().fg(MUTED_DIM);
pub const BORDER_FOCUSED: Style = Style::new().fg(Color::Yellow);
pub const BORDER_ERROR: Style = Style::new().fg(Color::Red);
pub const BORDER_SUCCESS: Style = Style::new().fg(Color::Green);
pub const BORDER_MODAL: Style = Style::new().fg(MUTED);
pub const DETAIL_SCROLLBAR: Style = Style::new().fg(MUTED_DIM).bg(CANVAS);

pub const PR_NUMBER: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
pub const PR_TITLE: Style = Style::new().add_modifier(Modifier::BOLD);
pub const METADATA_LABEL: Style = Style::new().fg(MUTED);
pub const PR_METADATA_CHIP: Style = Style::new().fg(TEXT_BODY).bg(SURFACE_RAISED);
pub const AUTHOR: Style = Style::new().add_modifier(Modifier::BOLD);
pub const COMMENT_AUTHOR: Style = AUTHOR.fg(TEXT_STRONG);
pub const MUTED_TEXT: Style = Style::new().fg(MUTED);
pub const PLACEHOLDER: Style = Style::new().fg(MUTED);

// Headings also appear on unshaded surfaces, so inherit their foreground.
pub const HEADING_1: Style = Style::new()
    .add_modifier(Modifier::BOLD)
    .add_modifier(Modifier::UNDERLINED);
pub const HEADING: Style = Style::new().add_modifier(Modifier::BOLD);

pub const SELECTED_ITEM: Style = Style::new().bg(SUBTLE_BG).add_modifier(Modifier::BOLD);
pub const CHOICE_SELECTED: Style = Style::new()
    .fg(Color::Black)
    .bg(Color::Yellow)
    .add_modifier(Modifier::BOLD);
pub const SEARCH_MATCH: Style = Style::new().fg(Color::Black).bg(Color::Yellow);
pub const VISUAL_SELECTION: Style = Style::new().fg(Color::White).bg(Color::Indexed(25));
pub const CURSOR: Style = Style::new().add_modifier(Modifier::REVERSED);

pub const CODE_BODY: Style = Style::new().fg(TEXT_BODY).bg(SURFACE_RECESSED);
pub const INLINE_CODE: Style = CODE_BODY.fg(TEXT_STRONG);
// Keep an RGB foreground: syntax highlighting uses its luminance to brighten dim tokens.
pub const CODE_BORDER: Style = Style::new().fg(TEXT_MUTED);
pub const CODE_HEADER: Style = Style::new().fg(TEXT_MUTED).bg(SURFACE_RAISED);
pub const CODE_HEADER_FOCUSED: Style = CODE_HEADER.fg(TEXT_STRONG).add_modifier(Modifier::BOLD);
pub const LINK: Style = Style::new()
    .fg(INFORMATION)
    .add_modifier(Modifier::UNDERLINED);

// Reflow recognizes quote gutters by this colour; keep it distinct from BLOCKQUOTE_TEXT.
pub const BLOCKQUOTE_BAR: Style = Style::new().fg(TEXT_MUTED);
pub const BLOCKQUOTE_TEXT: Style = Style::new().fg(TEXT_SECONDARY);
pub const RULE_LINE: Style = Style::new().fg(TEXT_MUTED);
pub const LIST_BULLET: Style = Style::new().fg(TEXT_MUTED);
pub const TASK_MARKER: Style = Style::new().fg(TEXT_MUTED);
pub const IMAGE_ALT: Style = Style::new().fg(TEXT_MUTED);

pub const COMMENT_BODY_BACKGROUND: Style = Style::new().fg(TEXT_BODY).bg(SURFACE);
pub const COMMENT_HEADER_BACKGROUND: Style = Style::new().fg(TEXT_BODY).bg(SURFACE_RAISED);
pub const COMMENT_GAP_BACKGROUND: Style = Style::new().bg(CANVAS);
pub const SECTION_HEADER: Style = Style::new().fg(TEXT_BODY);
pub const SECTION_HEADER_ACTIVE: Style = SECTION_HEADER.add_modifier(Modifier::BOLD);
pub const COMMENT_HEADER_ACTIVE: Style = SECTION_HEADER_ACTIVE.add_modifier(Modifier::UNDERLINED);
pub const COMMENT_META: Style = Style::new().fg(TEXT_MUTED);
pub const COMMENT_PATH: Style = Style::new().fg(TEXT_SECONDARY);

pub const REVIEW_BACKGROUND: Style = Style::new().fg(TEXT_BODY).bg(CANVAS);
pub const REVIEW_BRANCH: Style = Style::new().fg(TEXT_MUTED).bg(CANVAS);
pub const REVIEW_HEADER: Style = COMMENT_AUTHOR;
pub const REVIEW_META: Style = COMMENT_META;
pub const REVIEW_PATH: Style = COMMENT_PATH;
pub const REVIEW_APPROVED: Style = Style::new().fg(SUCCESS);
pub const REVIEW_CHANGES: Style = Style::new().fg(ATTENTION);
pub const REVIEW_STATUS: Style = Style::new().fg(TEXT_MUTED).bg(SURFACE_HIGHLIGHT);
pub const REVIEW_AUTHOR_BADGE: Style = Style::new().fg(TEXT_SECONDARY).bg(SURFACE_HIGHLIGHT);
pub const REVIEW_RESOLVED_BADGE: Style = Style::new().fg(SUCCESS).bg(SUCCESS_BG);

pub const DIFF_CONTEXT: Style = CODE_BODY;
pub const DIFF_INSERT: Style = Style::new().fg(SUCCESS).bg(SUCCESS_BG);
pub const DIFF_DELETE: Style = Style::new().fg(REMOVAL).bg(REMOVAL_BG);
pub const DIFF_HEADER: Style = CODE_HEADER;
pub const DIFF_GUTTER: Style = Style::new().fg(TEXT_MUTED);

pub const STATUS_BAR: Style = Style::new().fg(MUTED).bg(SUBTLE_BG);
pub const STATUS_BAR_LOADING: Style = Style::new().fg(Color::Yellow).bg(SUBTLE_BG);

pub const MODAL_HINT: Style = Style::new().fg(MUTED);
pub const INPUT_TEXT: Style = Style::new().fg(Color::White);

pub const FILTER_SURFACE: Style = APP_BACKGROUND;
pub const FILTER_TITLE: Style = PR_TITLE;
pub const FILTER_SECTION: Style = SECTION_HEADER_ACTIVE;
pub const FILTER_META: Style = MODAL_HINT;
pub const FILTER_ACCENT: Style = CONTEXT_VALUE;
pub const FILTER_SELECTED: Style = SELECTED_ITEM;
pub const FILTER_KEY: Style = PR_METADATA_CHIP;

pub const FOCUS_GUTTER: Style = Style::new().fg(TEXT_STRONG).add_modifier(Modifier::BOLD);
pub const FOCUS_BLOCK_BG: Style = Style::new().bg(SURFACE_FOCUSED);

pub const APP_BACKGROUND: Style = Style::new().fg(TEXT_BODY).bg(CANVAS);
pub const FRAME_BORDER: Style = Style::new().fg(MUTED_DIM);
pub const TAB_ACTIVE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
pub const TAB_INACTIVE: Style = Style::new().fg(MUTED);
pub const CONTEXT_LINE: Style = Style::new().fg(MUTED);
pub const CONTEXT_VALUE: Style = Style::new().fg(Color::Yellow);

pub const DROPDOWN_BORDER: Style = Style::new().fg(Color::Yellow);
pub const DROPDOWN_ITEM: Style = Style::new().fg(Color::White);

pub const DRAFT_MARKER: Style = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
pub const INACTIVE_OVERDUE: Style = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
pub const INACTIVE_STALE: Style = Style::new().fg(Color::Yellow);
pub const INACTIVE_RECENT: Style = MUTED_TEXT;
pub const STALE_NONE: Style = Style::new().fg(Color::Green);
