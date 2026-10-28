//! Deterministic MS-CFB v3 writer for tests, fixtures, and encrypted package output.

use std::collections::BTreeMap;

use crate::budget::CfbError;
use crate::directory::{DirEntry, ENTRY_SIZE, EntryType, NOSTREAM};
use crate::header::{
    CFB_SIGNATURE, ENDOFCHAIN, FATSECT, FREESECT, HEADER_SIZE, MINI_STREAM_CUTOFF_DEFAULT,
};

pub struct CfbWriter {
    streams: BTreeMap<String, Vec<u8>>,
}

impl Default for CfbWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl CfbWriter {
    pub fn new() -> Self {
        Self {
            streams: BTreeMap::new(),
        }
    }

    pub fn add_stream(&mut self, name: &str, data: Vec<u8>) {
        self.streams.insert(name.to_string(), data);
    }

    /// Build a valid v3 Compound File Binary byte array.
    pub fn build(self) -> Result<Vec<u8>, CfbError> {
        let sector_size = 512usize;
        let mini_sector_size = 64usize;
        let cutoff = MINI_STREAM_CUTOFF_DEFAULT as usize;

        // 1. Separate streams into mini (< 4096) and regular (>= 4096)
        let mut mini_stream_data = Vec::new();
        let mut mini_fat = Vec::new();

        struct StreamAlloc {
            name: String,
            size: usize,
            is_mini: bool,
            start_sector: u32,
            data: Vec<u8>,
        }

        let mut allocs = Vec::new();

        for (name, data) in self.streams {
            let size = data.len();
            if size < cutoff && size > 0 {
                let start_mini_sec = (mini_stream_data.len() / mini_sector_size) as u32;
                let needed_mini_secs = size.div_ceil(mini_sector_size);
                for i in 0..needed_mini_secs {
                    let next = if i + 1 == needed_mini_secs {
                        ENDOFCHAIN
                    } else {
                        start_mini_sec + (i as u32) + 1
                    };
                    mini_fat.push(next);
                }
                mini_stream_data.extend_from_slice(&data);
                let rem = mini_stream_data.len() % mini_sector_size;
                if rem != 0 {
                    mini_stream_data.resize(mini_stream_data.len() + (mini_sector_size - rem), 0);
                }
                allocs.push(StreamAlloc {
                    name,
                    size,
                    is_mini: true,
                    start_sector: start_mini_sec,
                    data: Vec::new(),
                });
            } else {
                allocs.push(StreamAlloc {
                    name,
                    size,
                    is_mini: false,
                    start_sector: 0, // will assign regular sector
                    data,
                });
            }
        }

        // 2. Prepare Directory entries
        let entries_per_sector = sector_size / ENTRY_SIZE; // 4 entries per 512B
        let total_dir_entries = 1 + allocs.len(); // Entry 0 is Root
        let num_dir_sectors = total_dir_entries.div_ceil(entries_per_sector);

        // 3. Regular sectors planning
        let mut regular_sectors_data: Vec<Vec<u8>> = Vec::new();
        let mut fat: Vec<u32> = Vec::new();

        let mut alloc_regular_chain = |data: &[u8]| -> (u32, usize) {
            if data.is_empty() {
                return (ENDOFCHAIN, 0);
            }
            let start = regular_sectors_data.len() as u32;
            let needed = data.len().div_ceil(sector_size);
            for i in 0..needed {
                let off = i * sector_size;
                let end = (off + sector_size).min(data.len());
                let mut sec = vec![0u8; sector_size];
                sec[..end - off].copy_from_slice(&data[off..end]);
                regular_sectors_data.push(sec);

                let next = if i + 1 == needed {
                    ENDOFCHAIN
                } else {
                    start + (i as u32) + 1
                };
                fat.push(next);
            }
            (start, needed)
        };

        // Allocate for large streams
        for item in &mut allocs {
            if !item.is_mini {
                let (start, _) = alloc_regular_chain(&item.data);
                item.start_sector = start;
            }
        }

        // Allocate for MiniStream container
        let (root_mini_start, _) = alloc_regular_chain(&mini_stream_data);

        // Allocate for MiniFAT sectors
        let mut mini_fat_bytes = Vec::new();
        for &entry in &mini_fat {
            mini_fat_bytes.extend_from_slice(&entry.to_le_bytes());
        }
        let (first_mini_fat_sector, num_mini_fat_sectors) = alloc_regular_chain(&mini_fat_bytes);

        // Build directory bytes
        let mut dir_entries = Vec::new();

        // Root entry
        let root_child_id = if allocs.is_empty() { NOSTREAM } else { 1 };

        dir_entries.push(DirEntry {
            id: 0,
            name: "Root Entry".to_string(),
            entry_type: EntryType::Root,
            left_sibling: NOSTREAM,
            right_sibling: NOSTREAM,
            child_id: root_child_id,
            clsid: [0; 16],
            start_sector: root_mini_start,
            stream_size: mini_stream_data.len() as u64,
        });

        // Child stream entries arranged in simple right-sibling chain
        for (i, item) in allocs.iter().enumerate() {
            let id = (i + 1) as u32;
            let right = if i + 1 < allocs.len() {
                id + 1
            } else {
                NOSTREAM
            };
            dir_entries.push(DirEntry {
                id,
                name: item.name.clone(),
                entry_type: EntryType::Stream,
                left_sibling: NOSTREAM,
                right_sibling: right,
                child_id: NOSTREAM,
                clsid: [0; 16],
                start_sector: item.start_sector,
                stream_size: item.size as u64,
            });
        }

        // Pad directory to full sectors
        while dir_entries.len() < num_dir_sectors * entries_per_sector {
            let id = dir_entries.len() as u32;
            dir_entries.push(DirEntry {
                id,
                name: String::new(),
                entry_type: EntryType::Unallocated,
                left_sibling: NOSTREAM,
                right_sibling: NOSTREAM,
                child_id: NOSTREAM,
                clsid: [0; 16],
                start_sector: FREESECT,
                stream_size: 0,
            });
        }

        let mut dir_bytes = Vec::with_capacity(num_dir_sectors * sector_size);
        for entry in &dir_entries {
            let mut buf = [0u8; ENTRY_SIZE];
            if !entry.name.is_empty() {
                let utf16: Vec<u16> = entry.name.encode_utf16().collect();
                let name_len = (utf16.len() + 1) * 2;
                buf[0x40..0x42].copy_from_slice(&(name_len as u16).to_le_bytes());
                for (j, &c) in utf16.iter().enumerate() {
                    let off = j * 2;
                    buf[off..off + 2].copy_from_slice(&c.to_le_bytes());
                }
            }
            buf[0x42] = entry.entry_type as u8;
            buf[0x43] = 1; // Black
            buf[0x44..0x48].copy_from_slice(&entry.left_sibling.to_le_bytes());
            buf[0x48..0x4C].copy_from_slice(&entry.right_sibling.to_le_bytes());
            buf[0x4C..0x50].copy_from_slice(&entry.child_id.to_le_bytes());
            buf[0x50..0x60].copy_from_slice(&entry.clsid);
            buf[0x74..0x78].copy_from_slice(&entry.start_sector.to_le_bytes());
            buf[0x78..0x80].copy_from_slice(&entry.stream_size.to_le_bytes());
            dir_bytes.extend_from_slice(&buf);
        }

        // Allocate for Directory sectors
        let (first_dir_sector, _) = alloc_regular_chain(&dir_bytes);

        // Now calculate FAT sectors needed
        // Each FAT sector can hold 128 entries (512 / 4).
        // Let N be the number of regular sectors currently in `regular_sectors_data`.
        // If we add F FAT sectors, total sectors = N + F.
        // We need F * 128 >= N + F.
        let mut fat_sectors_count = 1usize;
        while fat_sectors_count * 128 < regular_sectors_data.len() + fat_sectors_count {
            fat_sectors_count += 1;
        }

        let first_fat_sector = regular_sectors_data.len() as u32;
        fat.extend(std::iter::repeat_n(FATSECT, fat_sectors_count));

        // Pad FAT array to exact multiple of 128
        let total_fat_capacity = fat_sectors_count * 128;
        while fat.len() < total_fat_capacity {
            fat.push(FREESECT);
        }

        // Build FAT sector bytes
        for i in 0..fat_sectors_count {
            let mut sec = Vec::with_capacity(sector_size);
            for j in 0..128 {
                let idx = i * 128 + j;
                sec.extend_from_slice(&fat[idx].to_le_bytes());
            }
            regular_sectors_data.push(sec);
        }

        // 4. Assemble final file bytes
        let mut file_bytes =
            Vec::with_capacity(HEADER_SIZE + regular_sectors_data.len() * sector_size);

        // Header (512 bytes)
        let mut header = [0u8; HEADER_SIZE];
        header[0..8].copy_from_slice(&CFB_SIGNATURE);
        header[0x18..0x1A].copy_from_slice(&0x003Eu16.to_le_bytes()); // Minor
        header[0x1A..0x1C].copy_from_slice(&3u16.to_le_bytes()); // Major v3
        header[0x1C..0x1E].copy_from_slice(&0xFFFEu16.to_le_bytes()); // Little Endian
        header[0x1E..0x20].copy_from_slice(&9u16.to_le_bytes()); // Sector shift 9 = 512
        header[0x20..0x22].copy_from_slice(&6u16.to_le_bytes()); // Mini sector shift 6 = 64
        header[0x2C..0x30].copy_from_slice(&(fat_sectors_count as u32).to_le_bytes());
        header[0x30..0x34].copy_from_slice(&first_dir_sector.to_le_bytes());
        header[0x38..0x3C].copy_from_slice(&(cutoff as u32).to_le_bytes());
        header[0x3C..0x40].copy_from_slice(&first_mini_fat_sector.to_le_bytes());
        header[0x40..0x44].copy_from_slice(&(num_mini_fat_sectors as u32).to_le_bytes());
        header[0x44..0x48].copy_from_slice(&ENDOFCHAIN.to_le_bytes()); // No DIFAT sectors needed for <= 109
        header[0x48..0x4C].copy_from_slice(&0u32.to_le_bytes());

        // Fill initial DIFAT in header
        for i in 0..109 {
            let val = if i < fat_sectors_count {
                first_fat_sector + (i as u32)
            } else {
                FREESECT
            };
            let off = 0x4C + i * 4;
            header[off..off + 4].copy_from_slice(&val.to_le_bytes());
        }

        file_bytes.extend_from_slice(&header);

        for sec in regular_sectors_data {
            file_bytes.extend_from_slice(&sec);
        }

        Ok(file_bytes)
    }
}
