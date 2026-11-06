#![allow(clippy::unwrap_used, reason = "Test assertions")]
use joydeck_engine::{
    CellBorders, CellFormat, CellRange, Command, Session, TableAction, TextEdit, TextTarget,
    original_bindings, text::Draft,
};
use joydeck_model::{
    Presentation, ShapeId, ShapeKind, TextBody,
    text::{Anchor, TextDir},
};
fn session() -> Session {
    let p = Presentation::default();
    Session::new(p.clone(), original_bindings(&p), true)
}

#[test]
fn clipboard_links_require_document_identity_and_a_live_target() {
    use joydeck_engine::{
        clipboard,
        transfer::{Resource, Transfer},
    };
    use joydeck_model::{
        MediaId, Shape, SlideId,
        text::{Action, Hyperlink, Run},
    };

    let p = Presentation::default();
    let slide = p.slides[0].id;
    let link = |action| Hyperlink {
        action,
        tooltip: "tooltip".into(),
        highlight_click: false,
        sound: None,
        stop_sound: false,
        zoom_return: None,
    };
    let mut rich = TextBody::from_text("linked");
    let paragraph = &mut rich.paragraphs[0];
    paragraph.runs[0].props.link = Some(link(Action::Slide { slide }));
    paragraph.runs[0].props.bold = Some(true);
    paragraph.end_props.hover = Some(link(Action::Slide {
        slide: SlideId(u32::MAX),
    }));
    let mut external = link(Action::Url {
        url: "https://example.org/".into(),
    });
    external.sound = Some(MediaId(u32::MAX));
    let mut run = Run::new(" web");
    run.props.link = Some(external);
    paragraph.runs.push(run);

    let mut same = rich.clone();
    assert!(clipboard::scope_text(&mut same, Some(&p)).unwrap());
    let runs = &same.paragraphs[0].runs;
    assert_eq!(runs[0].props.link, rich.paragraphs[0].runs[0].props.link);
    assert!(runs[1].props.link.as_ref().unwrap().sound.is_none());
    assert!(same.paragraphs[0].end_props.hover.is_none());

    let mut foreign = rich.clone();
    assert!(clipboard::scope_text(&mut foreign, None).unwrap());
    assert_eq!(foreign.text(), rich.text());
    let runs = &foreign.paragraphs[0].runs;
    assert!(runs[0].props.link.is_none());
    assert_eq!(runs[0].props.bold, Some(true));
    assert!(matches!(
        runs[1].props.link.as_ref().unwrap().action,
        Action::Url { .. }
    ));

    let mut click = link(Action::Slide { slide });
    click.sound = Some(MediaId(3));
    let sound = |id| Resource {
        id,
        name: "sound.wav".into(),
        content_type: "audio/wav".into(),
        data: vec![1],
    };
    let mut transfer = Transfer {
        joydeck_objects: 1,
        shapes: vec![Shape {
            click: Some(click),
            text: Some(rich),
            ..Default::default()
        }],
        media: vec![sound(MediaId(3)), sound(MediaId(u32::MAX))],
    };
    assert!(clipboard::scope_objects(&mut transfer, None));
    let shape = &transfer.shapes[0];
    assert!(shape.click.is_none());
    let runs = &shape.text.as_ref().unwrap().paragraphs[0].runs;
    assert!(runs[0].props.link.is_none());
    assert_eq!(
        runs[1].props.link.as_ref().unwrap().sound,
        Some(MediaId(u32::MAX))
    );
    assert_eq!(transfer.media.len(), 1);
    assert_eq!(transfer.media[0].id, MediaId(u32::MAX));
}

