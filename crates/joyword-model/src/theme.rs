//! DrawingML theme scheme used while resolving styles (ECMA-376 5th edition
//! Part 1 §14.2.7 theme, §20.1.4.1.14 font scheme, §20.1.6.2 colour scheme).
//!
//! Only the font scheme and the colour scheme are read. The theme part itself
//! stays in the package; this struct is the subset style resolution needs.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::props::{Color, FontRef, FontSlots, ThemeFont};

/// `a:clrScheme` slots. `w:themeColor` uses the longer ST_ThemeColor names
/// (ECMA-376 §17.18.97); the theme part uses the short scheme names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeColorSlot {
    Dark1,
    Light1,
    Dark2,
    Light2,
    Accent1,
    Accent2,
    Accent3,
    Accent4,
    Accent5,
    Accent6,
    Hyperlink,
    FollowedHyperlink,
}

impl ThemeColorSlot {
    /// `w:themeColor` attribute value.
    pub fn from_word_name(name: &str) -> Option<Self> {
        Some(match name {
            "dark1" => Self::Dark1,
            "light1" => Self::Light1,
            "dark2" => Self::Dark2,
            "light2" => Self::Light2,
            "accent1" => Self::Accent1,
            "accent2" => Self::Accent2,
            "accent3" => Self::Accent3,
            "accent4" => Self::Accent4,
            "accent5" => Self::Accent5,
            "accent6" => Self::Accent6,
            "hyperlink" => Self::Hyperlink,
            "followedHyperlink" => Self::FollowedHyperlink,
            _ => return None,
        })
    }

    /// Child element of `a:clrScheme`.
    pub fn from_scheme_name(name: &str) -> Option<Self> {
        Some(match name {
            "dk1" => Self::Dark1,
            "lt1" => Self::Light1,
            "dk2" => Self::Dark2,
            "lt2" => Self::Light2,
            "accent1" => Self::Accent1,
            "accent2" => Self::Accent2,
            "accent3" => Self::Accent3,
            "accent4" => Self::Accent4,
            "accent5" => Self::Accent5,
            "accent6" => Self::Accent6,
            "hlink" => Self::Hyperlink,
            "folHlink" => Self::FollowedHyperlink,
            _ => return None,
        })
    }

    pub fn word_name(self) -> &'static str {
        match self {
            Self::Dark1 => "dark1",
            Self::Light1 => "light1",
            Self::Dark2 => "dark2",
            Self::Light2 => "light2",
            Self::Accent1 => "accent1",
            Self::Accent2 => "accent2",
            Self::Accent3 => "accent3",
            Self::Accent4 => "accent4",
            Self::Accent5 => "accent5",
            Self::Accent6 => "accent6",
            Self::Hyperlink => "hyperlink",
            Self::FollowedHyperlink => "followedHyperlink",
        }
    }
}

/// Major and minor typefaces. An empty `typeface` is stored as absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThemeFonts {
    pub major_latin: Option<Arc<str>>,
    pub major_east_asia: Option<Arc<str>>,
    pub major_complex: Option<Arc<str>>,
    pub minor_latin: Option<Arc<str>>,
    pub minor_east_asia: Option<Arc<str>>,
    pub minor_complex: Option<Arc<str>>,
    /// Slots the scheme declares with an empty typeface and no supplemental
    /// `a:font` for the document's theme language (ECMA-376 Part 1
    /// §17.15.1.88, §20.1.4.1.16). They name no face, so a reference to one
    /// resolves to no font instead of staying unresolved.
    pub unnamed: Vec<ThemeFont>,
}

impl ThemeFonts {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    fn name(&self, slot: ThemeFont) -> Option<&str> {
        match slot {
            ThemeFont::MajorAscii | ThemeFont::MajorHAnsi => self.major_latin.as_deref(),
            ThemeFont::MajorEastAsia => self.major_east_asia.as_deref(),
            ThemeFont::MajorBidi => self.major_complex.as_deref(),
            ThemeFont::MinorAscii | ThemeFont::MinorHAnsi => self.minor_latin.as_deref(),
            ThemeFont::MinorEastAsia => self.minor_east_asia.as_deref(),
            ThemeFont::MinorBidi => self.minor_complex.as_deref(),
        }
    }
}

