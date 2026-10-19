//! Mixed character formatting inside one paragraph (T105 follow-up).
//!
//! The editor splits runs at selection boundaries, so a paragraph may carry
//! several faces. Layout must paint each span with its own size, weight,
//! posture and colour instead of refusing the document. These tests pin the
//! pipeline end to end: run-span resolution, per-span shaping when sizes
//! differ, glyph-run splits at placement, and the caret geometry that the
//! editor reads back.

use std::sync::Arc;

use joyword_engine::{Command, Outcome, RunChange, Session, Transaction};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, layout_document,
};
use joyword_model::{
    Bias, Block, BlockSeq, Color, Document, Measure, Paragraph, Position, RunProps, ScalarOffset,
    Underline,
};
use joyword_render::{Projection, RasterCancellation, RasterLimits, rasterize};

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
fn layout(document: &Document) -> LayoutSnapshot {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap()
}
/// One paragraph from `(text, props)` pairs, one run per pair.
fn make_document(parts: &[(&str, RunProps)]) -> Document {
    let mut page = joyword_model::PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(100).raw());
    page.height = Measure::Emu(pt(12 + 18 * 3).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut paragraph = Paragraph::new(document.ids().node());
    for (text, props) in parts {
        paragraph
            .push_text(text, None, Arc::new(props.clone()))
            .unwrap();
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![Block::from(paragraph)]);
    assert!(!document.validate().has_errors());
    document
}
fn bold() -> RunProps {
    RunProps {
        bold: Some(true),
        ..RunProps::default()
    }
}
fn line_runs(snapshot: &LayoutSnapshot, line: usize) -> Vec<joyword_layout::GlyphRun> {
    let placed = &snapshot.lines()[line];
    let page = &snapshot.pages()[placed.page];
    let end = snapshot
        .lines()
        .get(line + 1)
        .filter(|next| next.page == placed.page)
        .map(|next| next.run)
        .unwrap_or(page.runs.len());
    page.runs[placed.run..end].to_vec()
}
fn position(document: &Document, offset: usize, bias: Bias) -> Position {
    Position {
        story: document.body_id(),
        paragraph: document.body().blocks.get(0).unwrap().id(),
        offset: ScalarOffset(offset),
        bias,
    }
}

#[test]
fn partial_bold_paints_two_glyph_runs_with_contiguous_positions() {
    let document = make_document(&[("AB", RunProps::default()), ("CD", bold())]);
    let snapshot = layout(&document);
    assert_eq!(snapshot.lines().len(), 1);
    let runs = line_runs(&snapshot, 0);
    assert_eq!(runs.len(), 2, "one glyph run per formatting span");
    assert_eq!(runs[0].text, "AB");
    assert!(!runs[0].bold);
    assert_eq!(runs[1].text, "CD");
    assert!(runs[1].bold);
    assert_eq!(runs[0].origin_x, pt(6));
    // Each piece is a valid GlyphRun on its own: clusters and glyph x
    // restart at zero, and its origin carries the first span's advance, so
    // the second span continues exactly where the first one ended.
    assert_eq!(runs[1].glyphs[0].x.raw(), 0);
    assert_eq!(
        runs[1].origin_x,
        Emu::new(runs[0].origin_x.raw() + runs[0].advance())
    );
    // The same baseline and line box for both spans.
    assert_eq!(runs[0].baseline_y, runs[1].baseline_y);
    // Caret geometry is untouched by the split: one stop per boundary.
    assert_eq!(snapshot.lines()[0].carets.len(), 5);
}

#[test]
fn uniform_paragraph_still_paints_one_glyph_run() {
    let document = make_document(&[("ABCD", RunProps::default())]);
    let snapshot = layout(&document);
    assert_eq!(line_runs(&snapshot, 0).len(), 1);
}

