//! Adapted from craft-pdf at 92d392c; uses the host registry Krilla.
use std::panic::{AssertUnwindSafe, catch_unwind};

pub use krilla as backend;

// Stock Krilla is shared with JoyOffice. paint_page contains drawing panics without
// dropping an unbalanced Surface during unwind. Print/PDF-X and optional content
// extensions of the upstream fork are outside this reader's export contract.

use backend::configure::ConfigurationError;
use backend::text::{Font, Tag};

/// Largest individual font copied for embedding (512 MiB).
pub const MAX_FONT_BYTES: usize = 512 * 1024 * 1024;
/// Maximum number of variation coordinates accepted for one font instance.
pub const MAX_FONT_VARIATIONS: usize = 256;

/// Shared failures retain backend diagnostics instead of silently emitting a partial PDF.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("{0:?}")]
    Configuration(ConfigurationError),
    #[error("{0:?}")]
    Serialization(#[source] Box<backend::error::KrillaError>),
    #[error("PDF export failed internally: {0}")]
    BackendPanic(String),
    #[error("font exceeds the {MAX_FONT_BYTES}-byte embedding limit")]
    FontLimit,
    #[error("font variation coordinates must be finite and number at most {MAX_FONT_VARIATIONS}")]
    FontVariations,
    #[error("font data could not be allocated")]
    Allocation,
    #[error("font could not be read for embedding")]
    InvalidFont,
    #[error("the print profile is not a readable CMYK ICC profile")]
    InvalidProfile,
}

/// Run an entire app export, including drawing and postprocessing, behind a native
/// unwind guard. Existing application errors pass through unchanged.
///
/// Discard any writer or drawing state captured by the closure after a panic. Prefer
/// constructing the writer inside the closure. WASM aborts and process-wide allocation
/// failures cannot be caught; this is not a memory or execution-time budget.
pub fn guard<T, E: From<WriteError>>(operation: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    catch_unwind(AssertUnwindSafe(operation)).map_err(|payload| {
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("non-string panic payload");
        E::from(WriteError::BackendPanic(
            message.chars().take(1024).collect(),
        ))
    })?
}

/// Common document information. Callers decide which values to provide and whether to
/// trim them. Dates are supplied explicitly; this crate never consults a platform clock.
#[derive(Clone, Debug, Default)]
pub struct Metadata {
    pub creator: String,
    pub producer: String,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub created: Option<backend::metadata::DateTime>,
}

impl Metadata {
    fn into_backend(self) -> backend::metadata::Metadata {
        let mut metadata = backend::metadata::Metadata::new()
            .creator(self.creator)
            .producer(self.producer)
            .authors(self.authors)
            .keywords(self.keywords);
        if let Some(title) = self.title {
            metadata = metadata.title(title);
        }
        if let Some(description) = self.description {
            metadata = metadata.description(description);
        }
        if let Some(created) = self.created {
            metadata = metadata.creation_date(created);
        }

        metadata
    }
}

/// A writer whose serialization is fallible and consumes the document on every outcome.
pub struct Writer {
    document: backend::Document,
}

impl Writer {
    /// Set an optional document language without changing the common metadata fields.
    /// Empty language is treated as absent. Callers validate nonempty tags, which pass through unchanged.
    pub fn new_with_language(
        settings: backend::SerializeSettings,
        metadata: Metadata,
        language: Option<String>,
    ) -> Self {
        let mut metadata = metadata.into_backend();
        if let Some(language) = language.filter(|language| !language.is_empty()) {
            metadata = metadata.language(language);
        }
        let mut document = backend::Document::new_with(settings);
        document.set_metadata(metadata);
        Self { document }
    }

    /// Advanced drawing, resource, outline and tag-tree access for an application's adapter.
    /// Use [`guard`] around the whole export to cover drawing as well as serialization.
    pub fn document_mut(&mut self) -> &mut backend::Document {
        &mut self.document
    }

    /// Serialize, preserving validation failures and catching native backend unwinds.
    /// The returned byte vector is not a bound on peak serialization memory.
    pub fn finish(self) -> Result<Vec<u8>, WriteError> {
        guard(|| {
            self.document
                .finish()
                .map_err(|error| WriteError::Serialization(Box::new(error)))
        })
    }
}

