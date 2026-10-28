//! Paragraph text storage and typed offsets (ARCHITECTURE §2.1).
//!
//! Text is stored as chunks of at most [`CHUNK_MAX_BYTES`] in a [`SeqTree`], so a
//! one-character edit copies one chunk plus O(log n) tree nodes regardless of
//! paragraph length. A paragraph of up to one chunk costs a single small
//! allocation; T100's benchmark (`examples/text_storage.rs`) chose this layout.
//!
//! Positions persist as Unicode scalar offsets. UTF-8 and UTF-16 offsets exist
//! for parsers and platform IME APIs, and every conversion rejects positions
//! inside a character or surrogate pair. Text is never normalized.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use crate::tree::{Item, SeqTree, Summary};

/// Reserved code point holding the atomic position of an inline object. It is
/// a Unicode noncharacter (reserved for internal use), rejected in ordinary
/// text, and never exported: readers use [`ParagraphText::chars`] only inside
/// the model, and paragraphs expose typed segments instead.
pub const OBJECT_MARKER: char = '\u{FDD0}';

/// Upper bound of one storage chunk in UTF-8 bytes.
pub const CHUNK_MAX_BYTES: usize = 1024;

macro_rules! offset_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub usize);
    };
}

offset_type!(Utf8Offset, "Offset in UTF-8 bytes.");
offset_type!(
    ScalarOffset,
    "Offset in Unicode scalar values; the persistent unit of positions."
);
offset_type!(
    Utf16Offset,
    "Offset in UTF-16 code units, as exchanged with platform text input APIs."
);

/// Errors from text offsets and edits. All are recoverable and leave the text
/// unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextError {
    OutOfBounds {
        offset: usize,
        len: usize,
    },
    InvalidRange {
        start: usize,
        end: usize,
    },
    NotCharBoundary {
        offset: usize,
    },
    InsideSurrogatePair {
        offset: usize,
    },
    /// Ordinary text contained [`OBJECT_MARKER`] at this scalar index.
    ReservedCodePoint {
        index: usize,
    },
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds { offset, len } => write!(f, "offset {offset} beyond length {len}"),
            Self::InvalidRange { start, end } => write!(f, "invalid range {start}..{end}"),
            Self::NotCharBoundary { offset } => write!(f, "byte {offset} is inside a character"),
            Self::InsideSurrogatePair { offset } => {
                write!(f, "UTF-16 offset {offset} is inside a surrogate pair")
            }
            Self::ReservedCodePoint { index } => {
                write!(f, "reserved object marker at scalar {index}")
            }
        }
    }
}

impl std::error::Error for TextError {}

/// Lengths of a text span in every unit the model exchanges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextSummary {
    pub bytes: usize,
    pub scalars: usize,
    pub utf16: usize,
    pub markers: usize,
}

impl TextSummary {
    pub fn of(text: &str) -> Self {
        let mut summary = Self {
            bytes: text.len(),
            ..Self::default()
        };
        for ch in text.chars() {
            summary.scalars += 1;
            summary.utf16 += ch.len_utf16();
            summary.markers += usize::from(ch == OBJECT_MARKER);
        }
        summary
    }
}

impl Summary for TextSummary {
    fn add_summary(&mut self, other: &Self) {
        self.bytes += other.bytes;
        self.scalars += other.scalars;
        self.utf16 += other.utf16;
        self.markers += other.markers;
    }
}

#[derive(Clone)]
struct Chunk {
    text: Arc<str>,
    summary: TextSummary,
}

impl Chunk {
    fn new(text: &str) -> Self {
        Self {
            summary: TextSummary::of(text),
            text: text.into(),
        }
    }

    /// Byte index of scalar `scalar` within this chunk (`scalar <= len`).
    fn byte_at(&self, scalar: usize) -> usize {
        self.text
            .char_indices()
            .nth(scalar)
            .map_or(self.text.len(), |(byte, _)| byte)
    }
}

impl Item for Chunk {
    type Summary = TextSummary;

