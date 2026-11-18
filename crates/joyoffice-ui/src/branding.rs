//! Embedded product artwork for both application chrome and native windows.
use crate::Brand;
use gpui_kit::{Image, ImageFormat, Img, Styled, WindowOptions, img, px};
use std::sync::{Arc, OnceLock};

struct Artwork {
    app_id: &'static str,
    ico: &'static [u8],
    svg: &'static [u8],
    native: OnceLock<Option<Arc<image::RgbaImage>>>,
    logo: OnceLock<Arc<Image>>,
}

macro_rules! artwork {
    ($id:literal) => {
        Artwork {
            app_id: $id,
            ico: include_bytes!(concat!("../../../assets/brand/", $id, ".ico")),
            svg: include_bytes!(concat!("../../../assets/brand/", $id, ".svg")),
            native: OnceLock::new(),
            logo: OnceLock::new(),
        }
    };
}

static WORD: Artwork = artwork!("joyword");
static DECK: Artwork = artwork!("joydeck");
static SHEET: Artwork = artwork!("joysheet");

fn artwork(brand: Brand) -> &'static Artwork {
    match brand {
        Brand::Word => &WORD,
        Brand::Deck => &DECK,
        Brand::Sheet => &SHEET,
    }
}

/// Use on every product window, including secondary and presentation windows.
/// GPUI's X11 backend needs pixels; an app_id or a packaged SVG alone does not
/// set _NET_WM_ICON. Windows uses EXE resource 1 and macOS uses the app bundle.
pub fn product_window_options(brand: Brand) -> WindowOptions {
    let art = artwork(brand);
    let icon = art.native.get_or_init(|| {
        match image::load_from_memory_with_format(art.ico, image::ImageFormat::Ico) {
            Ok(image) => Some(Arc::new(image.into_rgba8())),
            Err(error) => {
                eprintln!(
                    "{}: could not decode embedded application icon: {error}",
                    art.app_id
                );
                None
            }
        }
    });
    WindowOptions {
        app_id: Some(art.app_id.into()),
        icon: icon.clone(),
        ..Default::default()
    }
}

/// Full-colour product logo, independent of the working directory and theme.
pub fn product_logo(brand: Brand, size: f32) -> Img {
    let art = artwork(brand);
    let image = art
        .logo
        .get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Svg, art.svg.to_vec())));
    img(image.clone()).w(px(size)).h(px(size)).flex_shrink_0()
}
