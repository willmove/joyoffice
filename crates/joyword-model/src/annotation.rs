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

/// An attribute kept on `w:comment` or `w15:commentEx` that this slice does
/// not interpret. Namespace is `None` for an un-namespaced attribute.
///
/// ECMA-376 Part 1 §17.13.4.2; [MS-DOCX] commentsExtended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentAttribute {
    pub namespace: Option<Arc<str>>,
    pub local: Arc<str>,
    pub value: Arc<str>,
}

/// One comment in a thread. The body is a [`crate::StoryKind::Comment`] story.
/// `resolved` is not deletion: a resolved comment keeps its anchor
/// (SPEC REVIEW-005, REVIEW-006).
///
/// `source_id` is `w:id`. `para_id` is the first paragraph's `w14:paraId`
/// (eight hex digits). A reply sets `parent_para_id` from `w15:paraIdParent`
/// and usually has no range of its own ([MS-DOCX] commentsExtended).
/// `durable_id` and `date_utc` come from commentsIds and commentsExtensible.
/// `extra_markup` is the raw XML of children that are not `w:p`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub body: StoryId,
    pub source_id: u32,
    pub author: Option<Arc<str>>,
    pub date: Option<Arc<str>>,
    pub initials: Option<Arc<str>>,
    pub para_id: Option<Arc<str>>,
    pub parent_para_id: Option<Arc<str>>,
    pub resolved: bool,
    pub durable_id: Option<Arc<str>>,
    pub date_utc: Option<Arc<str>>,
    pub extra_attributes: Arc<[CommentAttribute]>,
    pub extra_markup: Option<Arc<str>>,
}

impl Comment {
    /// A comment body with no author, thread or extension fields yet.
    pub fn new(body: StoryId) -> Self {
        Self {
            body,
            source_id: 0,
            author: None,
            date: None,
            initials: None,
            para_id: None,
            parent_para_id: None,
            resolved: false,
            durable_id: None,
            date_utc: None,
            extra_attributes: Arc::from([]),
            extra_markup: None,
        }
    }
}

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
    Comment(Comment),
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
    /// Footnote reference anchoring to a [`crate::StoryKind::Footnote`] story.
    Footnote {
        story: StoryId,
        custom_mark: Option<Arc<str>>,
    },
    /// Endnote reference anchoring to a [`crate::StoryKind::Endnote`] story.
    Endnote {
        story: StoryId,
        custom_mark: Option<Arc<str>>,
    },
}

impl AnnotationKind {
    /// A comment anchor with no author, date or thread fields yet.
    pub fn comment(body: StoryId) -> Self {
        Self::Comment(Comment::new(body))
    }
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
