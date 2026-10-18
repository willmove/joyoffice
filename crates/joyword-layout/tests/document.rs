use std::sync::Arc;

use joyword_engine::{Command, Session, Transaction};
use joyword_foundation::{Emu, ScreenScale};
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutLimit, LayoutLimits,
    LayoutSnapshot, LoadedFont, SingleFontProfile, StaleReason, layout_document, shape, wrap_mixed,
};
use joyword_model::{
    AbstractNum, Bias, Block, BlockSeq, Document, DumpOptions, FontRef, Indent, Justification,
    Measure, NumInstance, NumberFormat, NumberSuffix, NumberingLevel, NumberingRef, PageSetup,
    Paragraph, Position, ScalarOffset, Style, StyleId, StyleKind, TabAlign, TabLeader, TabStop,
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
        features: FeatureSet {
            standard_ligatures: true,
            ..FeatureSet::default()
        },
    }
}
fn make_document(texts: &[&str], lines_per_page: i64) -> Document {
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(100).raw());
    page.height = Measure::Emu(pt(12 + 18 * lines_per_page).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut blocks = Vec::new();
    for text in texts {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text(text, None, Arc::default()).unwrap();
        blocks.push(paragraph.into());
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    assert!(!document.validate().has_errors());
    document
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
fn position(document: &Document, index: usize, offset: usize, bias: Bias) -> Position {
    Position {
        story: document.body_id(),
        paragraph: document.body().blocks.get(index).unwrap().id(),
        offset: ScalarOffset(offset),
        bias,
    }
}
fn change_paragraph(document: &mut Document, index: usize, change: impl FnOnce(&mut Paragraph)) {
    let body = document.body_id();
    document
        .story_mut(body)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(p) = block else {
                unreachable!()
            };
            change(Arc::make_mut(p));
        });
}

#[test]
fn exact_page_boundary_preserves_all_source_and_model() {
    let document = make_document(&["AV", "fi", "AV", "fi", "AV", "fi", "AV"], 3);
    let before = document.canonical_dump(DumpOptions { node_ids: true });
    let result = layout(&document);
    assert_eq!(
        result
            .pages()
            .iter()
            .map(|p| p.runs.len())
            .collect::<Vec<_>>(),
        [3, 3, 1]
    );
    assert_eq!(result.lines().len(), 7);
    assert_eq!(result.key().version, document.version());
    assert_eq!(result.key().font, font().key());
    for (index, line) in result.lines().iter().enumerate() {
        assert_eq!(
            line.paragraph,
            document.body().blocks.get(index).unwrap().id()
        );
        assert_eq!(line.top, pt(6 + 18 * (index as i64 % 3)));
        assert_eq!(line.baseline, Emu::new(line.top.raw() + pt(14).raw()));
        let page = &result.pages()[line.page];
        let run = &page.runs[line.run];
        assert_eq!(run.text, if index % 2 == 0 { "AV" } else { "fi" });
        assert_eq!(run.origin_x, pt(6));
        assert_eq!(run.baseline_y, line.baseline);
        assert!(line.top.raw() + line.height.raw() <= page.height.raw() - pt(6).raw());
    }
    assert_eq!(
        document.canonical_dump(DumpOptions { node_ids: true }),
        before
    );
}

#[test]
fn hit_positions_drive_real_engine_edits_and_old_snapshots_are_rejected() {
    let mut session = Session::new(make_document(&["AV", "fi", "AV", "fi"], 2));
    let original = layout(session.document());
    let scenes = original.pages().to_vec();
    let line = &original.lines()[2];
    let stop = line.carets.last().unwrap();
    let hit = original
        .hit_test(
            line.page,
            stop.x,
            Emu::new(line.top.raw() + line.height.raw() / 2),
        )
        .unwrap();
    assert_eq!(hit.paragraph, line.paragraph);
    assert_eq!(hit.offset, stop.offset);
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::InsertText {
                at: hit,
                text: " fi".into(),
            }],
        ))
        .unwrap();
    assert_eq!(
        original.ensure_current(&session.document().snapshot(), &font(), profile()),
        Err(LayoutError::StaleSnapshot(StaleReason::Version))
    );
    let edited = layout(session.document());
    assert_ne!(edited.key().content_hash(), original.key().content_hash());
    assert_eq!(original.pages(), scenes);
    let text: String = edited
        .pages()
        .iter()
        .flat_map(|p| &p.runs)
        .map(|r| r.text.as_str())
        .collect();
    assert!(text.contains("AV fi"));
    session.undo().unwrap();
    let restored = layout(session.document());
    assert_eq!(restored.key().content_hash(), original.key().content_hash());
    assert_ne!(restored.key().version, original.key().version);
    assert_eq!(restored.pages(), original.pages());
}

