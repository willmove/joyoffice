//! Old-host compatibility checks use the exact retained public-format fixture.
use crate::{Cancellation, Document};
use joydeck_engine::{Command, Session, original_bindings};
use std::io::{Cursor, Read};

const OLD_PUBLIC: &[u8] = include_bytes!("../../../tests/fixtures/joydeck/public-format-20.pptx");

fn part(bytes: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut out = vec![];
    zip.by_name(name)
        .map_err(|e| e.to_string())?
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[test]
fn r9_old_host_public_pptx_uses_the_root_relationship() -> Result<(), String> {
    let doc = Document::from_bytes(
        "old-host.pptx".into(),
        OLD_PUBLIC.to_vec(),
        &Cancellation::default(),
    )?;
    assert_eq!(doc.info.format, "pptx");
    assert_eq!(doc.presentation.slides.len(), 20);
    assert_eq!(doc.original.as_ref(), OLD_PUBLIC);
    let session = Session::new(
        doc.presentation.clone(),
        original_bindings(&doc.presentation),
        true,
    );
    let snapshot = session.snapshot();
    let bindings = snapshot
        .origins
        .iter()
        .map(|(slide, origin)| joydeck_pptx::SlideBinding {
            slide: *slide,
            source: origin.slide,
            shapes: origin.shapes.clone(),
        })
        .collect::<Vec<_>>();
    let saved =
        joydeck_pptx::export_preserving(OLD_PUBLIC, &doc.presentation, &snapshot.model, &bindings)
            .map_err(|e| e.to_string())?;
    let mut original = zip::ZipArchive::new(Cursor::new(OLD_PUBLIC)).map_err(|e| e.to_string())?;
    for index in 0..original.len() {
        let entry = original.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?.into_owned();
        assert_eq!(part(OLD_PUBLIC, &name)?, part(&saved, &name)?, "{name}");
    }
    Ok(())
}

#[test]
fn r9_custom_part_paths_survive_edit_undo_redo_and_slide_duplication() -> Result<(), String> {
    let doc = Document::from_bytes(
        "old-host.pptx".into(),
        OLD_PUBLIC.to_vec(),
        &Cancellation::default(),
    )?;
    let mut session = Session::new(
        doc.presentation.clone(),
        original_bindings(&doc.presentation),
        true,
    );
    session.execute(Command::InsertText {
        x: 10.,
        y: 10.,
        text: "R9 中文保留".into(),
    })?;
    session.execute(Command::Undo)?;
    session.execute(Command::Redo)?;
    let duplicate: Command =
        serde_json::from_str(r#"{"command":"duplicate-slide"}"#).map_err(|e| e.to_string())?;
    session.execute(duplicate)?;
    let snapshot = session.snapshot();
    let bindings = snapshot
        .origins
        .iter()
        .map(|(slide, origin)| joydeck_pptx::SlideBinding {
            slide: *slide,
            source: origin.slide,
            shapes: origin.shapes.clone(),
        })
        .collect::<Vec<_>>();
    let saved =
        joydeck_pptx::export_preserving(OLD_PUBLIC, &doc.presentation, &snapshot.model, &bindings)
            .map_err(|e| e.to_string())?;
    let reopened = Document::from_bytes(
        "edited.pptx".into(),
        saved.clone(),
        &Cancellation::default(),
    )?;
    assert_eq!(reopened.presentation.slides.len(), 21);
    for slide in &reopened.presentation.slides[..2] {
        assert!(
            slide
                .shapes
                .iter()
                .any(|s| s.text.as_ref().is_some_and(|t| t.text() == "R9 中文保留"))
        );
    }
    assert_eq!(
        part(OLD_PUBLIC, "_rels/.rels")?,
        part(&saved, "_rels/.rels")?
    );
    assert_eq!(
        part(OLD_PUBLIC, "slideset/pages/p1.xml")?,
        part(&saved, "slideset/pages/p1.xml")?
    );
    assert!(part(&saved, "slideset/main.xml").is_ok());
    assert!(part(&saved, "ppt/presentation.xml").is_err());
    Ok(())
}

#[test]
#[allow(clippy::unwrap_used, reason = "Test assertions")]
fn clear_character_format_preserves_links_in_native_and_preserving_pptx_save() {
    use joydeck_engine::{TextEdit, TextTarget, formatting::clear_character_format};
    use joydeck_model::{
        Presentation, Shape, ShapeId, TextBody, edit,
        text::{Action, Hyperlink},
    };
    use std::sync::Arc;
    let mut p = Presentation::default();
    let id = ShapeId(p.alloc_id().unwrap());
    let mut body = TextBody::from_text("中文😀 linked text");
    let props = &mut body.paragraphs[0].runs[0].props;
    let link = |action| Hyperlink {
        action,
        tooltip: "keep tooltip".into(),
        highlight_click: true,
        sound: None,
        stop_sound: false,
        zoom_return: None,
    };
    props.link = Some(link(Action::Url {
        url: "https://example.org/clear-format".into(),
    }));
    props.hover = Some(link(Action::Slide {
        slide: p.slides[0].id,
    }));
    props.lang = Some("zh-CN".into());
    props.no_proof = Some(true);
    props.bold = Some(true);
    props.size = Some(32.);
    Arc::make_mut(&mut p.slides[0]).shapes.push(Shape {
        id,
        text: Some(body),
        ..Default::default()
    });
    let source = joydeck_pptx::export(&p).unwrap();
    let baseline = joydeck_pptx::import(&source).unwrap();
    let shape = baseline.slides[0].shapes.last().unwrap();
    let target = TextTarget::Shape { shape: shape.id };
    let original = shape.text.clone().unwrap();
    let before_props = original.paragraphs[0].runs[0].props.clone();
    let mut cleared = original.clone();
    edit::format_all(&mut cleared, &clear_character_format);
    let mut session = Session::new(baseline.clone(), original_bindings(&baseline), true);
    session
        .execute(Command::SetTexts {
            edits: vec![TextEdit {
                target,
                body: cleared.clone(),
            }],
        })
        .unwrap();
    let snapshot = session.snapshot();
    let bindings: Vec<_> = snapshot
        .origins
        .iter()
        .map(|(slide, origin)| joydeck_pptx::SlideBinding {
            slide: *slide,
            source: origin.slide,
            shapes: origin.shapes.clone(),
        })
        .collect();
    let public =
        joydeck_pptx::export_preserving(&source, &baseline, &snapshot.model, &bindings).unwrap();
    let native = joydeck_format::save(&snapshot.model).unwrap();
    for reopened in [
        joydeck_pptx::import(&public).unwrap(),
        joydeck_format::load(&native).unwrap(),
    ] {
        let body = reopened.slides[0]
            .shapes
            .last()
            .unwrap()
            .text
            .as_ref()
            .unwrap();
        assert_eq!(body.text(), original.text());
        let props = &body.paragraphs[0].runs[0].props;
        assert_eq!(props.link, before_props.link);
        assert_eq!(props.hover, before_props.hover);
        assert_eq!(props.lang, before_props.lang);
        assert_eq!(props.no_proof, before_props.no_proof);
        assert_ne!(props.bold, Some(true));
        assert_ne!(props.size, Some(32.));
    }
    assert_eq!(joydeck_pptx::import(&source).unwrap(), baseline);
}

#[test]
#[allow(clippy::unwrap_used, reason = "Test assertions")]
fn existing_footer_command_roundtrips_without_flattening_page_styles_or_dates() {
    use joydeck_model::{
        PhType, Placeholder, Presentation, Shape, ShapeId, SlideId, TextBody, text::RunKind,
    };
    use std::sync::Arc;
    let mut p = Presentation::default();
    for page in 0..3 {
        let mut slide = (*p.slides[0]).clone();
        slide.id = SlideId(p.alloc_id().unwrap());
        slide.shapes.clear();
        for kind in [PhType::Footer, PhType::Date] {
            if page == 2 && kind == PhType::Footer {
                continue;
            }
            let mut body = TextBody::from_text(if kind == PhType::Footer {
                "Original footer"
            } else {
                "Saved date"
            });
            body.paragraphs[0].runs[0].props.size = Some(11. + f64::from(page));
            if kind == PhType::Date {
                body.paragraphs[0].runs[0].kind = RunKind::Field {
                    field: "datetime13".into(),
                };
            }
            slide.shapes.push(Shape {
                id: ShapeId(p.alloc_id().unwrap()),
                ph: Some(Placeholder {
                    kind,
                    idx: if kind == PhType::Footer { 11 } else { 10 },
                    ..Default::default()
                }),
                text: Some(body),
                ..Default::default()
            });
        }
        if page == 0 {
            p.slides[0] = Arc::new(slide);
        } else {
            p.slides.push(Arc::new(slide));
        }
    }
    let source = joydeck_pptx::export(&p).unwrap();
    let base = joydeck_pptx::import(&source).unwrap();
    let mut session = Session::new(base.clone(), original_bindings(&base), true);
    let cmd: Command = serde_json::from_str(
        r#"{"command":"set-existing-footer-text","text":"Shared footer 中文😀"}"#,
    )
    .unwrap();
    assert!(session.execute(cmd).unwrap());
    let snapshot = session.snapshot();
    let bindings: Vec<_> = snapshot
        .origins
        .iter()
        .map(|(slide, origin)| joydeck_pptx::SlideBinding {
            slide: *slide,
            source: origin.slide,
            shapes: origin.shapes.clone(),
        })
        .collect();
    let public =
        joydeck_pptx::export_preserving(&source, &base, &snapshot.model, &bindings).unwrap();
    let native = joydeck_format::save(&snapshot.model).unwrap();
    assert_eq!(joydeck_format::load(&native).unwrap(), *snapshot.model);
    let reopened = joydeck_pptx::import(&public).unwrap();
    for (before, after) in base.slides.iter().zip(&reopened.slides) {
        assert_eq!(before.shapes.len(), after.shapes.len());
        for (before, after) in before.shapes.iter().zip(&after.shapes) {
            let mut expected = before.clone();
            if expected.ph_type() == Some(PhType::Footer) {
                expected.text.as_mut().unwrap().paragraphs[0].runs[0].text =
                    "Shared footer 中文😀".into();
            }
            assert_eq!(&expected, after);
        }
    }
    assert_eq!(
        part(&source, "ppt/slides/slide3.xml").unwrap(),
        part(&public, "ppt/slides/slide3.xml").unwrap()
    );
    assert!(session.execute(Command::Undo).unwrap());
    assert_eq!(session.model(), &base);
    assert!(session.execute(Command::Redo).unwrap());
    assert_eq!(session.model(), snapshot.model.as_ref());
}
