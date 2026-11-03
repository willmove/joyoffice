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
