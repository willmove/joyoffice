//! T205 slice 3: underline and sRGB on the shared page scene, view options in
//! the snapshot key, and cache equality for each option.
//!
//! Underline position and thickness come from the font (`post`, then
//! `OS/2.yStrikeoutSize`, then `unitsPerEm / 20`). Double and dotted repeat
//! that one stroke; the gap is not a Word measurement. Formatting marks are
//! gray rectangles added after line breaking.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutInvalidation, LayoutLimits,
    LayoutReuse, LayoutSnapshot, LoadedFont, PaintedUnderline, ParagraphLayoutCache,
    SingleFontProfile, UnderlineMetricSource, ViewOptions, layout_incremental_with_view,
    layout_with_view, resolve_underline_metrics,
};
use joyword_model::{
    Anchor, AnnotationKind, Bias, BlockSeq, Color, Document, NodeId, PageSetup, Paragraph,
    Position, RunProps, ScalarOffset, StoryId, ThemeColorSlot, ThemeScheme, Underline,
    apply_tint_shade,
};

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

fn paragraph_text(document: &mut Document, text: &str, props: RunProps) -> Paragraph {
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text(text, None, Arc::new(props))
        .expect("text");
    paragraph
}

fn lay(document: &Document, view: ViewOptions) -> LayoutSnapshot {
    layout_with_view(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        view,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

fn lay_cached(
    document: &Document,
    view: ViewOptions,
    cache: &mut ParagraphLayoutCache,
    previous: Option<&LayoutSnapshot>,
) -> LayoutSnapshot {
    let invalidation = LayoutInvalidation::default();
    layout_incremental_with_view(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        LayoutReuse {
            cache,
            previous,
            invalidation: &invalidation,
        },
        view,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

fn same_scene(full: &LayoutSnapshot, cached: &LayoutSnapshot) {
    assert_eq!(full.key(), cached.key());
    assert_eq!(full.pages(), cached.pages());
    assert_eq!(full.lines().len(), cached.lines().len());
    for (left, right) in full.lines().iter().zip(cached.lines()) {
        assert_eq!(left.paragraph, right.paragraph);
        assert_eq!(left.source, right.source);
        assert_eq!(left.page, right.page);
        assert_eq!(left.top, right.top);
        assert_eq!(left.baseline, right.baseline);
        assert_eq!(left.height, right.height);
        assert_eq!(left.tab_gaps, right.tab_gaps);
    }
}

fn cache_matches(document: &Document, view: ViewOptions) {
    let full = lay(document, view);
    let mut cache = ParagraphLayoutCache::default();
    let cold = lay_cached(document, view, &mut cache, None);
    same_scene(&full, &cold);
    let warm = lay_cached(document, view, &mut cache, Some(&cold));
    same_scene(&full, &warm);
}

fn page_text(snapshot: &LayoutSnapshot) -> String {
    snapshot
        .pages()
        .iter()
        .flat_map(|page| page.runs.iter().map(|run| run.text.as_str()))
        .collect()
}

fn position(story: StoryId, paragraph: NodeId, offset: usize) -> Position {
    Position {
        story,
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::Before,
    }
}

#[test]
fn underline_metrics_prefer_post_then_strikeout_then_em() {
    let post = resolve_underline_metrics(Some((-100, 50)), Some(40), 1000);
    assert_eq!(post.position, -100);
    assert_eq!(post.thickness, 50);
    assert_eq!(post.source, UnderlineMetricSource::Post);
    let on_baseline = resolve_underline_metrics(Some((0, 20)), None, 1000);
    assert_eq!(on_baseline.position, 0);
    assert_eq!(on_baseline.thickness, 20);
    assert_eq!(on_baseline.source, UnderlineMetricSource::Post);
    let strikeout = resolve_underline_metrics(Some((0, 0)), Some(40), 1000);
    assert_eq!(strikeout.position, -40);
    assert_eq!(strikeout.thickness, 40);
    assert_eq!(strikeout.source, UnderlineMetricSource::Os2StrikeoutSize);
    let fallback = resolve_underline_metrics(None, None, 1000);
    assert_eq!(fallback.thickness, 50);
    assert_eq!(fallback.position, -50);
    assert_eq!(fallback.source, UnderlineMetricSource::EmFallback);
}

#[test]
fn single_underline_and_srgb_are_on_the_shared_scene() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let props = RunProps {
        underline: Some(Underline::Single),
        color: Some(Color::Rgb(180, 20, 20)),
        ..RunProps::default()
    };
    let paragraph = paragraph_text(&mut document, "Word", props);
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let snapshot = lay(&document, ViewOptions::default());
    let run = &snapshot.pages()[0].runs[0];
    assert_eq!(run.color, [180, 20, 20]);
    let paint = run.underline.expect("single underline");
    assert_eq!(paint.kind, PaintedUnderline::Single);
    assert!(paint.thickness.raw() >= 1);
    let line = &snapshot.lines()[0];
    let rect = snapshot.pages()[0]
        .rects
        .iter()
        .find(|rect| rect.red == 180 && rect.green == 20 && rect.blue == 20)
        .expect("underline rect");
    assert_eq!(rect.width.raw(), run.advance());
    assert_eq!(rect.height.raw(), paint.thickness.raw());
    assert_eq!(rect.x.raw(), run.origin_x.raw());
    assert_eq!(
        rect.y.raw(),
        line.baseline.raw().saturating_add(paint.offset.raw())
    );
    cache_matches(&document, ViewOptions::default());
}

#[test]
fn theme_colour_text_paints_the_resolved_srgb() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let mut theme = ThemeScheme::default();
    theme
        .colors
        .insert(ThemeColorSlot::Accent1, (0x2E, 0x75, 0xB6));
    document.styles_mut().set_theme(theme);
    let props = RunProps {
        color: Some(Color::Theme {
            slot: ThemeColorSlot::Accent1,
            tint: Some(0x99),
            shade: Some(0xBF),
        }),
        ..RunProps::default()
    };
    let paragraph = paragraph_text(&mut document, "色", props);
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let snapshot = lay(&document, ViewOptions::default());
    let (red, green, blue) = apply_tint_shade(0x2E, 0x75, 0xB6, Some(0x99), Some(0xBF));
    assert_eq!(snapshot.pages()[0].runs[0].color, [red, green, blue]);
    assert_ne!([red, green, blue], [0, 0, 0]);
    cache_matches(&document, ViewOptions::default());

    let mut bare = Document::new(PageSetup::a4_zh_cn());
    let props = RunProps {
        color: Some(Color::theme(ThemeColorSlot::Accent1)),
        ..RunProps::default()
    };
    let paragraph = paragraph_text(&mut bare, "色", props);
    let body = bare.body_id();
    bare.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let error = layout_with_view(
        &bare.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        ViewOptions::default(),
    )
    .expect_err("a theme colour with no scheme entry is not painted");
    let LayoutError::Unsupported { feature, .. } = error else {
        panic!("{error}");
    };
    assert!(
        feature.contains("style") || feature.contains("colour"),
        "{feature}"
    );
}

#[test]
fn double_and_dotted_repeat_the_same_stroke() {
    for kind in [Underline::Double, Underline::Dotted] {
        let mut document = Document::new(PageSetup::a4_zh_cn());
        let props = RunProps {
            underline: Some(kind),
            color: Some(Color::Rgb(10, 20, 30)),
            ..RunProps::default()
        };
        let paragraph = paragraph_text(&mut document, "Word", props);
        let body = document.body_id();
        document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
        let snapshot = lay(&document, ViewOptions::default());
        let run = &snapshot.pages()[0].runs[0];
        let paint = run.underline.expect("underline");
        let rects: Vec<_> = snapshot.pages()[0]
            .rects
            .iter()
            .filter(|rect| rect.red == 10 && rect.green == 20 && rect.blue == 30)
            .collect();
        match kind {
            Underline::Double => {
                assert_eq!(paint.kind, PaintedUnderline::Double);
                assert_eq!(rects.len(), 2);
                assert_eq!(
                    rects[1].y.raw(),
                    rects[0].y.raw() + paint.thickness.raw() * 2
                );
            }
            Underline::Dotted => {
                assert_eq!(paint.kind, PaintedUnderline::Dotted);
                assert!(rects.len() > 1, "{}", rects.len());
                assert!(
                    rects
                        .iter()
                        .all(|rect| rect.width.raw() <= paint.thickness.raw())
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn dash_underline_stays_unpainted() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let props = RunProps {
        underline: Some(Underline::Dash),
        ..RunProps::default()
    };
    let paragraph = paragraph_text(&mut document, "Word", props);
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let error = layout_with_view(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        ViewOptions::default(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            LayoutError::Unsupported {
                feature: "character formatting",
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn formatting_marks_do_not_change_line_breaks() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let paragraph = paragraph_text(&mut document, "a b\tc", RunProps::default());
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let off = lay(&document, ViewOptions::default());
    let on = lay(
        &document,
        ViewOptions {
            formatting_marks: true,
            ..ViewOptions::default()
        },
    );
    assert_ne!(off.key(), on.key());
    assert_eq!(off.key().content_hash(), on.key().content_hash());
    assert_eq!(off.lines().len(), on.lines().len());
    for (left, right) in off.lines().iter().zip(on.lines()) {
        assert_eq!(left.source, right.source);
        assert_eq!(left.top, right.top);
        assert_eq!(left.baseline, right.baseline);
        assert_eq!(left.tab_gaps, right.tab_gaps);
    }
    assert_eq!(
        off.pages()[0].runs[0].advance(),
        on.pages()[0].runs[0].advance()
    );
    assert!(
        off.pages()[0]
            .rects
            .iter()
            .all(|rect| rect.red != 128 || rect.green != 128 || rect.blue != 128)
    );
    assert!(
        on.pages()[0]
            .rects
            .iter()
            .any(|rect| { rect.red == 128 && rect.green == 128 && rect.blue == 128 })
    );
    cache_matches(
        &document,
        ViewOptions {
            formatting_marks: true,
            ..ViewOptions::default()
        },
    );
}

#[test]
fn hidden_text_changes_layout_and_the_cache_matches() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("see ", None, Arc::default()).unwrap();
    let hidden = RunProps {
        hidden: Some(true),
        ..RunProps::default()
    };
    paragraph.push_text("hide", None, Arc::new(hidden)).unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let hidden_view = ViewOptions::default();
    let shown = ViewOptions {
        show_hidden: true,
        ..ViewOptions::default()
    };
    assert_eq!(page_text(&lay(&document, hidden_view)), "see ");
    assert_eq!(page_text(&lay(&document, shown)), "see hide");
    assert_ne!(
        lay(&document, hidden_view).key(),
        lay(&document, shown).key()
    );
    cache_matches(&document, hidden_view);
    cache_matches(&document, shown);
    let mut cache = ParagraphLayoutCache::default();
    let first = lay_cached(&document, hidden_view, &mut cache, None);
    let second = lay_cached(&document, shown, &mut cache, Some(&first));
    same_scene(&lay(&document, shown), &second);
}

#[test]
fn field_codes_swap_instruction_and_result() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let paragraph = paragraph_text(&mut document, "INSTR RESULT", RunProps::default());
    let id = paragraph.id();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let story = document.body_id();
    document.insert_annotation(
        AnnotationKind::Field {
            separator: Some(position(story, id, 6)),
        },
        Anchor::Range {
            start: position(story, id, 0),
            end: position(story, id, 12),
        },
    );
    let results = ViewOptions::default();
    let codes = ViewOptions {
        field_codes: true,
        ..ViewOptions::default()
    };
    assert_eq!(page_text(&lay(&document, results)), "RESULT");
    assert_eq!(page_text(&lay(&document, codes)), "INSTR ");
    cache_matches(&document, results);
    cache_matches(&document, codes);
}

#[test]
fn bookmarks_are_metadata_at_layout_and_handoff_not_fabricated_geometry() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let paragraph = paragraph_text(&mut document, "Word", RunProps::default());
    let story = document.body_id();
    let point = position(story, paragraph.id(), 2);
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let plain = lay(&document, ViewOptions::default());
    for anchor in [
        Anchor::Range {
            start: point,
            end: point,
        },
        Anchor::Detached,
    ] {
        document.insert_annotation(
            AnnotationKind::Bookmark {
                name: Arc::from("target"),
            },
            anchor,
        );
        assert!(!document.validate().has_errors());
        let snapshot = lay(&document, ViewOptions::default());
        snapshot
            .ensure_current(&document.snapshot(), &font(), profile())
            .unwrap();
        assert_eq!(snapshot.pages(), plain.pages());
        assert_eq!(snapshot.lines().len(), plain.lines().len());
        assert_eq!(snapshot.caret(point).unwrap(), plain.caret(point).unwrap());
        cache_matches(&document, ViewOptions::default());
    }
}

#[test]
fn malformed_bookmark_ranges_still_fail_model_layout_and_handoff() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let paragraph = paragraph_text(&mut document, "Word", RunProps::default());
    let story = document.body_id();
    let point = position(story, paragraph.id(), 2);
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let snapshot = lay(&document, ViewOptions::default());
    for (start, end) in [
        (
            point,
            Position {
                offset: ScalarOffset(5),
                ..point
            },
        ),
        (
            point,
            Position {
                offset: ScalarOffset(1),
                ..point
            },
        ),
        (
            point,
            Position {
                paragraph: document.ids().node(),
                ..point
            },
        ),
    ] {
        let mut invalid = document.clone();
        invalid.insert_annotation(
            AnnotationKind::Bookmark {
                name: Arc::from("target"),
            },
            Anchor::Range { start, end },
        );
        assert!(invalid.validate().has_errors());
        assert!(matches!(
            layout_with_view(
                &invalid.snapshot(),
                FaceSet::uniform(&font()),
                profile(),
                LayoutLimits::default(),
                &LayoutCancellation::default(),
                ViewOptions::default(),
            ),
            Err(LayoutError::InvalidDocument)
        ));
        assert!(
            snapshot
                .ensure_current(&invalid.snapshot(), &font(), profile())
                .is_err()
        );
    }
}

#[test]
fn bookmark_admission_keeps_field_and_other_annotation_refusals() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let first = paragraph_text(&mut document, "A", RunProps::default());
    let second = paragraph_text(&mut document, "B", RunProps::default());
    let story = document.body_id();
    let start = position(story, first.id(), 0);
    let end = position(story, second.id(), 1);
    document.story_mut(story).unwrap().blocks =
        BlockSeq::from_vec(vec![first.into(), second.into()]);
    for (kind, anchor) in [
        (
            AnnotationKind::Field { separator: None },
            Anchor::Range { start, end },
        ),
        (AnnotationKind::Field { separator: None }, Anchor::Detached),
        (
            AnnotationKind::Permission {
                editor: None,
                group: None,
            },
            Anchor::Range { start, end },
        ),
    ] {
        let mut unsupported = document.clone();
        unsupported.insert_annotation(kind, anchor);
        assert!(matches!(
            layout_with_view(
                &unsupported.snapshot(),
                FaceSet::uniform(&font()),
                profile(),
                LayoutLimits::default(),
                &LayoutCancellation::default(),
                ViewOptions::default(),
            ),
            Err(LayoutError::Unsupported { .. })
        ));
    }
}
