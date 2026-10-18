//! Zoom, raster cache, and PDF consume one laid-out scene.
//!
//! Geometry stays in EMU. Zoom only changes the screen projection. The PDF
//! writer is given those same scene positions, and the content stream's text
//! matrices are read back from the bytes `joyword_pdf::render` returns.

use std::io::Read;
use std::sync::Arc;

use joyword_foundation::{Emu, ScreenScale};
use joyword_layout::{
    FaceSet, FeatureSet, FontSlot, GlyphRun, LayoutCancellation, LayoutLimits, LayoutSnapshot,
    LoadedFont, SingleFontProfile, adjust_cjk_break, layout_document, prohibited_line_edge,
    slot_for, to_points, to_screen, wrap_mixed,
};
use joyword_model::{Bias, BlockSeq, Document, FontHint, Measure, PageSetup, Paragraph, Position};
use joyword_render::{NeighborhoodCache, NeighborhoodRequest, Projection, RasterCancellation};

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

fn font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap()
}

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    }
}

fn document() -> Document {
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(100).raw());
    page.height = Measure::Emu(pt(12 + 18 * 3).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut blocks = Vec::new();
    for text in ["AV fi", "Hello", "AV", "fi"] {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text(text, None, Arc::default()).unwrap();
        blocks.push(paragraph.into());
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    document
}

fn lay(document: &Document, font: &LoadedFont) -> LayoutSnapshot {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(font),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap()
}

fn round_trip_points(length: Emu, zoom: f64) -> f64 {
    let scale = ScreenScale::new(zoom).unwrap();
    to_screen(length, scale) / zoom * 72.0 / 96.0
}

fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 0.01
}

/// Page position of a glyph origin. Font offsets are y-up; the scene is y-down.
fn glyph_origin(run: &GlyphRun, index: usize) -> (Emu, Emu) {
    let glyph = &run.glyphs[index];
    (
        Emu::new(run.glyph_x(index).raw() + glyph.x_offset.raw()),
        Emu::new(run.baseline_y.raw() - glyph.y_offset.raw()),
    )
}

fn inflate_streams(pdf: &[u8]) -> Vec<Vec<u8>> {
    let mut streams = Vec::new();
    let mut rest = pdf;
    while let Some(index) = rest.windows(6).position(|window| window == b"stream") {
        let after = &rest[index + 6..];
        let data = if after.starts_with(b"\r\n") {
            &after[2..]
        } else if let Some(stripped) = after.strip_prefix(b"\n") {
            stripped
        } else {
            rest = &rest[index + 6..];
            continue;
        };
        let mut decoder = flate2::read::ZlibDecoder::new(data);
        let mut inflated = Vec::new();
        if decoder.read_to_end(&mut inflated).is_ok() && !inflated.is_empty() {
            streams.push(inflated);
        }
        rest = if data.is_empty() { &[] } else { &data[1..] };
    }
    streams
}

fn text_matrices(pdf: &[u8]) -> Vec<(f64, f64)> {
    let mut matrices = Vec::new();
    for stream in inflate_streams(pdf) {
        let text = String::from_utf8_lossy(&stream);
        let tokens: Vec<&str> = text.split_whitespace().collect();
        for (index, token) in tokens.iter().enumerate() {
            // krilla writes `Tm` immediately against the following operand.
            let operator = *token == "Tm"
                || token.starts_with("Tm[")
                || token.starts_with("Tm(")
                || token.starts_with("Tm/");
            if !operator || index < 6 {
                continue;
            }
            let numbers: Vec<f64> = tokens[index - 6..index]
                .iter()
                .filter_map(|token| token.parse().ok())
                .collect();
            if numbers.len() == 6 {
                matrices.push((numbers[4], numbers[5]));
            }
        }
    }
    matrices
}

