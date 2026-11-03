//! Official crates.io Krilla bridge. No global craft-libs patch or filesystem access.
use super::{PdfError, Result};
use krilla::text::Font;
use skrifa::raw::TableProvider;

pub fn guard<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
        .map_err(|_| PdfError::Write("PDF backend panicked; no output published".into()))?
}

pub fn font(data: &[u8], index: u32, coords: &[([u8; 4], f32)]) -> Result<Font> {
    let face = skrifa::FontRef::from_index(data, index)
        .map_err(|e| PdfError::Write(format!("invalid font: {e:?}")))?;
    if face
        .os2()
        .is_ok_and(|os2| os2.fs_type() & 2 != 0 || os2.fs_type() & 0x300 != 0)
    {
        return Err(PdfError::Write(
            "font forbids embedding or subsetting".into(),
        ));
    }
    let coords = coords
        .iter()
        .map(|(t, v)| (krilla::text::Tag::new(t), *v))
        .collect::<Vec<_>>();
    Font::new_variable(data.to_vec().into(), index, &coords)
        .ok_or_else(|| PdfError::Write("Krilla rejected font face".into()))
}