#[test]
fn slide_order_and_group_transform_preserve_references_and_undo() {
    use joydeck_engine::WorkspaceAction;
    let mut s = session();
    for _ in 0..3 {
        s.execute(Command::NewSlide).unwrap();
    }
    let ids: Vec<_> = s.model().slides.iter().map(|s| s.id).collect();
    let before = s.snapshot();
    s.execute(Command::Workspace {
        action: WorkspaceAction::ReorderSlides {
            slides: vec![ids[1], ids[2]],
            before: Some(ids[0]),
        },
    })
    .unwrap();
    assert_eq!(
        s.model().slides.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![ids[1], ids[2], ids[0], ids[3]]
    );
    s.execute(Command::Undo).unwrap();
    assert_eq!(s.model(), before.model.as_ref());
    let mut shapes = vec![];
    for x in [0., 100.] {
        s.execute(Command::InsertRect {
            x,
            y: 0.,
            width: 100.,
            height: 100.,
        })
        .unwrap();
        shapes.push(s.selection().shapes[0]);
    }
    s.select(3, shapes.clone()).unwrap();
    let before = s.snapshot();
    s.execute(Command::Workspace {
        action: WorkspaceAction::TransformSelection {
            dx: 100.,
            dy: 50.,
            scale_x: 2.,
            scale_y: 2.,
            rotation: 0.,
        },
    })
    .unwrap();
    let first = s.model().slides[3].shape(shapes[0]).unwrap().xfrm.unwrap();
    let second = s.model().slides[3].shape(shapes[1]).unwrap().xfrm.unwrap();
    assert_eq!(first.w, 200.);
    assert_eq!(second.x - first.x, 200.);
    s.execute(Command::Undo).unwrap();
    assert_eq!(s.model(), before.model.as_ref());
}

#[test]
fn multiple_text_targets_commit_once_and_failed_later_target_commits_nothing() {
    let mut s = session();
    let mut ids = vec![];
    for value in ["甲😀", "第二段"] {
        s.execute(Command::InsertText {
            x: 10.,
            y: 20.,
            text: value.into(),
        })
        .unwrap();
        ids.push(s.selection().shapes[0]);
    }
    let before = s.snapshot();
    let revision = s.revision();
    let edit = |shape, value| TextEdit {
        target: TextTarget::Shape { shape },
        body: TextBody::from_text(value),
    };
    assert!(
        s.execute(Command::SetTexts {
            edits: vec![edit(ids[0], "wrong"), edit(ShapeId(u32::MAX), "missing")]
        })
        .is_err()
    );
    assert_eq!(s.model(), before.model.as_ref());
    assert_eq!(s.revision(), revision);
    s.execute(Command::SetTexts {
        edits: vec![edit(ids[0], "新甲😀"), edit(ids[1], "新乙")],
    })
    .unwrap();
    assert_eq!(s.revision(), revision + 1);
    let after = s.snapshot();
    s.execute(Command::Undo).unwrap();
    assert_eq!(s.model(), before.model.as_ref());
    s.execute(Command::Redo).unwrap();
    assert_eq!(s.model(), after.model.as_ref());
}

#[test]
fn table_formats_dimensions_and_invalid_batches_are_atomic() {
    let mut s = session();
    s.execute(Command::InsertTable {
        x: 20.,
        y: 20.,
        rows: 2,
        cols: 2,
        width: 300.,
        row_height: 40.,
    })
    .unwrap();
    let before = s.snapshot();
    let id = s.selection().shapes[0];
    let revision = s.revision();
    assert!(
        s.execute(Command::Batch {
            commands: vec![
                Command::Table {
                    action: TableAction::RowHeight {
                        row: 0,
                        height: 60.
                    }
                },
                Command::Table {
                    action: TableAction::ColWidth {
                        col: 99,
                        width: 80.
                    }
                }
            ]
        })
        .is_err()
    );
    assert_eq!(s.model(), before.model.as_ref());
    assert_eq!(s.revision(), revision);
    let range = CellRange {
        row: 0,
        col: 0,
        end_row: 1,
        end_col: 1,
    };
    s.execute(Command::Batch {
        commands: vec![
            Command::Table {
                action: TableAction::Format {
                    range,
                    format: CellFormat {
                        fill_rgb: Some(0xffeedd),
                        borders: Some(CellBorders {
                            sides: [true; 4],
                            rgb: 0x123456,
                            width: 1.5,
                            remove: false,
                        }),
                        margins: Some([5.; 4]),
                        anchor: Some(Anchor::Middle),
                        vertical: Some(TextDir::Horizontal),
                        ..Default::default()
                    },
                },
            },
            Command::Table {
                action: TableAction::RowHeight {
                    row: 0,
                    height: 60.,
                },
            },
            Command::Table {
                action: TableAction::ColWidth { col: 1, width: 80. },
            },
        ],
    })
    .unwrap();
    let ShapeKind::Table(t) = &s.model().slides[0].shape(id).unwrap().kind else {
        unreachable!()
    };
    assert_eq!(t.rows[0].height, 60.);
    assert_eq!(t.cols[1], 80.);
    for c in t.rows.iter().flat_map(|r| &r.cells) {
        assert_eq!(c.anchor, Some(Anchor::Middle));
        assert_eq!(c.borders[0].as_ref().unwrap().width, Some(1.5));
    }
    assert_eq!(s.revision(), revision + 1);
    s.execute(Command::Undo).unwrap();
    assert_eq!(s.model(), before.model.as_ref());
    assert!(
        s.execute(Command::Batch {
            commands: vec![Command::Undo]
        })
        .is_err()
    );
}