#[test]
fn font_profile_and_same_version_content_cannot_reuse_a_snapshot() {
    let mut document = make_document(&["AV"], 2);
    let result = layout(&document);
    let mut bytes = font().data().to_vec();
    bytes.push(0);
    let different_bytes = LoadedFont::from_bytes(bytes.into(), 0).unwrap();
    assert_eq!(
        result.ensure_current(&document.snapshot(), &different_bytes, profile()),
        Err(LayoutError::StaleSnapshot(StaleReason::Font))
    );
    let different_profile = SingleFontProfile {
        leading: pt(19),
        ..profile()
    };
    assert_eq!(
        result.ensure_current(&document.snapshot(), &font(), different_profile),
        Err(LayoutError::StaleSnapshot(StaleReason::Profile))
    );
    change_paragraph(&mut document, 0, |p| {
        p.insert_text(ScalarOffset(0), "fi ").unwrap()
    });
    assert_eq!(document.version(), result.key().version);
    assert_eq!(
        result.ensure_current(&document.snapshot(), &font(), profile()),
        Err(LayoutError::StaleSnapshot(StaleReason::Content))
    );
}

#[test]
fn wrap_boundary_affinity_selects_upstream_or_downstream_line() {
    let text = "AV ".repeat(25);
    let document = make_document(&[&text], 2);
    let result = layout(&document);
    assert!(result.lines().len() > 2);
    for pair in result.lines().windows(2) {
        let end = pair[0].source.scalars.end;
        assert_eq!(pair[1].source.scalars.start, end);
        let before = result
            .caret(position(&document, 0, end, Bias::Before))
            .unwrap();
        let after = result
            .caret(position(&document, 0, end, Bias::After))
            .unwrap();
        assert_eq!(before.page, pair[0].page);
        assert_eq!(before.top, pair[0].top);
        assert_eq!(after.page, pair[1].page);
        assert_eq!(after.top, pair[1].top);
        assert_ne!((before.page, before.top), (after.page, after.top));
    }
}

#[test]
fn every_caret_coordinate_hits_equivalent_geometry() {
    let document = make_document(
        &["AV fi AV fi AV fi AV fi AV fi", "", "e\u{301} 👩‍💻 甲乙"],
        2,
    );
    let result = layout(&document);
    for line in result.lines() {
        for stop in &line.carets {
            let hit = result
                .hit_test(
                    line.page,
                    stop.x,
                    Emu::new(line.top.raw() + line.height.raw() / 2),
                )
                .unwrap();
            assert_eq!(hit.paragraph, line.paragraph);
            let caret = result.caret(hit).unwrap();
            assert_eq!(caret.x, stop.x);
            assert_eq!(caret.page, line.page);
            assert_eq!(caret.top, line.top);
        }
    }
    assert!(matches!(
        result.hit_test(result.pages().len(), pt(0), pt(0)),
        Err(LayoutError::UnknownPage(_))
    ));
    assert!(
        result
            .hit_test(0, Emu::new(i64::MIN), Emu::new(i64::MAX))
            .is_ok()
    );
}

