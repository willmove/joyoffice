//! Replace missing whole shaping clusters before fitting, retaining actual face IDs.
use crate::{FeatureSet, LoadedFont, ShapedRun, shape};
use joyword_foundation::Emu;
pub(crate) fn apply(
    font: &LoadedFont,
    fallback: &LoadedFont,
    run: &mut ShapedRun,
    features: FeatureSet,
    size: impl Fn(usize) -> Emu,
    mut charge: impl FnMut(usize) -> Result<(), crate::LineError>,
) -> Result<(), crate::LineError> {
    if font.key() == fallback.key() || run.missing.is_empty() {
        return Ok(());
    }
    let mut glyphs = Vec::with_capacity(run.glyphs.len());
    let mut missing = Vec::new();
    let mut pen = 0i64;
    let mut at = 0;
    while at < run.glyphs.len() {
        let cluster = run.glyphs[at].cluster;
        let end = run.glyphs[at..]
            .iter()
            .position(|g| g.cluster != cluster)
            .map(|n| at + n)
            .unwrap_or(run.glyphs.len());
        let byte_end = run
            .glyphs
            .get(end)
            .map(|g| g.cluster)
            .unwrap_or(run.text.len());
        let replacement = if run.missing.contains(&cluster) && cluster <= byte_end {
            let text = run
                .text
                .get(cluster..byte_end)
                .ok_or(crate::LineError::InvalidShaping)?;
            charge(text.len())?;
            let shaped = shape(fallback, text, size(cluster), features);
            (shaped.missing.is_empty() && !shaped.glyphs.is_empty()).then_some(shaped)
        } else {
            None
        };
        if let Some(shaped) = replacement {
            for mut g in shaped.glyphs {
                g.fallback = true;
                run.fallback_font = Some(fallback.key());
                g.cluster += cluster;
                g.x = Emu::new(
                    g.x.raw()
                        .checked_add(pen)
                        .ok_or(crate::LineError::InvalidShaping)?,
                );
                glyphs.push(g);
            }
            pen = pen
                .checked_add(shaped.width.raw())
                .ok_or(crate::LineError::InvalidShaping)?;
        } else {
            let start_x = run.glyphs[at].x.raw();
            for g in &run.glyphs[at..end] {
                let mut g = *g;
                g.x = Emu::new(
                    pen.checked_add(g.x.raw() - start_x)
                        .ok_or(crate::LineError::InvalidShaping)?,
                );
                glyphs.push(g);
            }
            let advance: i64 = run.glyphs[at..end].iter().map(|g| g.advance.raw()).sum();
            pen = pen
                .checked_add(advance)
                .ok_or(crate::LineError::InvalidShaping)?;
            if run.missing.contains(&cluster) {
                missing.push(cluster);
            }
        }
        at = end;
    }
    run.glyphs = glyphs;
    run.width = Emu::new(pen);
    run.missing = missing;
    Ok(())
}
