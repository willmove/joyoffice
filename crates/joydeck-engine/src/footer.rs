//! Conservative document-wide editing of existing slide footer placeholders.
use crate::{EditorSnapshot, TextTarget, set_draft_body, text};
use joydeck_model::{
    PhType, ShapeKind,
    text::{Run, RunKind},
    walk,
};
use std::sync::Arc;

pub(super) fn set_existing(st: &mut EditorSnapshot, value: &str) -> Result<(), String> {
    if value.len() > 65536 || value.chars().any(char::is_control) {
        return Err("footer text must be a single line of at most 64 KiB".into());
    }
    let mut edits = Vec::new();
    for (page, slide) in st.model.slides.iter().enumerate() {
        let mut grouped = false;
        walk(&slide.shapes, &mut |shape, depth| {
            grouped |= depth > 0 && shape.ph_type() == Some(PhType::Footer);
        });
        if grouped {
            return Err("grouped footer placeholders require individual editing".into());
        }
        for shape in &slide.shapes {
            if shape.ph_type() != Some(PhType::Footer) {
                continue;
            }
            if shape.locked || shape.hidden || !matches!(shape.kind, ShapeKind::Shape) {
                return Err("locked, hidden or non-text footers require individual editing".into());
            }
            let mut body = shape.text.clone().unwrap_or_default();
            if body.paragraphs.is_empty() {
                body.paragraphs.push(Default::default());
            }
            if body.paragraphs.len() != 1
                || body.paragraphs[0].runs.len() > 1
                || body.paragraphs[0]
                    .runs
                    .iter()
                    .any(|r| r.kind != RunKind::Text)
            {
                return Err(
                    "multi-paragraph, mixed-run or field footers require individual editing".into(),
                );
            }
            let para = &mut body.paragraphs[0];
            if let Some(run) = para.runs.first_mut() {
                run.text = value.into();
            } else if !value.is_empty() {
                para.runs.push(Run::with(value, para.end_props.clone()));
            }
            text::validate(&body)?;
            edits.push((page, TextTarget::Shape { shape: shape.id }, body));
            if edits.len() > 256 || value.len().saturating_mul(edits.len()) > 1 << 20 {
                return Err("footer update exceeds 256 targets or 1 MiB".into());
            }
        }
    }
    if edits.is_empty() {
        return Err("document has no editable footer placeholders".into());
    }
    let model = Arc::make_mut(&mut st.model);
    for (page, target, body) in edits {
        set_draft_body(model, page, &target, body)?;
    }
    model.header_footer.footer = true;
    model.header_footer.footer_text = value.into();
    Ok(())
}