    fn summary(&self) -> TextSummary {
        self.summary
    }
}

/// Splits text into chunks of at most [`CHUNK_MAX_BYTES`] on scalar boundaries.
fn split_chunks(text: &str) -> Vec<Chunk> {
    let mut chunks = Vec::with_capacity(text.len().div_ceil(CHUNK_MAX_BYTES));
    let mut rest = text;
    while rest.len() > CHUNK_MAX_BYTES {
        let mut cut = CHUNK_MAX_BYTES;
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        chunks.push(Chunk::new(&rest[..cut]));
        rest = &rest[cut..];
    }
    if !rest.is_empty() {
        chunks.push(Chunk::new(rest));
    }
    chunks
}

fn reject_markers(text: &str) -> Result<(), TextError> {
    match text.chars().position(|ch| ch == OBJECT_MARKER) {
        Some(index) => Err(TextError::ReservedCodePoint { index }),
        None => Ok(()),
    }
}

/// Text of one paragraph; see the module documentation.
#[derive(Clone, Default)]
pub struct ParagraphText {
    chunks: SeqTree<Chunk>,
}

impl fmt::Debug for ParagraphText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.to_raw_string(), f)
    }
}

impl PartialEq for ParagraphText {
    fn eq(&self, other: &Self) -> bool {
        self.summary() == other.summary() && self.chars().eq(other.chars())
    }
}

impl Eq for ParagraphText {}

impl ParagraphText {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ordinary text; rejects [`OBJECT_MARKER`].
    pub fn from_text(text: &str) -> Result<Self, TextError> {
        reject_markers(text)?;
        Ok(Self::from_raw(text))
    }

    pub(crate) fn from_raw(text: &str) -> Self {
        Self {
            chunks: SeqTree::from_vec(split_chunks(text)),
        }
    }

    pub fn summary(&self) -> TextSummary {
        self.chunks.summary()
    }

    pub fn len_scalars(&self) -> usize {
        self.summary().scalars
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Storage chunk count; exposed for structural-sharing tests and benchmarks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Number of tree nodes not shared with `base`.
    pub fn unshared_nodes(&self, base: &Self) -> usize {
        self.chunks.unshared_nodes(&base.chunks)
    }

    /// Scalars including object markers. Crate-internal so markers never leak.
    pub(crate) fn chars(&self) -> impl Iterator<Item = char> + '_ {
        self.chunks.iter().flat_map(|chunk| chunk.text.chars())
    }

    pub fn to_raw_string(&self) -> String {
        let mut text = String::with_capacity(self.summary().bytes);
        for chunk in self.chunks.iter() {
            text.push_str(&chunk.text);
        }
        text
    }

    fn check_offset(&self, offset: ScalarOffset) -> Result<(), TextError> {
        let len = self.len_scalars();
        if offset.0 > len {
            return Err(TextError::OutOfBounds {
                offset: offset.0,
                len,
            });
        }
        Ok(())
    }

    fn check_range(&self, range: &Range<ScalarOffset>) -> Result<(), TextError> {
        if range.start > range.end {
            return Err(TextError::InvalidRange {
                start: range.start.0,
                end: range.end.0,
            });
        }
        self.check_offset(range.end)
    }

    /// Chunk containing scalar `offset` (validated), its prefix summary and the
    /// scalar offset within the chunk.
    fn locate_scalar(&self, offset: ScalarOffset) -> Option<(usize, TextSummary, usize)> {
        let (index, prefix) = self.chunks.locate(offset.0, |s| s.scalars)?;
        Some((index, prefix, offset.0 - prefix.scalars))
    }

    /// Inserts ordinary text; rejects [`OBJECT_MARKER`].
    pub fn insert_str(&mut self, at: ScalarOffset, text: &str) -> Result<(), TextError> {
        reject_markers(text)?;
        self.insert_raw(at, text)
    }

