//! Explicit-face exports over the same document layout and display list as the editor.

use joyword_engine::{
    doc::Document,
    io::{open_bytes, read_regular_file_within},
};
use joyword_fonts::{FaceRef, FontDb};
use joyword_layout::{
    DocLayout, LayoutCache, LayoutOptions,
    display::{DisplayOptions, Draw, page_display},
    layout,
};
use joyword_render::{RenderOptions, Rendered, render_page};
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct FixedVisual {
    document: Document,
    layout: DocLayout,
    face: FaceRef,
}

impl FixedVisual {
    pub fn open(input: &Path, font: &str) -> Result<Self, String> {
        let (font, index) = font.rsplit_once(':').ok_or("font must be PATH:FACE")?;
        let index = index.parse().map_err(|_| "invalid font face")?;
        let bytes = read_regular_file_within(Path::new(font), 128 << 20)
            .ok_or("font must be a regular file of at most 128 MiB")?;
        let face = FontDb::init_export_face(bytes, index)?;
        let bytes = read_regular_file_within(input, 32 << 20)
            .ok_or("input must be a regular file of at most 32 MiB")?;
        crate::reader::preflight_word_package(&bytes)?;
        let document = open_bytes("input.docx", &bytes)?;
        let layout = layout(
            &document,
            &mut LayoutCache::new(),
            &LayoutOptions {
                hide_deleted: true,
                ..Default::default()
            },
        );
        if layout.pages.is_empty() || layout.pages.len() > 64 {
            return Err("visual page budget exceeded".into());
        }
        for page in &layout.pages {
            check_glyphs(&page_display(&document, page, &display_options()), face)?;
        }
        Ok(Self {
            document,
            layout,
            face,
        })
    }

    pub fn page_count(&self) -> usize {
        self.layout.pages.len()
    }

    pub fn font_hash(&self) -> String {
        format!("{:x}", Sha256::digest(self.face.data()))
    }

    pub fn font_index(&self) -> u32 {
        self.face.index()
    }

    pub fn raster(&self, index: usize, dpi: u16) -> Result<Rendered, String> {
        let page = self
            .layout
            .pages
            .get(index)
            .ok_or("page is outside the document")?;
        let scale = f32::from(dpi) / 72.0;
        let (width, height) = ((page.w * scale).ceil(), (page.h * scale).ceil());
        if !width.is_finite()
            || !height.is_finite()
            || width < 1.0
            || height < 1.0
            || width > joyword_render::MAX_SIDE as f32
            || height > joyword_render::MAX_SIDE as f32
            || width as u64 * height as u64 > 16_777_216
        {
            return Err("page raster exceeds the visual pixel budget".into());
        }
        Ok(render_page(
            &self.document,
            page,
            scale,
            &RenderOptions {
                display: display_options(),
                ..Default::default()
            },
        ))
    }

    pub fn pdf(&self, archive_year: Option<u16>) -> Result<Vec<u8>, String> {
        if !self.face.permits_subset_embedding() {
            return Err("font cannot be embedded; export refused".into());
        }
        joyword_pdf::export_fixed_layout(
            &self.document,
            &self.layout,
            &joyword_pdf::PdfOptions::default(),
            archive_year,
        )
        .map_err(|error| error.to_string())
    }
}

fn display_options() -> DisplayOptions {
    DisplayOptions {
        dim_header: false,
        ..Default::default()
    }
}

fn check_glyphs(items: &[Draw], expected: FaceRef) -> Result<(), String> {
    for item in items {
        match item {
            Draw::Glyphs { face, glyphs, .. }
                if *face != expected || glyphs.iter().any(|g| g.0 == 0) =>
            {
                return Err("font cannot display the complete document".into());
            }
            Draw::Clip { items, .. } | Draw::Turned { items, .. } | Draw::Rotated { items, .. } => {
                check_glyphs(items, expected)?;
            }
            _ => {}
        }
    }
    Ok(())
}
