//! Latin theme-font references kept separately from the resolved font used by layout.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeFont {
    pub ascii: Option<String>,
    pub h_ansi: Option<String>,
    /// The imported/resolved value. Mismatched direct mutations suppress the binding.
    /// Explicit edits must use `CharProps::set_fixed_font`, including same-value edits.
    pub resolved: String,
}

impl ThemeFont {
    pub fn supported(slot: &str) -> bool {
        matches!(
            slot,
            "majorAscii" | "minorAscii" | "majorHAnsi" | "minorHAnsi"
        )
    }

    pub fn valid_for(&self, font: Option<&str>) -> bool {
        font == Some(self.resolved.as_str())
            && (self.ascii.is_some() || self.h_ansi.is_some())
            && self
                .ascii
                .iter()
                .chain(&self.h_ansi)
                .all(|s| Self::supported(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::CharProps;

    fn bound() -> CharProps {
        CharProps {
            font: Some("Old".into()),
            theme_font: Some(ThemeFont {
                ascii: Some("majorAscii".into()),
                h_ansi: Some("majorHAnsi".into()),
                resolved: "Old".into(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn explicit_same_font_detaches_and_survives_minus_overlay() {
        let base = bound();
        let fixed = CharProps {
            font: base.font.clone(),
            ..Default::default()
        };
        let delta = fixed.clone().minus(&base);
        assert_eq!(delta.font, fixed.font);
        assert_eq!(base.clone().overlaid(&delta), fixed);
        let derived = base.clone().overlaid(&CharProps {
            bold: Some(true),
            ..Default::default()
        });
        assert_eq!(derived.theme_font, base.theme_font);
        let mut same = base.clone();
        same.set_fixed_font(base.font.clone());
        assert!(same.theme_font.is_none());
        assert!(base.clone().minus(&base).theme_font.is_none());
    }

    #[test]
    fn bindings_roundtrip_and_older_snapshots_default() -> Result<(), serde_json::Error> {
        let value = bound();
        let bytes = serde_json::to_vec(&value)?;
        assert_eq!(serde_json::from_slice::<CharProps>(&bytes)?, value);
        let old: CharProps = serde_json::from_str(r#"{"font":"Old"}"#)?;
        assert!(old.theme_font.is_none());
        let mut changed = value;
        assert!(changed.refresh_theme_font("New", "Body"));
        assert_eq!(changed.font.as_deref(), Some("New"));
        changed.font = Some("Direct API mutation".into());
        assert!(!changed.refresh_theme_font("Other", "Other"));
        Ok(())
    }
    #[test]
    fn cross_document_import_keeps_source_font_fixed() {
        let mut source = crate::Document::new();
        source.body = vec![crate::para_block(crate::Paragraph::with_text(
            "source",
            bound(),
        ))]
        .into();
        let mut destination = crate::Document::new();
        destination.settings.major_font = "Destination".into();
        let fragment = destination.adopt(source);
        let paragraph = fragment.blocks[0].as_para();
        assert!(
            paragraph.is_some_and(|p| p.runs[0].props.font.as_deref() == Some("Old")
                && p.runs[0].props.theme_font.is_none())
        );
    }
}
