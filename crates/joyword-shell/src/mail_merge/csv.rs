use super::{Limits, MergeError};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct CsvData {
    pub(super) bytes: usize,
    pub(super) headers: Vec<String>,
    pub(super) rows: Vec<Vec<String>>,
}

impl CsvData {
    pub fn headers(&self) -> &[String] {
        &self.headers
    }
    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    pub fn parse(bytes: &[u8], limits: &Limits) -> Result<Self, MergeError> {
        limits.validate()?;
        let source_bytes = bytes.len();
        if bytes.len() > limits.csv_bytes {
            return Err(MergeError::Invalid("CSV byte budget exceeded"));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| MergeError::Invalid("CSV must be UTF-8 (optional UTF-8 BOM)"))?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let bytes = text.as_bytes();
        let mut records: Vec<Vec<String>> = Vec::new();
        let mut record = Vec::new();
        let mut cell = Vec::new();
        // 0 start, 1 unquoted, 2 quoted, 3 after closing quote.
        let mut state = 0;
        let mut i = 0;
        while i < bytes.len() {
            let byte = bytes[i];
            let number = records.len() + 1;
            let fail = |reason| MergeError::Csv {
                record: number,
                reason,
            };
            if byte == 0 {
                return Err(fail("NUL is not allowed"));
            }
            if state == 2 {
                if byte == b'"' {
                    if bytes.get(i + 1) == Some(&b'"') {
                        cell.push(b'"');
                        i += 1;
                    } else {
                        state = 3;
                    }
                } else if byte == b'\r' {
                    cell.push(b'\n');
                    if bytes.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                } else {
                    cell.push(byte);
                }
            } else if matches!(byte, b',' | b'\r' | b'\n') {
                if byte == b'\r' {
                    if bytes.get(i + 1) != Some(&b'\n') {
                        return Err(fail("bare CR record separator"));
                    }
                    i += 1;
                }
                push_cell(&mut record, &mut cell, limits)?;
                state = 0;
                if byte != b',' {
                    records.push(std::mem::take(&mut record));
                    if records.len() > limits.rows + 1 {
                        return Err(fail("record budget exceeded"));
                    }
                }
            } else if state == 0 && byte == b'"' {
                state = 2;
            } else if byte == b'"' || state == 3 {
                return Err(fail("unexpected character or quote after field"));
            } else {
                cell.push(byte);
                state = 1;
            }
            if cell.len() > limits.cell_bytes {
                return Err(fail("cell byte budget exceeded"));
            }
            i += 1;
        }
        if state == 2 {
            return Err(MergeError::Csv {
                record: records.len() + 1,
                reason: "unclosed quoted field",
            });
        }
        if state != 0 || !record.is_empty() || !cell.is_empty() {
            push_cell(&mut record, &mut cell, limits)?;
            records.push(record);
        }
        if records.len() < 2 {
            return Err(MergeError::Invalid(
                "CSV needs a header and at least one data record",
            ));
        }
        if records.len() > limits.rows + 1 {
            return Err(MergeError::Invalid("CSV record budget exceeded"));
        }
        let headers = records.remove(0);
        if headers
            .iter()
            .any(|header| header.trim().is_empty() || header.chars().any(char::is_control))
            || headers.iter().collect::<BTreeSet<_>>().len() != headers.len()
        {
            return Err(MergeError::Invalid(
                "CSV header names must be nonempty and unique",
            ));
        }
        for (i, row) in records.iter().enumerate() {
            if row.len() != headers.len() {
                return Err(MergeError::Csv {
                    record: i + 2,
                    reason: "column count does not match header",
                });
            }
            if row.iter().any(|cell| {
                cell.chars()
                    .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\t'))
                    || cell.contains(joyword_model::OBJECT_MARKER)
            }) {
                return Err(MergeError::Csv {
                    record: i + 2,
                    reason: "cell contains unsupported control character",
                });
            }
        }
        Ok(Self {
            bytes: source_bytes,
            headers,
            rows: records,
        })
    }
}

fn push_cell(
    record: &mut Vec<String>,
    cell: &mut Vec<u8>,
    limits: &Limits,
) -> Result<(), MergeError> {
    if record.len() >= limits.columns {
        return Err(MergeError::Invalid("CSV column budget exceeded"));
    }
    record.push(
        String::from_utf8(std::mem::take(cell))
            .map_err(|_| MergeError::Invalid("CSV field UTF-8 boundary invalid"))?,
    );
    Ok(())
}
