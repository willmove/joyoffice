//! Clx (Piece Table) parser for MS-DOC binary format (§2.9.43, §2.9.187).

use crate::budget::{DocBudget, DocError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceDescriptor {
    pub cp_start: u32,
    pub cp_end: u32,
    pub is_compressed: bool,
    pub file_offset: usize,
}

pub struct Clx {
    pub pieces: Vec<PieceDescriptor>,
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16, DocError> {
    let sub = bytes
        .get(offset..offset + 2)
        .ok_or(DocError::MalformedClx)?;
    Ok(u16::from_le_bytes([sub[0], sub[1]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, DocError> {
    let sub = bytes
        .get(offset..offset + 4)
        .ok_or(DocError::MalformedClx)?;
    Ok(u32::from_le_bytes([sub[0], sub[1], sub[2], sub[3]]))
}

impl Clx {
    pub fn parse(
        table_stream: &[u8],
        fc_clx: u32,
        lcb_clx: u32,
        budget: &DocBudget,
    ) -> Result<Self, DocError> {
        let start = fc_clx as usize;
        let len = lcb_clx as usize;
        if start + len > table_stream.len() {
            return Err(DocError::MalformedClx);
        }

        let clx_bytes = &table_stream[start..start + len];
        let mut pos = 0;

        // Skip Prc structures (clxt = 0x01) until Pcdt (clxt = 0x02)
        while pos < clx_bytes.len() {
            let clxt = clx_bytes[pos];
            pos += 1;
            match clxt {
                0x01 => {
                    // Prc: cbGrpprl (u16) followed by grpprl
                    let cb_grpprl = read_u16_le(clx_bytes, pos)? as usize;
                    pos += 2 + cb_grpprl;
                }
                0x02 => {
                    // Pcdt found: lcb (u32) followed by PlcPcd
                    let lcb_pcd = read_u32_le(clx_bytes, pos)? as usize;
                    pos += 4;
                    if pos + lcb_pcd > clx_bytes.len() {
                        return Err(DocError::MalformedClx);
                    }
                    let plc_bytes = &clx_bytes[pos..pos + lcb_pcd];
                    return Self::parse_plc_pcd(plc_bytes, budget);
                }
                _ => return Err(DocError::MalformedClx),
            }
        }

        Err(DocError::MalformedClx)
    }

    fn parse_plc_pcd(bytes: &[u8], budget: &DocBudget) -> Result<Self, DocError> {
        // PlcPcd: (n + 1) * 4 bytes of CPs, then n * 8 bytes of PCDs
        // Total bytes = 4 + 12 * n
        if bytes.len() < 4 || !(bytes.len() - 4).is_multiple_of(12) {
            return Err(DocError::MalformedPieceTable);
        }

        let n = (bytes.len() - 4) / 12;
        if n > budget.max_pieces {
            return Err(DocError::BudgetExceeded("too many piece descriptors"));
        }

        let cp_array_len = (n + 1) * 4;
        let mut cps = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let offset = i * 4;
            let cp = read_u32_le(bytes, offset).map_err(|_| DocError::MalformedPieceTable)?;
            cps.push(cp);
        }

        let mut pieces = Vec::with_capacity(n);
        let pcd_start = cp_array_len;
        for i in 0..n {
            let pcd_offset = pcd_start + i * 8;
            // PCD: 2 bytes flags, 4 bytes fc, 2 bytes prm
            let raw_fc =
                read_u32_le(bytes, pcd_offset + 2).map_err(|_| DocError::MalformedPieceTable)?;
            let is_compressed = (raw_fc & 0x4000_0000) != 0;
            let file_offset = if is_compressed {
                ((raw_fc & 0x3FFF_FFFF) / 2) as usize
            } else {
                raw_fc as usize
            };

            pieces.push(PieceDescriptor {
                cp_start: cps[i],
                cp_end: cps[i + 1],
                is_compressed,
                file_offset,
            });
        }

        Ok(Self { pieces })
    }

    /// Extract text string for the main document from WordDocument stream bytes.
    pub fn extract_text(
        &self,
        word_doc_bytes: &[u8],
        ccp_text: u32,
        budget: &DocBudget,
    ) -> Result<String, DocError> {
        let mut full_text = String::new();

        for piece in &self.pieces {
            if piece.cp_start >= ccp_text {
                break;
            }
            let cp_limit = piece.cp_end.min(ccp_text);
            let char_count = (cp_limit - piece.cp_start) as usize;

            if full_text.len() + char_count > budget.max_text_chars {
                return Err(DocError::BudgetExceeded("document text exceeds budget"));
            }

            if piece.is_compressed {
                let start = piece.file_offset;
                let end = start + char_count;
                if end > word_doc_bytes.len() {
                    return Err(DocError::MalformedPieceTable);
                }
                for &b in &word_doc_bytes[start..end] {
                    // ANSI / Latin-1 byte to char
                    full_text.push(b as char);
                }
            } else {
                let start = piece.file_offset;
                let byte_count = char_count * 2;
                let end = start + byte_count;
                if end > word_doc_bytes.len() {
                    return Err(DocError::MalformedPieceTable);
                }
                let slice = &word_doc_bytes[start..end];
                let u16_chars: Vec<u16> = slice
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|chunk| u16::from_le_bytes(*chunk))
                    .collect();

                let s = String::from_utf16(&u16_chars).map_err(|_| DocError::Utf16Error)?;
                full_text.push_str(&s);
            }
        }

        Ok(full_text)
    }
}