    pub(crate) fn insert_raw(&mut self, at: ScalarOffset, text: &str) -> Result<(), TextError> {
        self.check_offset(at)?;
        if text.is_empty() {
            return Ok(());
        }
        let Some((index, _, within)) = self.locate_scalar(at) else {
            self.chunks = SeqTree::from_vec(split_chunks(text));
            return Ok(());
        };
        let chunk = self.chunks.get(index).expect("located chunk exists");
        let byte = chunk.byte_at(within);
        let mut joined = String::with_capacity(chunk.text.len() + text.len());
        joined.push_str(&chunk.text[..byte]);
        joined.push_str(text);
        joined.push_str(&chunk.text[byte..]);
        let mut pieces = split_chunks(&joined).into_iter();
        self.chunks
            .replace(index, pieces.next().expect("joined text is non-empty"));
        for (step, piece) in pieces.enumerate() {
            self.chunks.insert(index + 1 + step, piece);
        }
        Ok(())
    }

    /// Removes a scalar range and returns it, markers included.
    pub(crate) fn remove_raw(&mut self, range: Range<ScalarOffset>) -> Result<String, TextError> {
        self.check_range(&range)?;
        if range.start == range.end {
            return Ok(String::new());
        }
        let (first, _, start_within) = self.locate_scalar(range.start).expect("non-empty text");
        let (last, _, end_within) = self.locate_scalar(range.end).expect("non-empty text");
        let first_chunk = self
            .chunks
            .get(first)
            .expect("located chunk exists")
            .clone();
        let last_chunk = self.chunks.get(last).expect("located chunk exists").clone();
        let start_byte = first_chunk.byte_at(start_within);
        let end_byte = last_chunk.byte_at(end_within);

        let mut removed = String::new();
        if first == last {
            removed.push_str(&first_chunk.text[start_byte..end_byte]);
        } else {
            removed.push_str(&first_chunk.text[start_byte..]);
            for index in first + 1..last {
                removed.push_str(&self.chunks.get(index).expect("chunk in range").text);
            }
            removed.push_str(&last_chunk.text[..end_byte]);
        }

        let mut kept = String::with_capacity(start_byte + last_chunk.text.len() - end_byte);
        kept.push_str(&first_chunk.text[..start_byte]);
        kept.push_str(&last_chunk.text[end_byte..]);
        for _ in first..last {
            self.chunks.remove(first + 1);
        }
        // Absorb a following chunk while the result still fits, so repeated
        // deletions do not leave long runs of tiny chunks.
        if let Some(next) = self.chunks.get(first + 1)
            && kept.len() + next.text.len() <= CHUNK_MAX_BYTES
        {
            kept.push_str(&next.text);
            self.chunks.remove(first + 1);
        }
        if kept.is_empty() {
            self.chunks.remove(first);
        } else {
            self.chunks.replace(first, Chunk::new(&kept));
        }
        Ok(removed)
    }

    /// Splits off the text after `at`, returning it as a new paragraph text.
    pub(crate) fn split_off(&mut self, at: ScalarOffset) -> Result<Self, TextError> {
        let len = ScalarOffset(self.len_scalars());
        let tail = self.remove_raw(at..len)?;
        Ok(Self::from_raw(&tail))
    }

    pub(crate) fn append(&mut self, other: &Self) {
        for chunk in other.chunks.iter() {
            self.chunks.push(chunk.clone());
        }
    }

    pub fn scalar_to_utf16(&self, offset: ScalarOffset) -> Result<Utf16Offset, TextError> {
        self.check_offset(offset)?;
        let Some((index, prefix, within)) = self.locate_scalar(offset) else {
            return Ok(Utf16Offset(0));
        };
        let chunk = self.chunks.get(index).expect("located chunk exists");
        let partial: usize = chunk.text.chars().take(within).map(char::len_utf16).sum();
        Ok(Utf16Offset(prefix.utf16 + partial))
    }

