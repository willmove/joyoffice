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

/// A regular sector in an MS-CFB file. The last sector can be short in unpadded
/// files. Metadata consumers require a full sector; stream callers may use its tail.
pub fn sector_bytes(bytes: &[u8], sector_size: usize, id: u32) -> Option<&[u8]> {
    if !matches!(sector_size, 512 | 4096) {
        return None;
    }
    let offset = fat::sector_offset(id, sector_size).ok()?;
    let tail = bytes.get(offset..)?;
    Some(&tail[..tail.len().min(sector_size)])
}

pub fn complete_sector(bytes: &[u8], sector_size: usize, id: u32) -> Option<&[u8]> {
    sector_bytes(bytes, sector_size, id)?.get(..sector_size)
}
