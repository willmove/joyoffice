//! PDF output from page scenes (ADR-005, LAYOUT-009; first used by the T003
//! unified-scene check).
//!
//! The writer only converts units: every glyph is drawn with the glyph id
//! and position the scene already holds, through krilla's positioned-glyph
//! interface, with the same font bytes the shaper used. Text stays text
//! (fonts are embedded as subsets with Unicode mappings), so the PDF remains
//! searchable. krilla's own shaping (`simple-text`) is disabled. Still
//! images are decoded from the scene bytes and drawn at the scene rectangle;
//! nothing is fetched.
//!
//! Document information and outlines are optional additions on that same
//! scene (T204). The producer is the fixed name [`PRODUCER`]. Creation and
//! modification dates are omitted unless a caller later supplies a date that
//! came from the document; this writer does not read the clock.

mod check;
mod info;

pub use check::PdfReport;
pub use info::{
    OutlineEntry, PdfExport, PdfLink, core_title, links_from_document, outline_from_headings,
};

/// Tool name written to the PDF information dictionary. It does not include
/// a time or a build stamp.
pub const PRODUCER: &str = "JoyWord";

use std::collections::HashMap;
use std::ops::Range;

use joyword_layout::{FontKey, GlyphRun, LoadedFont, PageScene, SceneImage, SceneRect, to_points};
use krilla::Document;
use krilla::geom::{Point, Transform};
use krilla::page::PageSettings;
use krilla::surface::Location;
use krilla::text::{Font, GlyphId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfError {
    /// A run uses a font that was not supplied.
    MissingFont(FontKey),
    /// krilla rejected a font or failed to serialize.
    Writer(String),
    InvalidPage,
    /// An admitted scene image could not be drawn. The text does not echo bytes.
    Image(&'static str),
}

/// Glyph handed to krilla: the advance is the distance to the next glyph's
/// scene position, so krilla's pen reproduces scene coordinates exactly.
/// krilla asks for metrics at a given font size (it uses size 1 when writing
/// text), so values are stored per em and scaled on request.
struct SceneGlyph {
    id: u16,
    text_range: Range<usize>,
    advance_em: f32,
    x_offset_em: f32,
    y_offset_em: f32,
}

impl krilla::text::Glyph for SceneGlyph {
    fn glyph_id(&self) -> GlyphId {
        GlyphId::new(u32::from(self.id))
    }

    fn text_range(&self) -> Range<usize> {
        self.text_range.clone()
    }

    fn x_advance(&self, size: f32) -> f32 {
        self.advance_em * size
    }

    fn x_offset(&self, size: f32) -> f32 {
        self.x_offset_em * size
    }

    fn y_offset(&self, size: f32) -> f32 {
        self.y_offset_em * size
    }

    fn y_advance(&self, _size: f32) -> f32 {
        0.0
    }

    fn location(&self) -> Option<Location> {
        None
    }
}

fn scene_glyphs(run: &GlyphRun, searchable: bool) -> Vec<SceneGlyph> {
    let em = run.size.raw() as f64;
    let per_em = |emu: i64| (emu as f64 / em) as f32;
    (0..run.glyphs.len())
        .map(|index| {
            let glyph = &run.glyphs[index];
            let next = run
                .glyphs
                .get(index + 1)
                .map_or(glyph.x.raw() + glyph.advance.raw(), |next| next.x.raw());
            SceneGlyph {
                id: glyph.glyph_id,
                text_range: if searchable {
                    run.cluster_range(index)
                } else {
                    0..0
                },
                advance_em: per_em(next - glyph.x.raw()),
                x_offset_em: per_em(glyph.x_offset.raw()),
                // Scene and krilla are y-down; font offsets are y-up.
                y_offset_em: -per_em(glyph.y_offset.raw()),
            }
        })
        .collect()
}

/// Renders scenes to a PDF. `fonts` must contain every font the runs use.
/// The producer is [`PRODUCER`]; there is no title, outline or timestamp.
pub fn render(pages: &[PageScene], fonts: &[LoadedFont]) -> Result<Vec<u8>, PdfError> {
    render_export(pages, fonts, &PdfExport::default())
}

/// Same scene writer as [`render`], plus a title and heading outline.
/// Bytes stay deterministic: metadata has no creation or modification date.
pub fn render_export(
    pages: &[PageScene],
    fonts: &[LoadedFont],
    export: &PdfExport,
) -> Result<Vec<u8>, PdfError> {
    // XMP is omitted. krilla only writes dates into XMP when a creation date
    // is supplied, but leaving the packet out keeps the file free of any
    // metadata clock and of an instance-id that is not part of the scene.
    let settings = krilla::SerializeSettings {
        xmp_metadata: false,
        ..krilla::SerializeSettings::default()
    };
    let mut document = Document::new_with(settings);
    let mut cache: HashMap<FontKey, Font> = HashMap::new();
    for (page_index, scene) in pages.iter().enumerate() {
        let settings = PageSettings::from_wh(
            to_points(scene.width) as f32,
            to_points(scene.height) as f32,
        )
        .ok_or(PdfError::InvalidPage)?;
        let mut page = document.start_page_with(settings);
        let mut surface = page.surface();
        if !scene.rects.is_empty() {
            for rect in &scene.rects {
                fill_rect(&mut surface, rect);
            }
            surface.set_fill(None);
            surface.set_stroke(None);
        }
        for image in &scene.images {
            draw_image(&mut surface, image)?;
        }
        for run in &scene.runs {
            if run.glyphs.is_empty() && run.text.is_empty() {
                continue;
            }
            let font = match cache.get(&run.font) {
                Some(font) => font.clone(),
                None => {
                    let loaded = fonts
                        .iter()
                        .find(|font| font.key() == run.font)
                        .ok_or(PdfError::MissingFont(run.font))?;
                    let font = Font::new(loaded.data().to_vec().into(), run.font.index())
                        .ok_or_else(|| PdfError::Writer("font rejected".into()))?;
                    cache.insert(run.font, font.clone());
                    font
                }
            };
            let glyphs = scene_glyphs(run, true);
            let start = Point::from_xy(
                to_points(run.origin_x) as f32,
                to_points(run.baseline_y) as f32,
            );
            let size = to_points(run.size) as f32;
            set_rgb_fill(&mut surface, run.color);
            surface.draw_glyphs(start, &glyphs, font.clone(), &run.text, size, false);
            if run.bold {
                // Paint the offset copy as outlines: empty cluster ranges alone
                // still reuse the font's ToUnicode mapping in PDF readers.
                // Only the first draw contributes searchable text.
                let visual = scene_glyphs(run, false);
                let shifted = Point::from_xy(start.x + size * 0.04, start.y);
                surface.draw_glyphs(shifted, &visual, font, "", size, true);
            }
        }
        surface.finish();
        for link in export.links.iter().filter(|link| link.page == page_index) {
            let Some(rect) = krilla::geom::Rect::from_xywh(link.x, link.y, link.width, link.height)
            else {
                continue;
            };
            let annotation = krilla::annotation::LinkAnnotation::new(
                rect,
                krilla::annotation::Target::Action(krilla::action::Action::Link(
                    krilla::action::LinkAction::new(link.uri.clone()),
                )),
            );
            page.add_annotation(krilla::annotation::Annotation::new_link(annotation, None));
        }
        page.finish();
    }
    let mut metadata = krilla::metadata::Metadata::new().producer(PRODUCER.to_string());
    if let Some(title) = export
        .title
        .as_ref()
        .map(|title| title.trim())
        .filter(|title| !title.is_empty())
    {
        metadata = metadata.title(title.to_string());
    }
    document.set_metadata(metadata);
    if let Some(outline) = info::krilla_outline(&export.outline, pages.len()) {
        document.set_outline(outline);
    }
    document
        .finish()
        .map_err(|error| PdfError::Writer(format!("{error:?}")))
}

fn set_rgb_fill(surface: &mut krilla::surface::Surface<'_>, color: [u8; 3]) {
    let Some(opacity) = krilla::num::NormalizedF32::new(1.0) else {
        return;
    };
    surface.set_fill(Some(krilla::paint::Fill {
        paint: krilla::color::rgb::Color::new(color[0], color[1], color[2]).into(),
        opacity,
        rule: krilla::paint::FillRule::NonZero,
    }));
}

/// Reads the PDF structure without using the writer that produced it.
pub fn inspect_pdf(bytes: &[u8]) -> Result<PdfReport, &'static str> {
    check::inspect(bytes)
}

fn fill_rect(surface: &mut krilla::surface::Surface<'_>, rect: &SceneRect) {
    let x = to_points(rect.x) as f32;
    let y = to_points(rect.y) as f32;
    let width = to_points(rect.width) as f32;
    let height = to_points(rect.height) as f32;
    let Some(bounds) = krilla::geom::Rect::from_xywh(x, y, width, height) else {
        return;
    };
    let mut builder = krilla::geom::PathBuilder::new();
    builder.push_rect(bounds);
    let Some(path) = builder.finish() else {
        return;
    };
    let Some(opacity) = krilla::num::NormalizedF32::new(1.0) else {
        return;
    };
    surface.set_fill(Some(krilla::paint::Fill {
        paint: krilla::color::rgb::Color::new(rect.red, rect.green, rect.blue).into(),
        opacity,
        rule: krilla::paint::FillRule::NonZero,
    }));
    surface.set_stroke(None);
    surface.draw_path(&path);
}

fn draw_image(
    surface: &mut krilla::surface::Surface<'_>,
    image: &SceneImage,
) -> Result<(), PdfError> {
    let decoded =
        joyword_model::decode_rgba(image.bytes.as_ref(), &joyword_model::ImageLimits::default())
            .map_err(|_| PdfError::Image("image decode failed"))?;
    let expected = (decoded.width as usize)
        .checked_mul(decoded.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(PdfError::Image("image decode failed"))?;
    if decoded.rgba.len() != expected || decoded.width == 0 || decoded.height == 0 {
        return Err(PdfError::Image("image decode failed"));
    }
    let bitmap = krilla::image::Image::from_rgba8(decoded.rgba, decoded.width, decoded.height);
    let width = to_points(image.width) as f32;
    let height = to_points(image.height) as f32;
    let Some(size) = krilla::geom::Size::from_wh(width, height) else {
        return Err(PdfError::Image("image extent rejected"));
    };
    surface.push_transform(&Transform::from_translate(
        to_points(image.x) as f32,
        to_points(image.y) as f32,
    ));
    surface.draw_image(bitmap, size);
    surface.pop();
    Ok(())
}