    pub fn utf16_to_scalar(&self, offset: Utf16Offset) -> Result<ScalarOffset, TextError> {
        let total = self.summary().utf16;
        if offset.0 > total {
            return Err(TextError::OutOfBounds {
                offset: offset.0,
                len: total,
            });
        }
        let Some((index, prefix)) = self.chunks.locate(offset.0, |s| s.utf16) else {
            return Ok(ScalarOffset(0));
        };
        let chunk = self.chunks.get(index).expect("located chunk exists");
        let mut units = prefix.utf16;
        let mut scalars = prefix.scalars;
        for ch in chunk.text.chars() {
            if units >= offset.0 {
                break;
            }
            units += ch.len_utf16();
            scalars += 1;
        }
        if units != offset.0 {
            return Err(TextError::InsideSurrogatePair { offset: offset.0 });
        }
        Ok(ScalarOffset(scalars))
    }

    pub fn scalar_to_utf8(&self, offset: ScalarOffset) -> Result<Utf8Offset, TextError> {
        self.check_offset(offset)?;
        let Some((index, prefix, within)) = self.locate_scalar(offset) else {
            return Ok(Utf8Offset(0));
        };
        let chunk = self.chunks.get(index).expect("located chunk exists");
        Ok(Utf8Offset(prefix.bytes + chunk.byte_at(within)))
    }

    pub fn utf8_to_scalar(&self, offset: Utf8Offset) -> Result<ScalarOffset, TextError> {
        let total = self.summary().bytes;
        if offset.0 > total {
            return Err(TextError::OutOfBounds {
                offset: offset.0,
                len: total,
            });
        }
        let Some((index, prefix)) = self.chunks.locate(offset.0, |s| s.bytes) else {
            return Ok(ScalarOffset(0));
        };
        let chunk = self.chunks.get(index).expect("located chunk exists");
        let within = offset.0 - prefix.bytes;
        if !chunk.text.is_char_boundary(within) {
            return Err(TextError::NotCharBoundary { offset: offset.0 });
        }
        Ok(ScalarOffset(
            prefix.scalars + chunk.text[..within].chars().count(),
        ))
    }

