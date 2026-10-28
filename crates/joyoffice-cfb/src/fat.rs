//! FAT, DIFAT, and MiniFAT parsing and sector chain traversal with loop detection.

use crate::budget::{CfbBudget, CfbError};
use crate::header::{CfbHeader, DIFSECT, ENDOFCHAIN, FATSECT, FREESECT};

#[derive(Debug)]
pub struct FatTable {
    pub fat: Vec<u32>,
    pub mini_fat: Vec<u32>,
    pub total_sectors: usize,
}

impl FatTable {
    pub fn parse(bytes: &[u8], header: &CfbHeader, budget: &CfbBudget) -> Result<Self, CfbError> {
        let sector_size = header.sector_size;
        let data_bytes = bytes.len().saturating_sub(sector_size);
        let total_sectors = data_bytes / sector_size;

        if total_sectors > budget.max_sectors {
            return Err(CfbError::BudgetExceeded("total sectors exceeds budget"));
        }

        // 1. Gather all FAT sector IDs from DIFAT
        let mut fat_sector_ids = header.initial_difat.clone();

        if header.num_difat_sectors > 0 && header.first_difat_sector != ENDOFCHAIN {
            let mut curr_difat = header.first_difat_sector;
            let entries_per_sector = (sector_size / 4).saturating_sub(1);
            let mut difat_count = 0usize;

            while curr_difat != ENDOFCHAIN && curr_difat != FREESECT {
                if difat_count >= header.num_difat_sectors as usize || difat_count > total_sectors {
                    return Err(CfbError::InvalidSectorChain(curr_difat));
                }
                difat_count += 1;
                let offset = sector_offset(curr_difat, sector_size)?;
                if offset + sector_size > bytes.len() {
                    return Err(CfbError::UnexpectedEof);
                }
                let sec_bytes = &bytes[offset..offset + sector_size];

                for i in 0..entries_per_sector {
                    let off = i * 4;
                    let fat_sec = u32::from_le_bytes([
                        sec_bytes[off],
                        sec_bytes[off + 1],
                        sec_bytes[off + 2],
                        sec_bytes[off + 3],
                    ]);
                    if fat_sec != FREESECT && fat_sec != ENDOFCHAIN {
                        fat_sector_ids.push(fat_sec);
                    }
                }

                let next_off = entries_per_sector * 4;
                curr_difat = u32::from_le_bytes([
                    sec_bytes[next_off],
                    sec_bytes[next_off + 1],
                    sec_bytes[next_off + 2],
                    sec_bytes[next_off + 3],
                ]);
            }
        }

        // 2. Read all FAT sectors and assemble FAT
        let entries_per_fat_sector = sector_size / 4;
        let mut fat = Vec::with_capacity(fat_sector_ids.len() * entries_per_fat_sector);

        for &fat_sec in &fat_sector_ids {
            if fat_sec as usize >= total_sectors {
                return Err(CfbError::InvalidSectorChain(fat_sec));
            }
            let offset = sector_offset(fat_sec, sector_size)?;
            if offset + sector_size > bytes.len() {
                return Err(CfbError::UnexpectedEof);
            }
            let sec_bytes = &bytes[offset..offset + sector_size];
            let (chunks, _) = sec_bytes.as_chunks::<4>();
            for chunk in chunks {
                fat.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
        }

        // 3. Assemble MiniFAT
        let mut mini_fat = Vec::new();
        if header.first_mini_fat_sector != ENDOFCHAIN && header.first_mini_fat_sector != FREESECT {
            let mini_fat_chain = traverse_chain(&fat, header.first_mini_fat_sector, total_sectors)?;
            for &sec in &mini_fat_chain {
                let offset = sector_offset(sec, sector_size)?;
                if offset + sector_size > bytes.len() {
                    return Err(CfbError::UnexpectedEof);
                }
                let sec_bytes = &bytes[offset..offset + sector_size];
                let (chunks, _) = sec_bytes.as_chunks::<4>();
                for chunk in chunks {
                    mini_fat.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
                }
            }
        }

        Ok(Self {
            fat,
            mini_fat,
            total_sectors,
        })
    }
}

pub fn sector_offset(sector_id: u32, sector_size: usize) -> Result<usize, CfbError> {
    let offset = (sector_id as usize)
        .checked_add(1)
        .and_then(|idx| idx.checked_mul(sector_size))
        .ok_or(CfbError::InvalidSectorChain(sector_id))?;
    Ok(offset)
}

pub fn traverse_chain(
    fat: &[u32],
    start_sector: u32,
    max_sectors: usize,
) -> Result<Vec<u32>, CfbError> {
    let mut chain = Vec::new();
    let mut curr = start_sector;
    let mut visited_count = 0usize;

    while curr != ENDOFCHAIN && curr != FREESECT {
        if visited_count > max_sectors || curr as usize >= fat.len() {
            return Err(CfbError::InvalidSectorChain(curr));
        }
        if curr == FATSECT || curr == DIFSECT {
            return Err(CfbError::InvalidSectorChain(curr));
        }
        chain.push(curr);
        visited_count += 1;
        curr = fat[curr as usize];
    }

    Ok(chain)
}
