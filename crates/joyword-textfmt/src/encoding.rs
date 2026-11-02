//! Character encoding detection and newline normalization.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Ascii,
    FallbackLossy,
}

pub struct DecodedText {
    pub text: String,
    pub encoding: DetectedEncoding,
    pub unmapped_chars: usize,
}

pub fn decode_and_normalize(bytes: &[u8]) -> DecodedText {
    // 1. Check BOM
    if bytes.len() >= 3 && bytes[0..3] == [0xEF, 0xBB, 0xBF] {
        let payload = &bytes[3..];
        let text = String::from_utf8_lossy(payload).into_owned();
        return DecodedText {
            text: normalize_newlines(&text),
            encoding: DetectedEncoding::Utf8Bom,
            unmapped_chars: invalid_utf8_count(payload),
        };
    }
    if bytes.len() >= 2 && bytes[0..2] == [0xFF, 0xFE] {
        let (chunks, remainder) = bytes[2..].as_chunks::<2>();
        let u16s: Vec<u16> = chunks
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let mut text = String::from_utf16_lossy(&u16s);
        let unmapped_chars = char::decode_utf16(u16s.iter().copied())
            .filter(Result::is_err)
            .count()
            + remainder.len();
        if !remainder.is_empty() {
            text.push('\u{fffd}');
        }
        return DecodedText {
            text: normalize_newlines(&text),
            encoding: DetectedEncoding::Utf16Le,
            unmapped_chars,
        };
    }
    if bytes.len() >= 2 && bytes[0..2] == [0xFE, 0xFF] {
        let (chunks, remainder) = bytes[2..].as_chunks::<2>();
        let u16s: Vec<u16> = chunks
            .iter()
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        let mut text = String::from_utf16_lossy(&u16s);
        let unmapped_chars = char::decode_utf16(u16s.iter().copied())
            .filter(Result::is_err)
            .count()
            + remainder.len();
        if !remainder.is_empty() {
            text.push('\u{fffd}');
        }
        return DecodedText {
            text: normalize_newlines(&text),
            encoding: DetectedEncoding::Utf16Be,
            unmapped_chars,
        };
    }

    // 2. Try clean UTF-8
    if let Ok(valid_utf8) = std::str::from_utf8(bytes) {
        let is_ascii = bytes.iter().all(|&b| b < 128);
        return DecodedText {
            text: normalize_newlines(valid_utf8),
            encoding: if is_ascii {
                DetectedEncoding::Ascii
            } else {
                DetectedEncoding::Utf8
            },
            unmapped_chars: 0,
        };
    }

    // 3. Fallback to lossy UTF-8
    let lossy = String::from_utf8_lossy(bytes);
    let unmapped = invalid_utf8_count(bytes);
    DecodedText {
        text: normalize_newlines(&lossy),
        encoding: DetectedEncoding::FallbackLossy,
        unmapped_chars: unmapped,
    }
}

fn invalid_utf8_count(mut bytes: &[u8]) -> usize {
    let mut count = 0;
    while let Err(error) = std::str::from_utf8(bytes) {
        count += 1;
        let skip = error.valid_up_to()
            + error
                .error_len()
                .unwrap_or(bytes.len() - error.valid_up_to());
        bytes = &bytes[skip..];
    }
    count
}

pub fn normalize_newlines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}
