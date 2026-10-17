//! Original fixtures with independently specified page/line expectations.
use std::sync::Arc;

use joyword_engine::{Command, Session, Transaction};
use joyword_foundation::{Emu, ScreenScale};
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutLimit, LayoutLimits,
    LayoutSnapshot, LoadedFont, SingleFontProfile, StaleReason, layout_document, shape,
};
use joyword_model::{
    Bias, Block, BlockSeq, Document, DumpOptions, Justification, Measure, PageSetup, Paragraph,
    Position, ScalarOffset,
};

fn pt(n: i64) -> Emu {
    Emu::new(n * Emu::PER_POINT)
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
fn document(counts: &[usize], rows: i64) -> Document {
    // Each nonempty paragraph consists of N identical, individually breakable
    // words. The width is exactly one word, including its trailing space.
    let width = shape(&font(), "AV ", profile().size, profile().features).width;
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(width.raw() + pt(12).raw());
    page.height = Measure::Emu(pt(12 + 18 * rows).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.gutter = Measure::Emu(0);
    let mut doc = Document::new(page);
    let blocks = counts
        .iter()
        .map(|&count| {
            let mut p = Paragraph::new(doc.ids().node());
            p.push_text(&"AV ".repeat(count), None, Arc::default())
                .unwrap();
            p.into()
        })
        .collect();
    let body = doc.body_id();
    doc.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    doc
}
fn change(doc: &mut Document, index: usize, f: impl FnOnce(&mut Paragraph)) {
    let body = doc.body_id();
    doc.story_mut(body).unwrap().blocks.update(index, |b| {
        let Block::Paragraph(p) = b else {
            unreachable!()
        };
        f(Arc::make_mut(p));
    });
}
fn keep(doc: &mut Document, index: usize, next: bool, lines: bool) {
    change(doc, index, |p| {
        let props = Arc::make_mut(&mut p.props);
        props.keep_next = Some(next);
        props.keep_lines = Some(lines);
    });
}
fn run(doc: &Document, limits: LayoutLimits) -> Result<LayoutSnapshot, LayoutError> {
    layout_document(
        &doc.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        limits,
        &LayoutCancellation::default(),
    )
}
fn layout(doc: &Document) -> LayoutSnapshot {
    run(doc, LayoutLimits::default()).unwrap()
}
fn pages(result: &LayoutSnapshot) -> Vec<usize> {
    result.lines().iter().map(|l| l.page).collect()
}
fn position(doc: &Document, p: usize, offset: usize, bias: Bias) -> Position {
    Position {
        story: doc.body_id(),
        paragraph: doc.body().blocks.get(p).unwrap().id(),
        offset: ScalarOffset(offset),
        bias,
    }
}
fn verify_source_and_geometry(doc: &Document, result: &LayoutSnapshot) {
    assert!(result.pages().iter().all(|p| !p.runs.is_empty()));
    for block in doc.body().blocks.iter() {
        let p = block.as_paragraph().unwrap();
        let text = p.plain_text(|_| unreachable!());
        let mut bytes = 0;
        let mut scalars = 0;
        for line in result.lines().iter().filter(|l| l.paragraph == p.id()) {
            assert_eq!(line.source.bytes.start, bytes);
            assert_eq!(line.source.scalars.start, scalars);
            let run = &result.pages()[line.page].runs[line.run];
            assert_eq!(run.text, text[line.source.bytes.clone()]);
            assert_eq!(run.baseline_y, line.baseline);
            assert_eq!(run.font, result.key().font);
            assert_eq!(
                line.baseline.raw(),
                line.top.raw() + profile().baseline.raw()
            );
            assert!(line.top >= result.geometry().top);
            assert!(
                line.top.raw() + line.height.raw()
                    <= result.geometry().height.raw() - result.geometry().bottom.raw()
            );
            for stop in &line.carets {
                let bias = if stop.offset.0 == line.source.scalars.end
                    && !line.source.scalars.is_empty()
                {
                    Bias::Before
                } else {
                    Bias::After
                };
                let at = Position {
                    story: doc.body_id(),
                    paragraph: p.id(),
                    offset: stop.offset,
                    bias,
                };
                let caret = result.caret(at).unwrap();
                assert_eq!(
                    (caret.page, caret.top, caret.x),
                    (line.page, line.top, stop.x)
                );
                let hit = result
                    .hit_test(
                        line.page,
                        stop.x,
                        Emu::new(line.top.raw() + line.height.raw() / 2),
                    )
                    .unwrap();
                assert_eq!(result.caret(hit).unwrap(), caret);
            }
            bytes = line.source.bytes.end;
            scalars = line.source.scalars.end;
        }
        assert_eq!((bytes, scalars), (text.len(), text.chars().count()));
    }
}

#[test]
fn keep_lines_moves_whole_paragraph_and_false_keeps_original_flow() {
    let mut doc = document(&[2, 2], 3);
    assert_eq!(pages(&layout(&doc)), [0, 0, 0, 1]);
    keep(&mut doc, 1, false, true);
    let result = layout(&doc);
    assert_eq!(pages(&result), [0, 0, 1, 1]);
    assert_eq!(result.lines()[2].top, pt(6));
    verify_source_and_geometry(&doc, &result);
    keep(&mut doc, 1, false, false);
    assert_eq!(pages(&layout(&doc)), [0, 0, 0, 1]);
}

#[test]
fn empty_paragraph_inside_chain_counts_toward_group_height() {
    // Chain 1→2→3 with an empty middle paragraph: counting the empty
    // paragraph's line, the group is 3 lines and does not fit the 2 remaining
    // rows; ignoring it would fit and the chain would wrongly stay on page 0.
    let mut doc = document(&[1, 1, 0, 1], 3);
    keep(&mut doc, 1, true, false);
    keep(&mut doc, 2, true, false);
    let result = layout(&doc);
    assert_eq!(pages(&result), [0, 1, 1, 1]);
    let empty = doc.body().blocks.get(2).unwrap().id();
    let line = result
        .lines()
        .iter()
        .find(|line| line.paragraph == empty)
        .unwrap();
    assert_eq!((line.page, line.top), (1, pt(6 + 18)));
    assert!(line.source.scalars.is_empty() && line.source.bytes.is_empty());
    assert_eq!(line.carets.len(), 1);
    assert_eq!(
        result
            .caret(position(&doc, 2, 0, Bias::After))
            .unwrap()
            .page,
        1
    );
    verify_source_and_geometry(&doc, &result);
}

#[test]
fn keep_chain_includes_terminal_paragraph_and_stops_at_false() {
    let mut doc = document(&[1, 1, 1, 1, 1], 3);
    keep(&mut doc, 1, true, false);
    keep(&mut doc, 2, true, true);
    let result = layout(&doc);
    assert_eq!(pages(&result), [0, 1, 1, 1, 2]);
    verify_source_and_geometry(&doc, &result);
    // No successor: final keep-next must not act like keep-lines.
    let mut tail = document(&[2, 2], 3);
    keep(&mut tail, 1, true, false);
    assert_eq!(pages(&layout(&tail)), [0, 0, 0, 1]);
}

#[test]
fn exact_fit_and_one_emu_short_respect_spacing_and_alignment() {
    let mut doc = document(&[1, 1, 1], 4);
    keep(&mut doc, 1, true, false);
    change(&mut doc, 1, |p| {
        let props = Arc::make_mut(&mut p.props);
        props.spacing.before = Some(Measure::Emu(pt(9).raw()));
        props.spacing.after = Some(Measure::Emu(pt(9).raw()));
        props.justification = Some(Justification::Center);
    });
    // Terminal after-spacing is not part of the group's required height.
    change(&mut doc, 2, |p| {
        Arc::make_mut(&mut p.props).spacing.after = Some(Measure::Emu(pt(72).raw()))
    });
    let exact = layout(&doc);
    assert_eq!(pages(&exact), [0, 0, 0]);
    assert_eq!(
        exact.lines().iter().map(|l| l.top).collect::<Vec<_>>(),
        [pt(6), pt(33), pt(60)]
    );
    change(&mut doc, 1, |p| {
        Arc::make_mut(&mut p.props).spacing.before = Some(Measure::Emu(pt(9).raw() + 1))
    });
    let short = layout(&doc);
    assert_eq!(pages(&short), [0, 1, 1]);
    assert_eq!(short.lines()[1].top.raw(), pt(15).raw() + 1);
    verify_source_and_geometry(&doc, &short);
}

#[test]
fn explicit_break_splits_keep_chain_and_empty_paragraphs_count() {
    let mut doc = document(&[1, 1, 0, 1], 3);
    keep(&mut doc, 0, true, false);
    keep(&mut doc, 1, true, true);
    keep(&mut doc, 2, true, true);
    change(&mut doc, 2, |p| {
        Arc::make_mut(&mut p.props).page_break_before = Some(true)
    });
    change(&mut doc, 0, |p| {
        Arc::make_mut(&mut p.props).page_break_before = Some(true)
    });
    let result = layout(&doc);
    assert_eq!(pages(&result), [0, 0, 1, 1]);
    verify_source_and_geometry(&doc, &result);
    let rects = result
        .selection_rects(
            position(&doc, 1, 0, Bias::After),
            position(&doc, 3, 3, Bias::Before),
        )
        .unwrap();
    assert_eq!(rects.iter().map(|r| r.page).collect::<Vec<_>>(), [0, 1, 1]);
    assert!(rects[1].width.raw() > 0);
    assert_eq!(
        result.selection_rects(
            position(&doc, 3, 3, Bias::Before),
            position(&doc, 1, 0, Bias::After)
        ),
        Err(LayoutError::ReversedSelection)
    );
}

#[test]
fn oversized_constraints_start_fresh_once_and_flow_with_member_keeps() {
    let mut paragraph = document(&[1, 5], 3);
    keep(&mut paragraph, 1, false, true);
    assert_eq!(pages(&layout(&paragraph)), [0, 1, 1, 1, 2, 2]);
    let mut chain = document(&[1, 2, 2, 1], 3);
    keep(&mut chain, 1, true, false);
    keep(&mut chain, 2, true, true);
    let result = layout(&chain);
    assert_eq!(pages(&result), [0, 1, 1, 2, 2, 2]);
    verify_source_and_geometry(&chain, &result);
    let mut first = document(&[5], 3);
    keep(&mut first, 0, true, true);
    assert_eq!(pages(&layout(&first)), [0, 0, 0, 1, 1]);
}

#[test]
fn long_chain_is_bounded_and_has_no_blank_or_repeated_pages() {
    let mut doc = document(&vec![1; 300], 7);
    for i in 0..300 {
        keep(&mut doc, i, true, true);
    }
    let result = layout(&doc);
    assert_eq!(result.pages().len(), 43);
    assert_eq!(pages(&result), (0..300).map(|i| i / 7).collect::<Vec<_>>());
    verify_source_and_geometry(&doc, &result);
}

#[test]
fn continuation_does_not_repeat_spacing_and_wrap_bias_survives_pages() {
    let mut doc = document(&[1, 5], 3);
    keep(&mut doc, 1, false, true);
    change(&mut doc, 1, |p| {
        Arc::make_mut(&mut p.props).spacing.before = Some(Measure::Emu(pt(18).raw()))
    });
    let result = layout(&doc);
    assert_eq!(pages(&result), [0, 1, 1, 2, 2, 2]);
    assert_eq!(result.lines()[1].top, pt(24));
    assert_eq!(result.lines()[3].top, pt(6));
    let boundary = result.lines()[3].source.scalars.start;
    assert_eq!(
        result
            .caret(position(&doc, 1, boundary, Bias::Before))
            .unwrap()
            .page,
        1
    );
    assert_eq!(
        result
            .caret(position(&doc, 1, boundary, Bias::After))
            .unwrap()
            .page,
        2
    );
    let rects = result
        .selection_rects(
            position(&doc, 1, 0, Bias::After),
            position(&doc, 1, 15, Bias::Before),
        )
        .unwrap();
    assert_eq!(
        rects.iter().map(|r| r.page).collect::<Vec<_>>(),
        [1, 1, 2, 2, 2]
    );
    verify_source_and_geometry(&doc, &result);
}

#[test]
fn keeps_do_not_reset_work_budgets_or_publish_partial_results() {
    let mut doc = document(&[1, 1, 1, 1], 2);
    for i in 0..4 {
        keep(&mut doc, i, true, true);
    }
    let baseline = layout(&doc);
    let before = doc.canonical_dump(DumpOptions { node_ids: true });
    let stats = baseline.stats();
    let cases = [
        LayoutLimits {
            max_pages: 1,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_lines: 3,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_glyphs: stats.glyphs - 1,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_shape_calls: stats.shape_calls - 1,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_shaped_bytes: stats.shaped_bytes - 1,
            ..LayoutLimits::default()
        },
    ];
    for limits in cases {
        assert!(run(&doc, limits).is_err(), "{limits:?}");
        assert_eq!(doc.canonical_dump(DumpOptions { node_ids: true }), before);
    }
    assert_eq!(
        run(
            &doc,
            LayoutLimits {
                max_pages: 1,
                ..LayoutLimits::default()
            }
        )
        .unwrap_err(),
        LayoutError::LimitExceeded(LayoutLimit::Pages)
    );
    let cancel = LayoutCancellation::default();
    cancel.clone().cancel();
    assert_eq!(
        layout_document(
            &doc.snapshot(),
            FaceSet::uniform(&font()),
            profile(),
            LayoutLimits::default(),
            &cancel
        )
        .unwrap_err(),
        LayoutError::Cancelled
    );
    assert_eq!(baseline.pages(), layout(&doc).pages());
}

#[test]
fn changed_keep_properties_invalidate_same_version_and_engine_edits_use_new_geometry() {
    let mut doc = document(&[2, 2], 3);
    let old = layout(&doc);
    keep(&mut doc, 1, false, true);
    assert_eq!(
        old.ensure_current(&doc.snapshot(), &font(), profile()),
        Err(LayoutError::StaleSnapshot(StaleReason::Content))
    );
    let mut session = Session::new(doc);
    let kept = layout(session.document());
    let line = &kept.lines()[2];
    let at = kept
        .hit_test(line.page, line.carets[0].x, line.top)
        .unwrap();
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::InsertText {
                at,
                text: "AV ".into(),
            }],
        ))
        .unwrap();
    assert_eq!(
        kept.ensure_current(&session.document().snapshot(), &font(), profile()),
        Err(LayoutError::StaleSnapshot(StaleReason::Version))
    );
    let edited = layout(session.document());
    assert_eq!(pages(&edited), [0, 0, 1, 1, 1]);
    verify_source_and_geometry(session.document(), &edited);
    session.undo().unwrap();
    assert_eq!(layout(session.document()).pages(), kept.pages());
    session.redo().unwrap();
    assert_eq!(layout(session.document()).pages(), edited.pages());
    // Font and profile still participate in the handoff identity.
    let current = layout(session.document());
    let mut bytes = font().data().to_vec();
    bytes.push(0);
    let other = LoadedFont::from_bytes(bytes.into(), 0).unwrap();
    assert_eq!(
        current.ensure_current(&session.document().snapshot(), &other, profile()),
        Err(LayoutError::StaleSnapshot(StaleReason::Font))
    );
    assert_eq!(
        current.ensure_current(
            &session.document().snapshot(),
            &font(),
            SingleFontProfile {
                baseline: pt(13),
                ..profile()
            }
        ),
        Err(LayoutError::StaleSnapshot(StaleReason::Profile))
    );
    for scale in [0.5, 1.0, 1.5, 2.0] {
        let screen = ScreenScale::new(scale).unwrap();
        for line in current.lines() {
            let _ = joyword_layout::to_screen(line.top, screen);
        }
        assert_eq!(layout(session.document()).pages(), current.pages());
    }
}

