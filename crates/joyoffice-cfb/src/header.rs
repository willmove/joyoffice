//! MS-CFB 512-byte Header parsing and validation.

use crate::budget::{CfbBudget, CfbError};

pub const CFB_SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
pub const HEADER_SIZE: usize = 512;
pub const MINI_STREAM_CUTOFF_DEFAULT: u32 = 4096;

pub const ENDOFCHAIN: u32 = 0xFFFFFFFE;
pub const FREESECT: u32 = 0xFFFFFFFF;
pub const FATSECT: u32 = 0xFFFFFFFD;
pub const DIFSECT: u32 = 0xFFFFFFFC;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfbHeader {
    pub major_version: u16,
    pub minor_version: u16,
    pub sector_shift: u16,
    pub sector_size: usize,
    pub mini_sector_shift: u16,
    pub mini_sector_size: usize,
    pub num_dir_sectors: u32,
    pub num_fat_sectors: u32,
    pub first_dir_sector: u32,
    pub mini_stream_cutoff_size: u32,
    pub first_mini_fat_sector: u32,
    pub num_mini_fat_sectors: u32,
    pub first_difat_sector: u32,
    pub num_difat_sectors: u32,
    pub initial_difat: Vec<u32>,
}

impl CfbHeader {
    pub fn parse(bytes: &[u8], budget: &CfbBudget) -> Result<Self, CfbError> {
        if bytes.len() < HEADER_SIZE {
            return Err(CfbError::TruncatedHeader);
        }
        if bytes.len() > budget.max_file_size {
            return Err(CfbError::BudgetExceeded("file size exceeds budget"));
        }
        if bytes[0..8] != CFB_SIGNATURE {
            return Err(CfbError::InvalidSignature);
        }

        let minor_version = u16::from_le_bytes([bytes[0x18], bytes[0x19]]);
        let major_version = u16::from_le_bytes([bytes[0x1A], bytes[0x1B]]);
        let byte_order = u16::from_le_bytes([bytes[0x1C], bytes[0x1D]]);
        if byte_order != 0xFFFE {
            return Err(CfbError::UnsupportedByteOrder(byte_order));
        }

        let sector_shift = u16::from_le_bytes([bytes[0x1E], bytes[0x1F]]);
        if sector_shift != 9 && sector_shift != 12 {
            return Err(CfbError::UnsupportedSectorShift(sector_shift));
        }
        let sector_size = 1usize << sector_shift;

        let mini_sector_shift = u16::from_le_bytes([bytes[0x20], bytes[0x21]]);
        if mini_sector_shift != 6 {
            return Err(CfbError::UnsupportedMiniSectorShift(mini_sector_shift));
        }
        let mini_sector_size = 1usize << mini_sector_shift;

        let num_dir_sectors =
            u32::from_le_bytes([bytes[0x28], bytes[0x29], bytes[0x2A], bytes[0x2B]]);
        let num_fat_sectors =
            u32::from_le_bytes([bytes[0x2C], bytes[0x2D], bytes[0x2E], bytes[0x2F]]);
        let first_dir_sector =
            u32::from_le_bytes([bytes[0x30], bytes[0x31], bytes[0x32], bytes[0x33]]);
        let mini_stream_cutoff_size =
            u32::from_le_bytes([bytes[0x38], bytes[0x39], bytes[0x3A], bytes[0x3B]]);
        let first_mini_fat_sector =
            u32::from_le_bytes([bytes[0x3C], bytes[0x3D], bytes[0x3E], bytes[0x3F]]);
        let num_mini_fat_sectors =
            u32::from_le_bytes([bytes[0x40], bytes[0x41], bytes[0x42], bytes[0x43]]);
        let first_difat_sector =
            u32::from_le_bytes([bytes[0x44], bytes[0x45], bytes[0x46], bytes[0x47]]);
        let num_difat_sectors =
            u32::from_le_bytes([bytes[0x48], bytes[0x49], bytes[0x4A], bytes[0x4B]]);

        let mut initial_difat = Vec::with_capacity(109);
        for i in 0..109 {
            let offset = 0x4C + i * 4;
            let sec = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            if sec != FREESECT {
                initial_difat.push(sec);
            }
        }

        Ok(Self {
            major_version,
            minor_version,
            sector_shift,
            sector_size,
            mini_sector_shift,
            mini_sector_size,
            num_dir_sectors,
            num_fat_sectors,
            first_dir_sector,
            mini_stream_cutoff_size,
            first_mini_fat_sector,
            num_mini_fat_sectors,
            first_difat_sector,
            num_difat_sectors,
            initial_difat,
        })
    }
}