#[test]
fn typing_style_survives_composition_cancel_without_changing_existing_text() {
    let mut d = Draft::new(TextBody::from_text("原😀文"));
    d.move_to(3, false);
    let before = d.body.clone();
    d.typing_props = Some(joydeck_model::RunProps {
        bold: Some(true),
        size: Some(36.),
        ..Default::default()
    });
    d.replace(None, "zhong", true, None).unwrap();
    d.cancel();
    assert_eq!(d.body, before);
    d.replace(None, "中", false, None).unwrap();
    assert_eq!(d.body.text(), "原中😀文");
    let p = joydeck_model::edit::props_at(&d.body, (0, 2));
    assert_eq!(p.size, Some(36.));
    assert_ne!(
        joydeck_model::edit::props_at(&d.body, (0, 1)).size,
        Some(36.)
    );
    d.move_to(0, false);
    assert!(d.typing_props.is_none());
}

#[test]
fn native_ranges_keep_surrogates_and_committed_graphemes_intact() -> Result<(), String> {
    use joydeck_engine::text::{to_utf16, utf16_range};

    let initial = "A😀e\u{301}👩‍💻Z";
    let mut draft = Draft::new(TextBody::from_text(initial));
    draft.select_utf16(4..5)?;
    assert_eq!(&initial[draft.selection.clone()], "e\u{301}");
    assert_eq!(to_utf16(initial, draft.selection.clone()), 3..5);
    assert_eq!(utf16_range(initial, 1..3).map(|r| &initial[r]), Some("😀"));
    assert_eq!(
        utf16_range(initial, 4..5).map(|r| &initial[r]),
        Some("\u{301}")
    );
    assert!(draft.select_utf16(2..2).is_err());
    assert!(draft.replace(Some(2..3), "", false, None).is_err());
    assert_eq!(draft.body.text(), initial);
    assert_eq!(&initial[draft.selection.clone()], "e\u{301}");

    draft.select_utf16(4..4)?;
    assert_eq!(to_utf16(initial, draft.selection.clone()), 3..3);
    draft.replace(Some(7..8), "", false, None)?;
    assert_eq!(draft.body.text(), "A😀e\u{301}Z");
    draft.replace(Some(4..5), "é", false, None)?;
    assert_eq!(draft.body.text(), "A😀éZ");
    Ok(())
}

