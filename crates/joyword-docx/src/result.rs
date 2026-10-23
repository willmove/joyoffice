//! Import outcome: errors that refuse a package, diagnostics reported with
//! an imported one, and the editable boundary (ARCHITECTURE §3 ImportResult).

use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use joyoffice_opc::{ExternalKind, OpcError, XmlError};
use joyword_model::{Document, Issue, NodeId};

use crate::backing::{PackageBacking, PreservedReason};

/// The package is refused; nothing was imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// Unsafe or unreadable container, invalid part names, unreadable
    /// content types or relationships, or an exceeded budget.
    Package(OpcError),
    /// No `officeDocument` relationship from the package.
    MissingMainDocument,
    /// More than one `officeDocument` relationship.
    AmbiguousMainDocument,
    /// Strict conformance (ISO/IEC 29500 Strict namespaces or relationship
    /// types) is recognised but not implemented.
    StrictUnsupported,
    /// The main document relationship is external or cannot be resolved.
    InvalidMainDocumentTarget {
        target: String,
    },
    /// The main document relationship points at a part that does not exist.
    MissingPart {
        part: String,
    },
    /// The main document part is not a WordprocessingML document.
    UnexpectedContentType {
        part: String,
        content_type: Option<String>,
    },
    Xml {
        part: String,
        error: XmlError,
    },
    /// The root or a required element is not what the part type requires.
    UnexpectedElement {
        part: String,
        expected: &'static str,
    },
}

impl From<OpcError> for ImportError {
    fn from(error: OpcError) -> Self {
        ImportError::Package(error)
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Package(error) => write!(f, "package: {error}"),
            ImportError::MissingMainDocument => f.write_str("no main document relationship"),
            ImportError::AmbiguousMainDocument => {
                f.write_str("more than one main document relationship")
            }
            ImportError::StrictUnsupported => {
                f.write_str("Strict Open XML documents are not supported yet")
            }
            ImportError::InvalidMainDocumentTarget { target } => {
                write!(f, "main document target {target:?} is not a package part")
            }
            ImportError::MissingPart { part } => write!(f, "part {part} is missing"),
            ImportError::UnexpectedContentType { part, content_type } => {
                write!(f, "{part} has content type {content_type:?}")
            }
            ImportError::Xml { part, error } => write!(f, "{part}: {error:?}"),
            ImportError::UnexpectedElement { part, expected } => {
                write!(f, "{part}: expected {expected}")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// Why the whole document must open read-only.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReadOnlyReason {
    /// Enforced `w:documentProtection`; `edit` is the restriction as written.
    DocumentProtection { edit: Option<String> },
    /// `w:writeProtection`: the author asked for read-only opening.
    WriteProtection,
    /// `w:trackRevisions` used to force read-only opening. Text revisions are
    /// modeled now, so this reason is no longer produced. Document protection
    /// still is.
    TrackRevisions,
    /// Revision markup this slice does not edit: table structure revisions
    /// (`w:tblPrChange`, `w:trPrChange`, `w:tcPrChange`, `w:cellIns`,
    /// `w:cellDel`, `w:cellMerge`, and the other non-text revisions) anywhere,
    /// and format revisions (`w:rPrChange`, `w:pPrChange`) inside tables.
    /// Format revisions in paragraphs outside tables are typed (T301).
    Revisions,
    /// `mc:MustUnderstand` names a namespace this importer does not
    /// understand (ECMA-376 Part 3).
    MustUnderstand { namespace: String },
    /// An element in a namespace that is neither understood, a known Office
    /// namespace, nor ignorable.
    UnknownNamespace { namespace: String },
    /// The imported model failed validation; an importer defect.
    ModelInvalid,
}

/// The editable boundary of an imported document.
///
/// Read-only reasons apply to the whole document. Locked nodes are blocks
/// and paragraphs that contain preserved content; editing them could break
/// what the backing keeps. Edited output additionally requires a successful
/// export plan; editability alone is not a guarantee that an edit can export.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Editability {
    read_only: BTreeSet<ReadOnlyReason>,
    locked: BTreeSet<NodeId>,
}

impl Editability {
    pub(crate) fn deny(&mut self, reason: ReadOnlyReason) {
        self.read_only.insert(reason);
    }

    pub(crate) fn lock(&mut self, node: NodeId) {
        self.locked.insert(node);
    }

