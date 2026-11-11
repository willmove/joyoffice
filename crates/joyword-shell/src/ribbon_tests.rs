//! Desktop ribbon commands must affect the same rendered and saved document.
use crate::reader::Reader;
use serde_json::json;
use std::sync::atomic::AtomicBool;

#[test]
fn watermark_measures_new_text_and_preserves_its_box_through_color_edits_and_docx()
-> Result<(), String> {
    let cancel = AtomicBool::new(false);
    let mut r = Reader::blank(&cancel)?;
    for text in ["WIDE WATERMARK", "草稿"] {
        let depth = r.engine().undo_depth();
        r.execute("design.watermark", &json!({"text":text,"diagonal":true}))?;
        let wm = r
            .engine()
            .doc
            .settings
            .watermark
            .as_ref()
            .ok_or("No watermark")?;
        let [width, height] = wm.size.ok_or("Watermark was not measured")?;
        let face = joyword_fonts::word::resolve_text(&wm.font, text, false, false).face;
        let advance = joyword_fonts::shape(&face, text, &[], |c| c)
            .iter()
            .map(|g| g.x_advance as f32)
            .sum::<f32>()
            / face.upem as f32;
        assert!((height - (width / advance).clamp(12.0, 400.0)).abs() < 0.01);
        assert_eq!(r.engine().undo_depth(), depth + 1);
        r.execute("design.watermark", &json!({"color":"315FA8"}))?;
        assert_eq!(
            r.engine()
                .doc
                .settings
                .watermark
                .as_ref()
                .and_then(|w| w.size),
            Some([width, height])
        );
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let path = dir.path().join("watermark.docx");
        r.save_job(path.clone(), true)?.run()?;
        let reopened = Reader::open(&path, &cancel)?;
        let saved = reopened
            .engine()
            .doc
            .settings
            .watermark
            .as_ref()
            .ok_or("Lost watermark")?;
        assert_eq!(saved.text, text);
        let size = saved.frame(
            reopened.engine().doc.last_section.page_w,
            reopened.engine().doc.last_section.page_h,
        );
        assert!((size[0] - width).abs() < 0.06 && (size[1] - height).abs() < 0.06);
        assert!(!reopened.print_job("1")?.pdf(false)?.is_empty());
        r.execute("edit.undo", &json!({}))?;
        r.execute("edit.undo", &json!({}))?;
        assert!(r.engine().doc.settings.watermark.is_none());
    }
    Ok(())
}

#[test]
fn merge_field_shading_maps_to_visible_fields_without_changing_the_document() -> Result<(), String>
{
    let mut r = Reader::blank(&AtomicBool::new(false))?;
    r.execute("text.insert", &json!({"text":"Dear "}))?;
    r.execute("mailings.insertField", &json!({"field":"Name"}))?;
    r.execute("text.insert", &json!({"text":", welcome"}))?;
    let before = r.export_snapshot()?;
    let rev = r.engine().rev();
    let rects = r.merge_field_rects(0);
    assert!(!rects.is_empty());
    assert!(rects.iter().all(|r| r.w > 0. && r.h > 0.));
    assert!(r.merge_field_rects(999).is_empty());
    r.execute("mailings.highlightFields", &json!({"value":true}))?;
    assert!(r.engine().merge.highlight);
    assert_eq!(r.export_snapshot()?, before);
    assert_eq!(r.engine().rev(), rev);
    Ok(())
}

#[test]
fn hidden_ink_changes_the_screen_only_and_save_reopen_keeps_the_stroke() -> Result<(), String> {
    let cancel = AtomicBool::new(false);
    let mut r = Reader::blank(&cancel)?;
    r.execute(
        "draw.stroke",
        &json!({"page":0,"points":[[100,100],[180,130],[250,100]],"color":"C00000","width":4}),
    )?;
    let original = r.scene().raster(0, 1.)?.pixels;
    let rev = r.engine().rev();
    let depth = r.engine().undo_depth();
    let printed = r.print_job("1")?.pdf(false)?;
    let presentation = r.presentation_revision();
    r.execute("review.hideInk", &json!({"value":true}))?;
    assert!(
        r.print_job("1")?.pdf(false)? == printed,
        "Hide Ink must not change print output"
    );
    assert_eq!(r.engine().rev(), rev);
    assert_eq!(r.engine().undo_depth(), depth);
    assert!(r.presentation_revision() > presentation);
    assert_ne!(r.scene().raster(0, 1.)?.pixels, original);
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = dir.path().join("ink.docx");
    r.save_job(path.clone(), true)?.run()?;
    let reopened = Reader::open(&path, &cancel)?;
    assert!(!reopened.engine().view.hide_ink);
    assert!(
        reopened.scene().raster(0, 1.)?.pixels == original,
        "Ink changed after DOCX save/reopen"
    );
    r.execute("review.hideInk", &json!({"value":false}))?;
    assert!(
        r.scene().raster(0, 1.)?.pixels == original,
        "Ink was not restored on screen"
    );
    Ok(())
}

