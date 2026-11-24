//! Preset fields accepted by the JoyDeck v1 migration bridge.
use crate::document::{Deck, Object, Result};
use serde::{Deserialize, Serialize};

mod defaults;
pub use defaults::defaults;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_fill: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjustments: Vec<f64>,
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
