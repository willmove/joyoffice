//! Bounded JoyDeck adapter for DeckCraft's MIT-licensed preset geometry.
//! Source revision and retained licence: licenses/joydeck/deckcraft-geometry/.
use crate::document::{Deck, Object, Result};
use kurbo::{ParamCurveNearest, Point, Shape};
use serde::{Deserialize, Serialize};

// Keep the upstream geometry and its regression tests together. Gallery metadata
// and connection/adjust handles beyond the current UI remain available internally.
#[allow(dead_code)]
mod vendor;
pub use vendor::{FillMode, Geometry};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_fill: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjustments: Vec<f64>,
}

pub fn defaults(name: &str) -> Option<&'static [f64]> {
    vendor::info(name).map(|p| p.defaults)
}

pub fn names() -> impl Iterator<Item = &'static str> {
    vendor::CATALOG.iter().map(|p| p.name)
}

impl Preset {
    pub fn validate(&self) -> Result<()> {
        let values = defaults(&self.name).ok_or("不支持的预设形状")?;
        if self.adjustments.len() > values.len()
            || self
                .adjustments
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000_000. || v.fract() != 0.)
        {
            return Err("预设形状调整参数无效或超过预算".into());
        }
        Ok(())
    }
}

pub fn validate(o: &Object) -> Result<()> {
    match (&o.preset, o.kind.as_str()) {
        (Some(p), "preset") => p.validate(),
        (None, "preset") | (Some(_), _) => Err("预设形状模型与对象类型不匹配".into()),
        (None, _) => Ok(()),
    }
}

pub fn geometry(o: &Object) -> Result<Geometry> {
    validate(o)?;
    let p = o.preset.as_ref().ok_or("缺少预设形状模型")?;
    vendor::build(
        &p.name,
        f64::from(o.width),
        f64::from(o.height),
        &p.adjustments,
    )
    .ok_or_else(|| "预设形状几何无效".into())
}

pub fn object(name: &str) -> Result<Object> {
    let mut o = crate::identity::object("preset");
    o.preset = Some(Preset {
        name: name.into(),
        no_fill: false,
        adjustments: vec![],
    });
    validate(&o)?;
    if geometry(&o)?.is_open() {
        o.stroke_width = 2.;
        o.stroke = o.fill.clone();
    }
    Ok(o)
}

/// Filled and open paths share exactly the outlines sent to the scene renderer.
pub fn hit(o: &Object, x: f32, y: f32) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    let Ok(g) = geometry(o) else { return false };
    let point = Point::new(f64::from(x), f64::from(y));
    let tolerance = f64::from(4. + o.stroke_width / 2.);
    g.paths.iter().any(|p| {
        let winding = p.path.winding(point);
        (!o.preset.as_ref().is_some_and(|p| p.no_fill)
            && p.fill != FillMode::None
            && if p.even_odd {
                winding % 2 != 0
            } else {
                winding != 0
            })
            || (p.stroke
                && o.stroke_width > 0.
                && p.path
                    .segments()
                    .any(|s| s.nearest(point, 0.01).distance_sq <= tolerance * tolerance))
    })
}

pub fn has_presets(deck: &Deck) -> bool {
    let contains = |d: &Deck| {
        d.slides
            .iter()
            .flat_map(|s| &s.objects)
            .any(|o| o.preset.is_some())
    };
    if contains(deck) {
        return true;
    }
    // Source baselines are also typed models read by older applications. Removing
    // the last visible preset must not downgrade a file that still retains one.
    deck.source.as_ref().is_some_and(|s| {
        s.baseline.len() > crate::document::MAX_PART
            || serde_json::from_slice::<Deck>(&s.baseline)
                .map(|d| contains(&d))
                .unwrap_or(true)
    })
}

/// Adjust buttons use the same bounded handle projection as the source geometry.
pub fn adjustment_handles(o: &Object) -> Result<Vec<(usize, f64, f64)>> {
    Ok(geometry(o)?
        .handles
        .iter()
        .map(|h| (h.index, h.min, h.max))
        .collect())
}

pub fn adjust(o: &Object, index: usize, direction: f64) -> Result<Object> {
    let (i, min, max) = adjustment_handles(o)?
        .into_iter()
        .find(|(i, _, _)| *i == index)
        .ok_or("此参数没有调整手柄")?;
    if !direction.is_finite() || direction.abs() != 1. {
        return Err("调整方向无效".into());
    }
    let mut out = o.clone();
    let p = out.preset.as_mut().ok_or("缺少预设形状")?;
    let values = defaults(&p.name).ok_or("不支持的预设形状")?;
    while p.adjustments.len() < values.len() {
        p.adjustments.push(values[p.adjustments.len()]);
    }
    let value = p.adjustments.get_mut(i).ok_or("调整参数不存在")?;
    *value = (*value + direction * (max - min).abs() / 20.)
        .clamp(min.min(max), min.max(max))
        .round();
    validate(&out)?;
    Ok(out)
}

#[cfg(test)]
#[path = "preset/tests.rs"]
mod tests;