#[test]
fn ligature_cluster_interior_is_refused_without_fake_cursor_geometry() {
    let document = make_document(&["fi"], 2);
    let result = layout(&document);
    assert_eq!(
        result.lines()[0]
            .carets
            .iter()
            .map(|c| c.offset.0)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    let interior = position(&document, 0, 1, Bias::After);
    assert_eq!(
        result.caret(interior),
        Err(LayoutError::InsideCluster(interior))
    );
    assert_eq!(
        result.selection_rects(interior, position(&document, 0, 2, Bias::Before)),
        Err(LayoutError::InsideCluster(interior))
    );
}

#[test]
fn cross_page_selection_includes_empty_paragraph_marks() {
    let document = make_document(&["AV", "", "fi", "AV"], 2);
    let result = layout(&document);
    let from = position(&document, 0, 0, Bias::After);
    let to = position(&document, 3, 2, Bias::Before);
    let rects = result.selection_rects(from, to).unwrap();
    assert_eq!(rects.len(), 4);
    assert_eq!(
        rects.iter().map(|r| r.page).collect::<Vec<_>>(),
        [0, 0, 1, 1]
    );
    assert!(
        rects
            .iter()
            .all(|r| r.width.raw() > 0 && r.height == pt(18))
    );
    assert_eq!(rects[1].width, pt(6));
    assert_eq!(
        result.selection_rects(to, from),
        Err(LayoutError::ReversedSelection)
    );
    assert!(result.selection_rects(from, from).unwrap().is_empty());
    let beyond = position(&document, 0, 99, Bias::After);
    assert_eq!(
        result.caret(beyond),
        Err(LayoutError::UnknownPosition(beyond))
    );
}

#[test]
fn paragraph_alignment_spacing_and_page_break_are_applied() {
    let mut document = make_document(&["AV", "fi", "AV"], 4);
    change_paragraph(&mut document, 0, |p| {
        Arc::make_mut(&mut p.props).justification = Some(Justification::Center)
    });
    change_paragraph(&mut document, 1, |p| {
        let props = Arc::make_mut(&mut p.props);
        props.justification = Some(Justification::End);
        props.spacing.before = Some(Measure::Twips(120));
    });
    change_paragraph(&mut document, 2, |p| {
        Arc::make_mut(&mut p.props).page_break_before = Some(true)
    });
    let result = layout(&document);
    assert_eq!(result.pages().len(), 2);
    let w0 = shape(&font(), "AV", profile().size, profile().features)
        .width
        .raw();
    let w1 = shape(&font(), "fi", profile().size, profile().features)
        .width
        .raw();
    assert_eq!(
        result.pages()[0].runs[0].origin_x.raw(),
        pt(6).raw() + (pt(88).raw() - w0) / 2
    );
    assert_eq!(result.pages()[0].runs[1].origin_x.raw(), pt(94).raw() - w1);
    assert_eq!(result.lines()[1].top, pt(30));
    assert_eq!(result.lines()[2].top, pt(6));
}

#[test]
fn paragraph_before_spacing_is_not_repeated_on_continuation_pages() {
    let text = "AV ".repeat(50);
    let mut document = make_document(&[&text], 3);
    change_paragraph(&mut document, 0, |p| {
        Arc::make_mut(&mut p.props).spacing.before = Some(Measure::Twips(360))
    });
    let result = layout(&document);
    assert!(result.pages().len() > 1);
    assert_eq!(result.lines()[0].top, pt(24));
    assert_eq!(
        result
            .lines()
            .iter()
            .find(|line| line.page == 1)
            .unwrap()
            .top,
        pt(6)
    );
}

#[test]
fn layout_budget_failures_do_not_mutate_the_document_or_existing_snapshot() {
    let document = make_document(&["AV", "fi", "AV"], 1);
    let old = layout(&document);
    let scenes = old.pages().to_vec();
    for limits in [
        LayoutLimits {
            max_paragraphs: 0,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_text_bytes: 1,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_runs: 0,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_lines: 0,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_glyphs: 0,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_pages: 1,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_shape_calls: 0,
            ..LayoutLimits::default()
        },
        LayoutLimits {
            max_shaped_bytes: 0,
            ..LayoutLimits::default()
        },
    ] {
        assert!(
            layout_document(
                &document.snapshot(),
                FaceSet::uniform(&font()),
                profile(),
                limits,
                &LayoutCancellation::default()
            )
            .is_err()
        );
        old.ensure_current(&document.snapshot(), &font(), profile())
            .unwrap();
        assert_eq!(old.pages(), scenes);
    }
}

#[test]
fn paragraph_requests_do_not_reset_global_work_budgets() {
    let document = make_document(&["AV", "AV"], 3);
    for (limits, expected) in [
        (
            LayoutLimits {
                max_shape_calls: 1,
                ..LayoutLimits::default()
            },
            LayoutLimit::ShapeCalls,
        ),
        (
            LayoutLimits {
                max_shaped_bytes: 2,
                ..LayoutLimits::default()
            },
            LayoutLimit::ShapedBytes,
        ),
        (
            LayoutLimits {
                max_lines: 1,
                ..LayoutLimits::default()
            },
            LayoutLimit::Lines,
        ),
        (
            LayoutLimits {
                max_glyphs: 2,
                ..LayoutLimits::default()
            },
            LayoutLimit::Glyphs,
        ),
    ] {
        assert!(
            matches!(layout_document(&document.snapshot(),FaceSet::uniform(&font()), profile(), limits, &LayoutCancellation::default()), Err(LayoutError::LimitExceeded(actual)) if actual == expected)
        );
    }
}

#[test]
fn selection_output_and_diagnostics_are_bounded() {
    let document = make_document(&["甲乙", "fi"], 2);
    let result = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits {
            max_selection_rects: 1,
            max_diagnostics: 0,
            ..LayoutLimits::default()
        },
        &LayoutCancellation::default(),
    )
    .unwrap();
    assert!(result.diagnostics().is_empty());
    assert!(result.omitted_diagnostics() > 0);
    assert_eq!(
        result.selection_rects(
            position(&document, 0, 0, Bias::After),
            position(&document, 1, 2, Bias::Before)
        ),
        Err(LayoutError::LimitExceeded(LayoutLimit::SelectionRects))
    );
}