/// Font scheme plus sRGB entries of the colour scheme. Tint and shade are
/// applied by [`ThemeScheme::concretize_color`] using the linear sRGB
/// formulas in ECMA-376 §17.3.2.6.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThemeScheme {
    pub fonts: ThemeFonts,
    pub colors: BTreeMap<ThemeColorSlot, (u8, u8, u8)>,
}

impl ThemeScheme {
    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty() && self.colors.is_empty()
    }

    /// Replace theme font slots whose scheme entry exists. A slot listed in
    /// [`ThemeFonts::unnamed`] becomes `None`. Others stay
    /// [`FontRef::Theme`] so a caller can refuse them instead of guessing.
    pub fn concretize_fonts(&self, fonts: &mut FontSlots) -> Vec<ThemeFont> {
        let mut missing = Vec::new();
        for slot in [
            &mut fonts.ascii,
            &mut fonts.h_ansi,
            &mut fonts.east_asia,
            &mut fonts.complex_script,
        ] {
            let Some(FontRef::Theme(theme)) = slot else {
                continue;
            };
            let theme = *theme;
            match self.fonts.name(theme).filter(|name| !name.is_empty()) {
                Some(name) => *slot = Some(FontRef::Named(Arc::from(name))),
                None if self.fonts.unnamed.contains(&theme) => *slot = None,
                None => missing.push(theme),
            }
        }
        missing
    }

    pub fn concretize_color(&self, color: &mut Option<Color>) -> Option<ThemeColorSlot> {
        let Some(Color::Theme { slot, tint, shade }) = color.as_ref().copied() else {
            return None;
        };
        match self.colors.get(&slot).copied() {
            Some((red, green, blue)) => {
                let (red, green, blue) = apply_tint_shade(red, green, blue, tint, shade);
                *color = Some(Color::Rgb(red, green, blue));
                None
            }
            None => Some(slot),
        }
    }
}

/// ECMA-376 §17.3.2.6 linear sRGB modifiers.
///
/// The stored byte is `percent * 255 / 100` (the examples use `66` for a
/// 40% shade and `99` for a 60% tint). Shade is `C' = C * byte / 255`.
/// Tint is `C' = C * byte / 255 + (255 - byte)`. Each product rounds half
/// up. The specification does not say what happens when both attributes
/// are present; this applies shade first, then tint.
pub fn apply_tint_shade(
    red: u8,
    green: u8,
    blue: u8,
    tint: Option<u8>,
    shade: Option<u8>,
) -> (u8, u8, u8) {
    let mut channels = [red, green, blue];
    if let Some(factor) = shade {
        for channel in &mut channels {
            *channel = scale(*channel, factor);
        }
    }
    if let Some(factor) = tint {
        for channel in &mut channels {
            let scaled = u16::from(scale(*channel, factor));
            *channel = u8::try_from(scaled + u16::from(255 - factor)).unwrap_or(255);
        }
    }
    (channels[0], channels[1], channels[2])
}

fn scale(channel: u8, factor: u8) -> u8 {
    let product = u16::from(channel) * u16::from(factor) + 127;
    (product / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_and_shade_follow_the_linear_examples() {
        // 60% tint is byte 0x99. C = 80 becomes 150.
        assert_eq!(
            apply_tint_shade(80, 80, 80, Some(0x99), None),
            (150, 150, 150)
        );
        // 50% shade. Byte 128 and the rounded 127 both take 150 to 75.
        assert_eq!(apply_tint_shade(150, 0, 0, None, Some(128)), (75, 0, 0));
        assert_eq!(apply_tint_shade(150, 0, 0, None, Some(127)), (75, 0, 0));
        // 40% shade is byte 0x66. 255 * 102 / 255 = 102.
        assert_eq!(apply_tint_shade(255, 0, 0, None, Some(0x66)), (102, 0, 0));
        // Both attributes: shade, then tint. Not stated by the specification.
        // 80 shaded by 128 is 40; tint 0x99 then lifts 40 to 126 and 0 to 102.
        assert_eq!(
            apply_tint_shade(80, 0, 0, Some(0x99), Some(128)),
            (126, 102, 102)
        );
    }
}
