//! Stable identifiers (ARCHITECTURE §2). Identifiers are allocated from one
//! per-document counter and never reused, so a stale reference cannot silently
//! resolve to newer content.

use std::fmt;
use std::num::NonZeroU64;
use std::sync::Arc;

macro_rules! id_type {
    ($name:ident, $prefix:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU64);

        impl $name {
            pub fn get(self) -> u64 {
                self.0.get()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", $prefix, self.0)
            }
        }
    };
}

id_type!(NodeId, "#", "Block, row, cell, inline object or section.");
id_type!(
    StoryId,
    "s",
    "Independent text flow: body, header, note, comment, text box."
);
id_type!(
    AnnotationId,
    "a",
    "Range annotation: bookmark, comment anchor, field, permission."
);
id_type!(
    ResourceId,
    "r",
    "Immutable binary resource such as an image."
);
id_type!(
    PreservedId,
    "p",
    "Unexplained source content held by the format layer."
);

/// Monotonic allocator shared by every identifier kind of one document.
#[derive(Clone, Debug)]
pub struct IdAllocator {
    next: NonZeroU64,
}

impl Default for IdAllocator {
    fn default() -> Self {
        Self {
            next: NonZeroU64::MIN,
        }
    }
}

impl IdAllocator {
    fn take(&mut self) -> NonZeroU64 {
        let id = self.next;
        self.next = id.checked_add(1).expect("identifier space exhausted");
        id
    }

    pub fn node(&mut self) -> NodeId {
        NodeId(self.take())
    }

    pub fn story(&mut self) -> StoryId {
        StoryId(self.take())
    }

    pub fn annotation(&mut self) -> AnnotationId {
        AnnotationId(self.take())
    }

    pub fn resource(&mut self) -> ResourceId {
        ResourceId(self.take())
    }

    pub fn preserved(&mut self) -> PreservedId {
        PreservedId(self.take())
    }

    /// Advances to cover every identifier `other` has issued, so restoring an
    /// older document state never re-issues identifiers.
    pub fn merge(&mut self, other: &IdAllocator) {
        self.next = self.next.max(other.next);
    }

    /// Whether `raw` was handed out by this allocator.
    pub fn issued(&self, raw: u64) -> bool {
        raw < self.next.get()
    }
}

/// Where imported content came from, kept so the format layer can match model
/// nodes to preserved package content. It is not a semantic identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SourceRef {
    /// OPC part name, e.g. `/word/document.xml`.
    pub part: Arc<str>,
    /// Locator inside the part, e.g. a `w14:paraId` value or element path.
    pub locator: Arc<str>,
}