#[test]
fn view_modes_refresh_the_host_layout_and_read_mode_rejects_native_and_command_edits()
-> Result<(), String> {
    let mut r = Reader::blank(&AtomicBool::new(false))?;
    r.execute("tools.recordMacro", &json!({"name":"EditingMacro"}))?;
    r.execute(
        "text.insert",
        &json!({"text":"A heading\nBody text for a reading view."}),
    )?;
    r.execute("tools.recordMacro", &json!({}))?;
    let id = r.engine().document_id();
    let rev = r.engine().rev();
    let depth = r.engine().undo_depth();
    let printed = r.print_job("1")?.pdf(false)?;
    for command in [
        "view.webLayout",
        "view.draft",
        "view.outline",
        "view.printLayout",
        "view.readMode",
    ] {
        let presentation = r.presentation_revision();
        r.execute(command, &json!({}))?;
        assert!(r.presentation_revision() > presentation, "{command}");
        assert!(!r.summary.pages.is_empty());
        assert_eq!(
            (
                r.engine().document_id(),
                r.engine().rev(),
                r.engine().undo_depth()
            ),
            (id, rev, depth)
        );
        assert!(
            r.print_job("1")?.pdf(false)? == printed,
            "Print output changed in {command}"
        );
    }
    let text = r.body_text();
    assert!(r.formatting_disabled("text.insert").is_some());
    assert!(
        r.execute("text.insert", &json!({"text":"blocked"}))
            .is_err()
    );
    assert!(r.replace_text(None, "blocked").is_err());
    assert!(r.mark_text(None, "blocked", None).is_err());
    assert_eq!(r.body_text(), text);
    assert!(!r.is_composing());
    assert!(!r.can_undo() && !r.can_redo());
    for (command, args) in [
        ("edit.undo", json!({})),
        ("edit.redo", json!({})),
        ("edit.undoTo", json!({"count":1})),
        ("tools.macros", json!({"run":"EditingMacro"})),
    ] {
        assert!(r.execute(command, &args).is_err());
        assert_eq!(r.body_text(), text);
        assert_eq!(r.engine().undo_depth(), depth);
        assert_eq!(r.engine().rev(), rev);
    }
    r.execute("view.printLayout", &json!({}))?;
    assert!(r.can_undo());
    r.replace_text(None, " allowed")?;
    assert!(r.body_text().contains("allowed"));
    Ok(())
}

#[test]
fn label_copy_preserves_unsaved_template_and_roundtrips_a_real_table() -> Result<(), String> {
    let cancel = AtomicBool::new(false);
    let mut r = Reader::blank(&cancel)?;
    r.execute("text.insert", &json!({"text":"Unsaved source document"}))?;
    let before = r.export_snapshot()?;
    let identity = (r.engine().document_id(), r.engine().rev());
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = dir.path().join("labels.docx");
    r.labels_job(&json!({"text":"示例标签","product":"a4-3x7"}))?
        .publish(path.clone())?;
    assert_eq!(r.export_snapshot()?, before);
    assert_eq!((r.engine().document_id(), r.engine().rev()), identity);
    assert!(r.is_dirty());
    let result = Reader::open(&path, &cancel)?;
    assert!(result.body_text().contains("示例标签"));
    assert!(
        result
            .engine()
            .doc
            .body
            .iter()
            .filter_map(|b| b.as_table())
            .any(|t| t.rows.len() == 7 && t.grid.len() == 5)
    );
    assert_eq!(result.body_text().matches("示例标签").count(), 21);
    // A recovered source can retain an exact original package for Save As.
    // That package must never replace the newly generated label document.
    r.recovered_package = Some((
        r.engine().rev(),
        std::sync::Arc::from(crate::reader::WELCOME),
    ));
    let recovered_labels = dir.path().join("recovered-labels.docx");
    r.labels_job(&json!({"text":"Recovered label copy","product":"a4-3x7"}))?
        .publish(recovered_labels.clone())?;
    assert_eq!(
        Reader::open(&recovered_labels, &cancel)?
            .body_text()
            .matches("Recovered label copy")
            .count(),
        21
    );
    assert!(
        r.labels_job(&json!({"text":"label","product":"invalid"}))
            .is_err()
    );
    assert!(
        r.labels_job(&json!({"text":"label","product":"a4-3x7"}))?
            .publish(path)
            .is_err()
    );
    assert_eq!(r.export_snapshot()?, before);
    Ok(())
}
