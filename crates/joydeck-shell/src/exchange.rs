//! Explicit new-copy exchange. Preserving and regenerating are separate user choices.
use crate::{Cancellation, editor::Editor, storage};
use serde_json::{Value, json};
use std::path::Path;

pub fn export(
    editor: &Editor,
    path: &Path,
    mode: &str,
    cancel: &Cancellation,
) -> Result<Value, String> {
    cancel.check()?;
    if path.exists() {
        return Err("exchange output must be a new path".into());
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["pptx", "potx", "ppsx"].contains(&ext.as_str()) {
        return Err("exchange output must be pptx/potx/ppsx".into());
    }
    let mut warnings = editor.display()?.info.warnings;
    let bytes = match mode {
        "preserve" => {
            if ext != "pptx" || editor.base.info.format != "pptx" {
                return Err("preserve requires a public PPTX source and .pptx destination".into());
            }
            let snapshot = editor.session.snapshot();
            let bindings = snapshot
                .origins
                .iter()
                .map(|(id, o)| joydeck_pptx::SlideBinding {
                    slide: *id,
                    source: o.slide,
                    shapes: o.shapes.clone(),
                })
                .collect::<Vec<_>>();
            joydeck_pptx::export_preserving(
                &editor.base.original,
                &editor.base.presentation,
                &snapshot.model,
                &bindings,
            )
            .map_err(|e| e.to_string())?
        }
        "regenerate" => {
            warnings.push("Explicit regeneration writes the supported public model; unknown source parts, relationships and unsupported semantics are not carried over. Keep the source or .joydeck archive.".into());
            joydeck_pptx::export_as(editor.session.model(), &ext).map_err(|e| e.to_string())?
        }
        _ => return Err("exchange mode must be preserve or regenerate".into()),
    };
    if bytes.len() > 64 << 20 {
        return Err("exchange output exceeds 64 MiB".into());
    }
    cancel.check()?;
    storage::save(path, &bytes, None)?;
    Ok(
        json!({"mode":mode,"format":ext,"bytes":bytes.len(),"sourceUnchanged":true,"warnings":warnings}),
    )
}
