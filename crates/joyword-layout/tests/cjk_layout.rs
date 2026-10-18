//! T207 wiring: the page scene consumes measured widths only for the face and
//! rule that evidence allows. The renamed fixture opens the 等线 name gate; its
//! outlines are still the Latin test face, so expectations are the pure
//! functions' EMU results, not a new Word measurement.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::cjk::{
    CjkFamily, face_cjk_family, measured_line_family, measured_line_grid, metric_set,
    punctuation_advance, single_line_height, word_line_span,
};
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, layout_document, shape,
};
use joyword_model::ParaProps;
use joyword_model::{
    BlockSeq, Document, Justification, LineSpacing, Measure, PageSetup, Paragraph,
};

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    }
}

fn test_font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap()
}

fn be_u16(data: &[u8], offset: usize) -> usize {
    usize::from(u16::from_be_bytes([data[offset], data[offset + 1]]))
}

/// Shorten name ID 1 to `family`. This is a name gate for tests, not a DengXian file.
fn font_named(family: &str) -> LoadedFont {
    let mut data =
        include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").to_vec();
    let tables = be_u16(&data, 4);
    let mut name_at = None;
    for index in 0..tables {
        let entry = 12 + index * 16;
        if &data[entry..entry + 4] == b"name" {
            name_at = Some(
                usize::try_from(u32::from_be_bytes(
                    data[entry + 8..entry + 12].try_into().unwrap(),
                ))
                .unwrap(),
            );
        }
    }
    let name_at = name_at.expect("name table");
    let records = be_u16(&data, name_at + 2);
    let storage = name_at + be_u16(&data, name_at + 4);
    let encoded: Vec<u8> = family.encode_utf16().flat_map(u16::to_be_bytes).collect();
    let mut patched = false;
    for index in 0..records {
        let record = name_at + 6 + index * 12;
        let platform = be_u16(&data, record);
        let name_id = be_u16(&data, record + 6);
        let length = be_u16(&data, record + 8);
        let offset = be_u16(&data, record + 10);
        if platform == 3 && name_id == 1 && length >= encoded.len() {
            let start = storage + offset;
            data[start..start + encoded.len()].copy_from_slice(&encoded);
            let len = u16::try_from(encoded.len()).unwrap();
            data[record + 8..record + 10].copy_from_slice(&len.to_be_bytes());
            patched = true;
            break;
        }
    }
    assert!(patched, "Windows family name");
    LoadedFont::from_bytes(Arc::from(data), 0).unwrap()
}

fn lay_box(
    font: &LoadedFont,
    text: &str,
    content: Emu,
    edit: impl FnOnce(&mut ParaProps),
) -> LayoutSnapshot {
    let margin = pt(6);
    lay_page(
        font,
        text,
        content.raw() + margin.raw() * 2,
        pt(400).raw(),
        edit,
    )
}

fn lay_page(
    font: &LoadedFont,
    text: &str,
    width: i64,
    height: i64,
    edit: impl FnOnce(&mut ParaProps),
) -> LayoutSnapshot {
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(width);
    page.height = Measure::Emu(height);
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    edit(Arc::make_mut(&mut paragraph.props));
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(font),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap()
}

fn lay(font: &LoadedFont, text: &str, width_pt: i64, height_pt: i64) -> LayoutSnapshot {
    lay_with(font, text, width_pt, height_pt, |_| {})
}

fn lay_with(
    font: &LoadedFont,
    text: &str,
    width_pt: i64,
    height_pt: i64,
    edit: impl FnOnce(&mut ParaProps),
) -> LayoutSnapshot {
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(width_pt).raw());
    page.height = Measure::Emu(pt(height_pt).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    edit(Arc::make_mut(&mut paragraph.props));
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(font),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap()
}

fn lay_err(
    font: &LoadedFont,
    text: &str,
    edit: impl FnOnce(&mut ParaProps),
) -> Result<LayoutSnapshot, LayoutError> {
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(200).raw());
    page.height = Measure::Emu(pt(400).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    edit(Arc::make_mut(&mut paragraph.props));
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(font),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
}

