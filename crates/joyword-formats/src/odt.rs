//! OpenDocument Text (ODF 1.3), including template packages and flat XML exports.
//!
//! Export writes `mimetype` (stored, first), `META-INF/manifest.xml`, `content.xml` (paragraphs,
//! `text:h` headings with outline levels, spans with automatic text styles, links, bookmarks,
//! line breaks / tabs / `text:s`, nested `text:list`s with generated list styles, tables with
//! header rows, column/row spans and cell shading, pictures as `draw:frame`/`draw:image` with the
//! bytes under `Pictures/`, page breaks), `styles.xml` (Standard, Heading 1–6, Title,
//! Quotations, Preformatted Text, page layout) and `meta.xml`.
//!
//! Import reads the same subset back from packages or standalone FODT XML, including automatic
//! and common styles. Notes, annotations, text boxes and drawing shapes are outside this subset.
//!
//! Tracked changes (ODF 1.3 Part 3 §5.5) go both ways: `text:tracked-changes` holds one
//! `text:changed-region` per change; an insertion is the body text between
//! `text:change-start`/`text:change-end`, a deletion keeps the deleted paragraphs in its region
//! and leaves a `text:change` point in the body. They map to `Document::revisions` and the
//! `ins`/`del` marks of runs and paragraph marks; a deleted paragraph mark is a region of
//! several paragraphs. Format changes are read as no change.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use joyword_model::props::VAlign;
use joyword_model::section::{Columns, SectionStart};
use joyword_model::{Align, Document, RevisionKind, Rgb, SectionProps};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

use crate::model::{
    self, Cell, FBlock, FTable, Flow, Fmt, Inline, Kind, ListInfo, Meta, Para, RevMark, make_img,
    mime_of,
};

mod body;
mod flat;
mod import;
mod limits;
mod package;
mod styles;
mod writer;
mod xml;

use body::Body;
use import::Pages;
pub(crate) use import::import_flat;
#[cfg(test)]
use import::{Pictures, apply_page_layout, read_entry, read_meta};
pub use import::{import, parse};
#[cfg(test)]
use limits::MAX_XML_DEPTH;
pub use package::export;
pub(crate) use package::{export_flat, export_template};
use styles::Styles;
#[cfg(test)]
use writer::x;
#[cfg(test)]
use xml::length;
use xml::{Ev, get};

mod page_parts;

#[cfg(test)]
mod tests;
