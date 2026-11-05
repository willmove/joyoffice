use super::{MAX_CSV, MAX_RECORDS};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub(super) struct CsvData {
    pub(super) headers: Vec<String>,
    pub(super) rows: Vec<Vec<String>>,
}

impl CsvData {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_CSV as usize {
            return Err("CSV byte budget exceeded".to_owned());
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| "CSV must be UTF-8 (optional UTF-8 BOM)".to_owned())?;
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
            let fail = |reason| format!("CSV record {number}: {reason}");
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
                push_cell(&mut record, &mut cell)?;
                state = 0;
                if byte != b',' {
                    records.push(std::mem::take(&mut record));
                    if records.len() > MAX_RECORDS + 1 {
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
            if cell.len() > (16 << 10) {
                return Err(fail("cell byte budget exceeded"));
            }
            i += 1;
        }
        if state == 2 {
            return Err(format!(
                "CSV record {}: unclosed quoted field",
                records.len() + 1
            ));
        }
        if state != 0 || !record.is_empty() || !cell.is_empty() {
            push_cell(&mut record, &mut cell)?;
            records.push(record);
        }
        if records.len() < 2 {
            return Err("CSV needs a header and at least one data record".to_owned());
        }
        if records.len() > MAX_RECORDS + 1 {
            return Err("CSV record budget exceeded".to_owned());
        }
        let headers = records.remove(0);
        if headers
            .iter()
            .any(|header| header.trim().is_empty() || header.chars().any(char::is_control))
            || headers.iter().collect::<BTreeSet<_>>().len() != headers.len()
        {
            return Err("CSV header names must be nonempty and unique".to_owned());
        }
        for (i, row) in records.iter().enumerate() {
            if row.len() != headers.len() {
                return Err(format!(
                    "CSV record {}: column count does not match header",
                    i + 2
                ));
            }
            if row.iter().any(|cell| {
                cell.chars()
                    .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\t'))
                    || cell.contains(joyword_engine::doc::para::OBJ)
            }) {
                return Err(format!(
                    "CSV record {}: cell contains unsupported control character",
                    i + 2
                ));
            }
        }
        Ok(Self {
            headers,
            rows: records,
        })
    }
}

fn push_cell(record: &mut Vec<String>, cell: &mut Vec<u8>) -> Result<(), String> {
    if record.len() >= 128 {
        return Err("CSV column budget exceeded".to_owned());
    }
    record.push(
        String::from_utf8(std::mem::take(cell))
            .map_err(|_| "CSV field UTF-8 boundary invalid".to_owned())?,
    );
    Ok(())
}
