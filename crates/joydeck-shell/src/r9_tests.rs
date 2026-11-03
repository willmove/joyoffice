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
