//! WordprocessingML run color references, separate from layout's resolved RGB.
use super::{Rgb, TextColor};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColor {
    pub name: String,
    /// Physical palette index after the imported clrSchemeMapping.
    pub slot: usize,
    /// Keep attribute spellings (including unsupported values) for serialization.
    pub tint: Option<String>,
    pub shade: Option<String>,
    pub resolved: Rgb,
}
impl ThemeColor {
    pub fn slot(name: &str, map: Option<&[usize; 12]>) -> Option<usize> {
        let logical = |i| map.map_or(i, |m| m[i]);
        Some(match name {
            "dark1" => 0,
            "light1" => 1,
            "dark2" => 2,
            "light2" => 3,
            "text1" => logical(0),
            "background1" => logical(1),
            "text2" => logical(2),
            "background2" => logical(3),
            "accent1" => logical(4),
            "accent2" => logical(5),
            "accent3" => logical(6),
            "accent4" => logical(7),
            "accent5" => logical(8),
            "accent6" => logical(9),
            "hyperlink" => logical(10),
            "followedHyperlink" => logical(11),
            _ => return None,
        })
    }
    pub fn valid_for(&self, color: Option<TextColor>) -> bool {
        color == Some(TextColor::Rgb(self.resolved))
            && self.slot < 12
            && Self::slot(&self.name, None).is_some()
    }
    pub fn resolve(&self, colors: &[Rgb]) -> Option<Rgb> {
        let mut color = *colors.get(self.slot)?;
        let byte = |s: &str| {
            u8::from_str_radix(s.trim(), 16)
                .ok()
                .map(|v| f64::from(v) / 255.0)
        };
        // Keep the existing import interpretation; retaining both attributes avoids
        // destructive normalization of documents with combined modifiers.
        if let Some(t) = self.tint.as_deref().and_then(byte) {
            color = color.with_lum(|l| l * t + (1.0 - t));
        }
        if let Some(s) = self.shade.as_deref().and_then(byte) {
            color = color.with_lum(|l| l * s);
        }
        Some(color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, Paragraph, para_block, props::CharProps};
    fn bound() -> CharProps {
        CharProps {
            color: Some(TextColor::Rgb(Rgb(255, 0, 0))),
            theme_color: Some(ThemeColor {
                name: "accent1".into(),
                slot: 4,
                tint: None,
                shade: None,
                resolved: Rgb(255, 0, 0),
            }),
            ..Default::default()
        }
    }
    #[test]
    fn overlay_minus_snapshot_and_explicit_same_value() {
        let base = bound();
        let mut fixed = base.clone();
        fixed.set_fixed_color(base.color);
        let delta = fixed.clone().minus(&base);
        assert_eq!(delta.color, base.color);
        assert_eq!(base.clone().overlaid(&delta), fixed);
        assert!(base.clone().minus(&base).theme_color.is_none());
        assert_eq!(
            serde_json::from_slice::<CharProps>(&serde_json::to_vec(&base).unwrap()).unwrap(),
            base
        );
        assert!(
            serde_json::from_str::<CharProps>("{}")
                .unwrap()
                .theme_color
                .is_none()
        );
        let mut stale = base;
        stale.color = Some(TextColor::Auto);
        assert!(!stale.refresh_theme_color(&crate::THEME_COLORS));
    }
    #[test]
    fn source_color_is_frozen_on_cross_document_adoption() {
        let mut source = Document::new();
        source.body = vec![para_block(Paragraph::with_text("source", bound()))].into();
        let mut dest = Document::new();
        let fragment = dest.adopt(source);
        let props = &fragment.blocks[0].as_para().unwrap().runs[0].props;
        assert_eq!(props.color, bound().color);
        assert!(props.theme_color.is_none());
    }
    #[test]
    fn modifier_endpoints_and_shade() {
        let mut b = bound().theme_color.unwrap();
        let mut colors = crate::THEME_COLORS;
        colors[4] = Rgb(255, 0, 0);
        for (tint, shade, expected) in [
            (Some("00"), None, Rgb::WHITE),
            (Some("FF"), None, Rgb(255, 0, 0)),
            (None, Some("00"), Rgb::BLACK),
            (None, Some("FF"), Rgb(255, 0, 0)),
            (None, Some("BF"), Rgb(191, 0, 0)),
        ] {
            b.tint = tint.map(str::to_string);
            b.shade = shade.map(str::to_string);
            assert_eq!(b.resolve(&colors), Some(expected));
        }
    }
}