    /// Number of object markers before `offset`, i.e. the index of the inline
    /// object whose marker sits at `offset`.
    pub(crate) fn markers_before(&self, offset: ScalarOffset) -> Result<usize, TextError> {
        self.check_offset(offset)?;
        let Some((index, prefix, within)) = self.locate_scalar(offset) else {
            return Ok(0);
        };
        let chunk = self.chunks.get(index).expect("located chunk exists");
        let partial = chunk
            .text
            .chars()
            .take(within)
            .filter(|&ch| ch == OBJECT_MARKER)
            .count();
        Ok(prefix.markers + partial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "甲方 Party A：e\u{301}🇨🇳👩‍💻𠀋";

    #[test]
    fn preserves_text_without_normalization() {
        let text = ParagraphText::from_text(SAMPLE).unwrap();
        assert_eq!(text.to_raw_string(), SAMPLE);
        let summary = text.summary();
        assert_eq!(summary.bytes, SAMPLE.len());
        assert_eq!(summary.scalars, SAMPLE.chars().count());
        assert_eq!(summary.utf16, SAMPLE.encode_utf16().count());
        assert_eq!(summary.markers, 0);
    }

    #[test]
    fn offset_conversions_round_trip_and_reject_interior_positions() {
        let text = ParagraphText::from_text(SAMPLE).unwrap();
        let mut utf16 = 0;
        let mut utf8 = 0;
        for (scalar, ch) in SAMPLE.chars().enumerate() {
            assert_eq!(
                text.scalar_to_utf16(ScalarOffset(scalar)),
                Ok(Utf16Offset(utf16))
            );
            assert_eq!(
                text.utf16_to_scalar(Utf16Offset(utf16)),
                Ok(ScalarOffset(scalar))
            );
            assert_eq!(
                text.scalar_to_utf8(ScalarOffset(scalar)),
                Ok(Utf8Offset(utf8))
            );
            assert_eq!(
                text.utf8_to_scalar(Utf8Offset(utf8)),
                Ok(ScalarOffset(scalar))
            );
            if ch.len_utf16() == 2 {
                assert_eq!(
                    text.utf16_to_scalar(Utf16Offset(utf16 + 1)),
                    Err(TextError::InsideSurrogatePair { offset: utf16 + 1 })
                );
            }
            for interior in 1..ch.len_utf8() {
                assert_eq!(
                    text.utf8_to_scalar(Utf8Offset(utf8 + interior)),
                    Err(TextError::NotCharBoundary {
                        offset: utf8 + interior
                    })
                );
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        let end = SAMPLE.chars().count();
        assert_eq!(
            text.scalar_to_utf16(ScalarOffset(end)),
            Ok(Utf16Offset(utf16))
        );
        assert_eq!(
            text.scalar_to_utf16(ScalarOffset(end + 1)),
            Err(TextError::OutOfBounds {
                offset: end + 1,
                len: end
            })
        );
    }

    #[test]
    fn rejects_reserved_marker_in_ordinary_text() {
        assert_eq!(
            ParagraphText::from_text("ab\u{FDD0}").unwrap_err(),
            TextError::ReservedCodePoint { index: 2 }
        );
        let mut text = ParagraphText::from_text("ab").unwrap();
        assert!(text.insert_str(ScalarOffset(1), "\u{FDD0}").is_err());
        assert_eq!(text.to_raw_string(), "ab");
    }

    #[test]
    fn edits_match_string_model_across_chunk_boundaries() {
        // Long paragraph so edits cross several 1 KiB chunks.
        let base: String = "合同条款 clause 第一条。".repeat(400);
        let mut text = ParagraphText::from_text(&base).unwrap();
        let mut model: Vec<char> = base.chars().collect();
        assert!(text.chunk_count() > 5);
        let mut seed = 7_u64;
        let mut next = |bound: usize| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            ((seed >> 33) as usize) % bound.max(1)
        };
        for _ in 0..500 {
            if next(2) == 0 {
                let at = next(model.len() + 1);
                let insert = ["乙", "x", "🙂", "e\u{301}", "长文本插入 long insert "][next(5)];
                text.insert_str(ScalarOffset(at), insert).unwrap();
                model.splice(at..at, insert.chars());
            } else if !model.is_empty() {
                let start = next(model.len());
                let end = (start + next(3_000)).min(model.len());
                let removed = text
                    .remove_raw(ScalarOffset(start)..ScalarOffset(end))
                    .unwrap();
                let expected: String = model.drain(start..end).collect();
                assert_eq!(removed, expected);
            }
            assert_eq!(text.len_scalars(), model.len());
        }
        assert_eq!(text.to_raw_string(), model.iter().collect::<String>());
        text.chunks.assert_invariants();
    }

    #[test]
    fn split_and_append_restore_original() {
        let mut text = ParagraphText::from_text(SAMPLE).unwrap();
        let tail = text.split_off(ScalarOffset(4)).unwrap();
        assert_eq!(text.to_raw_string(), "甲方 P");
        text.append(&tail);
        assert_eq!(text.to_raw_string(), SAMPLE);
        let mut empty = ParagraphText::new();
        assert_eq!(empty.split_off(ScalarOffset(0)).unwrap().len_scalars(), 0);
        assert_eq!(
            empty.remove_raw(ScalarOffset(1)..ScalarOffset(0)),
            Err(TextError::InvalidRange { start: 1, end: 0 })
        );
    }

    #[test]
    fn single_character_edit_in_long_paragraph_shares_storage() {
        let base = ParagraphText::from_text(&"段落".repeat(200_000)).unwrap();
        let mut edited = base.clone();
        edited.insert_str(ScalarOffset(200_000), "改").unwrap();
        assert!(edited.unshared_nodes(&base) <= 6);
        assert_eq!(base.len_scalars(), 400_000);
        assert_eq!(edited.len_scalars(), 400_001);
    }
}
