//! Shared application appearance. No GUI, document model or file access.
#![forbid(unsafe_code)]

/// Stable IDs persisted by each product in its existing preference store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePreset {
    VerdantLight,
    VerdantDark,
    ClassicLight,
    ClassicDark,
    AzureLight,
    AzureDark,
    SandstoneLight,
    SandstoneDark,
}

impl ThemePreset {
    pub const ALL: [Self; 8] = [
        Self::VerdantLight,
        Self::VerdantDark,
        Self::ClassicLight,
        Self::ClassicDark,
        Self::AzureLight,
        Self::AzureDark,
        Self::SandstoneLight,
        Self::SandstoneDark,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::VerdantLight => "verdant-light",
            Self::VerdantDark => "verdant-dark",
            Self::ClassicLight => "classic-light",
            Self::ClassicDark => "classic-dark",
            Self::AzureLight => "azure-light",
            Self::AzureDark => "azure-dark",
            Self::SandstoneLight => "sandstone-light",
            Self::SandstoneDark => "sandstone-dark",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.id() == id)
    }

    pub const fn is_dark(self) -> bool {
        matches!(
            self,
            Self::VerdantDark | Self::ClassicDark | Self::AzureDark | Self::SandstoneDark
        )
    }

    pub const fn with_dark(self, dark: bool) -> Self {
        match self {
            Self::VerdantLight | Self::VerdantDark => {
                if dark {
                    Self::VerdantDark
                } else {
                    Self::VerdantLight
                }
            }
            Self::ClassicLight | Self::ClassicDark => {
                if dark {
                    Self::ClassicDark
                } else {
                    Self::ClassicLight
                }
            }
            Self::AzureLight | Self::AzureDark => {
                if dark {
                    Self::AzureDark
                } else {
                    Self::AzureLight
                }
            }
            Self::SandstoneLight | Self::SandstoneDark => {
                if dark {
                    Self::SandstoneDark
                } else {
                    Self::SandstoneLight
                }
            }
        }
    }

    pub const fn default_for(brand: Brand, dark: bool) -> Self {
        match brand {
            Brand::Word | Brand::Sheet => Self::VerdantLight.with_dark(dark),
            Brand::Deck => Self::ClassicLight.with_dark(dark),
        }
    }

    pub const fn label(self, chinese: bool) -> &'static str {
        match (self, chinese) {
            (Self::VerdantLight, true) => "青岚 · 亮色",
            (Self::VerdantDark, true) => "青岚 · 暗色",
            (Self::ClassicLight, true) => "素雅 · 亮色",
            (Self::ClassicDark, true) => "素雅 · 暗色",
            (Self::AzureLight, true) => "晴空 · 亮色",
            (Self::AzureDark, true) => "晴空 · 暗色",
            (Self::SandstoneLight, true) => "暖砂 · 亮色",
            (Self::SandstoneDark, true) => "暖砂 · 暗色",
            (Self::VerdantLight, false) => "Verdant · Light",
            (Self::VerdantDark, false) => "Verdant · Dark",
            (Self::ClassicLight, false) => "Classic · Light",
            (Self::ClassicDark, false) => "Classic · Dark",
            (Self::AzureLight, false) => "Azure · Light",
            (Self::AzureDark, false) => "Azure · Dark",
            (Self::SandstoneLight, false) => "Sandstone · Light",
            (Self::SandstoneDark, false) => "Sandstone · Dark",
        }
    }

    pub const fn palette(self, brand: Brand) -> Palette {
        let mut p = if self.is_dark() {
            Palette::dark(brand)
        } else {
            Palette::light(brand)
        };
        match self {
            Self::VerdantLight | Self::VerdantDark => {}
            Self::ClassicLight => {
                p.canvas = 0xf0f0f0;
                p.chrome = 0xf3f3f3;
                p.header = 0xe9eef2;
                p.text = 0x242424;
                p.muted = 0x5c5c5c;
                p.disabled = 0x909090;
                p.border = 0xd4d4d4;
                p.separator = 0xe5e5e5;
                p.hover = 0xebebeb;
                p.pressed = 0xdcdcdc;
                p.selected = 0xe6e6e6;
            }
            Self::ClassicDark => {
                p.canvas = 0x1e1e1e;
                p.chrome = 0x262626;
                p.surface = 0x333333;
                p.header = 0x292929;
                p.text = 0xeeeeee;
                p.muted = 0xbbbbbb;
                p.disabled = 0x888888;
                p.border = 0x4a4a4a;
                p.separator = 0x3d3d3d;
                p.hover = 0x404040;
                p.pressed = 0x505050;
                p.selected = 0x474747;
                if matches!(brand, Brand::Deck) {
                    p.accent = 0xffad7b;
                    p.accent_hover = 0xffc8a6;
                    p.on_accent = 0x241b16;
                }
            }
            Self::AzureLight => {
                p.canvas = 0xeaf0f8;
                p.chrome = 0xf3f6fc;
                p.header = 0xe2eafa;
                p.text = 0x22334b;
                p.muted = 0x50627c;
                p.disabled = 0x8c9ab0;
                p.border = 0xccd7e8;
                p.separator = 0xe0e7f2;
                p.hover = 0xe5ecf8;
                p.pressed = 0xd6e1f5;
                p.selected = 0xdce7fc;
                p.accent = 0x2459b3;
                p.accent_hover = 0x19438c;
            }
            Self::AzureDark => {
                p.canvas = 0x121d30;
                p.chrome = 0x1b2a42;
                p.surface = 0x21324e;
                p.header = 0x1e304c;
                p.text = 0xe7effc;
                p.muted = 0xb0c2df;
                p.disabled = 0x7b8eab;
                p.border = 0x3c5273;
                p.separator = 0x304562;
                p.hover = 0x304966;
                p.pressed = 0x3c577d;
                p.selected = 0x304d77;
                p.accent = 0xa6caff;
                p.accent_hover = 0xc9dfff;
                p.on_accent = 0x12213a;
            }
            Self::SandstoneLight => {
                p.canvas = 0xf2ede5;
                p.surface = 0xfffcf6;
                p.chrome = 0xf9f5ed;
                p.header = 0xeee4d6;
                p.text = 0x40352c;
                p.muted = 0x6d5e50;
                p.disabled = 0xa39586;
                p.border = 0xdbcfbd;
                p.separator = 0xe8dfd2;
                p.hover = 0xeee5d8;
                p.pressed = 0xe2d4c1;
                p.selected = 0xebdcc6;
                p.accent = 0x8a4a1f;
                p.accent_hover = 0x703916;
            }
            Self::SandstoneDark => {
                p.canvas = 0x241e19;
                p.surface = 0x342b23;
                p.chrome = 0x30271f;
                p.header = 0x382c22;
                p.text = 0xf6ebdd;
                p.muted = 0xcdbba5;
                p.disabled = 0x9d8b77;
                p.border = 0x5b4b3b;
                p.separator = 0x4b3d30;
                p.hover = 0x4b3c2e;
                p.pressed = 0x60503c;
                p.selected = 0x59422d;
                p.accent = 0xf1c38d;
                p.accent_hover = 0xffdcb3;
                p.on_accent = 0x2e2013;
            }
        }
        p
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brand {
    Word,
    Sheet,
    Deck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub canvas: u32,
    pub surface: u32,
    pub chrome: u32,
    pub header: u32,
    pub text: u32,
    pub muted: u32,
    pub disabled: u32,
    pub border: u32,
    pub separator: u32,
    pub hover: u32,
    pub pressed: u32,
    pub selected: u32,
    pub accent: u32,
    pub accent_hover: u32,
    pub on_accent: u32,
}

impl Palette {
    pub const fn light(brand: Brand) -> Self {
        let (accent, accent_hover, selected, header) = match brand {
            Brand::Word => (0x315fa8, 0x244b87, 0xe6eefb, 0xeaf0f8),
            Brand::Sheet => (0x146c43, 0x105735, 0xe3f0e8, 0xe9f0ec),
            Brand::Deck => (0xa63d24, 0x87301b, 0xf9eae3, 0xf5ece7),
        };
        Self {
            canvas: 0xf0f3f2,
            surface: 0xffffff,
            chrome: 0xf7f9f8,
            header,
            text: 0x26352d,
            muted: 0x5b6b63,
            disabled: 0x9aa69f,
            border: 0xdce3df,
            separator: 0xe9eeeb,
            hover: 0xeef3f0,
            pressed: 0xdde7e1,
            selected,
            accent,
            accent_hover,
            on_accent: 0xffffff,
        }
    }

    pub const fn dark(brand: Brand) -> Self {
        let (accent, accent_hover, selected, header) = match brand {
            Brand::Word => (0xa8c8ff, 0xc5daff, 0x293d58, 0x202e40),
            Brand::Sheet => (0x94dbb4, 0xb2ebcb, 0x294638, 0x1d3529),
            Brand::Deck => (0xffba9e, 0xffd1be, 0x50382f, 0x3e3029),
        };
        Self {
            canvas: 0x172620,
            surface: 0x20332a,
            chrome: 0x23332a,
            header,
            text: 0xe5eee8,
            muted: 0xaebbb4,
            disabled: 0x75877c,
            border: 0x3b4f43,
            separator: 0x35463b,
            hover: 0x35443d,
            pressed: 0x40564a,
            selected,
            accent,
            accent_hover,
            on_accent: 0x172620,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: u32) -> f64 {
        let channel = |shift: u32| {
            let v = f64::from((color >> shift) & 255_u32) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    fn contrast(a: u32, b: u32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn reading_and_action_colors_remain_legible_across_brands_and_modes() {
        for brand in [Brand::Word, Brand::Sheet, Brand::Deck] {
            for preset in ThemePreset::ALL {
                let p = preset.palette(brand);
                for surface in [p.canvas, p.surface, p.chrome, p.header, p.selected, p.hover] {
                    assert!(contrast(p.text, surface) >= 4.5);
                    assert!(contrast(p.muted, surface) >= 4.5);
                    assert!(contrast(p.accent, surface) >= 3.0);
                }
                assert!(contrast(p.on_accent, p.accent) >= 4.5);
            }
        }
    }
}
