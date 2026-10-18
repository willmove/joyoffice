//! JoyWord application shell logic, independent of GPUI (T106).
//!
//! The command registry feeds menus, shortcuts and the command line from one
//! table; the localization catalog supports interpolation and plural
//! categories; preferences hold the author identity and survive damage and
//! version changes; diagnostics write a per-launch size-capped log and a
//! redacting panic hook; the document manager tracks unsaved changes and the
//! close and quit flows. The GPUI app renders these; tests run headless.

pub mod command;
pub mod diag;
pub mod documents;
pub mod docx;
pub mod host;
pub mod l10n;
pub mod prefs;

pub mod storage;

pub mod input;