#[test]
fn appkit_mark_ranges_are_relative_and_preserve_preedit_scalar_ranges() -> Result<(), String> {
    let mut draft = Draft::new(TextBody::from_text("A😀Z"));
    draft.select_utf16(3..3)?;
    draft.replace(None, "e\u{301}x", true, Some(1..2))?;
    let value = draft.body.text();
    assert_eq!(&value[draft.selection.clone()], "\u{301}");
    let absolute = draft.marked_relative_range(1..2).ok_or("range overflow")?;
    assert_eq!(absolute, 4..5);
    assert!(
        draft
            .marked_relative_range(usize::MAX..usize::MAX)
            .is_none()
    );
    draft.replace(Some(absolute), "\u{300}", true, None)?;
    assert_eq!(draft.body.text(), "A😀e\u{300}xZ");
    draft.replace(None, "\u{301}", false, None)?;
    assert_eq!(draft.body.text(), "A😀e\u{301}xZ");
    assert!(draft.marked.is_none());
    Ok(())
}

#[test]
fn preedit_selection_survives_newline_normalization_and_cancellation() -> Result<(), String> {
    let mut draft = Draft::new(TextBody::from_text("before"));
    draft.replace(None, "a\r\n😀", true, Some(3..5))?;
    let value = draft.body.text();
    assert_eq!(value, "a\n😀");
    assert_eq!(&value[draft.selection.clone()], "😀");
    draft.cancel();
    assert_eq!(draft.body.text(), "before");
    assert_eq!(draft.selection, 0..6);
    assert!(draft.marked.is_none());
    Ok(())
}

#[test]
fn repeated_preedit_and_reset_preserve_selection_and_committed_text() -> Result<(), String> {
    let mut draft = Draft::new(TextBody::from_text("A😀Z"));
    draft.move_to(5, false);
    draft.move_to(1, true);
    let original = draft.body.clone();
    for _ in 0..2 {
        draft.replace(None, "", true, None)?;
        assert_eq!(draft.body, original);
        assert_eq!(draft.selection, 1..5);
        assert!(draft.reversed);
        assert!(draft.marked.is_none());
    }
    for _ in 0..2 {
        draft.replace(None, "ni", true, Some(2..2))?;
        assert_eq!(draft.body.text(), "AniZ");
        assert_eq!(draft.marked, Some(1..3));
    }
    for _ in 0..2 {
        draft.replace(None, "", true, None)?;
        assert_eq!(draft.body, original);
        assert_eq!(draft.selection, 1..5);
        assert!(draft.reversed);
        assert!(draft.marked.is_none());
    }
    draft.replace(None, "ni", true, None)?;
    draft.replace(None, "你", false, None)?;
    draft.replace(None, "", true, None)?;
    assert_eq!(draft.body.text(), "A你Z");
    assert!(draft.marked.is_none());
    draft.replace(Some(1..2), "", false, None)?;
    assert_eq!(draft.body.text(), "AZ");
    Ok(())
}

