//! JoyWord editing engine (T101; ARCHITECTURE §3–§4).
//!
//! A [`Session`] owns the working document. Every change is a
//! [`Transaction`] of typed [`Command`]s that applies completely or not at
//! all, publishes a new monotonic version, reports a [`ChangeSet`] for layout
//! invalidation and a [`PositionMap`] for selections and other positions, and
//! records undo. Annotations (bookmarks, comment anchors, fields) are mapped
//! inside the transaction. No UI, file format or AI provider code lives here.

mod command;
mod locate;
mod map;
mod session;

pub use command::{Command, CommandError, ParagraphChange, Precondition, RunChange};
pub use map::{MapStep, Mapped, PositionMap, Selection};
pub use session::{
    ApplyResult, ChangeSet, EditPolicy, MAX_UNDO_GROUP_STEPS, Origin, Outcome, Session,
    Transaction, TxError, UndoError, UndoGroup, UndoResult,
};