#[test]
fn unicode_clusters_missing_glyphs_and_diagnostics_survive_keep_pagination() {
    let mut doc = document(&[1, 1], 2);
    let text = "e\u{301} 👩‍💻 甲乙 <&>";
    change(&mut doc, 1, |p| {
        p.remove(ScalarOffset(0)..ScalarOffset(p.len())).unwrap();
        p.insert_text(ScalarOffset(0), text).unwrap();
    });
    let unkept = layout(&doc);
    keep(&mut doc, 1, false, true);
    let kept = layout(&doc);
    assert_eq!(kept.lines()[1].page, 1);
    let paragraph = doc.body().blocks.get(1).unwrap().id();
    let source = |snapshot: &LayoutSnapshot| {
        snapshot
            .lines()
            .iter()
            .filter(|l| l.paragraph == paragraph)
            .map(|l| l.source.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(source(&kept), source(&unkept));
    assert_eq!(kept.diagnostics(), unkept.diagnostics());
    // This fixture has no CJK glyphs; pagination must retain source and the
    // actual missing-glyph diagnostics rather than treating blank ink as text.
    assert!(!kept.diagnostics().is_empty());
    verify_source_and_geometry(&doc, &kept);
    for inside in [1, 4, 5] {
        assert!(matches!(
            kept.caret(position(&doc, 1, inside, Bias::After)),
            Err(LayoutError::InsideCluster(_))
        ));
    }
    let capped = run(
        &doc,
        LayoutLimits {
            max_diagnostics: 1,
            max_selection_rects: 1,
            ..LayoutLimits::default()
        },
    )
    .unwrap();
    assert_eq!(capped.diagnostics().len(), 1);
    assert_eq!(
        capped.diagnostics().len() + capped.omitted_diagnostics(),
        kept.diagnostics().len() + kept.omitted_diagnostics()
    );
    assert_eq!(
        capped.selection_rects(
            position(&doc, 0, 0, Bias::After),
            position(&doc, 1, text.chars().count(), Bias::Before)
        ),
        Err(LayoutError::LimitExceeded(LayoutLimit::SelectionRects))
    );
    // Same engine version, different preview content: reject the old identity.
    change(&mut doc, 1, |p| {
        p.insert_text(ScalarOffset(0), "AV ").unwrap()
    });
    assert_eq!(
        kept.ensure_current(&doc.snapshot(), &font(), profile()),
        Err(LayoutError::StaleSnapshot(StaleReason::Content))
    );
}

#[test]
fn paragraph_and_keep_chain_height_overflow_fail_without_mutating_the_model() {
    // Physically unrealistic but representable page/line boxes exercise the
    // newly introduced height multiplication and chain sum independently.
    for counts in [&[3][..], &[1, 1, 1][..]] {
        let mut doc = document(counts, 3);
        let page = &mut doc.sections_mut()[0].page;
        page.height = Measure::Emu(i64::MAX - 1);
        page.top = Measure::Emu(0);
        page.bottom = Measure::Emu(0);
        for i in 0..counts.len() {
            keep(&mut doc, i, true, true);
        }
        let before = doc.canonical_dump(DumpOptions { node_ids: true });
        let extreme = SingleFontProfile {
            leading: Emu::new(i64::MAX / 2),
            baseline: Emu::new(0),
            ..profile()
        };
        assert_eq!(
            layout_document(
                &doc.snapshot(),
                FaceSet::uniform(&font()),
                extreme,
                LayoutLimits::default(),
                &LayoutCancellation::default()
            )
            .unwrap_err(),
            LayoutError::GeometryOverflow
        );
        assert_eq!(doc.canonical_dump(DumpOptions { node_ids: true }), before);
    }
    let mut doc = document(&[1], 1);
    keep(&mut doc, 0, false, true);
    change(&mut doc, 0, |p| {
        Arc::make_mut(&mut p.props).spacing.before = Some(Measure::Emu(1))
    });
    assert_eq!(
        run(&doc, LayoutLimits::default()).unwrap_err(),
        LayoutError::LineTooTall
    );
}
