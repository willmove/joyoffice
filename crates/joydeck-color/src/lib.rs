//! sRGB colors, theme palette roles and DrawingML color transforms.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

mod palette;
mod srgb;
mod transform;

pub use palette::ColorMapping;
pub use srgb::Rgba;
pub use transform::{apply, theme_variants};

/// The twelve theme colour slots, plus the mapped aliases (`bg1`, `tx1`…) and `phClr` (the
/// placeholder colour a style matrix entry is applied with).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SchemeSlot {
    Dk1,
    Lt1,
    Dk2,
    Lt2,
    Accent1,
    Accent2,
    Accent3,
    Accent4,
    Accent5,
    Accent6,
    Hlink,
    FolHlink,
    Bg1,
    Tx1,
    Bg2,
    Tx2,
    PhClr,
}

impl SchemeSlot {
    pub const THEME: [SchemeSlot; 12] = [
        SchemeSlot::Dk1,
        SchemeSlot::Lt1,
        SchemeSlot::Dk2,
        SchemeSlot::Lt2,
        SchemeSlot::Accent1,
        SchemeSlot::Accent2,
        SchemeSlot::Accent3,
        SchemeSlot::Accent4,
        SchemeSlot::Accent5,
        SchemeSlot::Accent6,
        SchemeSlot::Hlink,
        SchemeSlot::FolHlink,
    ];
    pub fn xml_name(self) -> &'static str {
        match self {
            SchemeSlot::Dk1 => "dk1",
            SchemeSlot::Lt1 => "lt1",
            SchemeSlot::Dk2 => "dk2",
            SchemeSlot::Lt2 => "lt2",
            SchemeSlot::Accent1 => "accent1",
            SchemeSlot::Accent2 => "accent2",
            SchemeSlot::Accent3 => "accent3",
            SchemeSlot::Accent4 => "accent4",
            SchemeSlot::Accent5 => "accent5",
            SchemeSlot::Accent6 => "accent6",
            SchemeSlot::Hlink => "hlink",
            SchemeSlot::FolHlink => "folHlink",
            SchemeSlot::Bg1 => "bg1",
            SchemeSlot::Tx1 => "tx1",
            SchemeSlot::Bg2 => "bg2",
            SchemeSlot::Tx2 => "tx2",
            SchemeSlot::PhClr => "phClr",
        }
    }
    pub fn from_xml(s: &str) -> Option<Self> {
        Some(match s {
            "dk1" => SchemeSlot::Dk1,
            "lt1" => SchemeSlot::Lt1,
            "dk2" => SchemeSlot::Dk2,
            "lt2" => SchemeSlot::Lt2,
            "accent1" => SchemeSlot::Accent1,
            "accent2" => SchemeSlot::Accent2,
            "accent3" => SchemeSlot::Accent3,
            "accent4" => SchemeSlot::Accent4,
            "accent5" => SchemeSlot::Accent5,
            "accent6" => SchemeSlot::Accent6,
            "hlink" => SchemeSlot::Hlink,
            "folHlink" => SchemeSlot::FolHlink,
            "bg1" => SchemeSlot::Bg1,
            "tx1" => SchemeSlot::Tx1,
            "bg2" => SchemeSlot::Bg2,
            "tx2" => SchemeSlot::Tx2,
            "phClr" => SchemeSlot::PhClr,
            _ => return None,
        })
    }
    pub fn label(self) -> &'static str {
        match self {
            SchemeSlot::Dk1 | SchemeSlot::Tx1 => "Text/Background - Dark 1",
            SchemeSlot::Lt1 | SchemeSlot::Bg1 => "Text/Background - Light 1",
            SchemeSlot::Dk2 | SchemeSlot::Tx2 => "Text/Background - Dark 2",
            SchemeSlot::Lt2 | SchemeSlot::Bg2 => "Text/Background - Light 2",
            SchemeSlot::Accent1 => "Accent 1",
            SchemeSlot::Accent2 => "Accent 2",
            SchemeSlot::Accent3 => "Accent 3",
            SchemeSlot::Accent4 => "Accent 4",
            SchemeSlot::Accent5 => "Accent 5",
            SchemeSlot::Accent6 => "Accent 6",
            SchemeSlot::Hlink => "Hyperlink",
            SchemeSlot::FolHlink => "Followed Hyperlink",
            SchemeSlot::PhClr => "Placeholder colour",
        }
    }
}

/// A colour scheme: the twelve theme colours.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorScheme {
    pub name: String,
    pub colors: [Rgba; 12],
}

