//! Shaping with document-driven features (ARCHITECTURE §6.1, LAYOUT-002).
//!
//! HarfRust enables kerning and standard ligatures by default; Word applies
//! them only when the document asks (`w:kern` thresholds, `w14:ligatures`).
//! Every request therefore states each feature explicitly. Glyph positions
//! are converted from font units to EMU from the cumulative pen position, so
//! rounding never accumulates along a line.

use std::collections::{HashMap, VecDeque};
use std::str::FromStr;
use std::sync::Mutex;

use harfrust::{Feature, ShapeOptions, ShaperData, UnicodeBuffer};
use joyword_foundation::Emu;

use crate::font::{FontKey, LoadedFont};

/// Faces retained in the process-wide shaper-data cache (T208). Each entry is
/// HarfRust's own parsed face, keyed by [`FontKey`], and the oldest face is
/// dropped when the cap is reached.
const MAX_SHAPER_FACES: usize = 8;

struct ShaperCache {
    order: VecDeque<FontKey>,
    map: HashMap<FontKey, ShaperData>,
}

impl ShaperCache {
    fn prepare(&mut self, font: &LoadedFont) {
        let key = font.key();
        if self.map.contains_key(&key) {
            if let Some(position) = self.order.iter().position(|item| *item == key) {
                self.order.remove(position);
            }
            self.order.push_back(key);
            return;
        }
        while self.order.len() >= MAX_SHAPER_FACES {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            self.map.remove(&old);
        }
        self.map.insert(key, ShaperData::new(&font.font_ref()));
        self.order.push_back(key);
    }
}

fn shaper_cache() -> &'static Mutex<ShaperCache> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Mutex<ShaperCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(ShaperCache {
            order: VecDeque::new(),
            map: HashMap::new(),
        })
    })
}

/// OpenType features a document can switch on. Everything not listed here
/// keeps HarfRust's required behavior (e.g. `ccmp`, `locl`, mark positioning).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FeatureSet {
    pub kerning: bool,
    pub standard_ligatures: bool,
    pub contextual_alternates: bool,
}

impl FeatureSet {
    fn features(self) -> Vec<Feature> {
        let flag = |on: bool, tag: &str| {
            Feature::from_str(&format!("{}{tag}", if on { "+" } else { "-" }))
                .expect("static feature syntax")
        };
        vec![
            flag(self.kerning, "kern"),
            flag(self.standard_ligatures, "liga"),
            flag(self.standard_ligatures, "clig"),
            flag(self.contextual_alternates, "calt"),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    /// UTF-8 byte offset of the glyph's cluster in the shaped text.
    pub cluster: usize,
    /// Pen position of the glyph origin relative to the run start.
    pub x: Emu,
    pub advance: Emu,
    pub x_offset: Emu,
    pub y_offset: Emu,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapedRun {
    pub font: FontKey,
    /// Font size as a length.
    pub size: Emu,
    pub text: String,
    pub glyphs: Vec<ShapedGlyph>,
    pub width: Emu,
    /// Byte offsets of clusters the font has no glyph for (missing-glyph
    /// diagnostics; the text itself is never replaced).
    pub missing: Vec<usize>,
}

fn to_emu(units: i64, size: Emu, units_per_em: u16) -> Emu {
    // Round half away from zero; i128 avoids overflow for large documents.
    let numerator = i128::from(units) * i128::from(size.raw());
    let denominator = i128::from(units_per_em);
    let half = denominator / 2;
    let value = if numerator >= 0 {
        (numerator + half) / denominator
    } else {
        (numerator - half) / denominator
    };
    Emu::new(value as i64)
}

/// Shapes one run of text in one font, size and feature set. Script,
/// language and direction are guessed from the text for now; the paragraph
/// analysis of T105 will supply them.
pub fn shape(font: &LoadedFont, text: &str, size: Emu, features: FeatureSet) -> ShapedRun {
    let font_ref = font.font_ref();
    let cache = shaper_cache();
    let mut guard = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.prepare(font);
    let units_per_em = font.units_per_em;
    let key = font.key();
    if let Some(data) = guard.map.get(&key) {
        return shape_with(data, &font_ref, key, text, size, features, units_per_em);
    }
    let data = ShaperData::new(&font_ref);
    shape_with(&data, &font_ref, key, text, size, features, units_per_em)
}

fn shape_with(
    data: &ShaperData,
    font_ref: &read_fonts::FontRef<'_>,
    key: FontKey,
    text: &str,
    size: Emu,
    features: FeatureSet,
    units_per_em: u16,
) -> ShapedRun {
    let shaper = data.shaper(font_ref).build();
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let feature_list = features.features();
    let output = shaper.shape(buffer, ShapeOptions::new().features(&feature_list));
    let mut pen: i64 = 0;
    let mut glyphs = Vec::with_capacity(output.len());
    let mut missing = Vec::new();
    for (info, position) in output.glyph_infos().iter().zip(output.glyph_positions()) {
        let start = to_emu(pen, size, units_per_em);
        let end = to_emu(pen + i64::from(position.x_advance), size, units_per_em);
        let glyph_id = u16::try_from(info.glyph_id).unwrap_or(0);
        let cluster = info.cluster as usize;
        if glyph_id == 0 && !missing.contains(&cluster) {
            missing.push(cluster);
        }
        glyphs.push(ShapedGlyph {
            glyph_id,
            cluster,
            x: start,
            advance: Emu::new(end.raw() - start.raw()),
            x_offset: to_emu(i64::from(position.x_offset), size, units_per_em),
            y_offset: to_emu(i64::from(position.y_offset), size, units_per_em),
        });
        pen += i64::from(position.x_advance);
    }
    ShapedRun {
        font: key,
        size,
        text: text.to_owned(),
        glyphs,
        width: to_emu(pen, size, units_per_em),
        missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_rounds_from_the_cumulative_pen() {
        // 1000 upem at 10.5 pt: one unit = 133.35 EMU, never accumulated.
        let size = Emu::new(Emu::PER_POINT * 21 / 2);
        let mut previous = Emu::new(0);
        let mut total = 0;
        for step in 1..=1000 {
            let x = to_emu(step, size, 1000);
            total += x.raw() - previous.raw();
            previous = x;
        }
        assert_eq!(total, to_emu(1000, size, 1000).raw());
        assert_eq!(total, size.raw());
        assert_eq!(to_emu(-1, size, 1000), Emu::new(-133));
        assert_eq!(to_emu(1, size, 1000), Emu::new(133));
    }

    #[test]
    fn features_are_always_explicit() {
        let tags: Vec<String> = FeatureSet::default()
            .features()
            .iter()
            .map(|feature| format!("{feature:?}"))
            .collect();
        assert_eq!(tags.len(), 4);
        let on = FeatureSet {
            kerning: true,
            standard_ligatures: true,
            contextual_alternates: true,
        };
        assert_ne!(
            format!("{:?}", on.features()),
            format!("{:?}", FeatureSet::default().features())
        );
    }
}
