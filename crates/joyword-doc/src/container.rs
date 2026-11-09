//! Compatibility for the compound file around a binary Word document.

use std::io::{self, Cursor, Read, Seek, SeekFrom};

use crate::DocbinError;

const MINIFAT_HEADER_OFFSET: usize = 0x3C;
const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
const NO_MINIFAT: [u8; 8] = [0xFE, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0];

pub(super) fn open_container(bytes: &[u8]) -> Result<cfb::CompoundFile<Source<'_>>, DocbinError> {
    joyword_model::cfb_guard::check(bytes).map_err(DocbinError::Container)?;
    let source = |ignore_minifat| Source {
        bytes: Cursor::new(bytes),
        ignore_minifat,
    };
    let error = match cfb::CompoundFile::open(source(false)) {
        Ok(compound) => return Ok(compound),
        Err(error) => error,
    };

    // Some textutil DOC files carry an unused, zero-filled MiniFAT. [MS-CFB] §2.4
    // permits omitting the MiniFAT when all nonempty streams use regular sectors.
    if error.kind() == io::ErrorKind::InvalidData
        && has_zeroed_minifat_sector(bytes)
        && let Ok(compound) = cfb::CompoundFile::open(source(true))
        && !compound
            .walk()
            .any(|entry| entry.is_stream() && (1..4096).contains(&entry.len()))
    {
        return Ok(compound);
    }
    Err(DocbinError::Container(error.to_string()))
}

/// Recognize a single zeroed MiniFAT sector whose FAT entry terminates the chain.
fn has_zeroed_minifat_sector(bytes: &[u8]) -> bool {
    let u32_at = |offset: usize| {
        let field = bytes.get(offset..offset.checked_add(4)?)?;
        Some(u32::from_le_bytes(field.try_into().ok()?))
    };
    let sector_size = match bytes.get(0x1E..0x20) {
        Some([9, 0]) => 512,
        Some([12, 0]) => 4096,
        _ => return false,
    };
    let sector = |id: u32| {
        let offset = (id as usize).checked_add(1)?.checked_mul(sector_size)?;
        bytes.get(offset..offset.checked_add(sector_size)?)
    };
    let Some(id) = u32_at(MINIFAT_HEADER_OFFSET) else {
        return false;
    };
    if u32_at(MINIFAT_HEADER_OFFSET + 4) != Some(1)
        || !sector(id).is_some_and(|data| data.iter().all(|&byte| byte == 0))
    {
        return false;
    }
    let entries_per_fat_sector = sector_size / 4;
    let fat_index = id as usize / entries_per_fat_sector;
    // The repair only uses the header DIFAT, whose 109 entries are already guarded.
    if fat_index >= 109 {
        return false;
    }
    let Some(fat) = u32_at(0x4C + fat_index * 4).and_then(sector) else {
        return false;
    };
    let at = id as usize % entries_per_fat_sector * 4;
    fat.get(at..at + 4) == Some(END_OF_CHAIN.to_le_bytes().as_slice())
}

/// A read-only view which can replace the MiniFAT location and count in the header.
/// The input and all stream bytes are shared with the regular reader.
pub(super) struct Source<'a> {
    bytes: Cursor<&'a [u8]>,
    ignore_minifat: bool,
}

impl Read for Source<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let start = self.bytes.position();
        let n = self.bytes.read(buffer)?;
        if self.ignore_minifat && n != 0 {
            let start = start as usize;
            let from = start.max(MINIFAT_HEADER_OFFSET);
            let to = (start + n).min(MINIFAT_HEADER_OFFSET + NO_MINIFAT.len());
            if from < to {
                buffer[from - start..to - start].copy_from_slice(
                    &NO_MINIFAT[from - MINIFAT_HEADER_OFFSET..to - MINIFAT_HEADER_OFFSET],
                );
            }
        }
        Ok(n)
    }
}

impl Seek for Source<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}
