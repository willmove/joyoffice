//! MS-CFB Directory Entry parsing (128 bytes per entry) and directory hierarchy.

use crate::budget::{CfbBudget, CfbError};
use crate::fat::{FatTable, sector_offset, traverse_chain};
use crate::header::CfbHeader;

pub const NOSTREAM: u32 = 0xFFFFFFFF;
pub const ENTRY_SIZE: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    Unallocated = 0,
    Storage = 1,
    Stream = 2,
    Root = 5,
}

impl EntryType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Unallocated),
            1 => Some(Self::Storage),
            2 => Some(Self::Stream),
            5 => Some(Self::Root),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub id: u32,
    pub name: String,
    pub entry_type: EntryType,
    pub left_sibling: u32,
    pub right_sibling: u32,
    pub child_id: u32,
    pub clsid: [u8; 16],
    pub start_sector: u32,
    pub stream_size: u64,
}

impl DirEntry {
    pub fn parse(id: u32, bytes: &[u8]) -> Result<Self, CfbError> {
        if bytes.len() < ENTRY_SIZE {
            return Err(CfbError::UnexpectedEof);
        }

        let name_bytes_len = u16::from_le_bytes([bytes[0x40], bytes[0x41]]) as usize;
        let name = if name_bytes_len > 2 && name_bytes_len <= 64 {
            let (chunks, _) = bytes[0..name_bytes_len - 2].as_chunks::<2>();
            let u16_chars: Vec<u16> = chunks
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&u16_chars)
                .map_err(|_| CfbError::MalformedDirectory("invalid UTF-16 in entry name"))?
        } else {
            String::new()
        };

        let raw_type = bytes[0x42];
        let entry_type = EntryType::from_u8(raw_type)
            .ok_or(CfbError::MalformedDirectory("unknown entry type"))?;

        let left_sibling = u32::from_le_bytes([bytes[0x44], bytes[0x45], bytes[0x46], bytes[0x47]]);
        let right_sibling =
            u32::from_le_bytes([bytes[0x48], bytes[0x49], bytes[0x4A], bytes[0x4B]]);
        let child_id = u32::from_le_bytes([bytes[0x4C], bytes[0x4D], bytes[0x4E], bytes[0x4F]]);

        let mut clsid = [0u8; 16];
        clsid.copy_from_slice(&bytes[0x50..0x60]);

        let start_sector = u32::from_le_bytes([bytes[0x74], bytes[0x75], bytes[0x76], bytes[0x77]]);
        let stream_size = u64::from_le_bytes([
            bytes[0x78],
            bytes[0x79],
            bytes[0x7A],
            bytes[0x7B],
            bytes[0x7C],
            bytes[0x7D],
            bytes[0x7E],
            bytes[0x7F],
        ]);

        Ok(Self {
            id,
            name,
            entry_type,
            left_sibling,
            right_sibling,
            child_id,
            clsid,
            start_sector,
            stream_size,
        })
    }
}

#[derive(Debug)]
pub struct Directory {
    pub entries: Vec<DirEntry>,
}

impl Directory {
    pub fn parse(
        bytes: &[u8],
        header: &CfbHeader,
        fat_table: &FatTable,
        budget: &CfbBudget,
    ) -> Result<Self, CfbError> {
        let dir_chain = traverse_chain(
            &fat_table.fat,
            header.first_dir_sector,
            fat_table.total_sectors,
        )?;
        let entries_per_sector = header.sector_size / ENTRY_SIZE;
        let mut entries = Vec::new();

        for &sec in &dir_chain {
            let offset = sector_offset(sec, header.sector_size)?;
            if offset + header.sector_size > bytes.len() {
                return Err(CfbError::UnexpectedEof);
            }
            let sec_bytes = &bytes[offset..offset + header.sector_size];
            for i in 0..entries_per_sector {
                if entries.len() >= budget.max_entries {
                    return Err(CfbError::BudgetExceeded("too many directory entries"));
                }
                let entry_bytes = &sec_bytes[i * ENTRY_SIZE..(i + 1) * ENTRY_SIZE];
                let entry = DirEntry::parse(entries.len() as u32, entry_bytes)?;
                entries.push(entry);
            }
        }

        if entries.is_empty() {
            return Err(CfbError::MalformedDirectory("empty directory"));
        }
        if entries[0].entry_type != EntryType::Root {
            return Err(CfbError::MalformedDirectory("entry 0 is not Root Storage"));
        }

        Ok(Self { entries })
    }

    /// Traverse directory tree starting at root, returning full path -> entry mapping.
    pub fn build_paths(&self, budget: &CfbBudget) -> Result<Vec<(String, u32)>, CfbError> {
        let mut results = Vec::new();
        if self.entries.is_empty() {
            return Ok(results);
        }

        let mut visited = vec![false; self.entries.len()];

        // Entry 0 is Root
        if self.entries[0].child_id != NOSTREAM {
            self.collect_subtree(
                self.entries[0].child_id,
                "",
                &mut visited,
                &mut results,
                0,
                budget,
            )?;
        }

        Ok(results)
    }

    fn collect_subtree(
        &self,
        node_id: u32,
        prefix: &str,
        visited: &mut [bool],
        results: &mut Vec<(String, u32)>,
        depth: usize,
        budget: &CfbBudget,
    ) -> Result<(), CfbError> {
        if depth > budget.max_tree_depth {
            return Err(CfbError::BudgetExceeded("directory tree depth exceeded"));
        }
        if node_id == NOSTREAM || node_id as usize >= self.entries.len() {
            return Ok(());
        }
        if visited[node_id as usize] {
            return Err(CfbError::MalformedDirectory(
                "cycle detected in directory tree",
            ));
        }
        visited[node_id as usize] = true;

        let entry = &self.entries[node_id as usize];

        // Traverse left sibling
        if entry.left_sibling != NOSTREAM {
            self.collect_subtree(entry.left_sibling, prefix, visited, results, depth, budget)?;
        }

        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };

        if entry.entry_type == EntryType::Stream {
            results.push((path.clone(), entry.id));
        }

        // Traverse children if storage
        if (entry.entry_type == EntryType::Storage || entry.entry_type == EntryType::Root)
            && entry.child_id != NOSTREAM
        {
            self.collect_subtree(entry.child_id, &path, visited, results, depth + 1, budget)?;
        }

        // Traverse right sibling
        if entry.right_sibling != NOSTREAM {
            self.collect_subtree(entry.right_sibling, prefix, visited, results, depth, budget)?;
        }

        Ok(())
    }
}