/// The link target that opens `url`, or `None` when [`safe_url`](crate::safe_url) refuses it.
/// Build URI links with this instead of `backend::action::LinkAction` so a refused target
/// writes no link annotation; the page content stays as drawn. The URI is written trimmed.
pub fn uri_target(url: &str) -> Option<backend::annotation::Target> {
    let url = url.trim();
    safe_url(url).then(|| {
        backend::annotation::Target::Action(backend::action::LinkAction::new(url.to_owned()).into())
    })
}

fn check_font_input(size: usize, variations: Option<&[([u8; 4], f32)]>) -> Result<(), WriteError> {
    if size > MAX_FONT_BYTES {
        return Err(WriteError::FontLimit);
    }
    if variations.is_some_and(|coords| {
        coords.len() > MAX_FONT_VARIATIONS || coords.iter().any(|(_, value)| !value.is_finite())
    }) {
        return Err(WriteError::FontVariations);
    }
    Ok(())
}

/// Copy and prepare an already selected, licensed font for Krilla's subset embedding.
/// `None` retains static-font construction; `Some` retains variable-font construction,
/// including an empty coordinate list. Selection, caching, shaping and fallback belong
/// to the app. An error never substitutes another font or drops text silently.
pub fn font(
    data: &[u8],
    index: u32,
    variations: Option<&[([u8; 4], f32)]>,
) -> Result<Font, WriteError> {
    check_font_input(data.len(), variations)?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(data.len())
        .map_err(|_| WriteError::Allocation)?;
    owned.extend_from_slice(data);
    guard(|| {
        let font = match variations {
            None => Font::new(owned.into(), index),
            Some(coords) => {
                let coords: Vec<_> = coords
                    .iter()
                    .map(|(tag, value)| (Tag::new(tag), *value))
                    .collect();
                Font::new_variable(owned.into(), index, &coords)
            }
        };
        font.ok_or(WriteError::InvalidFont)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn font_bounds_are_checked_without_allocating_the_input() {
        assert!(check_font_input(MAX_FONT_BYTES, None).is_ok());
        assert!(matches!(
            check_font_input(MAX_FONT_BYTES + 1, None),
            Err(WriteError::FontLimit)
        ));
        assert!(matches!(
            check_font_input(usize::MAX, None),
            Err(WriteError::FontLimit)
        ));
        let coords = vec![(*b"wght", 400.0); MAX_FONT_VARIATIONS + 1];
        assert!(matches!(
            check_font_input(0, Some(&coords)),
            Err(WriteError::FontVariations)
        ));
    }
}

fn safe_url(url: &str) -> bool {
    ["http:", "https:", "mailto:"]
        .iter()
        .any(|prefix| url.to_ascii_lowercase().starts_with(prefix))
        && !url.chars().any(char::is_control)
}

/// Hold the surface outside the unwind closure: stock Krilla must not drop a
/// surface with an outstanding transform/clip while the thread is unwinding.
/// On failure the caller discards the incomplete writer and returns the error.
pub fn paint_page(
    page: &mut backend::page::Page<'_>,
    paint: impl FnOnce(&mut backend::surface::Surface<'_>),
) -> Result<(), WriteError> {
    let mut surface = page.surface();
    let result = catch_unwind(AssertUnwindSafe(|| paint(&mut surface)));
    if result.is_err() {
        std::mem::forget(surface);
        return Err(WriteError::BackendPanic(
            "page drawing failed; incomplete output discarded".into(),
        ));
    }
    guard(|| {
        surface.finish();
        Ok(())
    })
}

#[cfg(test)]
mod unwind_tests {
    use super::*;
    #[test]
    fn unbalanced_surface_is_not_dropped_during_unwind() {
        let mut document = backend::Document::new();
        let mut page = document.start_page();
        let result = paint_page(&mut page, |surface| {
            surface.push_transform(&backend::geom::Transform::identity());
            panic!("injected drawing failure");
        });
        assert!(matches!(result, Err(WriteError::BackendPanic(_))));
        drop(page);
        drop(document);
    }
}
