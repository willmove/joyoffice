//! File Information Block (FIB) parser for MS-DOC binary format (§2.5).

use crate::budget::DocError;

pub const FIB_MAGIC: u16 = 0xA5EC;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fib {
    pub n_fib: u16,
    pub is_template: bool,
    pub is_encrypted: bool,
    pub which_table_stream: &'static str,
    pub ccp_text: u32,
    pub fc_clx: u32,
    pub lcb_clx: u32,
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16, DocError> {
    let sub = bytes
        .get(offset..offset + 2)
        .ok_or(DocError::MalformedFib)?;
    Ok(u16::from_le_bytes([sub[0], sub[1]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, DocError> {
    let sub = bytes
        .get(offset..offset + 4)
        .ok_or(DocError::MalformedFib)?;
    Ok(u32::from_le_bytes([sub[0], sub[1], sub[2], sub[3]]))
}

impl Fib {
    pub fn parse(bytes: &[u8]) -> Result<Self, DocError> {
        if bytes.len() < 32 {
            return Err(DocError::MalformedFib);
        }

        let w_ident = read_u16_le(bytes, 0)?;
        if w_ident != FIB_MAGIC {
            return Err(DocError::InvalidFibMagic);
        }

        let n_fib = read_u16_le(bytes, 2)?;
        if n_fib < 0x00C1 {
            return Err(DocError::UnsupportedFibVersion(n_fib));
        }

        let flags = read_u16_le(bytes, 10)?;
        let is_template = (flags & 0x0001) != 0;
        let is_encrypted = (flags & 0x0100) != 0;
        let which_table_stream = if (flags & 0x0200) != 0 {
            "1Table"
        } else {
            "0Table"
        };

        if is_encrypted {
            return Err(DocError::EncryptedDocument);
        }

        // FibRgW count and offset
        let mut offset = 32;
        let csw = read_u16_le(bytes, offset)? as usize;
        offset += 2 + csw * 2;

        // FibRgLw count and offset
        let cslw = read_u16_le(bytes, offset)? as usize;
        offset += 2;
        if cslw < 4 || bytes.len() < offset + cslw * 4 {
            return Err(DocError::MalformedFib);
        }

        // Index 3 in FibRgLw is ccpText
        let ccp_text = read_u32_le(bytes, offset + 12)?;
        offset += cslw * 4;

        // FibRgFcLcb count and offset
        let cb_rg_fc_lcb = read_u16_le(bytes, offset)? as usize;
        offset += 2;

        // Index 66 is fcClx, lcbClx
        if cb_rg_fc_lcb <= 66 || bytes.len() < offset + (66 + 1) * 8 {
            return Err(DocError::MalformedFib);
        }

        let clx_entry_offset = offset + 66 * 8;
        let fc_clx = read_u32_le(bytes, clx_entry_offset)?;
        let lcb_clx = read_u32_le(bytes, clx_entry_offset + 4)?;

        Ok(Self {
            n_fib,
            is_template,
            is_encrypted,
            which_table_stream,
            ccp_text,
            fc_clx,
            lcb_clx,
        })
    }
}