#[test]
fn unsupported_content_and_active_styles_are_not_silently_laid_out_as_plain_text() {
    let mut document = make_document(&["AV"], 2);
    document
        .styles_mut()
        .insert(Style::new("Unused", StyleKind::Paragraph, Some("unused")))
        .unwrap();
    layout(&document);
    let mut bold = Style::new("Bold", StyleKind::Paragraph, Some("bold"));
    bold.run.bold = Some(true);
    bold.is_default = true;
    document.styles_mut().insert(bold).unwrap();
    let laid = layout(&document);
    assert!(
        laid.pages()
            .iter()
            .all(|page| page.runs.iter().all(|run| run.bold))
    );
    let mut document = make_document(&["AV"], 2);
    change_paragraph(&mut document, 0, |p| {
        Arc::make_mut(&mut p.props).justification = Some(Justification::Both)
    });
    assert!(matches!(
        layout_document(
            &document.snapshot(),
            FaceSet::uniform(&font()),
            profile(),
            LayoutLimits::default(),
            &LayoutCancellation::default()
        ),
        Err(LayoutError::Unsupported { .. })
    ));
}

#[test]
fn invalid_geometry_and_oversized_line_fail_without_overflow_or_partial_pages() {
    for kind in 0..5 {
        let mut document = make_document(&["AV"], 2);
        let page = &mut document.sections_mut()[0].page;
        match kind {
            0 => page.width = Measure::Emu(0),
            1 => page.left = Measure::Emu(-1),
            2 => page.right = page.width,
            3 => page.width = Measure::Twips(i64::MAX),
            _ => page.top = page.height,
        }
        assert!(
            layout_document(
                &document.snapshot(),
                FaceSet::uniform(&font()),
                profile(),
                LayoutLimits::default(),
                &LayoutCancellation::default()
            )
            .is_err()
        );
    }
    let document = make_document(&["AV"], 1);
    let profile = SingleFontProfile {
        leading: pt(19),
        ..profile()
    };
    assert!(matches!(
        layout_document(
            &document.snapshot(),
            FaceSet::uniform(&font()),
            profile,
            LayoutLimits::default(),
            &LayoutCancellation::default()
        ),
        Err(LayoutError::LineTooTall)
    ));
}

#[test]
fn cancellation_is_monotonic_and_does_not_publish_a_candidate() {
    let document = make_document(&["AV"], 2);
    let token = LayoutCancellation::default();
    let other = token.clone();
    other.cancel();
    assert!(token.is_cancelled());
    assert!(matches!(
        layout_document(
            &document.snapshot(),
            FaceSet::uniform(&font()),
            profile(),
            LayoutLimits::default(),
            &token
        ),
        Err(LayoutError::Cancelled)
    ));
    assert_eq!(document.version().get(), 0);
}

#[test]
fn zoom_changes_projection_without_changing_page_or_hit_geometry() {
    let document = make_document(&["AV", "fi", "AV"], 2);
    let result = layout(&document);
    let caret = result
        .caret(position(&document, 0, 0, Bias::After))
        .unwrap();
    for zoom in [0.5, 1.0, 1.37, 2.0, 4.0] {
        let scale = ScreenScale::new(zoom).unwrap();
        let back = scale.logical_pixels(caret.x) / zoom * 72.0 / 96.0;
        assert!((back - caret.x.points()).abs() < 0.01);
        assert_eq!(result.geometry().width, pt(100));
    }
}