fn line_texts(snapshot: &LayoutSnapshot) -> Vec<String> {
    snapshot
        .lines()
        .iter()
        .map(|line| snapshot.pages()[line.page].runs[line.run].text.clone())
        .collect()
}

fn scene_advances(snapshot: &LayoutSnapshot) -> Vec<Emu> {
    let run = &snapshot.pages()[0].runs[0];
    let mut advances = Vec::new();
    for (index, glyph) in run.glyphs.iter().enumerate() {
        let next = run
            .glyphs
            .get(index + 1)
            .map_or(glyph.x.raw() + glyph.advance.raw(), |next| next.x.raw());
        assert_eq!(next - glyph.x.raw(), glyph.advance.raw());
        advances.push(glyph.advance);
    }
    advances
}

#[test]
fn the_test_face_is_not_a_measured_cjk_family() {
    let font = test_font();
    assert_eq!(face_cjk_family(&font), None);
    let named = font_named("DengXian");
    assert_eq!(face_cjk_family(&named), Some(CjkFamily::DengXian));
    assert!(measured_line_family(CjkFamily::DengXian));
    assert!(!measured_line_family(CjkFamily::SimHei));
}

#[test]
fn dengxian_quotes_compress_in_the_page_scene_and_other_faces_do_not() {
    let text = "\u{201C}成";
    let expected = punctuation_advance('\u{201C}', Some('成'), pt(12))
        .unwrap()
        .length;
    let plain = lay(&test_font(), text, 200, 80);
    let gated = lay(&font_named("DengXian"), text, 200, 80);
    let plain_advances = scene_advances(&plain);
    let gated_advances = scene_advances(&gated);
    assert!(plain_advances[0].raw() > expected.raw());
    assert_eq!(gated_advances[0], expected);
    assert_eq!(gated_advances[1], plain_advances[1]);
    assert_eq!(
        plain.lines()[0].height,
        profile().leading,
        "an unmeasured face keeps the profile line box"
    );
}

#[test]
fn consecutive_dengxian_marks_use_the_measured_half_width_until_the_tail() {
    let text = "：：";
    let half = punctuation_advance('：', Some('：'), pt(12))
        .unwrap()
        .length;
    assert_eq!(half, pt(6));
    let plain = lay(&test_font(), text, 200, 80);
    let gated = lay(&font_named("DengXian"), text, 200, 80);
    let plain_advances = scene_advances(&plain);
    let gated_advances = scene_advances(&gated);
    assert_eq!(gated_advances[0], half);
    assert!(half.raw() < plain_advances[0].raw());
    // No follower: R7 returns unverified, so the tail keeps the shaped advance.
    assert_eq!(gated_advances[1], plain_advances[1]);
}

#[test]
fn unverified_punctuation_keeps_the_shaped_advance_on_a_dengxian_face() {
    let text = ",字";
    let plain = lay(&test_font(), text, 200, 80);
    let gated = lay(&font_named("DengXian"), text, 200, 80);
    assert_eq!(scene_advances(&plain), scene_advances(&gated));
}

#[test]
fn isolated_stress_punctuation_does_not_change_breaks_when_the_face_is_dengxian() {
    let stress = include_str!("../../../tests/fixtures/reference/word2024/breaks/stress.txt");
    let text = stress.trim_end_matches(['\r', '\n']);
    let plain = lay(&test_font(), text, 120, 4000);
    let gated = lay(&font_named("DengXian"), text, 120, 4000);
    assert_eq!(line_texts(&plain), line_texts(&gated));
    assert_eq!(plain.lines().len(), gated.lines().len());
    assert!(plain.lines().len() > 1);
}

