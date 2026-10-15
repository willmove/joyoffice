//! What the importer kept beside the model (ARCHITECTURE §5): the original
//! file, every part's exact bytes with content types and relationships (the
//! [`Package`]), and the byte range and namespace context of every piece of
//! XML the model does not interpret, keyed by the [`PreservedId`] the model
//! refers to.
//!
//! Raw backing is preservation, not an edited export. An unchanged document
//! may copy [`PackageBacking::original_bytes`]. Edited output must use
//! [`crate::plan_export`], which only permits proven text-leaf patches and
//! rejects edits outside that boundary.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use joyoffice_opc::{Package, PartName};
use joyword_model::PreservedId;

/// Why a fragment was kept outside the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreservedReason {
    /// A block or inline construct this slice does not map (tables, content
    /// controls, drawings, fields, bookmarks…). Its paragraph or block is
    /// locked against editing.
    Unsupported,
    /// In a namespace listed by `mc:Ignorable`; kept although a consumer may
    /// ignore it (ECMA-376 Part 3).
    Ignorable,
    /// `mc:AlternateContent`; no branch is chosen in this slice.
    AlternateContent,
    /// Tracked change markup; the document is read-only.
    Revision,
    /// A property element (`w:pPr`, `w:rPr`, `w:sectPr`, a style or the
    /// settings) with children or values the model has no field for. The
    /// mapped subset is in the model; the element is kept whole.
    Properties,
    /// Text the model cannot store (it contains the model's reserved object
    /// marker); kept as an inline object.
    ReservedText,
}

/// One preserved element: where its exact bytes are and what declarations
/// were in scope around it (its own declarations are inside the bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragment {
    pub part: PartName,
    /// Qualified name as written, e.g. `w:tbl` or `jw:annotation`.
    pub element: Arc<str>,
    pub namespace: Option<Arc<str>>,
    /// Byte range of the element in the part's data.
    pub range: Range<usize>,
    /// In-scope namespace declarations from enclosing elements, outermost
    /// first; `""` is the default namespace.
    pub namespaces: Vec<(Arc<str>, Arc<str>)>,
    pub reason: PreservedReason,
}

#[derive(Clone, Debug)]
pub struct PackageBacking {
    original: Arc<[u8]>,
    package: Package,
    main_part: PartName,
    interpreted: Vec<PartName>,
    fragments: BTreeMap<PreservedId, Fragment>,
}

impl PackageBacking {
    pub(crate) fn new(original: Arc<[u8]>, package: Package, main_part: PartName) -> Self {
        Self {
            original,
            package,
            main_part,
            interpreted: Vec::new(),
            fragments: BTreeMap::new(),
        }
    }

    pub(crate) fn interpret(&mut self, part: PartName) {
        if !self.interpreted.contains(&part) {
            self.interpreted.push(part);
        }
    }

    pub(crate) fn insert(&mut self, id: PreservedId, fragment: Fragment) {
        let previous = self.fragments.insert(id, fragment);
        debug_assert!(previous.is_none(), "preserved id {id} reused");
    }

    /// The file exactly as read. Valid output only for a document that has
    /// not been edited.
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }

    /// Shared handle to the original bytes, so an export plan can record
    /// its origin without copying the package.
    pub(crate) fn shared_original(&self) -> Arc<[u8]> {
        self.original.clone()
    }

    /// Every part with its exact uncompressed bytes, content type and
    /// relationships, including parts the importer never interpreted.
    pub fn package(&self) -> &Package {
        &self.package
    }

    /// Target of the package's `officeDocument` relationship.
    pub fn main_part(&self) -> &PartName {
        &self.main_part
    }

    /// Parts mapped into the model (main document, styles, settings). All
    /// other parts are kept verbatim and not interpreted.
    pub fn interpreted_parts(&self) -> &[PartName] {
        &self.interpreted
    }

    pub fn fragment(&self, id: PreservedId) -> Option<&Fragment> {
        self.fragments.get(&id)
    }

    /// Exact source bytes of a preserved fragment.
    pub fn fragment_bytes(&self, id: PreservedId) -> Option<&[u8]> {
        let fragment = self.fragments.get(&id)?;
        let part = self.package.part(fragment.part.as_str())?;
        part.data.get(fragment.range.clone())
    }

    pub fn fragments(&self) -> impl Iterator<Item = (PreservedId, &Fragment)> {
        self.fragments.iter().map(|(id, fragment)| (*id, fragment))
    }
}