#[test]
fn line_boxes_are_half_open_at_shared_edges() {
    let document = make_document(&["AV", "fi"], 2);
    let result = layout(&document);
    let second = &result.lines()[1];
    let hit = result
        .hit_test(second.page, second.carets[0].x, second.top)
        .unwrap();
    assert_eq!(hit.paragraph, second.paragraph);
}

#[test]
fn unsupported_stories_sections_objects_and_field_projection_are_explicit() {
    use joyword_model::{Anchor, AnnotationKind, BreakKind, InlineKind, InlineObject, StoryKind};
    for case in 0..6 {
        let mut doc = make_document(&["AV"], 2);
        match case {
            0 => {
                doc.insert_story(StoryKind::Header, vec![]);
            }
            1 => {
                let s = doc.sections()[0].clone();
                doc.sections_mut().push(s);
            }
            2 => {
                let id = doc.ids().node();
                change_paragraph(&mut doc, 0, |p| {
                    p.insert_inline(
                        ScalarOffset(1),
                        InlineObject {
                            id,
                            kind: InlineKind::Break(BreakKind::Page),
                            source: None,
                        },
                        None,
                        Arc::default(),
                    )
                    .unwrap();
                });
            }
            3 => {
                doc.insert_annotation(AnnotationKind::Field { separator: None }, Anchor::Detached);
            }
            4 => {
                doc.settings_mut().track_revisions = Some(true);
            }
            _ => {
                change_paragraph(&mut doc, 0, |p| {
                    Arc::make_mut(&mut p.mark_props).hidden = Some(true)
                });
            }
        }
        let outcome = layout_document(
            &doc.snapshot(),
            FaceSet::uniform(&font()),
            profile(),
            LayoutLimits::default(),
            &LayoutCancellation::default(),
        );
        match case {
            // An explicit page break is a supported body inline.
            2 => {
                let snapshot = outcome.expect("page break");
                assert!(snapshot.pages().len() > 1, "page break splits the page");
            }
            // Two final sections (both without a boundary block) are not a chain.
            1 => assert!(
                matches!(outcome, Err(LayoutError::InvalidDocument)),
                "case {case}: {outcome:?}"
            ),
            _ => assert!(
                matches!(outcome, Err(LayoutError::Unsupported { .. })),
                "case {case}: {outcome:?}"
            ),
        }
    }
}

#[test]
fn repeated_engine_edits_keep_pages_source_ranges_and_hits_consistent() {
    let mut session = Session::new(make_document(
        &["AV fi AV fi AV fi", "", "AV fi", "AV fi AV fi", "fi"],
        2,
    ));
    let font = font();
    let mut seed = 0x0c05_2026_u64;
    for step in 0..64 {
        let original = layout_document(
            &session.document().snapshot(),
            FaceSet::uniform(&font),
            profile(),
            LayoutLimits::default(),
            &LayoutCancellation::default(),
        )
        .unwrap();
        for block in session.document().body().blocks.iter() {
            let p = block.as_paragraph().unwrap();
            let lines: Vec<_> = original
                .lines()
                .iter()
                .filter(|l| l.paragraph == p.id())
                .collect();
            let mut byte = 0;
            let mut scalar = 0;
            let mut text = String::new();
            for line in lines {
                assert_eq!(line.source.bytes.start, byte);
                assert_eq!(line.source.scalars.start, scalar);
                text.push_str(&original.pages()[line.page].runs[line.run].text);
                byte = line.source.bytes.end;
                scalar = line.source.scalars.end;
                for stop in &line.carets {
                    let hit = original
                        .hit_test(
                            line.page,
                            stop.x,
                            Emu::new(line.top.raw() + line.height.raw() / 2),
                        )
                        .unwrap();
                    let caret = original.caret(hit).unwrap();
                    assert_eq!(
                        (caret.page, caret.x, caret.top),
                        (line.page, stop.x, line.top)
                    );
                }
            }
            assert_eq!(text, p.plain_text(|_| unreachable!()));
            assert_eq!(scalar, p.len());
        }
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let line = &original.lines()[(seed as usize) % original.lines().len()];
        let stop = &line.carets[((seed >> 32) as usize) % line.carets.len()];
        let at = original
            .hit_test(
                line.page,
                stop.x,
                Emu::new(line.top.raw() + line.height.raw() / 2),
            )
            .unwrap();
        session
            .apply(Transaction::new(
                session.version(),
                vec![Command::InsertText {
                    at,
                    text: ["AV", " fi", " ", "A\u{301}"][step % 4].into(),
                }],
            ))
            .unwrap();
        assert_eq!(
            original.ensure_current(&session.document().snapshot(), &font, profile()),
            Err(LayoutError::StaleSnapshot(StaleReason::Version))
        );
        if step % 5 == 0 {
            session.undo().unwrap();
        }
    }
}

