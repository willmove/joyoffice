//! OpenType shaping with HarfRust.
//!
//! [`shape_range`] shapes one item of a paragraph (one face, one direction, one script) with the
//! rest of the paragraph as pre/post context, so Arabic letters at an item boundary (a colour
//! change inside a word, a bold letter) still take their joining forms. Glyphs come back in
//! visual order, exactly as the shaper produced them: right-to-left items are not reversed.

use std::ops::Range;

use harfrust::{Direction, Feature, Language, Script, ShapeOptions, Tag, UnicodeBuffer};
use skrifa::MetadataProvider;
use skrifa::instance::Size;

use crate::FontFace;
use crate::script::ScriptTag;

/// One shaped glyph, in font units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapedGlyph {
    pub gid: u32,
    /// Byte offset (in the shaped string) of the cluster this glyph belongs to.
    pub cluster: usize,
    pub x_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

/// How to shape an item.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShapeParams<'a> {
    pub features: &'a [Feature],
    /// Right-to-left (odd bidi level). Mirrored characters (brackets) take their mirrored glyphs.
    pub rtl: bool,
    /// ISO 15924 script (`*b"Arab"`); `None` lets the shaper guess from the text.
    pub script: Option<ScriptTag>,
    /// BCP 47 language (`"ar"`, `"ar-MA"`); `None` = the script's default.
    pub language: Option<&'a str>,
}

/// Shape `para[range]` with `face`, the rest of `para` as context. Glyphs are in visual order
/// (left to right) and clusters are byte offsets into `para`. `map` substitutes characters (All
/// Caps) without moving clusters. An empty or out-of-bounds range shapes nothing.
pub fn shape_range(
    face: &FontFace,
    para: &str,
    range: Range<usize>,
    params: &ShapeParams,
    map: impl Fn(char) -> char,
) -> Vec<ShapedGlyph> {
    let Some(input) = TextItem::new(para, range) else {
        return Vec::new();
    };
    if let Some(font) = face.hb() {
        let buffer = input.buffer(params, &map);
        let shaped = face
            .shaper
            .shaper(&font)
            .instance(face.instance.as_ref())
            .build()
            .shape(buffer, ShapeOptions::new().features(params.features));
        return shaped
            .glyph_infos()
            .iter()
            .zip(shaped.glyph_positions())
            .map(|(info, position)| ShapedGlyph {
                gid: info.glyph_id,
                cluster: info.cluster as usize,
                x_advance: position.x_advance,
                x_offset: position.x_offset,
                y_offset: position.y_offset,
            })
            .collect();
    }
    cmap_glyphs(face, input, params.rtl, map)
}

struct TextItem<'a> {
    before: &'a str,
    active: &'a str,
    after: &'a str,
}

impl<'a> TextItem<'a> {
    fn new(paragraph: &'a str, range: Range<usize>) -> Option<Self> {
        // HarfRust stores clusters as u32. Keep them tied to the original UTF-8 byte positions.
        u32::try_from(range.end).ok()?;
        let (before, rest) = paragraph.split_at_checked(range.start)?;
        let (active, after) = rest.split_at_checked(range.end.checked_sub(range.start)?)?;
        (!active.is_empty()).then_some(Self {
            before,
            active,
            after,
        })
    }

    fn characters(&self) -> impl Iterator<Item = (usize, char)> + '_ {
        self.active
            .char_indices()
            .map(|(offset, character)| (self.before.len() + offset, character))
    }

    fn buffer(&self, params: &ShapeParams, map: impl Fn(char) -> char) -> UnicodeBuffer {
        let mut buffer = UnicodeBuffer::new();
        buffer.set_pre_context(self.before);
        buffer.set_post_context(self.after);
        for (cluster, character) in self.characters() {
            buffer.add(map(character), cluster as u32);
        }
        buffer.guess_segment_properties();
        if let Some(script) = params
            .script
            .and_then(|tag| Script::from_iso15924_tag(Tag::new(&tag)))
        {
            buffer.set_script(script);
        }
        let language = params
            .language
            .or_else(|| params.script.and_then(crate::script::default_language));
        if let Some(language) = language.and_then(|tag| tag.parse::<Language>().ok()) {
            buffer.set_language(language);
        }
        buffer.set_direction(if params.rtl {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        });
        buffer
    }
}

fn cmap_glyphs(
    face: &FontFace,
    input: TextItem<'_>,
    rtl: bool,
    map: impl Fn(char) -> char,
) -> Vec<ShapedGlyph> {
    let Some(font) = face.skrifa() else {
        return Vec::new();
    };
    let cmap = font.charmap();
    let metrics = font.glyph_metrics(Size::unscaled(), face.location());
    let mut glyphs: Vec<_> = input
        .characters()
        .map(|(cluster, character)| {
            let glyph = cmap.map(map(character)).unwrap_or_default();
            let advance = metrics
                .advance_width(glyph)
                .unwrap_or(face.upem as f32 / 2.0);
            ShapedGlyph {
                gid: glyph.to_u32(),
                cluster,
                x_advance: advance.round() as i32,
                x_offset: 0,
                y_offset: 0,
            }
        })
        .collect();
    if rtl {
        glyphs.reverse();
    }
    glyphs
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
mod tests {
    use super::*;
    use crate::{DEFAULT_FAMILY, FontDb};

    #[test]
    fn rtl_items_come_in_visual_order_and_mirror() {
        let face = FontDb::global().face(DEFAULT_FAMILY, "Regular");
        let text = "(ab)";
        let p = ShapeParams {
            rtl: true,
            ..Default::default()
        };
        let g = shape_range(&face, text, 0..text.len(), &p, |c| c);
        // Visual order: last cluster first.
        assert_eq!(
            g.iter().map(|g| g.cluster).collect::<Vec<_>>(),
            vec![3, 2, 1, 0]
        );
        // L4: in a right-to-left run "(" is drawn with the ")" glyph and vice versa.
        if face.covers('(') && face.covers(')') {
            assert_eq!(g[3].gid, face.glyph_for(')'));
            assert_eq!(g[0].gid, face.glyph_for('('));
        }
    }

    #[test]
    fn clusters_point_into_the_whole_paragraph() {
        let face = FontDb::global().face(DEFAULT_FAMILY, "Regular");
        let para = "Hello world";
        let g = shape_range(&face, para, 6..11, &ShapeParams::default(), |c| c);
        assert_eq!(g.first().map(|g| g.cluster), Some(6));
        assert!(shape_range(&face, para, 6..99, &ShapeParams::default(), |c| c).is_empty());
        // Not a char boundary.
        assert!(shape_range(&face, "سنة", 1..2, &ShapeParams::default(), |c| c).is_empty());
    }
}
