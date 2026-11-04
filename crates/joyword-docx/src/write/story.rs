//! Blocks, paragraphs, runs, objects and tables → WordprocessingML.

use joyword_model::effects::ShapeEffects;
use joyword_model::fill::{Fill, GradientFlip, GradientKind, PathShape};
use joyword_model::para::{AltText, Anchor, Float, NoteKind, ShapeKind, Wrap};
use joyword_model::props::{CharProps, PropChange, Rgb, TextColor};
use joyword_model::section::{LineNumberRestart, NoteProps, SectionProps, SectionStart};
use joyword_model::table::Table;
use joyword_model::{Block, Blocks, InlineObject, Paragraph, RevisionKind};

use super::props::{
    ChangeAttrs, borders, open_change, ppr_inner, rpr, rpr_inner, tblpr, tcpr, trpr,
};
use super::{PartRels, Writer};
use crate::package::rt;
use crate::units::{emu, n, twips};
use crate::xml::W;

mod blocks;
mod drawing;
mod field;
mod object;
mod para;
mod section;
mod toc;

pub(super) use drawing::spin_attrs;
pub(super) use section::note_pr_inner;
pub(crate) use toc::written_toc_span;

const MAX_DEPTH: usize = 24;

enum Piece<'a> {
    Text(String),
    Tab,
    Br(Option<&'static str>),
    NbHyphen,
    SoftHyphen,
    /// A symbol character in its own font (`w:sym`, ECMA-376 §17.3.3.30).
    Sym(&'a str, char),
    /// An object and its byte offset in the paragraph.
    Obj(&'a InlineObject, usize),
}
