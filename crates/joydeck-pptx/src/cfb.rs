//! CFB directory probe for Office encrypted packages (MS-CFB and MS-OFFCRYPTO).
//! The Deck open path uses the result to select its password-protection diagnostic.

use joyoffice_cfb::{CFB_SIGNATURE, complete_sector, directory_name_units};

const DIRECTORY_LIMIT: usize = 1024;
const DIFAT_LIMIT: usize = 1024;
const END_OF_CHAIN: u32 = 0xffff_fffe;
const FIRST_RESERVED_SECTOR: u32 = 0xffff_fffa;
const STREAMS: [&str; 2] = ["EncryptionInfo", "EncryptedPackage"];

pub(crate) fn is_encrypted(bytes: &[u8]) -> bool {
    bytes.starts_with(&CFB_SIGNATURE)
        && directory_mentions_encryption(bytes).unwrap_or_else(|| {
            // Preserve the diagnostic for truncated containers carrying a literal stream marker.
            STREAMS.iter().any(|name| {
                let marker: Vec<_> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
                bytes.windows(marker.len()).any(|chunk| chunk == marker)
            })
        })
}

fn le_words(bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(u32::from_le_bytes)
}

/// Walk metadata only. Stream data and the mini FAT are irrelevant to detection.
fn directory_mentions_encryption(bytes: &[u8]) -> Option<bool> {
    let shift = u16::from_le_bytes(bytes.get(30..32)?.try_into().ok()?);
    let sector_size = match shift {
        9 => 512,
        12 => 4096,
        _ => return None,
    };
    let sector = |id| complete_sector(bytes, sector_size, id);
    let entries_per_fat = sector_size / 4;
    let is_regular = |id: &u32| *id < FIRST_RESERVED_SECTOR;
    let mut fat_sectors: Vec<_> = le_words(bytes.get(76..).unwrap_or_default())
        .take(109)
        .filter(is_regular)
        .collect();

    let mut index_sector = le_words(bytes.get(68..)?).next()?;
    for _ in 0..DIFAT_LIMIT {
        if !is_regular(&index_sector) {
            break;
        }
        let table = sector(index_sector)?;
        let (entries, next) = table.split_at(table.len() - 4);
        fat_sectors.extend(le_words(entries).filter(is_regular));
        index_sector = le_words(next).next()?;
    }

    let mut directory = le_words(bytes.get(48..)?).next()?;
    let mut match_found = false;
    for _ in 0..DIRECTORY_LIMIT {
        if directory == END_OF_CHAIN {
            break;
        }
        if !is_regular(&directory) {
            return None;
        }
        let contents = sector(directory)?;
        match_found |= contents.as_chunks::<128>().0.iter().any(|entry| {
            if !(2..=64).contains(&u16::from_le_bytes([entry[64], entry[65]])) {
                return false;
            }
            STREAMS.iter().any(|name| {
                let units = directory_name_units(entry);
                units.len() == name.len()
                    && units.zip(name.bytes()).all(|(unit, ascii)| {
                        u8::try_from(unit).is_ok_and(|b| b.eq_ignore_ascii_case(&ascii))
                    })
            })
        });
        let position = directory as usize;
        let fat = sector(*fat_sectors.get(position / entries_per_fat)?)?;
        let next_offset = (position % entries_per_fat) * 4;
        directory = le_words(fat.get(next_offset..)?).next()?;
    }
    Some(match_found)
}