#[test]
fn bold_span_does_not_move_line_breaks() {
    let text = "aaa bbb ccc ddd eee fff ggg hhh";
    let plain = make_document(&[(text, RunProps::default())]);
    let mixed = make_document(&[
        ("aaa ", RunProps::default()),
        ("bbb ccc", bold()),
        (" ddd eee fff ggg hhh", RunProps::default()),
    ]);
    let plain = layout(&plain);
    let mixed = layout(&mixed);
    let sources = |snapshot: &LayoutSnapshot| {
        snapshot
            .lines()
            .iter()
            .map(|line| (line.source.bytes.clone(), line.source.scalars.clone()))
            .collect::<Vec<_>>()
    };
    assert!(plain.lines().len() > 1, "the sample must wrap");
    assert_eq!(sources(&plain), sources(&mixed));
}

#[test]
fn mixed_sizes_shape_per_span_and_take_the_tallest_line_box() {
    let large = RunProps {
        size: Some(Measure::HalfPoints(48)),
        ..RunProps::default()
    };
    let document = make_document(&[("ab", RunProps::default()), ("cd", large)]);
    let snapshot = layout(&document);
    assert_eq!(snapshot.lines().len(), 1);
    // The line box follows the 24 pt run: leading 18 pt scaled by 24/12.
    assert_eq!(snapshot.lines()[0].height, pt(36));
    let runs = line_runs(&snapshot, 0);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].size, pt(12));
    assert_eq!(runs[1].size, pt(24));
    // The 24 pt span shaped at its own size: wider than the 12 pt span.
    assert!(runs[1].advance() > runs[0].advance());
    assert_eq!(runs[0].baseline_y, runs[1].baseline_y);
}

#[test]
fn italic_colour_and_underline_follow_their_spans() {
    let marked = RunProps {
        italic: Some(true),
        color: Some(Color::Rgb(200, 0, 0)),
        underline: Some(Underline::Single),
        ..RunProps::default()
    };
    let document = make_document(&[
        ("AA", RunProps::default()),
        ("BB", marked),
        ("CC", RunProps::default()),
    ]);
    let snapshot = layout(&document);
    let runs = line_runs(&snapshot, 0);
    assert_eq!(runs.len(), 3);
    assert!(!runs[0].italic && !runs[2].italic);
    assert!(runs[1].italic);
    assert_eq!(runs[1].color, [200, 0, 0]);
    assert!(runs[1].underline.is_some());
    assert!(runs[0].underline.is_none() && runs[2].underline.is_none());
    // The underline stroke is a scene rect in the span's own colour.
    assert!(
        snapshot
            .pages()
            .iter()
            .flat_map(|page| page.rects.iter())
            .any(|rect| rect.red == 200 && rect.green == 0 && rect.blue == 0)
    );
}

#[test]
fn engine_selection_bold_no_longer_refuses_the_document() {
    let document = make_document(&[("ABCD", RunProps::default())]);
    let mut session = Session::new(document);
    let outcome = session
        .apply(Transaction::new(
            session.document().version(),
            vec![Command::FormatText {
                start: position(session.document(), 2, Bias::After),
                end: position(session.document(), 4, Bias::Before),
                change: RunChange::Bold(Some(true)),
            }],
        ))
        .unwrap();
    assert!(matches!(outcome, Outcome::Applied(_)));
    let snapshot = layout(session.document());
    let runs = line_runs(&snapshot, 0);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].text, "AB");
    assert!(!runs[0].bold);
    assert_eq!(runs[1].text, "CD");
    assert!(runs[1].bold);
}

/// Every split piece must raster on its own: the rasterizer revalidates each
/// glyph run and refuses runs whose clusters or pen positions do not restart
/// at zero. This pins the rebase that keeps `origin_x + glyph.x` faithful.
#[test]
fn partial_formatting_scenes_rasterize() {
    let raster = |snapshot: &LayoutSnapshot| {
        rasterize(
            &snapshot.pages()[0],
            &[font()],
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits::default(),
            &RasterCancellation::default(),
        )
        .unwrap()
    };
    let bold_document = make_document(&[("AB", RunProps::default()), ("CD", bold())]);
    let rastered = raster(&layout(&bold_document));
    assert!(rastered.width() > 0 && rastered.height() > 0);

    let large = RunProps {
        size: Some(Measure::HalfPoints(48)),
        ..RunProps::default()
    };
    let sizes_document = make_document(&[("ab", RunProps::default()), ("cd", large)]);
    let rastered = raster(&layout(&sizes_document));
    assert!(rastered.width() > 0 && rastered.height() > 0);
}
