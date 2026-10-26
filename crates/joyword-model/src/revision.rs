//! Text, move and format revisions (ECMA-376 Part 1 §17.13.5, T300/T301).
//!
//! A run carries at most one [`TextRevision`]. `w:ins` and `w:moveTo` are
//! present in the final projection; `w:del` and `w:moveFrom` are present in
//! the original projection. Move range markers do not occupy a text position.
//! Format revisions (`w:rPrChange` §17.13.5.30, `w:pPrChange` §17.13.5.29)
//! are typed as [`FormatRevision`]: a run stores the pre-change run properties
//! and a paragraph stores the pre-change paragraph properties. Table
//! structure revisions (`w:tblPrChange`, `w:trPrChange`, `w:tcPrChange`,
//! `w:cellIns`, `w:cellDel`, `w:cellMerge`) are not represented here.

use std::sync::Arc;

use crate::annotation::{Bias, Position};
use crate::props::{ParaProps, RunProps};
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
            // Simple markup shows the final projection; only its marks differ.
            RevisionView::Final | RevisionView::Simple => {
                matches!(self, Self::Insert | Self::MoveTo)
            }
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

/// A tracked formatting change. `w:rPrChange` (§17.13.5.30) stores the run
/// properties before the change in `previous_run`; `w:pPrChange` (§17.13.5.29)
/// stores the paragraph properties before the change in `previous_para`.
/// `id` is `w:id`, `author` is `w:author`, `date` is the `w:date` string as
/// written. Accepting the revision keeps the current formatting and drops the
/// change; rejecting it restores the previous formatting. Previous values are
/// only typed for the properties JoyWord represents; content outside that
/// subset keeps its original bytes through the preserved-markup fallback and
/// never reaches this struct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatRevision {
    pub id: u32,
    pub author: Arc<str>,
    pub date: Option<Arc<str>>,
    /// Pre-change run properties (`w:rPrChange/w:rPr`), present for run-level
    /// changes.
    pub previous_run: Option<Arc<RunProps>>,
    /// Pre-change paragraph properties (`w:pPrChange/w:pPr`), present for
    /// paragraph-level changes.
    pub previous_para: Option<Arc<ParaProps>>,
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
/// or delete revisions (SPEC §6, REVIEW-003). Markup also paints a simple
/// bar in the page's left margin beside each changed line.
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
    /// Simple markup (REVIEW-003): the final projection with no inline
    /// revision decoration. The only mark is the author-colour margin bar
    /// beside lines of a paragraph with pending revisions.
    Simple,
}

impl RevisionView {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FinalWithMarkup => "markup",
            Self::Final => "final",
            Self::Original => "original",
            Self::Simple => "simple",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "markup" => Some(Self::FinalWithMarkup),
            "final" => Some(Self::Final),
            "original" => Some(Self::Original),
            "simple" => Some(Self::Simple),
            _ => None,
        }
    }
}