/// One colour transform, in file units (percentages ×1000, angles ×60000), applied in order.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "op", content = "val")]
pub enum ColorTransform {
    Tint(i32),
    Shade(i32),
    LumMod(i32),
    LumOff(i32),
    SatMod(i32),
    SatOff(i32),
    HueMod(i32),
    HueOff(i32),
    Alpha(i32),
    AlphaMod(i32),
    AlphaOff(i32),
    Comp,
    Inv,
    Gray,
    RedMod(i32),
    GreenMod(i32),
    BlueMod(i32),
}

impl ColorTransform {
    pub fn xml_name(&self) -> &'static str {
        match self {
            ColorTransform::Tint(_) => "tint",
            ColorTransform::Shade(_) => "shade",
            ColorTransform::LumMod(_) => "lumMod",
            ColorTransform::LumOff(_) => "lumOff",
            ColorTransform::SatMod(_) => "satMod",
            ColorTransform::SatOff(_) => "satOff",
            ColorTransform::HueMod(_) => "hueMod",
            ColorTransform::HueOff(_) => "hueOff",
            ColorTransform::Alpha(_) => "alpha",
            ColorTransform::AlphaMod(_) => "alphaMod",
            ColorTransform::AlphaOff(_) => "alphaOff",
            ColorTransform::Comp => "comp",
            ColorTransform::Inv => "inv",
            ColorTransform::Gray => "gray",
            ColorTransform::RedMod(_) => "redMod",
            ColorTransform::GreenMod(_) => "greenMod",
            ColorTransform::BlueMod(_) => "blueMod",
        }
    }
    pub fn value(&self) -> Option<i32> {
        match *self {
            ColorTransform::Tint(v)
            | ColorTransform::Shade(v)
            | ColorTransform::LumMod(v)
            | ColorTransform::LumOff(v)
            | ColorTransform::SatMod(v)
            | ColorTransform::SatOff(v)
            | ColorTransform::HueMod(v)
            | ColorTransform::HueOff(v)
            | ColorTransform::Alpha(v)
            | ColorTransform::AlphaMod(v)
            | ColorTransform::AlphaOff(v)
            | ColorTransform::RedMod(v)
            | ColorTransform::GreenMod(v)
            | ColorTransform::BlueMod(v) => Some(v),
            _ => None,
        }
    }
    pub fn from_xml(name: &str, val: Option<i32>) -> Option<Self> {
        let v = val.unwrap_or(0);
        Some(match name {
            "tint" => ColorTransform::Tint(v),
            "shade" => ColorTransform::Shade(v),
            "lumMod" => ColorTransform::LumMod(v),
            "lumOff" => ColorTransform::LumOff(v),
            "satMod" => ColorTransform::SatMod(v),
            "satOff" => ColorTransform::SatOff(v),
            "hueMod" => ColorTransform::HueMod(v),
            "hueOff" => ColorTransform::HueOff(v),
            "alpha" => ColorTransform::Alpha(v),
            "alphaMod" => ColorTransform::AlphaMod(v),
            "alphaOff" => ColorTransform::AlphaOff(v),
            "comp" => ColorTransform::Comp,
            "inv" => ColorTransform::Inv,
            "gray" => ColorTransform::Gray,
            "redMod" => ColorTransform::RedMod(v),
            "greenMod" => ColorTransform::GreenMod(v),
            "blueMod" => ColorTransform::BlueMod(v),
            _ => return None,
        })
    }
}

/// Standard colours row of the colour picker.
pub const STANDARD_COLORS: [(Rgba, &str); 10] = [
    (Rgba::rgb(0xC0, 0x00, 0x00), "Dark Red"),
    (Rgba::rgb(0xFF, 0x00, 0x00), "Red"),
    (Rgba::rgb(0xFF, 0xC0, 0x00), "Orange"),
    (Rgba::rgb(0xFF, 0xFF, 0x00), "Yellow"),
    (Rgba::rgb(0x92, 0xD0, 0x50), "Light Green"),
    (Rgba::rgb(0x00, 0xB0, 0x50), "Green"),
    (Rgba::rgb(0x00, 0xB0, 0xF0), "Light Blue"),
    (Rgba::rgb(0x00, 0x70, 0xC0), "Blue"),
    (Rgba::rgb(0x00, 0x20, 0x60), "Dark Blue"),
    (Rgba::rgb(0x70, 0x30, 0xA0), "Purple"),
];

