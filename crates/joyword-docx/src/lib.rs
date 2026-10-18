//! DOCX import and conservative text export (T103; ARCHITECTURE §3 ImportResult,
//! §5 format layer).
//!
//! [`import`] reads a WordprocessingML package from untrusted bytes through
//! `joyoffice-opc` (budgets, strict ZIP, part names, relationships) and maps
//! the declared subset into a [`joyword_model::Document`]:
//!
//! - body paragraphs with text (`xml:space` honoured), tabs and breaks;
//! - basic paragraph and run properties in source units, paragraph and
//!   character style references, and the paragraph mark's properties;
//! - style definitions and document defaults; built-in styles are found by
//!   canonical `w:name` through the model, never by `w:styleId`;
//! - page size, margins and title page of each section;
//! - settings: `compatibilityMode` as written (absent stays `None`, which the
//!   model interprets as 12; the new-document value 15 is never imposed),
//!   revision tracking and document protection.
//!
//! Everything else is kept, not dropped: the original file and every part's
//! exact bytes live in the [`PackageBacking`], and each element the model
//! does not interpret is registered as a [`Fragment`] with its byte range and
//! namespace context. A block or paragraph holding such content is locked in
//! [`Editability`]; document protection, revision tracking, tracked changes,
//! unsatisfied `mc:MustUnderstand` and unknown non-ignorable namespaces make
//! the whole document read-only. External relationship targets are recorded
//! and classified, never resolved or accessed. Strict documents are refused.
//!
//! [`plan_export`] supports text changes within existing paragraphs and
//! character formatting boundaries, and splitting, merging, inserting and
//! removing plain body paragraphs with default formatting. Unknown markup and
//! parts stay intact; unsupported edits are refused. The plan is versioned and
//! re-imported under the same budget. Exact copies retain the original
//! container; edited output explicitly reports regenerated ZIP metadata. Each
//! successful plan also keeps the backing's original package bytes as a
//! private origin ([`ExportPlan::origin_bytes`]). That binding is those
//! bytes, not a document version and not a caller-supplied substitute. No
//! file-system save is done.
//! Numbering definitions (`abstractNum`, `num`, level overrides) and
//! paragraph or style tab stops are mapped. `numbering.xml` is never
//! rewritten. A `w:tbl` is mapped only when every element is in the typed
//! subset (grid, rows, cells, merge state, `nil`/`single` borders and clear
//! shading); anything else keeps the whole table as an opaque block. Content
//! controls, headers/footers, notes, comments, bookmarks/fields as
//! annotations and images remain unmapped.
//!
//! Sources: ECMA-376 Part 1 (5th edition, Transitional) §11.3.10, §17.2,
//! §17.3, §17.6, §17.7, §17.13.5, §17.15; Part 2 (OPC); Part 3 (Markup
//! Compatibility); [MS-DOCX] for `compatSetting`. See the W01 report for
//! the clause list.

mod backing;
mod binding;
mod body;
mod export;
mod import;
mod ns;
mod numbering;
mod props;
mod result;
mod table;
mod xml;

pub use backing::{Fragment, PackageBacking, PreservedReason};
pub use binding::ExportBinding;
pub use export::{ExportError, ExportMode, ExportPlan, ExportTarget, ExportWarning, plan_export};
pub use import::import;
pub use ns::{
    MC, R, REL_NUMBERING, REL_OFFICE_DOCUMENT, REL_OFFICE_DOCUMENT_STRICT, REL_SETTINGS,
    REL_STYLES, W, W_STRICT,
};
pub use result::{
    Diagnostic, DiagnosticKind, Diagnostics, Editability, ImportError, ImportResult,
    MAX_DIAGNOSTICS, ReadOnlyReason, Severity,
};
