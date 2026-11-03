//! Immutable, bundled outline fallback shared by desktop layout, raster and PDF.
use joyword_layout::{FaceSet, LoadedFont};
use std::sync::{Arc, OnceLock};

fn emoji() -> Option<&'static LoadedFont> {
    static FONT: OnceLock<Option<LoadedFont>> = OnceLock::new();
    FONT.get_or_init(|| {
        LoadedFont::from_bytes(
            Arc::from(include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf").as_slice()),
            0,
        )
        .ok()
    })
    .as_ref()
}
pub(crate) fn faces(primary: &LoadedFont) -> FaceSet<'_> {
    FaceSet {
        fallback: emoji().unwrap_or(primary),
        ..FaceSet::uniform(primary)
    }
}
pub(crate) fn fonts(primary: &LoadedFont) -> Vec<LoadedFont> {
    let mut fonts = vec![primary.clone()];
    if let Some(emoji) = emoji()
        && emoji.key() != primary.key()
    {
        fonts.push(emoji.clone());
    }
    fonts
}

#[cfg(test)]
#[path = "font_fallback/tests.rs"]
mod tests;