mod selection_summary {
    use joydeck_engine::formatting::*;
    use joydeck_model::{Run, RunProps, TextBody, text::Align};
    use std::ops::Range;
    fn summary(body: &TextBody, range: Range<usize>) -> Summary {
        summarize(body, Some(range), |_, r| r.clone(), |p| p.props.clone()).unwrap()
    }
    #[test]
    fn mixed_unicode_runs_ignore_unselected_text_and_use_inheritance() {
        let mut b = TextBody::from_text("甲😀乙");
        b.paragraphs[0].runs = vec![
            Run::with(
                "甲😀",
                RunProps {
                    size: Some(20.),
                    ..Default::default()
                },
            ),
            Run::with(
                "乙",
                RunProps {
                    size: Some(36.),
                    bold: Some(true),
                    ..Default::default()
                },
            ),
        ];
        assert_eq!(summary(&b, 0..7).run.size, Some(20.));
        let mixed = summary(&b, 0..10);
        assert!(mixed.mixed.contains("size"));
        assert!(mixed.mixed.contains("bold"));
        assert_eq!(summary(&b, 7..7).run.size, Some(20.));
        assert!(summarize(&b, Some(1..2), |_, r| r.clone(), |p| p.props.clone()).is_err());
        b.paragraphs[0].def_props.size = Some(36.);
        b.paragraphs[0].runs[0].props.size = None;
        let inherited = summarize(
            &b,
            None,
            |p, r| {
                let mut r = r.clone();
                r.inherit(&p.def_props);
                r
            },
            |p| p.props.clone(),
        )
        .unwrap();
        assert_eq!(inherited.run.size, Some(36.));
        assert!(!inherited.mixed.contains("size"));
    }
    #[test]
    fn paragraph_end_boundary_and_multi_object_mixed_state_are_preserved() {
        let mut a = TextBody::from_text("first\nsecond");
        a.paragraphs[0].props.align = Some(Align::Center);
        a.paragraphs[1].props.align = Some(Align::Right);
        assert_eq!(paragraphs(&a, 0..6).unwrap(), 0..1);
        assert_eq!(summary(&a, 0..6).para.align, Some(Align::Center));
        let mut state = summary(&a, 0..5);
        state.add(summary(&a, 6..12));
        assert!(state.mixed.contains("align"));
        state.add(summary(&a, 0..5));
        assert!(state.mixed.contains("align"));
    }
    #[test]
    fn relative_formatting_keeps_mixed_sizes_and_the_untouched_paragraph() {
        let mut body = TextBody::from_text("ab\nlast");
        body.paragraphs[0].runs = vec![
            Run::with(
                "a",
                RunProps {
                    size: Some(20.),
                    ..Default::default()
                },
            ),
            Run::with(
                "b",
                RunProps {
                    size: Some(36.),
                    ..Default::default()
                },
            ),
        ];
        let last = body.paragraphs[1].clone();
        format_resolved(
            &mut body,
            Some(0..3),
            |_, _, r| r.clone(),
            |r, e| r.size = e.size.map(|s| s + 4.),
        )
        .unwrap();
        assert_eq!(body.paragraphs[0].runs[0].props.size, Some(24.));
        assert_eq!(body.paragraphs[0].runs[1].props.size, Some(40.));
        assert_eq!(body.paragraphs[1], last);
        assert!(
            format_resolved(&mut body, Some(999..999), |_, _, r| r.clone(), |_, _| {}).is_err()
        );
    }
}

#[test]
fn slide_copy_remaps_review_and_animation_targets_without_losing_the_timeline() {
    let mut model = joydeck_model::Presentation::default();
    let id = model.slides[0].shapes[0].id;
    let slide = std::sync::Arc::make_mut(&mut model.slides[0]);
    slide.comments.push(joydeck_model::Comment {
        id: "original-thread".into(),
        author: "author".into(),
        text: "review".into(),
        shape: Some(id),
        ..Default::default()
    });
    slide.animations.push(joydeck_model::Animation {
        shape: id,
        trigger: Some(id),
        ..Default::default()
    });
    let original = model.clone();
    let mut session = joydeck_engine::Session::new(
        model.clone(),
        joydeck_engine::original_bindings(&model),
        true,
    );
    session.execute(Command::DuplicateSlide).unwrap();
    let copy = &session.model().slides[1];
    assert_eq!(copy.animations.len(), 1);
    assert_eq!(copy.comments.len(), 1);
    assert_ne!(copy.animations[0].shape, id);
    assert_eq!(copy.animations[0].shape, copy.shapes[0].id);
    assert_eq!(copy.comments[0].shape, Some(copy.shapes[0].id));
    assert_ne!(copy.comments[0].id, "original-thread");
    session.execute(Command::Undo).unwrap();
    assert_eq!(session.model(), &original);
    session.execute(Command::NewSlide).unwrap();
    assert!(session.model().slides[1].comments.is_empty());
}

#[test]
fn link_label_is_editable_only_for_plain_single_line_ranges() -> Result<(), String> {
    let mut draft = Draft::new(TextBody::from_text("显示😀"));
    let label = joydeck_engine::text::links::plain_label(&draft.body, draft.selection.clone());
    assert_eq!(label.as_deref(), Some("显示😀"));
    draft.move_to(draft.body.text().len(), false);
    let start = draft.cursor();
    draft.insert_field("datetime1", "2026/11/06", Some("zh-CN"))?;
    assert!(joydeck_engine::text::links::plain_label(&draft.body, start..draft.cursor()).is_none());
    let multi = TextBody::from_text("first\nsecond");
    assert!(joydeck_engine::text::links::plain_label(&multi, 0..multi.text().len()).is_none());
    Ok(())
}

