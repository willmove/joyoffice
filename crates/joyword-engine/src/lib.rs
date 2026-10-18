//! JoyWord editing engine (T101; ARCHITECTURE §3–§4).
//!
//! A [`Session`] owns the working document. Every change is a
//! [`Transaction`] of typed [`Command`]s that applies completely or not at
//! all, publishes a new monotonic version, reports a [`ChangeSet`] for layout
//! invalidation and a [`PositionMap`] for selections and other positions, and
//! records undo. Annotations (bookmarks, comment anchors, fields) are mapped
//! inside the transaction. No UI, file format or AI provider code lives here.

mod command;
mod find;
mod locate;
mod map;
mod paint;
mod session;
mod table;

pub use command::{Command, CommandError, ParagraphChange, Precondition, RunChange};
pub use find::{
    FindError, FindHit, FindQuery, FindReport, MAX_HITS, MAX_QUERY_SCALARS,
    MAX_REPLACEMENT_SCALARS, ReplacePlan, paragraph_order, replace_commands, search,
};
pub use map::{MapStep, Mapped, PositionMap, Selection};
pub use paint::{FormatBrush, PaintError, capture, paint_commands};
pub use session::{
    ApplyResult, ChangeSet, EditPolicy, MAX_UNDO_GROUP_STEPS, Origin, Outcome, Session,
    Transaction, TxError, UndoError, UndoGroup, UndoResult,
};
pub use table::{CellAddress, cell_address, cell_caret, cell_selection};