#[test]
fn pure_chinese_justification_fills_every_line_except_the_last() {
    let font = test_font();
    let one = shape(&font, "甲", pt(12), FeatureSet::default()).width;
    assert!(one.raw() > 1);
    let content = one.raw() * 2 + one.raw() / 2;
    assert!(content < one.raw() * 3);
    let start = lay_box(&font, "甲乙丙丁", Emu::new(content), |_| {});
    let both = lay_box(&font, "甲乙丙丁", Emu::new(content), |props| {
        props.justification = Some(Justification::Both);
    });
    assert_eq!(line_texts(&start), line_texts(&both));
    assert_eq!(both.lines().len(), 2);
    let content_right = both.geometry().left.raw() + content;
    let edge = |snapshot: &LayoutSnapshot, index: usize| {
        snapshot.lines()[index].carets.last().unwrap().x.raw()
    };
    assert_eq!(edge(&both, 0), content_right);
    assert!(edge(&start, 0) < content_right);
    assert_eq!(edge(&both, 1), edge(&start, 1));
    assert!(edge(&both, 1) < content_right);
    scene_advances(&both);
}

#[test]
fn a_single_character_justified_line_is_not_given_an_edge_gap() {
    let font = test_font();
    let one = shape(&font, "甲", pt(12), FeatureSet::default()).width;
    let content = one.raw() + one.raw() / 2;
    let start = lay_box(&font, "甲乙", Emu::new(content), |_| {});
    let both = lay_box(&font, "甲乙", Emu::new(content), |props| {
        props.justification = Some(Justification::Both);
    });
    assert!(both.lines().len() >= 2);
    let edge = |snapshot: &LayoutSnapshot, index: usize| {
        snapshot.lines()[index].carets.last().unwrap().x.raw()
    };
    assert_eq!(edge(&both, 0), edge(&start, 0));
    assert!(edge(&both, 0) < both.geometry().left.raw() + content);
}

#[test]
fn latin_and_distribute_justification_stay_unsupported() {
    let font = test_font();
    assert!(matches!(
        lay_err(&font, "AV", |props| {
            props.justification = Some(Justification::Both);
        }),
        Err(LayoutError::Unsupported { .. })
    ));
    assert!(matches!(
        lay_err(&font, "甲乙", |props| {
            props.justification = Some(Justification::Distribute);
        }),
        Err(LayoutError::Unsupported { .. })
    ));
    assert!(matches!(
        lay_err(&font, "甲A", |props| {
            props.justification = Some(Justification::Both);
        }),
        Err(LayoutError::Unsupported { .. })
    ));
}

#[test]
fn single_spacing_snaps_only_for_a_measured_family() {
    let named = font_named("DengXian");
    let metrics = metric_set(&named).unwrap();
    let snapped = single_line_height(
        pt(12),
        word_line_span(&metrics),
        measured_line_grid(true).unwrap(),
    )
    .unwrap();
    let natural = single_line_height(
        pt(12),
        word_line_span(&metrics),
        measured_line_grid(false).unwrap(),
    )
    .unwrap();
    assert_ne!(snapped.length, profile().leading);
    assert_ne!(natural.length, profile().leading);
    assert!(snapped.length.raw() > natural.length.raw());
    assert_eq!(
        snapped.length.raw() % measured_line_grid(true).unwrap().pitch.raw(),
        0
    );

    let on = lay_with(&named, "甲乙丙丁戊己", 40, 800, |props| {
        props.spacing.line = Some(LineSpacing::Auto(240));
    });
    assert!(on.lines().len() >= 2);
    assert!(on.lines().iter().all(|line| line.height == snapped.length));
    assert_eq!(
        on.lines()[1].top.raw() - on.lines()[0].top.raw(),
        snapped.length.raw()
    );

    let off = lay_with(&named, "甲乙丙丁戊己", 40, 800, |props| {
        props.spacing.line = Some(LineSpacing::Auto(240));
        props.snap_to_grid = Some(false);
    });
    assert!(off.lines().iter().all(|line| line.height == natural.length));

    assert!(matches!(
        lay_err(&test_font(), "甲乙", |props| {
            props.spacing.line = Some(LineSpacing::Auto(240));
        }),
        Err(LayoutError::Unsupported { .. })
    ));
    assert!(matches!(
        lay_err(&named, "甲乙", |props| {
            props.spacing.line = Some(LineSpacing::Auto(480));
        }),
        Err(LayoutError::Unsupported { .. })
    ));
    assert!(matches!(
        lay_err(&named, "甲乙", |props| {
            props.snap_to_grid = Some(true);
        }),
        Err(LayoutError::Unsupported { .. })
    ));
}
