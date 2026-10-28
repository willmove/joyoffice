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
        let text = String::from_utf8_lossy(&bytes[3..]).into_owned();
        return DecodedText {
            text: normalize_newlines(&text),
            encoding: DetectedEncoding::Utf8Bom,
            unmapped_chars: 0,
        };
    }
    if bytes.len() >= 2 && bytes[0..2] == [0xFF, 0xFE] {
        let (chunks, _) = bytes[2..].as_chunks::<2>();
        let u16s: Vec<u16> = chunks
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let text = String::from_utf16_lossy(&u16s);
        return DecodedText {
            text: normalize_newlines(&text),
            encoding: DetectedEncoding::Utf16Le,
            unmapped_chars: 0,
        };
    }
    if bytes.len() >= 2 && bytes[0..2] == [0xFE, 0xFF] {
        let (chunks, _) = bytes[2..].as_chunks::<2>();
        let u16s: Vec<u16> = chunks
            .iter()
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        let text = String::from_utf16_lossy(&u16s);
        return DecodedText {
            text: normalize_newlines(&text),
            encoding: DetectedEncoding::Utf16Be,
            unmapped_chars: 0,
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
    let unmapped = lossy.matches('\u{FFFD}').count();
    DecodedText {
        text: normalize_newlines(&lossy),
        encoding: DetectedEncoding::FallbackLossy,
        unmapped_chars: unmapped,
    }
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
