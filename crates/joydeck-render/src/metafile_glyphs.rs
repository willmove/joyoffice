//! Paint EMF+ glyph indices directly from the named physical font.

use joydeck_fonts::FontDb;
use joydeck_metafile::{GlyphPlacement, IndexedGlyphs, Text};
use kurbo::{Affine, BezPath, Shape};
use skrifa::{GlyphId, MetadataProvider, instance::Size, raw::TableProvider};

pub fn draw(
    text: &Text,
    transform: Affine,
    family: &str,
    style: &str,
    indexed: &IndexedGlyphs,
    mut paint: impl FnMut(&BezPath),
) {
    if text.font.family.trim().is_empty() {
        return;
    }
    let db = FontDb::global();
    let resolved = db.document_match(family, style);
    let face = resolved.face;
    if resolved.substituted
        || !resolved.conflicts.is_empty()
        || face.italic != text.font.italic
        || (face.weight >= 600.0) != (text.font.weight >= 600)
    {
        return;
    }
    let Some(font) = face.skrifa() else { return };
    let size = Size::unscaled();
    let metrics = font.glyph_metrics(size, face.location());
    let vertical = font.vmtx().ok();
    let em = f64::from(-text.font.height);
    let scale = em / face.upem.max(1.0);
    if !scale.is_finite() || scale <= 0.0 || !transform.is_finite() {
        return;
    }
    let mut next = (0.0, 0.0);
    let mut elements = 0usize;
    for (index, &glyph) in indexed.ids.iter().enumerate() {
        let gid = u32::from(glyph);
        if gid >= metrics.glyph_count() {
            continue;
        }
        let at = match &indexed.placement {
            GlyphPlacement::Explicit(origins) => {
                let Some(&point) = origins.get(index) else {
                    return;
                };
                point
            }
            GlyphPlacement::Realized { vertical: true } => {
                let point = next;
                let height = vertical
                    .as_ref()
                    .and_then(|table| table.advance(GlyphId::new(gid)));
                next.1 += height.map_or(em, |value| f64::from(value) * scale);
                point
            }
            GlyphPlacement::Realized { vertical: false } => {
                let point = next;
                next.0 += face.advance(gid) * scale;
                point
            }
        };
        let outline = db.outline(&face, gid);
        let count = outline.elements().len();
        if count == 0 || count > 10_000 {
            continue;
        }
        elements += count;
        if elements > 250_000 {
            break;
        }
        let path = (transform * Affine::translate(at) * Affine::scale(scale)) * outline.as_ref();
        let bounds = path.bounding_box();
        if bounds.is_finite() && bounds.width() <= 1e6 && bounds.height() <= 1e6 {
            paint(&path);
        }
    }
}
