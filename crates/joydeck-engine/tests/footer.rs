#![allow(clippy::unwrap_used, reason = "Test assertions")]
use joydeck_engine::{Command, Session, existing_footer_summary, original_bindings};
use joydeck_model::{
    PhType, Placeholder, Presentation, Shape, ShapeId, ShapeKind, SlideId, TextBody,
    text::{Run, RunKind},
};
use std::sync::Arc;
fn deck() -> Presentation {
    let mut p = Presentation::default();
    for i in 0..3 {
        let mut slide = (*p.slides[0]).clone();
        slide.id = SlideId(p.alloc_id().unwrap());
        slide.shapes.clear();
        if i < 2 {
            let mut body = TextBody::from_text(if i == 0 { "First" } else { "Second" });
            body.paragraphs[0].runs[0].props.bold = Some(i == 0);
            body.paragraphs[0].runs[0].props.size = Some(12. + f64::from(i));
            slide.shapes.push(Shape {
                id: ShapeId(p.alloc_id().unwrap()),
                ph: Some(Placeholder {
                    kind: PhType::Footer,
                    ..Default::default()
                }),
                text: Some(body),
                ..Default::default()
            });
        }
        if i == 0 {
            p.slides[0] = Arc::new(slide);
        } else {
            p.slides.push(Arc::new(slide));
        }
    }
    p
}
fn session(p: Presentation) -> Session {
    Session::new(p.clone(), original_bindings(&p), true)
}
fn command() -> Command {
    serde_json::from_str(r#"{"command":"set-existing-footer-text","text":"统一页脚 😀"}"#).unwrap()
}
#[test]
fn footer_update_is_one_edit_preserving_page_styles_absent_footers_and_selection() {
    let p = deck();
    let mut s = session(p.clone());
    s.select(2, vec![]).unwrap();
    let selected = s.selection().clone();
    assert!(s.execute(command()).unwrap());
    assert_eq!(s.revision(), 1);
    assert_eq!(s.selection(), &selected);
    assert_eq!(s.model().slides[2], p.slides[2]);
    for i in 0..2 {
        let mut expected = p.slides[i].shapes[0].clone();
        expected.text.as_mut().unwrap().paragraphs[0].runs[0].text = "统一页脚 😀".into();
        assert_eq!(s.model().slides[i].shapes[0], expected);
    }
    let changed = s.model().clone();
    assert!(!s.execute(command()).unwrap());
    assert_eq!(s.revision(), 1);
    assert!(s.execute(Command::Undo).unwrap());
    assert_eq!(s.model(), &p);
    assert!(!s.can_undo());
    assert!(s.execute(Command::Redo).unwrap());
    assert_eq!(s.model(), &changed);
}
#[test]
fn unsupported_footer_on_later_page_leaves_every_page_and_history_unchanged() {
    for mode in 0..5 {
        let mut p = deck();
        let shape = &mut Arc::make_mut(&mut p.slides[1]).shapes[0];
        match mode {
            0 => shape.locked = true,
            1 => shape.hidden = true,
            2 => shape.text.as_mut().unwrap().paragraphs[0]
                .runs
                .push(Run::new("mixed")),
            3 => {
                shape.text.as_mut().unwrap().paragraphs[0].runs[0].kind = RunKind::Field {
                    field: "footer".into(),
                }
            }
            _ => {
                let child = shape.clone();
                shape.ph = None;
                shape.kind = ShapeKind::Group {
                    children: vec![child],
                    child: Default::default(),
                };
            }
        }
        let mut s = session(p.clone());
        let summary_error = existing_footer_summary(&p).unwrap_err();
        assert_eq!(s.execute(command()).unwrap_err(), summary_error);
        assert_eq!(s.model(), &p);
        assert_eq!(s.revision(), 0);
        assert!(!s.can_undo());
        assert!(!s.dirty());
    }
}
#[test]
fn missing_footers_and_invalid_text_are_explicit_errors() {
    assert!(session(Presentation::default()).execute(command()).is_err());
    for value in ["x\ny".to_owned(), "x\ty".to_owned(), "x".repeat(65537)] {
        let p = deck();
        let mut s = session(p.clone());
        assert!(
            s.execute(Command::SetExistingFooterText { text: value })
                .is_err()
        );
        assert_eq!(s.model(), &p);
    }
}

#[test]
fn footer_target_and_output_budgets_fail_atomically() {
    for count in [17, 257] {
        let mut p = deck();
        let template = p.slides[0].shapes[0].clone();
        let mut shapes = vec![];
        for _ in 0..count {
            let mut shape = template.clone();
            shape.id = ShapeId(p.alloc_id().unwrap());
            shapes.push(shape);
        }
        Arc::make_mut(&mut p.slides[0]).shapes = shapes;
        let mut s = session(p.clone());
        let value = if count == 17 {
            "x".repeat(65536)
        } else {
            "x".into()
        };
        assert!(
            s.execute(Command::SetExistingFooterText { text: value })
                .is_err()
        );
        assert_eq!(s.model(), &p);
        assert_eq!(s.revision(), 0);
        assert!(!s.can_undo());
    }
}

#[test]
fn footer_summary_distinguishes_mixed_empty_and_uniform_without_mutation() {
    let mut p = deck();
    p.header_footer.footer_text = "Misleading document summary".into();
    let mut s = session(p.clone());
    s.select(2, vec![]).unwrap();
    let summary = existing_footer_summary(s.model()).unwrap();
    assert_eq!(summary.text, None);
    assert_eq!(summary.targets, 2);
    assert_eq!(summary.pages_with_footer, 2);
    assert_eq!(summary.pages_without_footer, 1);
    assert_eq!(s.model(), &p);
    assert_eq!(s.selection().page, 2);
    assert_eq!(s.revision(), 0);
    assert!(!s.can_undo());
    for value in ["", "相同页脚"] {
        for slide in &mut p.slides[..2] {
            Arc::make_mut(slide).shapes[0].text = Some(TextBody::from_text(value));
        }
        assert_eq!(
            existing_footer_summary(&p).unwrap().text.as_deref(),
            Some(value)
        );
    }
    assert!(existing_footer_summary(&Presentation::default()).is_err());
}

#[test]
fn footer_summary_counts_multiple_placeholders_and_enforces_target_budget() {
    let mut p = deck();
    let template = p.slides[0].shapes[0].clone();
    let mut shapes = vec![];
    for _ in 0..257 {
        let mut shape = template.clone();
        shape.id = ShapeId(p.alloc_id().unwrap());
        shapes.push(shape);
    }
    Arc::make_mut(&mut p.slides[0]).shapes = shapes;
    assert!(existing_footer_summary(&p).is_err());
    Arc::make_mut(&mut p.slides[0]).shapes.truncate(2);
    let summary = existing_footer_summary(&p).unwrap();
    assert_eq!(summary.targets, 3);
    assert_eq!(summary.pages_with_footer, 2);
    assert_eq!(summary.pages_without_footer, 1);
    assert_eq!(summary.text, None);
}

fn insertable_deck() -> Presentation {
    use joydeck_model::{LayoutType, PlaceholderVisibility};
    let mut p = deck();
    let master = Arc::make_mut(&mut p.masters[0]);
    master.placeholder_visibility = Some(PlaceholderVisibility {
        footer: Some(true),
        ..Default::default()
    });
    let layout = master
        .layouts
        .iter_mut()
        .find(|l| l.id == p.slides[2].layout)
        .unwrap();
    layout.kind = LayoutType::TitleAndContent;
    layout.placeholder_visibility = master.placeholder_visibility.clone();
    p
}
fn add_command() -> Command {
    serde_json::from_str(r#"{"command":"add-missing-footer-text","text":"New footer 中文"}"#)
        .unwrap()
}
#[test]
fn missing_footer_insertion_is_selected_page_only_with_one_undo() {
    let mut p = insertable_deck();
    // An unrelated page uses a different, unclassified layout and stays untouched.
    Arc::make_mut(&mut p.slides[0]).layout = p.masters[0].layouts[1].id;
    let mut s = session(p.clone());
    s.select(2, vec![]).unwrap();
    let selection = s.selection().clone();
    assert!(s.validate(add_command()).unwrap());
    assert_eq!(s.model(), &p);
    assert!(s.execute(add_command()).unwrap());
    assert_eq!(s.selection(), &selection);
    assert_eq!(s.model().slides[..2], p.slides[..2]);
    assert_eq!(s.model().masters, p.masters);
    assert_eq!(s.model().header_footer, p.header_footer);
    let footer = &s.model().slides[2].shapes[0];
    assert_eq!(footer.ph_type(), Some(PhType::Footer));
    assert_eq!(footer.text.as_ref().unwrap().text(), "New footer 中文");
    let changed = s.model().clone();
    assert!(s.execute(add_command()).is_err());
    assert_eq!(s.model(), &changed);
    assert_eq!(s.revision(), 1);
    assert!(s.execute(Command::Undo).unwrap());
    assert_eq!(s.model(), &p);
    assert!(!s.can_undo());
    assert!(s.execute(Command::Redo).unwrap());
    assert_eq!(s.model(), &changed);
}
#[test]
fn missing_footer_rejects_ambiguity_without_touching_document_or_history() {
    for mode in 0..16 {
        let mut p = insertable_deck();
        let layout_id = p.slides[2].layout;
        let master = Arc::make_mut(&mut p.masters[0]);
        let layout = master
            .layouts
            .iter_mut()
            .find(|l| l.id == layout_id)
            .unwrap();
        let i = layout
            .shapes
            .iter()
            .position(|s| s.ph_type() == Some(PhType::Footer))
            .unwrap();
        match mode {
            0 => layout.placeholder_visibility = None,
            1 => layout.placeholder_visibility.as_mut().unwrap().footer = Some(false),
            2 => layout.shapes.push(layout.shapes[i].clone()),
            3 => layout.shapes[i].xfrm = None,
            4 => layout.shapes[i].locked = true,
            5 => layout.shapes[i].raw_ext = Some("<unknown/>".into()),
            6 => layout.kind = joydeck_model::LayoutType::Title,
            7 => layout.shapes[i].xfrm.as_mut().unwrap().w = f64::NAN,
            8 => master.placeholder_visibility = None,
            9 => {
                let duplicate = layout.clone();
                master.layouts.push(duplicate);
            }
            10 => {
                layout.shapes.remove(i);
            }
            11 => master.raw_ext = Some("<unknown/>".into()),
            12 => layout.shapes[i].hidden = true,
            13 => {
                let mut body = TextBody::from_text("Field");
                body.paragraphs[0].runs[0].kind = RunKind::Field {
                    field: "footer".into(),
                };
                layout.shapes[i].text = Some(body);
            }
            14 => {
                let child = layout.shapes[i].clone();
                layout.shapes[i].ph = None;
                layout.shapes[i].kind = ShapeKind::Group {
                    children: vec![child],
                    child: Default::default(),
                };
            }
            _ => p.next_id = u32::MAX,
        }
        let mut s = session(p.clone());
        s.select(2, vec![]).unwrap();
        assert!(s.execute(add_command()).is_err(), "mode {mode}");
        // NaN does not compare equal to itself: compare serialized snapshots.
        assert_eq!(
            serde_json::to_vec(s.model()).unwrap(),
            serde_json::to_vec(&p).unwrap()
        );
        assert_eq!(s.revision(), 0);
        assert!(!s.can_undo());
    }
}
