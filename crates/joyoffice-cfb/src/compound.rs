//! High-level CompoundFile reader interface.

use std::collections::HashMap;

use crate::budget::{CfbBudget, CfbError};
use crate::directory::{Directory, EntryType};
use crate::fat::{FatTable, sector_offset, traverse_chain};
use crate::header::{CfbHeader, ENDOFCHAIN, FREESECT};

#[derive(Debug)]
pub struct CompoundFile {
    header: CfbHeader,
    fat_table: FatTable,
    directory: Directory,
    data: Vec<u8>,
    paths: HashMap<String, u32>,
    budget: CfbBudget,
}

impl CompoundFile {
    /// Parse a Compound File Binary from untrusted bytes under budget limits.
    pub fn parse(bytes: &[u8], budget: CfbBudget) -> Result<Self, CfbError> {
        let header = CfbHeader::parse(bytes, &budget)?;
        let fat_table = FatTable::parse(bytes, &header, &budget)?;
        let directory = Directory::parse(bytes, &header, &fat_table, &budget)?;
        let stream_paths = directory.build_paths(&budget)?;

        let mut paths = HashMap::new();
        for (path, id) in stream_paths {
            paths.insert(path, id);
        }

        Ok(Self {
            header,
            fat_table,
            directory,
            data: bytes.to_vec(),
            paths,
            budget,
        })
    }

    /// List all stream names and their lengths.
    pub fn list_streams(&self) -> Vec<(String, usize)> {
        let mut list: Vec<_> = self
            .paths
            .iter()
            .map(|(path, &id)| {
                let size = self.directory.entries[id as usize].stream_size as usize;
                (path.clone(), size)
            })
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }

    /// Read stream bytes by exact path (e.g. "WordDocument" or "Data").
    pub fn read_stream(&self, path: &str) -> Result<Vec<u8>, CfbError> {
        let id = self
            .paths
            .get(path)
            .copied()
            .ok_or_else(|| CfbError::StreamNotFound(path.to_string()))?;
        self.read_stream_by_id(id)
    }

    /// Read stream bytes by directory entry ID.
    pub fn read_stream_by_id(&self, id: u32) -> Result<Vec<u8>, CfbError> {
        if id as usize >= self.directory.entries.len() {
            return Err(CfbError::MalformedDirectory("entry ID out of range"));
        }
        let entry = &self.directory.entries[id as usize];
        if entry.entry_type != EntryType::Stream && entry.entry_type != EntryType::Root {
            return Err(CfbError::MalformedDirectory("entry is not a stream"));
        }

        let size = entry.stream_size as usize;
        if size > self.budget.max_stream_size {
            return Err(CfbError::BudgetExceeded("stream size exceeds budget"));
        }
        if size == 0 {
            return Ok(Vec::new());
        }

        if entry.start_sector == ENDOFCHAIN || entry.start_sector == FREESECT {
            return Ok(Vec::new());
        }

        // Small stream (< cutoff) lives in Mini Stream
        if (size as u32) < self.header.mini_stream_cutoff_size
            && entry.entry_type == EntryType::Stream
        {
            self.read_mini_stream(entry.start_sector, size)
        } else {
            self.read_fat_stream(entry.start_sector, size)
        }
    }

    /// Read all streams into a HashMap of name -> bytes (useful for extraction/preservation).
    pub fn extract_all_streams(&self) -> Result<HashMap<String, Vec<u8>>, CfbError> {
        let mut map = HashMap::new();
        for (path, &id) in &self.paths {
            let bytes = self.read_stream_by_id(id)?;
            map.insert(path.clone(), bytes);
        }
        Ok(map)
    }

    fn read_fat_stream(&self, start_sec: u32, size: usize) -> Result<Vec<u8>, CfbError> {
        let chain = traverse_chain(&self.fat_table.fat, start_sec, self.fat_table.total_sectors)?;
        let mut out = Vec::with_capacity(size);
        let sec_size = self.header.sector_size;

        for &sec in &chain {
            let offset = sector_offset(sec, sec_size)?;
            if offset + sec_size > self.data.len() {
                return Err(CfbError::UnexpectedEof);
            }
            let chunk = &self.data[offset..offset + sec_size];
            let needed = size.saturating_sub(out.len());
            let take = needed.min(sec_size);
            out.extend_from_slice(&chunk[..take]);
            if out.len() >= size {
                break;
            }
        }

        Ok(out)
    }

    fn read_mini_stream(&self, start_mini_sec: u32, size: usize) -> Result<Vec<u8>, CfbError> {
        let root = &self.directory.entries[0];
        let mini_stream_data =
            self.read_fat_stream(root.start_sector, root.stream_size as usize)?;

        let chain = traverse_chain(
            &self.fat_table.mini_fat,
            start_mini_sec,
            self.fat_table.mini_fat.len(),
        )?;

        let mini_sec_size = self.header.mini_sector_size;
        let mut out = Vec::with_capacity(size);

        for &mini_sec in &chain {
            let offset = (mini_sec as usize) * mini_sec_size;
            if offset + mini_sec_size > mini_stream_data.len() {
                return Err(CfbError::UnexpectedEof);
            }
            let chunk = &mini_stream_data[offset..offset + mini_sec_size];
            let needed = size.saturating_sub(out.len());
            let take = needed.min(mini_sec_size);
            out.extend_from_slice(&chunk[..take]);
            if out.len() >= size {
                break;
            }
        }

        Ok(out)
    }
}
