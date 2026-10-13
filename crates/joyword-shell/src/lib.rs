//! JoyWord application shell logic, independent of GPUI (T106).
//!
//! The command registry feeds menus, shortcuts and the command line from one
//! table; the localization catalog supports interpolation and plural
//! categories; preferences hold the author identity and survive damage and
//! version changes; the document manager tracks unsaved changes and the
//! close and quit flows. The GPUI app renders these; tests run headless.

pub mod command;
pub mod documents;
pub mod l10n;
pub mod prefs;
