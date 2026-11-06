#![allow(clippy::unwrap_used, reason = "Test assertions")]
use crate::{SlideBinding, export, export_preserving, import};
use joydeck_model::{
    PhType, Placeholder, Presentation, Shape, ShapeId, SlideId, TextBody, text::RunKind,
};
use std::sync::Arc;

fn placeholder(p: &mut Presentation, kind: PhType, text: &str, field: Option<&str>) -> Shape {
    let mut body = TextBody::from_text(text);
    if let Some(field) = field {
        body.paragraphs[0].runs[0].kind = RunKind::Field {
            field: field.into(),
        };
    }
    Shape {
        id: ShapeId(p.alloc_id().unwrap()),
        ph: Some(Placeholder {
            kind,
            idx: 10,
            ..Default::default()
        }),
        text: Some(body),
        ..Default::default()
    }
}
fn fixture_with_date(format: &str) -> Vec<u8> {
    let mut p = Presentation::default();
    let first = vec![
        placeholder(&mut p, PhType::Footer, "First footer 中文", None),
        placeholder(&mut p, PhType::Date, "Fixed date", None),
        placeholder(&mut p, PhType::Header, "Header", None),
    ];
    Arc::make_mut(&mut p.slides[0]).shapes = first;
    let mut second = (*p.slides[0]).clone();
    second.id = SlideId(p.alloc_id().unwrap());
    second.shapes = vec![
        placeholder(&mut p, PhType::Footer, "Second footer 😀", None),
        placeholder(&mut p, PhType::Date, "Saved date", Some(format)),
        placeholder(&mut p, PhType::SlideNum, "2", Some("slidenum")),
    ];
    p.slides.push(Arc::new(second));
    export(&p).unwrap()
}
fn fixture() -> Vec<u8> {
    fixture_with_date("datetime13")
}

#[test]
fn footer_summary_keeps_header_and_automatic_date_format_without_flattening_slides() {
    let p = import(&fixture()).unwrap();
    let hf = &p.header_footer;
    assert!(hf.header);
    assert!(hf.footer && hf.date && hf.slide_number);
    assert_eq!(hf.footer_text, "First footer 中文");
    assert_eq!(hf.date_text, "Fixed date");
    assert_eq!(hf.date_format, "datetime13");
    assert_eq!(
        p.slides[1].shapes[0].text.as_ref().unwrap().text(),
        "Second footer 😀"
    );
    assert_eq!(
        p.slides[1].shapes[1].text.as_ref().unwrap().paragraphs[0].runs[0].kind,
        RunKind::Field {
            field: "datetime13".into()
        }
    );
}
#[test]
fn changing_one_footer_preserves_other_slide_and_automatic_date() {
    let source = fixture();
    let base = import(&source).unwrap();
    let bindings: Vec<_> = base
        .slides
        .iter()
        .map(|s| SlideBinding {
            slide: s.id,
            source: s.id,
            shapes: s.shapes.iter().map(|sh| (sh.id, sh.id)).collect(),
        })
        .collect();
    let mut current = base.clone();
    Arc::make_mut(&mut current.slides[0]).shapes[0]
        .text
        .as_mut()
        .unwrap()
        .paragraphs[0]
        .runs[0]
        .text = "Edited first footer".into();
    let saved = export_preserving(&source, &base, &current, &bindings).unwrap();
    let reopened = import(&saved).unwrap();
    assert_eq!(reopened.header_footer.footer_text, "Edited first footer");
    assert_eq!(reopened.header_footer.date_format, "datetime13");
    assert_eq!(reopened.slides[1], base.slides[1]);
    let original_pkg = crate::opc::Package::open(&source).unwrap();
    let saved_pkg = crate::opc::Package::open(&saved).unwrap();
    assert_eq!(
        original_pkg.get("ppt/slides/slide2.xml"),
        saved_pkg.get("ppt/slides/slide2.xml")
    );
    assert_eq!(import(&source).unwrap(), base);
}

#[test]
fn automatic_footer_date_summary_keeps_field_identifiers_and_ignores_other_fields() {
    for format in (1..=13).map(|n| format!("datetime{n}")).chain(
        ["datetime", "datetimeFigureOut", "datetime99"]
            .into_iter()
            .map(str::to_owned),
    ) {
        let p = import(&fixture_with_date(&format)).unwrap();
        assert_eq!(p.header_footer.date_format, format);
        assert_eq!(p.header_footer.date_text, "Fixed date");
    }
    let p = import(&fixture_with_date("slidenum")).unwrap();
    assert!(p.header_footer.date_format.is_empty());
}
