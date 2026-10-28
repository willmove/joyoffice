//! Format-neutral Compound File Binary (MS-CFB) layer with untrusted-input budgets.
//!
//! Shared by JoyWord's T602 (MS-DOC), T605 (WPS), and T604 (Encrypted OOXML).

mod budget;
mod compound;
mod directory;
mod fat;
mod header;
mod writer;

pub use budget::{CfbBudget, CfbError};
pub use compound::CompoundFile;
pub use directory::{DirEntry, EntryType};
pub use header::{CFB_SIGNATURE, HEADER_SIZE};
pub use writer::CfbWriter;
