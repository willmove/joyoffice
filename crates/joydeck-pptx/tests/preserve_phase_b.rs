use joydeck_model::style::{ColorRef, Dash, Fill, Line};
use joydeck_model::{Presentation, Rgba, ShapeId, ShapeKind, SlideId, Transition};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

fn source_bindings(p: &Presentation) -> joydeck_pptx::preserve::Source<'_> {
    let slides = p
        .slides
        .iter()
        .map(|s| {
            let mut shapes: BTreeMap<ShapeId, ShapeId> = BTreeMap::new();
            for sh in &s.shapes {
                shapes.insert(sh.id, sh.id);
            }
            (
                s.id,
                joydeck_pptx::preserve::SlideBinding {
                    slide: s.id,
                    shapes,
                },
            )
        })
        .collect::<BTreeMap<SlideId, _>>();
    joydeck_pptx::preserve::Source {
        bytes: &[],
        model: p,
        slides,
    }
}

#[test]
fn preserving_save_round_trips_shape_outline_and_slide_transition() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/joydeck/r4-r5/independent-complex.pptx");
    let raw = std::fs::read(&fixture).expect("fixture bytes");
    let original = joydeck_pptx::import(&raw).expect("import original");

    let mut edited = original.clone();
    {
        let slide = Arc::make_mut(&mut edited.slides[0]);
        let shape = slide
            .shapes
            .iter_mut()
            .find(|s| matches!(s.kind, ShapeKind::Shape))
            .expect("basic shape");
        shape.line = Some(Line {
            fill: Some(Fill::solid(ColorRef::rgb(Rgba::rgb(0xd0, 0x40, 0x20)))),
            width: Some(2.5),
            dash: Some(Dash::Preset {
                name: "dash".into(),
            }),
            ..Line::default()
        });
        slide.transition = Some(Transition {
            kind: "push".into(),
            option: "l".into(),
            duration_ms: 800,
            advance_on_click: true,
            advance_after_ms: Some(3000),
            ..Transition::default()
        });
    }

    let mut src = source_bindings(&original);
    src.bytes = &raw;
    let saved = joydeck_pptx::preserve::save(&src, &edited).expect("preserving save");
    let reopened = joydeck_pptx::import(&saved).expect("reopen saved");

    let shape = reopened.slides[0]
        .shapes
        .iter()
        .find(|s| matches!(s.kind, ShapeKind::Shape))
        .expect("reopened shape");
    let line = shape.line.as_ref().expect("outline preserved");
    assert_eq!(line.width, Some(2.5));
    assert_eq!(
        line.dash,
        Some(Dash::Preset {
            name: "dash".into()
        })
    );

    let tr = reopened.slides[0]
        .transition
        .as_ref()
        .expect("transition preserved");
    assert_eq!(tr.kind, "push");
    assert_eq!(tr.option, "l");
    assert_eq!(tr.duration_ms, 800);
    assert_eq!(tr.advance_after_ms, Some(3000));

    // Clearing the transition removes it on a subsequent preserving save.
    let mut cleared = reopened.clone();
    Arc::make_mut(&mut cleared.slides[0]).transition = None;
    let mut src2 = source_bindings(&reopened);
    src2.bytes = &saved;
    let saved2 = joydeck_pptx::preserve::save(&src2, &cleared).expect("preserving clear");
    let reopened2 = joydeck_pptx::import(&saved2).expect("reopen cleared");
    assert!(reopened2.slides[0].transition.is_none());
}

#[test]
fn pie_chart_show_percent_imports_and_regenerates() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/joydeck/m9-powerpoint-edited-chart.pptx");
    let raw = std::fs::read(&fixture).expect("m9 fixture bytes");
    let doc = joydeck_pptx::import(&raw).expect("import m9");
    let pie = doc.slides[3]
        .shapes
        .iter()
        .find_map(|s| match &s.kind {
            ShapeKind::Chart(c) => Some(c),
            _ => None,
        })
        .expect("pie chart on slide 4");
    assert!(pie.series_data_label_percent(&pie.series[0]));
    assert!(!pie.series_data_labels(&pie.series[0]));

    let exported = joydeck_pptx::export(&doc).expect("regenerate m9");
    let reopened = joydeck_pptx::import(&exported).expect("reopen regenerated m9");
    let pie2 = reopened.slides[3]
        .shapes
        .iter()
        .find_map(|s| match &s.kind {
            ShapeKind::Chart(c) => Some(c),
            _ => None,
        })
        .expect("reopened pie chart");
    assert!(pie2.series_data_label_percent(&pie2.series[0]));
}
