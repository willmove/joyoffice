#![allow(clippy::unwrap_used, reason = "Test assertions")]
use joydeck_engine::{Command, Session, original_bindings};
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
        assert!(s.execute(command()).is_err());
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
