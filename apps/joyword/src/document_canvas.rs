//! Read-only engineering document: real engine snapshot, immutable layout and
//! CPU scene output. Loading, layout and raster work run on background jobs.
use gpui::RenderImage;
use joyword_engine::{EditPolicy, Session};
use joyword_foundation::Emu;
use joyword_layout::{
    FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont, SingleFontProfile,
    layout_document,
};
use joyword_model::{BlockSeq, Document, PageSetup, Paragraph};
use joyword_render::{Projection, RasterCancellation, RasterLimits, RasterPage, rasterize};
use std::path::Path;
use std::sync::Arc;

const SEED: &str = include_str!("../../../tests/fixtures/text/cjk-contract.txt");
/// Total UI cache ceiling across pages, in addition to each renderer budget.
const MAX_CACHE_PIXELS: u64 = 16_777_216;

pub struct CanvasSource {
    pub session: Session,
    pub layout: LayoutSnapshot,
    pub font: LoadedFont,
    pub profile: SingleFontProfile,
}

fn load_font() -> Result<LoadedFont, String> {
    let requested = std::env::var("JOYWORD_FONT").ok();
    load_font_spec(requested.as_deref())
}
fn load_font_spec(requested: Option<&str>) -> Result<LoadedFont, String> {
    let candidates = if let Some(spec) = requested {
        let (path, index) = spec
            .rsplit_once(':')
            .ok_or("JOYWORD_FONT must be path:face")?;
        vec![(
            path.to_owned(),
            index.parse::<u32>().map_err(|_| "invalid font face")?,
        )]
    } else {
        vec![
            (
                "/System/Library/Fonts/Supplemental/Songti.ttc".to_owned(),
                6,
            ),
            (
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".to_owned(),
                2,
            ),
            ("C:\\Windows\\Fonts\\simsun.ttc".to_owned(), 0),
        ]
    };
    for (path, face) in candidates {
        if !Path::new(&path).is_file() {
            continue;
        }
        // Bound actual IO before allocating; metadata alone can race a writer.
        use std::io::Read as _;
        let mut data = Vec::new();
        std::fs::File::open(&path)
            .map_err(|e| e.to_string())?
            .take((128 << 20) + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > 128 << 20 {
            return Err("font exceeds 128 MiB".into());
        }
        return LoadedFont::from_bytes(data.into(), face)
            .map_err(|e| format!("font {path} face {face}: {e:?}"));
    }
    Err("CJK font unavailable; set JOYWORD_FONT=path:face".into())
}

pub fn load_source() -> Result<CanvasSource, String> {
    source_with_text(load_font()?, SEED)
}
fn source_with_text(font: LoadedFont, text: &str) -> Result<CanvasSource, String> {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let mut blocks = Vec::new();
    for text in text.lines() {
        let mut p = Paragraph::new(document.ids().node());
        p.push_text(text, None, Arc::default())
            .map_err(|e| e.to_string())?;
        blocks.push(p.into());
    }
    let story = document.body_id();
    document.story_mut(story).expect("body").blocks = BlockSeq::from_vec(blocks);
    let mut session = Session::new(document);
    session.set_policy(EditPolicy {
        read_only: true,
        agent_writes: false,
    });
    let profile = SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    };
    let layout = layout_document(
        &session.document().snapshot(),
        &font,
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .map_err(|e| e.to_string())?;
    if !layout.diagnostics().is_empty() || layout.omitted_diagnostics() != 0 {
        return Err("selected font cannot display the complete sample".into());
    }
    Ok(CanvasSource {
        session,
        layout,
        font,
        profile,
    })
}

pub fn raster_pages(
    source: &CanvasSource,
    projection: Projection,
    cancellation: &RasterCancellation,
) -> Result<Vec<RasterPage>, String> {
    source
        .layout
        .ensure_current(
            &source.session.document().snapshot(),
            &source.font,
            source.profile,
        )
        .map_err(|e| e.to_string())?;
    let mut pages = Vec::new();
    let mut remaining = MAX_CACHE_PIXELS;
    for scene in source.layout.pages() {
        let limits = RasterLimits {
            max_pixels: remaining,
            ..RasterLimits::default()
        };
        let page = rasterize(
            scene,
            std::slice::from_ref(&source.font),
            projection,
            limits,
            cancellation,
        )
        .map_err(|e| e.to_string())?;
        remaining -= u64::from(page.width()) * u64::from(page.height());
        pages.push(page);
    }
    Ok(pages)
}

pub fn image(page: RasterPage) -> Arc<RenderImage> {
    let (w, h) = (page.width(), page.height());
    let buffer =
        image::RgbaImage::from_raw(w, h, page.into_bgra()).expect("validated raster dimensions");
    Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use joyword_engine::{Command, Transaction, TxError};
    use joyword_model::{Bias, Position, ScalarOffset};
    fn font() -> LoadedFont {
        LoadedFont::from_bytes(
            Arc::from(
                include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
            ),
            0,
        )
        .unwrap()
    }
    fn start(source: &CanvasSource) -> Position {
        Position {
            story: source.session.document().body_id(),
            paragraph: source.session.document().body().blocks.get(0).unwrap().id(),
            offset: ScalarOffset(0),
            bias: Bias::Before,
        }
    }
    #[test]
    fn real_engine_session_is_read_only_and_layout_matches_source() {
        let mut source = source_with_text(font(), "AV\nfi").unwrap();
        let at = start(&source);
        assert_eq!(
            source.session.apply(Transaction::new(
                source.session.version(),
                vec![Command::InsertText {
                    at,
                    text: "A".into()
                }]
            )),
            Err(TxError::ReadOnly)
        );
        source
            .layout
            .ensure_current(&source.session.snapshot(), &source.font, source.profile)
            .unwrap();
        assert_eq!(
            source.layout.pages()[0]
                .runs
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>(),
            ["AV", "fi"]
        );
    }
    #[test]
    fn page_image_conversion_and_device_projection_need_no_window() {
        let source = source_with_text(font(), "AV\nfi").unwrap();
        let pages = raster_pages(
            &source,
            Projection::new(96.0, 0.8).unwrap(),
            &RasterCancellation::default(),
        )
        .unwrap();
        let pixels = pages[0].clone();
        let expected = pixels.clone().into_bgra();
        let image = image(pixels);
        assert_eq!(image.as_bytes(0).unwrap(), expected);
        assert_eq!(image.frame_count(), 1);
        let bigger = raster_pages(
            &source,
            Projection::new(192.0, 0.8).unwrap(),
            &RasterCancellation::default(),
        )
        .unwrap();
        assert!((i64::from(bigger[0].width()) - 2 * i64::from(pages[0].width())).abs() <= 1);
        assert!((i64::from(bigger[0].height()) - 2 * i64::from(pages[0].height())).abs() <= 1);
    }
    #[test]
    fn font_missing_invalid_spec_and_missing_sample_glyphs_are_errors() {
        assert!(load_font_spec(Some("invalid")).is_err());
        assert!(load_font_spec(Some("/no-such-joyword-test-font:0")).is_err());
        assert!(load_font_spec(Some("test:face")).is_err());
        assert!(source_with_text(font(), SEED).is_err());
    }
    #[test]
    fn cancelled_or_stale_layout_does_not_create_images() {
        let mut source = source_with_text(font(), "AV\nfi").unwrap();
        let cancel = RasterCancellation::default();
        cancel.cancel();
        assert!(raster_pages(&source, Projection::new(96.0, 1.0).unwrap(), &cancel).is_err());
        source.session.set_policy(EditPolicy::default());
        let at = start(&source);
        source
            .session
            .apply(Transaction::new(
                source.session.version(),
                vec![Command::InsertText {
                    at,
                    text: "A".into(),
                }],
            ))
            .unwrap();
        assert!(
            raster_pages(
                &source,
                Projection::new(96.0, 1.0).unwrap(),
                &RasterCancellation::default()
            )
            .is_err()
        );
    }
}