#[test]
fn native_field_edits_are_atomic_even_inside_a_utf16_range() -> Result<(), String> {
    let mut d = Draft::new(TextBody::from_text("A😀Z"));
    d.move_to("A😀".len(), false);
    d.insert_field("datetime1", "2026/11/05", Some("zh-CN"))?;
    assert_eq!(d.body.text(), "A😀2026/11/05Z");
    let with_field = d.body.clone();
    assert!(matches!(
        d.body.paragraphs[0].runs[1].kind,
        joydeck_model::text::RunKind::Field { .. }
    ));
    d.select_utf16(5..6)?;
    assert_eq!(&d.body.text()[d.selection.clone()], "2026/11/05");
    d.replace(None, "ni", true, None)?;
    assert_eq!(d.body.text(), "A😀niZ");
    d.cancel();
    assert_eq!(d.body, with_field);
    d.replace(Some(11..12), "", false, None)?;
    assert_eq!(d.body.text(), "A😀Z");
    Ok(())
}

#[test]
fn clear_character_format_preserves_actions_and_proofing_through_undo() {
    use joydeck_model::{
        RunProps, Shape, edit,
        text::{Action, Hyperlink, Run, RunKind},
    };
    use std::sync::Arc;
    let mut p = Presentation::default();
    let target_slide = p.slides[0].id;
    let link = |action| Hyperlink {
        action,
        tooltip: "Keep this tooltip".into(),
        highlight_click: true,
        sound: None,
        stop_sound: false,
        zoom_return: None,
    };
    let props = RunProps {
        bold: Some(true),
        size: Some(32.),
        underline: Some("dbl".into()),
        link: Some(link(Action::Url {
            url: "https://example.org/".into(),
        })),
        hover: Some(link(Action::Slide {
            slide: target_slide,
        })),
        lang: Some("zh-CN".into()),
        no_proof: Some(true),
        ..Default::default()
    };
    let expected = RunProps {
        link: props.link.clone(),
        hover: props.hover.clone(),
        lang: props.lang.clone(),
        no_proof: props.no_proof,
        ..Default::default()
    };
    let mut body = TextBody::from_text("中文😀 link");
    body.paragraphs[0].runs[0].props = props.clone();
    body.paragraphs[0].runs.push(Run {
        kind: RunKind::Field {
            field: "slidenum".into(),
        },
        ..Run::with("1", props)
    });
    let id = ShapeId(p.alloc_id().unwrap());
    Arc::make_mut(&mut p.slides[0]).shapes.push(Shape {
        id,
        text: Some(body.clone()),
        ..Default::default()
    });
    let mut s = Session::new(p.clone(), original_bindings(&p), true);
    let mut cleared = body.clone();
    edit::format_all(
        &mut cleared,
        &joydeck_engine::formatting::clear_character_format,
    );
    assert_eq!(cleared.text(), body.text());
    for (after, before) in cleared.paragraphs[0]
        .runs
        .iter()
        .zip(&body.paragraphs[0].runs)
    {
        assert_eq!(after.props, expected);
        assert_eq!(after.kind, before.kind);
    }
    assert!(
        s.execute(Command::SetTexts {
            edits: vec![TextEdit {
                target: TextTarget::Shape { shape: id },
                body: cleared.clone(),
            }]
        })
        .unwrap()
    );
    assert_eq!(s.revision(), 1);
    assert_eq!(
        s.model().slides[0].shape(id).unwrap().text.as_ref(),
        Some(&cleared)
    );
    assert!(s.execute(Command::Undo).unwrap());
    assert_eq!(s.model(), &p);
    assert!(s.execute(Command::Redo).unwrap());
    assert_eq!(
        s.model().slides[0].shape(id).unwrap().text.as_ref(),
        Some(&cleared)
    );
}
