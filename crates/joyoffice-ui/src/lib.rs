//! Product-neutral presentation primitives. No editor, command or persistence state.
//!
//! Applications own identity, accessibility, focus routing and activation. These
//! surfaces only supply the shared visual vocabulary and interaction states.
pub use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{Div, Styled, div, px, rgb};

#[derive(Clone, Copy, Debug)]
pub enum Brand {
    Word,
    Sheet,
}

#[derive(Clone, Copy, Debug)]
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

/// Logical pixels; document zoom never changes application chrome dimensions.
pub mod metrics {
    pub const CONTROL: f32 = 30.0;
    pub const HEADER: f32 = 48.0;
    pub const TAB: f32 = 36.0;
    pub const STATUS: f32 = 32.0;
    pub const ICON: f32 = 18.0;
    pub const RADIUS: f32 = 6.0;
    pub const CARD_RADIUS: f32 = 10.0;
}

pub fn control(palette: Palette, enabled: bool, selected: bool) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .h(px(metrics::CONTROL))
        .px_2()
        .gap_2()
        .rounded(px(metrics::RADIUS))
        .border_1()
        .border_color(if selected {
            rgb(palette.accent).into()
        } else {
            gpui_kit::transparent_black()
        })
        .bg(if selected {
            rgb(palette.selected).into()
        } else {
            gpui_kit::transparent_black()
        })
        .text_color(rgb(if !enabled {
            palette.disabled
        } else if selected {
            palette.accent
        } else {
            palette.text
        }))
        .text_size(px(13.0))
}

pub fn card(palette: Palette) -> Div {
    div()
        .bg(rgb(palette.surface))
        .text_color(rgb(palette.text))
        .border_1()
        .border_color(rgb(palette.border))
        .rounded(px(metrics::CARD_RADIUS))
}

pub fn icon(name: IconName, size: f32, color: u32) -> Icon {
    Icon::new(name).size(px(size)).text_color(rgb(color))
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
        for brand in [Brand::Word, Brand::Sheet] {
            for p in [Palette::light(brand), Palette::dark(brand)] {
                for surface in [p.canvas, p.surface, p.chrome, p.header] {
                    assert!(contrast(p.text, surface) >= 4.5);
                    assert!(contrast(p.muted, surface) >= 4.5);
                    assert!(contrast(p.accent, surface) >= 3.0);
                }
                assert!(contrast(p.on_accent, p.accent) >= 4.5);
            }
        }
    }
}