#[test]
fn zoom_overlay_cache_rebuild_and_pdf_glyphs_share_one_scene() {
    let font = font();
    let document = document();
    let first = lay(&document, &font);
    let second = lay(&document, &font);
    assert_eq!(first.geometry(), second.geometry());
    assert_eq!(first.pages(), second.pages());
    assert_eq!(first.lines(), second.lines());
    assert!(first.pages().len() >= 2);
    assert!(first.pages().iter().any(|page| !page.runs.is_empty()));

    let line = &first.lines()[0];
    let at = |offset| Position {
        story: document.body_id(),
        paragraph: line.paragraph,
        offset,
        bias: Bias::After,
    };
    let start = at(line.carets[0].offset);
    let end = at(line.carets.last().unwrap().offset);
    let rects = first.selection_rects(start, end).unwrap();
    assert_eq!(rects, second.selection_rects(start, end).unwrap());
    assert!(!rects.is_empty());
    for placed in first.lines() {
        for stop in &placed.carets {
            let hit = first.hit_test(placed.page, stop.x, placed.top).unwrap();
            let caret = first.caret(hit).unwrap();
            assert_eq!(caret.x, stop.x);
            assert_eq!(caret.page, placed.page);
            assert_eq!(second.caret(hit).unwrap(), caret);
        }
    }

    let geometry = (first.pages().to_vec(), first.lines().to_vec(), rects);
    for zoom in [0.5, 1.0, 1.37, 2.0, 4.0] {
        for page in first.pages() {
            assert!(near(
                round_trip_points(page.width, zoom),
                to_points(page.width)
            ));
            assert!(near(
                round_trip_points(page.height, zoom),
                to_points(page.height)
            ));
            for run in &page.runs {
                assert_eq!(run.font, font.key());
                assert!(near(
                    round_trip_points(run.origin_x, zoom),
                    to_points(run.origin_x)
                ));
                assert!(near(
                    round_trip_points(run.baseline_y, zoom),
                    to_points(run.baseline_y)
                ));
                for index in 0..run.glyphs.len() {
                    let (x, y) = glyph_origin(run, index);
                    assert!(
                        near(round_trip_points(x, zoom), to_points(x)),
                        "{zoom} {x:?}"
                    );
                    assert!(
                        near(round_trip_points(y, zoom), to_points(y)),
                        "{zoom} {y:?}"
                    );
                }
            }
        }
        for placed in first.lines() {
            assert!(near(
                round_trip_points(placed.top, zoom),
                to_points(placed.top)
            ));
            for stop in &placed.carets {
                assert!(near(round_trip_points(stop.x, zoom), to_points(stop.x)));
            }
        }
        for rect in &geometry.2 {
            assert!(near(round_trip_points(rect.x, zoom), to_points(rect.x)));
            assert!(near(
                round_trip_points(rect.width, zoom),
                to_points(rect.width)
            ));
        }
    }
    assert_eq!(first.pages(), geometry.0.as_slice());
    assert_eq!(first.lines(), geometry.1.as_slice());

    let pdf = joyword_pdf::render(first.pages(), std::slice::from_ref(&font)).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let matrices = text_matrices(&pdf);
    assert!(
        !matrices.is_empty(),
        "PDF content stream has no text matrix"
    );
    let mut glyph_points = Vec::new();
    for page in first.pages() {
        for run in &page.runs {
            let x = to_points(run.origin_x);
            let y = to_points(run.baseline_y);
            glyph_points.push((x, y));
            assert!(
                matrices.iter().any(|(mx, my)| near(*mx, x) && near(*my, y)),
                "no PDF text matrix within 0.01 pt of run origin ({x}, {y}); saw {matrices:?}"
            );
            for index in 0..run.glyphs.len() {
                let (glyph_x, glyph_y) = glyph_origin(run, index);
                glyph_points.push((to_points(glyph_x), to_points(glyph_y)));
            }
        }
    }
    for (mx, my) in &matrices {
        assert!(
            glyph_points
                .iter()
                .any(|(x, y)| near(*mx, *x) && near(*my, *y)),
            "PDF text matrix ({mx}, {my}) is not a scene glyph origin"
        );
    }

    let projection = Projection::new(96.0, 2.0).unwrap();
    let wider = Projection::new(96.0, 1.0).unwrap();
    assert!(
        (projection.pixels(first.pages()[0].width) - 2.0 * wider.pixels(first.pages()[0].width))
            .abs()
            < 1e-6
    );
    let mut cache = NeighborhoodCache::new(2);
    let cancel = RasterCancellation::default();
    cache
        .raster_neighborhood(NeighborhoodRequest {
            identity: 7,
            scenes: first.pages(),
            fonts: std::slice::from_ref(&font),
            projection,
            focus: 0,
            radius: 0,
            cancellation: &cancel,
        })
        .unwrap();
    let page = cache.cached(7, projection, 0).unwrap().clone();
    assert!(page.stats().glyphs > 0);
    let page_width = page.width();
    let page_height = page.height();
    let bytes = page.into_bgra();
    assert!(!bytes.is_empty());
    let draws = cache.draws();
    cache
        .raster_neighborhood(NeighborhoodRequest {
            identity: 7,
            scenes: first.pages(),
            fonts: std::slice::from_ref(&font),
            projection,
            focus: 0,
            radius: 0,
            cancellation: &cancel,
        })
        .unwrap();
    assert_eq!(cache.draws(), draws, "cache rebuild must not redraw");
    assert_eq!(
        cache.cached(7, projection, 0).unwrap().clone().into_bgra(),
        bytes
    );
    let caret = &first.lines()[0];
    let overlay = joyword_render::paint_caret_overlay(
        &bytes,
        page_width,
        page_height,
        projection,
        caret.carets[0].x,
        caret.top,
        caret.height,
    );
    assert_ne!(overlay, bytes, "the overlay buffer is a separate image");
    assert_eq!(
        cache.cached(7, projection, 0).unwrap().clone().into_bgra(),
        bytes,
        "painting the caret overlay must not change cached page bytes"
    );

    let scenes = vec![blank_scene(); 100];
    let mut long = NeighborhoodCache::new(5);
    let first_pages = long
        .raster_neighborhood(NeighborhoodRequest {
            identity: 1,
            scenes: &scenes,
            fonts: &[],
            projection,
            focus: 0,
            radius: 1,
            cancellation: &cancel,
        })
        .unwrap();
    assert_eq!(first_pages, vec![0, 1]);
    let far = long
        .raster_neighborhood(NeighborhoodRequest {
            identity: 1,
            scenes: &scenes,
            fonts: &[],
            projection,
            focus: 99,
            radius: 1,
            cancellation: &cancel,
        })
        .unwrap();
    assert!(far.contains(&99));
    let far_bytes = long.cached(1, projection, 99).unwrap().clone().into_bgra();
    assert!(!far_bytes.is_empty());
    let before = long.draws();
    long.raster_neighborhood(NeighborhoodRequest {
        identity: 1,
        scenes: &scenes,
        fonts: &[],
        projection,
        focus: 99,
        radius: 0,
        cancellation: &cancel,
    })
    .unwrap();
    assert_eq!(long.draws(), before);
    assert_eq!(
        long.cached(1, projection, 99).unwrap().clone().into_bgra(),
        far_bytes
    );

    let text = "甲乙。丙";
    let naive = text.find('。').unwrap();
    let adjusted = adjust_cjk_break(text, naive);
    assert!(adjusted < naive);
    assert!(!prohibited_line_edge(&text[adjusted..]));
    assert!(prohibited_line_edge(&text[naive..]));
    assert_eq!(slot_for('A', None), FontSlot::Ascii);
    assert_eq!(slot_for('甲', None), FontSlot::EastAsia);
    assert_eq!(slot_for('A', Some(FontHint::EastAsia)), FontSlot::EastAsia);
    let size = pt(12);
    let width = Emu::new(12 * Emu::PER_POINT * 6);
    let uniform = FaceSet::uniform(&font);
    let plain = wrap_mixed(uniform, "AAAA甲甲甲甲。BBBB", size, width, None);
    assert!(!plain.lines.is_empty());
    let cjk_path = "/System/Library/Fonts/Supplemental/Songti.ttc";
    if let Some(cjk) = std::fs::read(cjk_path)
        .ok()
        .and_then(|bytes| LoadedFont::from_bytes(Arc::from(bytes), 0).ok())
    {
        let mixed = FaceSet {
            east_asia: &cjk,
            fallback: &cjk,
            ..uniform
        };
        let shaped = wrap_mixed(mixed, "AAAA甲甲甲甲。BBBB", size, width, None);
        assert_ne!(
            plain
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            shaped
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
        );
        assert!(shaped.lines.iter().any(|line| line.font == cjk.key()));
        assert!(
            shaped
                .lines
                .iter()
                .all(|line| !prohibited_line_edge(&line.text))
        );
    }
}

fn blank_scene() -> joyword_layout::PageScene {
    joyword_layout::PageScene {
        width: Emu::new(8 * Emu::PER_INCH),
        height: Emu::new(8 * Emu::PER_INCH),
        runs: Vec::new(),
        rects: Vec::new(),
        images: Vec::new(),
    }
}
