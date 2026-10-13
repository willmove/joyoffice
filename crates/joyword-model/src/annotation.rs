//! Range annotations shared by bookmarks, comment anchors, complex fields,
//! permission ranges and inline content controls (ARCHITECTURE §2, REVIEW-006).
//!
//! An annotation is anchored by two positions in one story or explicitly
//! detached. Detached annotations stay in the document so a lost anchor is
//! visible instead of silently deleting a comment.

use std::sync::Arc;

use crate::block::SdtProps;
use crate::id::{AnnotationId, NodeId, SourceRef, StoryId};
use crate::text::ScalarOffset;

/// Which side of an offset a position sticks to when text is inserted there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bias {
    Before,
    After,
}

/// A text position: paragraph, scalar offset and boundary bias.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    pub story: StoryId,
    pub paragraph: NodeId,
    pub offset: ScalarOffset,
    pub bias: Bias,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnnotationKind {
    Bookmark {
        name: Arc<str>,
    },
    /// Anchor of a comment whose body is a [`crate::StoryKind::Comment`] story.
    Comment {
        body: StoryId,
    },
    /// Complex field: instruction from start to `separator`, cached result
    /// from `separator` to end. Fields may span paragraphs.
    Field {
        separator: Option<Position>,
    },
    Permission {
        editor: Option<Arc<str>>,
        group: Option<Arc<str>>,
    },
    /// Run-level content control (block-level ones are [`crate::Sdt`]).
    ContentControl(SdtProps),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Anchor {
    Range {
        start: Position,
        end: Position,
    },
    /// The anchored content was deleted or could not be mapped.
    Detached,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub id: AnnotationId,
    pub kind: AnnotationKind,
    pub anchor: Anchor,
    pub source: Option<Arc<SourceRef>>,
}