    pub fn is_read_only(&self) -> bool {
        !self.read_only.is_empty()
    }

    pub fn read_only_reasons(&self) -> impl Iterator<Item = &ReadOnlyReason> {
        self.read_only.iter()
    }

    pub fn locked(&self) -> &BTreeSet<NodeId> {
        &self.locked
    }

    /// Whether `node` (a top-level or nested block) may be edited.
    pub fn can_edit(&self, node: NodeId) -> bool {
        !self.is_read_only() && !self.locked.contains(&node)
    }

    /// Whether nothing at all is restricted.
    pub fn is_unrestricted(&self) -> bool {
        self.read_only.is_empty() && self.locked.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Recorded for completeness; content is kept and nothing is restricted.
    Info,
    /// Content is restricted, defaulted or not imported, but kept.
    Warning,
    /// The document is read-only because of this.
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// Recoverable finding of the OPC layer.
    Package(joyoffice_opc::Diagnostic),
    /// A relationship target outside the package, recorded and classified
    /// but never resolved or accessed.
    ExternalTarget {
        source: String,
        id: String,
        relationship: String,
        target: String,
        kind: ExternalKind,
    },
    /// An element kept in the backing instead of the model.
    Preserved {
        element: String,
        reason: PreservedReason,
    },
    ReadOnly(ReadOnlyReason),
    /// A relationship from the main document points at a missing part.
    MissingPart {
        relationship: String,
        part: String,
    },
    /// A part related to the main document that this slice keeps but does not
    /// import (unreferenced headers and footers, notes, comments, custom
    /// parts, and image relationships whose targets were not stored).
    NotImported {
        relationship: String,
        part: String,
    },
    /// An attribute value outside the supported set; the property is unset in
    /// the model and its element is preserved.
    InvalidValue {
        element: String,
        attribute: String,
        value: String,
    },
    /// A style without `w:styleId`, or with an id already used; kept in the
    /// backing, not in the style sheet.
    StyleSkipped {
        id: Option<String>,
    },
    /// The section omits page size or margins; product defaults were used.
    SectionDefaults {
        missing: &'static str,
    },
    /// Property elements (`w:pPr`, `w:rPr`, `w:sectPr`, styles, settings)
    /// in this part kept whole because they hold something the model has no
    /// field for; see [`crate::PackageBacking::fragments`].
    PropertiesPreserved {
        count: usize,
    },
    /// Proofing state and layout cache hints (`w:proofErr`,
    /// `w:lastRenderedPageBreak`) not represented in the model; they remain
    /// in the part's original bytes.
    HintsSkipped {
        element: String,
        count: usize,
    },
    /// `w:t` without `xml:space="preserve"` begins or ends with white
    /// space. The characters are kept as written; whether Word or WPS
    /// display them is not yet verified.
    UnpreservedWhitespace,
    /// The body has no block; one empty paragraph was added.
    EmptyBody,
    /// A VBA project is present; it is kept and never executed.
    MacroProject,
    /// An inline picture and how this slice treated it. The relationship id
    /// is kept; a URL or UNC target is not copied into the diagnostic.
    Image {
        relationship: String,
        outcome: ImageOutcome,
    },
    /// Validation finding of the imported model.
    Model(Issue),
    /// A text or move revision was preserved because it is outside the typed
    /// subset (missing id or author, extra attributes, nested markup, or a
    /// child this slice does not map). The paragraph is locked.
    RevisionSubset {
        element: String,
    },
    /// A move range is missing a marker, repeats one, or crosses its pair.
    /// The paragraphs it names are locked. Accept and reject refuse the name.
    MoveRange {
        name: String,
    },
    /// `w:comment` has no `w:author`. The comment is kept.
    CommentAuthorMissing {
        id: u32,
    },
    /// A root comment has no range markers. The anchor is detached.
    CommentAnchorMissing {
        id: u32,
    },
    /// Range markers sit in markup this slice does not map, such as an
    /// opaque table. The anchor is detached.
    CommentAnchorUnmapped {
        id: u32,
    },
    /// Markers are duplicated, reversed, split across stories, or a reply
    /// cycle cannot be anchored.
    CommentAnchorInvalid {
        id: u32,
    },
    /// `w15:paraIdParent` names no comment. The reply is kept and detached.
    OrphanReply {
        para_id: String,
    },
    /// A `w15:commentEx` `paraId` matches no comment. The element is kept.
    CommentExtensionUnmatched {
        para_id: String,
    },
    /// A later `w:comment` reuses `w:id`. The first comment is kept.
    CommentIdDuplicate {
        id: u32,
    },
    /// A `w:comment` has no usable `w:id` and was not imported.
    CommentIdMissing,
    /// Comment markup is outside the shape this slice reads. The part bytes
    /// are kept.
    CommentPartInvalid,
    /// A comment has no `w:p`. An empty paragraph was added.
    CommentBodyEmpty {
        id: u32,
    },
    /// `w:bookmarkStart` or `w:bookmarkEnd` has no usable `w:id`. The marker
    /// bytes are kept. No annotation is inserted.
    BookmarkIdMissing,
    /// The bookmark name is empty or only whitespace. PDF would refuse it.
    /// The name is not echoed. Not inserted.
    BookmarkNameEmpty {
        id: u32,
    },
    /// The bookmark name contains a control character. PDF would refuse it.
    /// The name is not echoed. Not inserted.
    BookmarkNameInvalid {
        id: u32,
    },
    /// A later bookmark reuses a name. ECMA-376 keeps the first. The name is
    /// not echoed. The duplicate is not inserted.
    BookmarkNameDuplicate {
        id: u32,
    },
    /// Markers are missing, repeated, reversed, split across stories, or
    /// partly inside unmapped markup. Not inserted. PDF would refuse a
    /// detached bookmark.
    BookmarkAnchorInvalid {
        id: u32,
    },
    /// The bookmark sits only in markup this slice does not map, such as an
    /// opaque table. Not inserted.
    BookmarkAnchorUnmapped {
        id: u32,
    },
    /// The range is not in the body story. Layout carets are body positions,
    /// so the bookmark is not a PDF destination. Not inserted.
    BookmarkOutsideBody {
        id: u32,
    },
    /// `w:colFirst` / `w:colLast` mark table columns, not a text range.
    /// Not inserted.
    BookmarkColumn {
        id: u32,
    },
    /// The name exceeds the PDF name budget, or the document already has the
    /// PDF bookmark budget of accepted names. Not inserted.
    BookmarkBudget {
        id: u32,
    },
    /// The marker has attributes or child content this slice does not
    /// represent. Not inserted.
    BookmarkUnsupported {
        id: u32,
    },
    /// Bookmark markers kept as fragments. Their paragraphs stay locked
    /// because this slice does not write the markers back. `count` is the
    /// number of `w:bookmarkStart` / `w:bookmarkEnd` elements preserved.
    BookmarksPreserved {
        count: usize,
    },
}

/// What happened to one inline picture. Remote and active payloads are not
/// fetched or executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOutcome {
    Imported,
    Remote,
    Active,
    Malformed,
    Oversize,
    /// Wrap, behind-document, or a position this slice does not place.
    /// The drawing stays preserved; its bytes are not fetched.
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Part the finding concerns, when it concerns one.
    pub part: Option<String>,
    /// Byte range in that part, when known.
    pub range: Option<Range<usize>>,
    pub kind: DiagnosticKind,
}

/// Most diagnostics kept; further ones are only counted, so hostile input
/// cannot produce unbounded output. Read-only reasons are kept separately in
/// [`Editability`] and are never dropped.
pub const MAX_DIAGNOSTICS: usize = 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
    suppressed: usize,
}

impl Diagnostics {
    pub(crate) fn push(&mut self, diagnostic: Diagnostic) {
        if self.items.len() < MAX_DIAGNOSTICS {
            self.items.push(diagnostic);
        } else {
            self.suppressed += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.suppressed == 0
    }

    /// Diagnostics beyond [`MAX_DIAGNOSTICS`] that were counted, not kept.
    pub fn suppressed(&self) -> usize {
        self.suppressed
    }

    pub fn max_severity(&self) -> Option<Severity> {
        self.items.iter().map(|item| item.severity).max()
    }
}

/// A document imported read-only from DOCX: the model, what was kept beside
/// it, what was found, and what may be edited.
#[derive(Clone, Debug)]
pub struct ImportResult {
    pub document: Document,
    pub backing: PackageBacking,
    pub diagnostics: Diagnostics,
    pub editability: Editability,
}
