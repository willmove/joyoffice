use crate::document::{self, Deck, Result};
pub use joydeck_model::interop::*;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
pub fn import(path: &Path) -> Result<Deck> {
    let cancel = AtomicBool::new(false);
    let parts = document::read_package(path, &cancel)?;
    if parts.contains_key("mimetype") {
        return crate::edit::open_editable(path);
    }
    validate_graph(&parts)?;
    if let Some(d) = crate::pptx_write::read_snapshot(&parts)? {
        return Ok(d);
    }
    let mut d = crate::pptx::import(&parts, &cancel)?;
    d.validate()?;
    d = crate::edit::Session::from_deck(d, false)
        .deck
        .as_ref()
        .clone();
    let mut bindings = vec![];
    // Match unique single-run text against original XML. Ambiguous/multi-run text stays preserved but not editable in-place.
    let a = "http://schemas.openxmlformats.org/drawingml/2006/main";
    for s in &d.slides {
        for o in &s.objects {
            if o.kind != "text" {
                continue;
            }
            let mut matches = vec![];
            for (name, bytes) in &parts {
                if !name.ends_with(".xml") {
                    continue;
                }
                let Ok(text) = std::str::from_utf8(bytes) else {
                    continue;
                };
                let Ok(doc) = xml(text) else {
                    continue;
                };
                if !doc.root_element().has_tag_name((
                    "http://schemas.openxmlformats.org/presentationml/2006/main",
                    "sld",
                )) {
                    continue;
                }
                for body in doc
                    .descendants()
                    .filter(|n| n.tag_name().name() == "txBody")
                {
                    let texts: Vec<_> = body
                        .descendants()
                        .filter(|n| n.has_tag_name((a, "t")))
                        .collect();
                    if texts.len() != 1
                        || body
                            .descendants()
                            .filter(|n| n.has_tag_name((a, "p")))
                            .count()
                            != 1
                        || texts[0].text().unwrap_or("") != o.text
                    {
                        continue;
                    }
                    if let Some(node) = texts[0].children().find(|n| n.is_text()) {
                        let range = node.range();
                        matches.push(TextBinding {
                            object: o.id.clone(),
                            part: name.clone(),
                            start: range.start,
                            end: range.end,
                        });
                    }
                }
            }
            if matches.len() == 1 {
                bindings.push(matches.remove(0));
            }
        }
    }
    let baseline = serde_json::to_vec(&d).map_err(|e| e.to_string())?;
    d.source = Some(Arc::new(SourcePackage {
        parts,
        baseline,
        text: bindings,
    }));
    d.validate()?;
    Ok(d)
}