/// A few CSS-like preset colour names (`prstClr`).
pub fn preset(name: &str) -> Option<Rgba> {
    let c = match name {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "lime" => (0, 255, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" | "aqua" => (0, 255, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "maroon" => (128, 0, 0),
        "navy" => (0, 0, 128),
        "olive" => (128, 128, 0),
        "purple" => (128, 0, 128),
        "teal" => (0, 128, 128),
        "orange" => (255, 165, 0),
        "pink" => (255, 192, 203),
        "brown" => (165, 42, 42),
        "gold" => (255, 215, 0),
        "ltGray" | "lightGray" => (211, 211, 211),
        "dkGray" | "darkGray" => (169, 169, 169),
        _ => return None,
    };
    Some(Rgba::rgb(c.0, c.1, c.2))
}

/// Every preset colour name a file may write (`ST_PresetColorVal`, with the later editions' aliases).
pub const PRESET_NAMES: [&str; 190] = [
    "aliceBlue",
    "antiqueWhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedAlmond",
    "blue",
    "blueViolet",
    "brown",
    "burlyWood",
    "cadetBlue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerBlue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkBlue",
    "darkCyan",
    "darkGoldenrod",
    "darkGray",
    "darkGrey",
    "darkGreen",
    "darkKhaki",
    "darkMagenta",
    "darkOliveGreen",
    "darkOrange",
    "darkOrchid",
    "darkRed",
    "darkSalmon",
    "darkSeaGreen",
    "darkSlateBlue",
    "darkSlateGray",
    "darkSlateGrey",
    "darkTurquoise",
    "darkViolet",
    "dkBlue",
    "dkCyan",
    "dkGoldenrod",
    "dkGray",
    "dkGrey",
    "dkGreen",
    "dkKhaki",
    "dkMagenta",
    "dkOliveGreen",
    "dkOrange",
    "dkOrchid",
    "dkRed",
    "dkSalmon",
    "dkSeaGreen",
    "dkSlateBlue",
    "dkSlateGray",
    "dkSlateGrey",
    "dkTurquoise",
    "dkViolet",
    "deepPink",
    "deepSkyBlue",
    "dimGray",
    "dimGrey",
    "dodgerBlue",
    "firebrick",
    "floralWhite",
    "forestGreen",
    "fuchsia",
    "gainsboro",
    "ghostWhite",
    "gold",
    "goldenrod",
    "gray",
    "grey",
    "green",
    "greenYellow",
    "honeydew",
    "hotPink",
    "indianRed",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderBlush",
    "lawnGreen",
    "lemonChiffon",
    "lightBlue",
    "lightCoral",
    "lightCyan",
    "lightGoldenrodYellow",
    "lightGray",
    "lightGrey",
    "lightGreen",
    "lightPink",
    "lightSalmon",
    "lightSeaGreen",
    "lightSkyBlue",
    "lightSlateGray",
    "lightSlateGrey",
    "lightSteelBlue",
    "lightYellow",
    "ltBlue",
    "ltCoral",
    "ltCyan",
    "ltGoldenrodYellow",
    "ltGray",
    "ltGrey",
    "ltGreen",
    "ltPink",
    "ltSalmon",
    "ltSeaGreen",
    "ltSkyBlue",
    "ltSlateGray",
    "ltSlateGrey",
    "ltSteelBlue",
    "ltYellow",
    "lime",
    "limeGreen",
    "linen",
    "magenta",
    "maroon",
    "medAquamarine",
    "medBlue",
    "medOrchid",
    "medPurple",
    "medSeaGreen",
    "medSlateBlue",
    "medSpringGreen",
    "medTurquoise",
    "medVioletRed",
    "mediumAquamarine",
    "mediumBlue",
    "mediumOrchid",
    "mediumPurple",
    "mediumSeaGreen",
    "mediumSlateBlue",
    "mediumSpringGreen",
    "mediumTurquoise",
    "mediumVioletRed",
    "midnightBlue",
    "mintCream",
    "mistyRose",
    "moccasin",
    "navajoWhite",
    "navy",
    "oldLace",
    "olive",
    "oliveDrab",
    "orange",
    "orangeRed",
    "orchid",
    "paleGoldenrod",
    "paleGreen",
    "paleTurquoise",
    "paleVioletRed",
    "papayaWhip",
    "peachPuff",
    "peru",
    "pink",
    "plum",
    "powderBlue",
    "purple",
    "red",
    "rosyBrown",
    "royalBlue",
    "saddleBrown",
    "salmon",
    "sandyBrown",
    "seaGreen",
    "seaShell",
    "sienna",
    "silver",
    "skyBlue",
    "slateBlue",
    "slateGray",
    "slateGrey",
    "snow",
    "springGreen",
    "steelBlue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whiteSmoke",
    "yellow",
    "yellowGreen",
];

/// Every system colour name a file may write (`ST_SystemColorVal`).
pub const SYSTEM_NAMES: [&str; 30] = [
    "scrollBar",
    "background",
    "activeCaption",
    "inactiveCaption",
    "menu",
    "window",
    "windowFrame",
    "menuText",
    "windowText",
    "captionText",
    "activeBorder",
    "inactiveBorder",
    "appWorkspace",
    "highlight",
    "highlightText",
    "btnFace",
    "btnShadow",
    "grayText",
    "btnText",
    "inactiveCaptionText",
    "btnHighlight",
    "3dDkShadow",
    "3dLight",
    "infoText",
    "infoBk",
    "hotLight",
    "gradientActiveCaption",
    "gradientInactiveCaption",
    "menuHighlight",
    "menuBar",
];

/// Whether `name` is a preset colour name a file may write.
pub fn is_preset_name(name: &str) -> bool {
    PRESET_NAMES.contains(&name)
}

/// Whether `name` is a system colour name a file may write.
pub fn is_system_name(name: &str) -> bool {
    SYSTEM_NAMES.contains(&name)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test fixtures use explicit failure assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn from_hex_rejects_multiple_hashes() {
        assert_eq!(Rgba::from_hex("##FF0000"), None);
        assert_eq!(Rgba::from_hex("####FF0000"), None);
        assert_eq!(Rgba::from_hex("#FF0000"), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(Rgba::from_hex("FF0000"), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(
            Rgba::from_hex("#FF000080"),
            Some(Rgba::rgba(255, 0, 0, 128))
        );
    }

    #[test]
    fn from_hex_rejects_signs() {
        assert_eq!(Rgba::from_hex("+12345"), None);
        assert_eq!(Rgba::from_hex("#+1+2+3"), None);
        assert_eq!(Rgba::from_hex("+1+2+3+4"), None);
        assert_eq!(Rgba::from_hex("+12"), None);
    }

    #[test]
    fn hex_roundtrip() {
        assert_eq!(Rgba::from_hex("#156082"), Some(Rgba::rgb(0x15, 0x60, 0x82)));
        assert_eq!(Rgba::rgb(0x15, 0x60, 0x82).hex(), "156082");
        assert_eq!(Rgba::from_hex("fff"), Some(Rgba::WHITE));
        assert_eq!(Rgba::from_hex("zz0000"), None);
        assert_eq!(Rgba::from_hex("ééé"), None);
        assert_eq!(Rgba::from_hex(""), None);
    }

    #[test]
    fn hsl_roundtrip() {
        for c in [
            Rgba::rgb(10, 200, 30),
            Rgba::rgb(255, 0, 0),
            Rgba::rgb(128, 128, 128),
            Rgba::rgb(21, 96, 130),
        ] {
            let (h, s, l) = c.to_hsl();
            let d = Rgba::from_hsl(h, s, l, 255);
            assert!(
                (d.r as i32 - c.r as i32).abs() <= 1
                    && (d.g as i32 - c.g as i32).abs() <= 1
                    && (d.b as i32 - c.b as i32).abs() <= 1,
                "{c:?} {d:?}"
            );
        }
    }

    #[test]
    fn transforms() {
        let c = Rgba::rgb(0x40, 0x80, 0xC0);
        assert_eq!(apply(c, &[ColorTransform::Alpha(50000)]).a, 128);
        let lighter = apply(
            c,
            &[ColorTransform::LumMod(20000), ColorTransform::LumOff(80000)],
        );
        assert!(lighter.luminance() > c.luminance());
        let darker = apply(c, &[ColorTransform::LumMod(50000)]);
        assert!(darker.luminance() < c.luminance());
        assert_eq!(apply(Rgba::WHITE, &[ColorTransform::Shade(0)]), Rgba::BLACK);
        assert_eq!(apply(Rgba::BLACK, &[ColorTransform::Tint(0)]), Rgba::WHITE);
        assert_eq!(
            apply(c, &[ColorTransform::Inv]),
            Rgba::rgb(0xF9, 0xE5, 0xB7)
        );
        // hostile values never panic
        let _ = apply(
            c,
            &[
                ColorTransform::LumMod(i32::MAX),
                ColorTransform::HueOff(i32::MIN),
                ColorTransform::AlphaMod(i32::MIN),
            ],
        );
    }

    #[test]
    fn contrast_ratio() {
        assert!((Rgba::BLACK.contrast(Rgba::WHITE) - 21.0).abs() < 0.01);
        assert!((Rgba::WHITE.contrast(Rgba::WHITE) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn scheme_slots() {
        let mut s = ColorScheme {
            name: "t".into(),
            colors: [Rgba::BLACK; 12],
        };
        s.set(SchemeSlot::Accent3, Rgba::WHITE);
        assert_eq!(s.get(SchemeSlot::Accent3), Rgba::WHITE);
        assert_eq!(s.get(SchemeSlot::Tx1), Rgba::BLACK);
        for slot in SchemeSlot::THEME {
            assert_eq!(SchemeSlot::from_xml(slot.xml_name()), Some(slot));
        }
    }
}
