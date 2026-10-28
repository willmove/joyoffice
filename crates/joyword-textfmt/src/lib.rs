//! HTML, Markdown, and plain text bidirectional conversion with loss reports (T601).

pub mod encoding;
pub mod html;
pub mod loss;
pub mod markdown;
pub mod plain_text;

pub use html::{export_html, import_html};
pub use loss::{LossItem, LossKind, LossReport};
pub use markdown::{export_markdown, import_markdown};
pub use plain_text::{LineEnding, PlainTextOptions, export_plain_text, import_plain_text};
