//! Text and move revisions (ECMA-376 Part 1 §17.13.5, T300).
//!
//! A run carries at most one [`TextRevision`]. `w:ins` and `w:moveTo` are
//! present in the final projection; `w:del` and `w:moveFrom` are present in
//! the original projection. Move range markers do not occupy a text position.
//! Format and table revisions (`w:rPrChange`, `w:pPrChange`, row and cell
//! revisions) are not represented here.

use std::sync::Arc;

use crate::annotation::{Bias, Position};
use crate::text::ScalarOffset;

/// `w:ins`, `w:del`, `w:moveFrom` or `w:moveTo` (§17.13.5.18, §17.13.5.14,
/// §17.13.5.22, §17.13.5.25).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RevisionKind {
    Insert,
    Delete,
    MoveFrom,
    MoveTo,
}

impl RevisionKind {
    /// Whether text of this kind is part of the projection `view` lays out.
    pub fn visible_in(self, view: RevisionView) -> bool {
        match view {
            RevisionView::FinalWithMarkup => true,
            RevisionView::Final => matches!(self, Self::Insert | Self::MoveTo),
            RevisionView::Original => matches!(self, Self::Delete | Self::MoveFrom),
        }
    }

    pub fn is_move(self) -> bool {
        matches!(self, Self::MoveFrom | Self::MoveTo)
    }

    /// Insertions and move destinations are new text in the final projection.
    pub fn is_final_text(self) -> bool {
        matches!(self, Self::Insert | Self::MoveTo)
    }
}

/// One tracked-change mark on a run. `id` is `w:id`. `date` is the `w:date`
/// string as written, not a reparsed clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRevision {
    pub id: u32,
    pub kind: RevisionKind,
    pub author: Arc<str>,
    pub date: Option<Arc<str>>,
}

/// Which side of a move a range marker belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoveSide {
    From,
    To,
}

/// `w:moveFromRangeStart` / `End` and `w:moveToRangeStart` / `End`
/// (§17.13.5.21–§17.13.5.26). `name` pairs the two sides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveMarker {
    pub id: u32,
    pub name: Arc<str>,
    pub author: Arc<str>,
    pub date: Option<Arc<str>>,
    pub side: MoveSide,
    /// `false` is the start marker, `true` the end marker.
    pub at_end: bool,
    pub position: Position,
}

impl MoveMarker {
    /// Start markers stay before text inserted on their offset. End markers
    /// move past it, so text typed at the end of a range stays inside it.
    pub fn position_for(
        paragraph_story: crate::id::StoryId,
        paragraph: crate::id::NodeId,
        offset: usize,
        at_end: bool,
    ) -> Position {
        Position {
            story: paragraph_story,
            paragraph,
            offset: ScalarOffset(offset),
            bias: if at_end { Bias::After } else { Bias::Before },
        }
    }
}

/// How layout projects tracked changes. Switching the view does not accept
/// or delete revisions (SPEC §6, REVIEW-003). Simple margin marks are not
/// this slice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RevisionView {
    /// Final text with markup: insertions underlined and coloured, deletions
    /// struck through. Move destinations follow insertions; move sources
    /// follow deletions.
    #[default]
    FinalWithMarkup,
    /// Final projection. Insertions and move destinations are shown.
    /// Deletions and move sources take no space.
    Final,
    /// Original projection. Deletions and move sources are shown as ordinary
    /// text. Insertions and move destinations take no space.
    Original,
}

impl RevisionView {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FinalWithMarkup => "markup",
            Self::Final => "final",
            Self::Original => "original",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "markup" => Some(Self::FinalWithMarkup),
            "final" => Some(Self::Final),
            "original" => Some(Self::Original),
            _ => None,
        }
    }
}
