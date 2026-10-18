//! Four-slot font choice, fallback, and the basic CJK break adjustment (LAYOUT-002,
//! LAYOUT-003). Layout stays read-only: it never writes the document.

use joyword_foundation::Emu;
use joyword_model::FontHint;

use crate::{FeatureSet, FontKey, LineLimits, LoadedFont, ShapedRun, shape};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontSlot {
    Ascii,
    HAnsi,
    EastAsia,
    Complex,
}

/// Which `w:rFonts` slot draws `ch`. A hint overrides the script class.
pub fn slot_for(ch: char, hint: Option<FontHint>) -> FontSlot {
    match hint {
        Some(FontHint::EastAsia) => return FontSlot::EastAsia,
        Some(FontHint::ComplexScript) => return FontSlot::Complex,
        Some(FontHint::Default) | None => {}
    }
    if ch.is_ascii() {
        FontSlot::Ascii
    } else if is_cjk(ch) {
        FontSlot::EastAsia
    } else if is_complex(ch) {
        FontSlot::Complex
    } else {
        FontSlot::HAnsi
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{2E80}'..='\u{9FFF}' | '\u{FF00}'..='\u{FFEF}' | '\u{20000}'..='\u{3FFFD}'
    )
}

fn is_complex(ch: char) -> bool {
    matches!(ch, '\u{0590}'..='\u{08FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

/// Closing punctuation must not start a line; opening punctuation must not end one.
pub fn prohibited_line_edge(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if is_closing(first) {
        return true;
    }
    text.chars().last().is_some_and(is_opening)
}

fn is_closing(ch: char) -> bool {
    crate::cjk::prohibits_line_start(ch)
}

fn is_opening(ch: char) -> bool {
    crate::cjk::prohibits_line_end(ch)
}

/// Move `at` so the following line does not start with closing punctuation and
/// this line does not end with opening punctuation. `at` is a byte offset.
pub fn adjust_cjk_break(text: &str, at: usize) -> usize {
    if at == 0 || at >= text.len() || !text.is_char_boundary(at) {
        return at;
    }
    let mut chosen = at;
    if text[at..].chars().next().is_some_and(is_closing) {
        chosen = text[..at]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
    }
    if text[..chosen].chars().next_back().is_some_and(is_opening) && chosen > 0 {
        chosen = text[..chosen]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
    }
    chosen
}

#[derive(Clone, Copy)]
pub struct FaceSet<'a> {
    pub ascii: &'a LoadedFont,
    pub h_ansi: &'a LoadedFont,
    pub east_asia: &'a LoadedFont,
    pub complex: &'a LoadedFont,
    pub fallback: &'a LoadedFont,
}

impl<'a> FaceSet<'a> {
    pub fn uniform(font: &'a LoadedFont) -> Self {
        Self {
            ascii: font,
            h_ansi: font,
            east_asia: font,
            complex: font,
            fallback: font,
        }
    }

    pub(crate) fn face(&self, slot: FontSlot) -> &'a LoadedFont {
        match slot {
            FontSlot::Ascii => self.ascii,
            FontSlot::HAnsi => self.h_ansi,
            FontSlot::EastAsia => self.east_asia,
            FontSlot::Complex => self.complex,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixedLine {
    pub text: String,
    pub font: FontKey,
    pub width: Emu,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixedParagraph {
    pub lines: Vec<MixedLine>,
}

/// Shape `text` with the four slots and fall back per cluster when the slot
/// font has no glyph. Line breaks use Unicode opportunities, then
/// [`adjust_cjk_break`], so a prohibition moves the break.
///
/// When the face that drew a one-glyph cluster is named 等线, a narrower R7
/// punctuation width replaces that cluster's advance before the break is
/// chosen. Other faces keep the shaped advance.
pub fn wrap_mixed(
    faces: FaceSet<'_>,
    text: &str,
    size: Emu,
    width: Emu,
    hint: Option<FontHint>,
) -> MixedParagraph {
    let _ = LineLimits::default();
    let pieces = shaped_pieces(faces, text, size, hint);
    let mut opportunities = vec![0];
    for (offset, _) in unicode_linebreak::linebreaks(text) {
        if offset > 0 && offset < text.len() && text.is_char_boundary(offset) {
            let adjusted = adjust_cjk_break(text, offset);
            if adjusted > 0 && !opportunities.contains(&adjusted) {
                opportunities.push(adjusted);
            }
        }
    }
    opportunities.push(text.len());
    opportunities.sort_unstable();
    opportunities.dedup();

    let mut lines = Vec::new();
    let mut start = 0usize;
    while start < text.len() {
        let mut best = start;
        for &end in opportunities.iter().filter(|end| **end > start) {
            let slice_width = width_of(&pieces, start, end);
            if slice_width.raw() <= width.raw() || best == start {
                best = end;
                if slice_width.raw() > width.raw() {
                    break;
                }
            } else {
                break;
            }
        }
        if best == start {
            best = text[start..]
                .chars()
                .next()
                .map(|ch| start + ch.len_utf8())
                .unwrap_or(text.len());
            best = adjust_cjk_break(text, best).max(start);
            if best == start {
                best = text[start..]
                    .chars()
                    .next()
                    .map(|ch| start + ch.len_utf8())
                    .unwrap_or(text.len());
            }
        }
        let line_text = text[start..best].to_owned();
        let font = pieces
            .iter()
            .find(|piece| piece.start < best && piece.end > start)
            .map(|piece| piece.font)
            .unwrap_or_else(|| faces.fallback.key());
        lines.push(MixedLine {
            width: width_of(&pieces, start, best),
            text: line_text,
            font,
        });
        start = best;
    }
    MixedParagraph { lines }
}

struct Piece {
    start: usize,
    end: usize,
    font: FontKey,
    width: Emu,
}

fn shaped_pieces(faces: FaceSet<'_>, text: &str, size: Emu, hint: Option<FontHint>) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut rest = text;
    let mut offset = 0usize;
    while !rest.is_empty() {
        let slot = slot_for(rest.chars().next().unwrap(), hint);
        let face = faces.face(slot);
        let run_end = rest
            .char_indices()
            .skip(1)
            .find(|(_, ch)| slot_for(*ch, hint) != slot)
            .map(|(index, _)| index)
            .unwrap_or(rest.len());
        let slice = &rest[..run_end];
        let shaped = shape_or_fallback(face, faces.fallback, slice, size);
        let family = if shaped.font == face.key() {
            crate::cjk::face_cjk_family(face)
        } else {
            crate::cjk::face_cjk_family(faces.fallback)
        };
        let mut cursor = 0usize;
        for (glyph_index, glyph) in shaped.glyphs.iter().enumerate() {
            if glyph.cluster < cursor {
                continue;
            }
            let next = shaped
                .glyphs
                .iter()
                .map(|item| item.cluster)
                .filter(|cluster| *cluster > glyph.cluster)
                .min()
                .unwrap_or(slice.len());
            if next > cursor {
                let one_glyph = shaped
                    .glyphs
                    .get(glyph_index + 1)
                    .is_none_or(|item| item.cluster != glyph.cluster)
                    && glyph_index
                        .checked_sub(1)
                        .is_none_or(|previous| shaped.glyphs[previous].cluster != glyph.cluster);
                let cluster_text = &slice[cursor..next];
                let mut chars = cluster_text.chars();
                let width = if one_glyph && chars.next().is_some() && chars.next().is_none() {
                    let ch = cluster_text.chars().next().expect("one char");
                    let absolute_end = offset + next;
                    crate::cjk::compressed_cluster_advance(
                        ch,
                        text[absolute_end..].chars().next(),
                        glyph.advance,
                        size,
                        family,
                    )
                } else {
                    glyph.advance
                };
                pieces.push(Piece {
                    start: offset + cursor,
                    end: offset + next,
                    font: shaped.font,
                    width,
                });
                cursor = next;
            }
        }
        if pieces
            .last()
            .is_none_or(|piece| piece.end < offset + slice.len())
        {
            pieces.push(Piece {
                start: offset + cursor,
                end: offset + slice.len(),
                font: shaped.font,
                width: Emu::new(0),
            });
        }
        offset += run_end;
        rest = &rest[run_end..];
    }
    let _ = FeatureSet::default();
    pieces
}

fn shape_or_fallback(
    primary: &LoadedFont,
    fallback: &LoadedFont,
    text: &str,
    size: Emu,
) -> ShapedRun {
    let shaped = shape(primary, text, size, FeatureSet::default());
    if shaped.missing.is_empty() || primary.key() == fallback.key() {
        return shaped;
    }
    let fallback_run = shape(fallback, text, size, FeatureSet::default());
    if fallback_run.missing.len() < shaped.missing.len() {
        fallback_run
    } else {
        shaped
    }
}

fn width_of(pieces: &[Piece], start: usize, end: usize) -> Emu {
    Emu::new(
        pieces
            .iter()
            .filter(|piece| piece.start >= start && piece.end <= end)
            .map(|piece| piece.width.raw())
            .sum(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_adjustment_moves_a_break_off_closing_punctuation() {
        let text = "甲乙。丙";
        let naive = text.find('。').unwrap();
        let adjusted = adjust_cjk_break(text, naive);
        assert!(adjusted < naive);
        assert!(!prohibited_line_edge(&text[adjusted..]));
        assert!(prohibited_line_edge(&text[naive..]));
    }

    #[test]
    fn mixed_faces_and_fallback_change_breaks() {
        let latin = LoadedFont::from_bytes(
            std::sync::Arc::from(
                include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
            ),
            0,
        )
        .unwrap();
        let cjk_path = "/System/Library/Fonts/Supplemental/Songti.ttc";
        let Some(cjk) = std::fs::read(cjk_path)
            .ok()
            .and_then(|bytes| LoadedFont::from_bytes(std::sync::Arc::from(bytes), 0).ok())
        else {
            return;
        };
        let uniform = FaceSet::uniform(&latin);
        let mixed = FaceSet {
            east_asia: &cjk,
            fallback: &cjk,
            ..uniform
        };
        let text = "AAAA甲甲甲甲。BBBB";
        let size = Emu::new(12 * Emu::PER_POINT);
        let width = Emu::new(12 * Emu::PER_POINT * 6);
        let plain = wrap_mixed(uniform, text, size, width, None);
        let shaped = wrap_mixed(mixed, text, size, width, None);
        assert_ne!(
            plain
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            shaped
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
        );
        assert!(shaped.lines.iter().any(|line| line.font == cjk.key()));
        assert!(
            shaped
                .lines
                .iter()
                .all(|line| !prohibited_line_edge(&line.text))
        );
    }

    #[test]
    fn slots_follow_script_and_hint() {
        assert_eq!(slot_for('A', None), FontSlot::Ascii);
        assert_eq!(slot_for('é', None), FontSlot::HAnsi);
        assert_eq!(slot_for('甲', None), FontSlot::EastAsia);
        assert_eq!(slot_for('ا', None), FontSlot::Complex);
        assert_eq!(slot_for('A', Some(FontHint::EastAsia)), FontSlot::EastAsia);
    }
}
