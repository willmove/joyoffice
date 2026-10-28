//! MS-DOC (.doc) and WPS (.wps/.wpt) binary document importer (T602, T605).

pub mod budget;
pub mod clx;
pub mod fib;
pub mod reader;
pub mod wps;

pub use budget::{DocBudget, DocError};
pub use fib::{FIB_MAGIC, Fib};
pub use reader::import_doc;
pub use wps::{DocMetadata, DocumentFormat, PreservedStream};