#[test]
fn cjk_slots_move_a_prohibited_break_in_the_page_scene() {
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(24).raw());
    page.height = Measure::Emu(pt(120).raw());
    page.left = Measure::Emu(pt(2).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text("甲乙。丙", None, Arc::default())
        .unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let result = layout(&document);
    assert!(result.lines().len() > 1, "the narrow measure must wrap");
    for line in result.lines() {
        let text = &result.pages()[line.page].runs[line.run].text;
        assert!(
            !text.starts_with('。'),
            "closing punctuation started a laid-out line: {text}"
        );
    }
    assert!(
        result
            .pages()
            .iter()
            .flat_map(|page| &page.runs)
            .any(|run| run.text.contains('。'))
    );
}

#[test]
fn mixed_slot_faces_change_the_page_scene() {
    let latin = font();
    let cjk_path = "/System/Library/Fonts/Supplemental/Songti.ttc";
    let Some(cjk) = std::fs::read(cjk_path)
        .ok()
        .and_then(|bytes| LoadedFont::from_bytes(Arc::from(bytes), 0).ok())
    else {
        return;
    };
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(72).raw());
    page.height = Measure::Emu(pt(200).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text("AAAA甲甲甲甲。BBBB", None, Arc::default())
        .unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let uniform = FaceSet::uniform(&latin);
    let mixed = FaceSet {
        east_asia: &cjk,
        fallback: &cjk,
        ..uniform
    };
    let plain = layout_document(
        &document.snapshot(),
        uniform,
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let shaped = layout_document(
        &document.snapshot(),
        mixed,
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let line_text = |snapshot: &LayoutSnapshot| {
        snapshot
            .lines()
            .iter()
            .map(|line| snapshot.pages()[line.page].runs[line.run].text.clone())
            .collect::<Vec<_>>()
    };
    let plain_lines = line_text(&plain);
    let shaped_lines = line_text(&shaped);
    assert_ne!(plain_lines, shaped_lines);
    let width = Emu::new(
        shaped.geometry().width.raw()
            - shaped.geometry().left.raw()
            - shaped.geometry().right.raw(),
    );
    let expected = wrap_mixed(mixed, "AAAA甲甲甲甲。BBBB", profile().size, width, None);
    assert_eq!(
        shaped_lines,
        expected
            .lines
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        shaped
            .pages()
            .iter()
            .any(|page| { page.runs.iter().any(|run| run.font == cjk.key()) })
    );
    assert!(
        plain
            .pages()
            .iter()
            .all(|page| { page.runs.iter().all(|run| run.font == latin.key()) })
    );
    for text in &shaped_lines {
        assert!(!text.starts_with('。'), "{text}");
    }
}

#[test]
fn resolved_styles_set_size_weight_and_requested_families() {
    let mut document = make_document(&["第一条 合作内容", "甲方委托乙方完成设计服务。"], 8);
    document.styles_mut().defaults.run.fonts.ascii = Some(FontRef::Named("Times New Roman".into()));
    document.styles_mut().defaults.run.fonts.h_ansi =
        Some(FontRef::Named("Times New Roman".into()));
    document.styles_mut().defaults.run.fonts.east_asia = Some(FontRef::Named("宋体".into()));
    document.styles_mut().defaults.run.size = Some(Measure::HalfPoints(21));
    let mut normal = Style::new("a", StyleKind::Paragraph, Some("Normal"));
    normal.is_default = true;
    let mut heading = Style::new("1", StyleKind::Paragraph, Some("heading 1"));
    heading.based_on = Some(StyleId::new("a"));
    heading.run.bold = Some(true);
    heading.run.size = Some(Measure::HalfPoints(32));
    heading.para.outline_level = Some(0);
    document.styles_mut().insert(normal).unwrap();
    document.styles_mut().insert(heading).unwrap();
    change_paragraph(&mut document, 0, |paragraph| {
        paragraph.style = Some(StyleId::new("1"));
    });
    let laid = layout(&document);
    let runs: Vec<_> = laid
        .pages()
        .iter()
        .flat_map(|page| page.runs.iter())
        .collect();
    assert!(runs.len() >= 2);
    let heading = runs.iter().find(|run| run.text.contains('第')).unwrap();
    assert!(heading.bold);
    assert!(!heading.italic);
    assert_eq!(heading.size, Emu::new(16 * Emu::PER_POINT));
    assert_eq!(heading.latin_family.as_deref(), Some("Times New Roman"));
    assert_eq!(heading.east_asia_family.as_deref(), Some("宋体"));
    let body = runs.iter().find(|run| run.text.contains('甲')).unwrap();
    assert!(!body.bold);
    assert_eq!(body.size, Emu::new(Emu::PER_POINT * 21 / 2));
    assert_eq!(body.east_asia_family.as_deref(), Some("宋体"));
}

fn install_list(document: &mut Document, format: NumberFormat, text: &str, start: i32) {
    let mut level = NumberingLevel::new(format, text);
    level.start = start;
    level.suffix = NumberSuffix::Nothing;
    level.para.indent = Indent {
        start: Some(Measure::Twips(1440)),
        hanging: Some(Measure::Twips(720)),
        ..Indent::default()
    };
    let mut abstract_num = AbstractNum::new(1);
    abstract_num.levels[0] = Some(level);
    document.numbering_mut().insert_abstract(abstract_num);
    document.numbering_mut().insert_instance(NumInstance {
        id: 1,
        abstract_id: 1,
        overrides: Vec::new(),
    });
}

fn mark_paragraph(document: &mut Document, index: usize) {
    change_paragraph(document, index, |paragraph| {
        let mut props = (*paragraph.props).clone();
        props.numbering = Some(NumberingRef {
            num_id: 1,
            level: 0,
        });
        paragraph.props = Arc::new(props);
    });
}

#[test]
fn chinese_numbering_labels_are_prefixed_and_unsupported_formats_are_diagnosed() {
    let mut document = make_document(&["正文"], 4);
    install_list(&mut document, NumberFormat::ChineseCounting, "%1", 10);
    mark_paragraph(&mut document, 0);
    let laid = layout(&document);
    let text = &laid.pages()[0].runs[0].text;
    assert!(text.starts_with('十'), "{text}");
    assert!(text.contains('正'), "{text}");
    assert!(!text.contains('•'));

    let mut document = make_document(&["正文"], 4);
    install_list(&mut document, NumberFormat::parse("bullet"), "%1", 1);
    mark_paragraph(&mut document, 0);
    let laid = layout(&document);
    let diagnostic = laid
        .diagnostics()
        .iter()
        .find(|item| {
            matches!(
                item.diagnostic,
                joyword_layout::LineDiagnostic::Numbering { .. }
            )
        })
        .expect("unsupported numbering diagnostic");
    let joyword_layout::LineDiagnostic::Numbering { detail } = &diagnostic.diagnostic else {
        unreachable!("filtered");
    };
    assert!(detail.contains("unsupported"), "{detail}");
    assert!(detail.contains("bullet"), "{detail}");
    let text = &laid.pages()[0].runs[0].text;
    assert_eq!(text, "正文");
}

#[test]
fn a_left_tab_places_the_following_glyph_at_the_stop() {
    let mut document = make_document(&["A\tB"], 4);
    change_paragraph(&mut document, 0, |paragraph| {
        let mut props = (*paragraph.props).clone();
        props.tabs = Some(vec![TabStop {
            position: Measure::Twips(2880),
            align: TabAlign::Left,
            leader: TabLeader::None,
        }]);
        paragraph.props = Arc::new(props);
    });
    let laid = layout(&document);
    let run = &laid.pages()[0].runs[0];
    assert!(
        run.text.contains('A') && run.text.contains('B'),
        "{}",
        run.text
    );
    let b = run
        .glyphs
        .iter()
        .find(|glyph| glyph.cluster < run.text.len() && run.text[glyph.cluster..].starts_with('B'))
        .expect("B glyph");
    assert_eq!(b.x, Emu::new(2880 * Emu::PER_TWIP));
}
