//! `w:hyperlink` (ECMA-376 Part 1 §17.16.22).
//!
//! A hyperlink whose only attributes are `r:id` and/or `w:anchor`, and whose
//! children are runs of text and tabs, becomes a link on those runs. The
//! relationship target is stored and never requested. Anything else — a
//! tooltip, a frame, a nested field, a break — stays preserved and locks the
//! paragraph, which is the previous wrapper behaviour.

use std::sync::Arc;

use joyoffice_opc::{PartName, Target};
use joyword_model::Hyperlink;

use crate::import::Importer;
use crate::ns::{R, REL_HYPERLINK, W};
use crate::xml::Element;

/// The link to stamp on runs inside `element`, when the element is in the
/// subset this slice maps. `None` means the caller preserves the wrapper.
pub(crate) fn mapped(
    importer: &Importer<'_>,
    part: &PartName,
    element: &Element,
) -> Option<Hyperlink> {
    if !element.is(W, "hyperlink") || !simple_runs(element) {
        return None;
    }
    let mut relationship = None;
    let mut anchor = None;
    for attribute in &element.attributes {
        if attribute.namespace.as_deref() == Some(R) && attribute.local.as_ref() == "id" {
            if relationship.is_some() {
                return None;
            }
            relationship = Some(attribute.value.as_str());
        } else if attribute.namespace.as_deref() == Some(W) && attribute.local.as_ref() == "anchor"
        {
            if anchor.is_some() {
                return None;
            }
            anchor = Some(attribute.value.as_str());
        } else {
            return None;
        }
    }
    let external = match relationship {
        Some(id) => Some(external_target(importer, part, id)?),
        None => None,
    };
    let anchor = match anchor {
        Some(value) if anchor_ok(value) => Some(Arc::<str>::from(value)),
        Some(_) => return None,
        None => None,
    };
    Hyperlink::imported(relationship.map(Arc::from), external, anchor)
}

fn external_target(importer: &Importer<'_>, part: &PartName, id: &str) -> Option<Arc<str>> {
    let relationship = importer
        .backing
        .package()
        .relationships(Some(part))
        .iter()
        .find(|item| item.id.as_ref() == id)?;
    if relationship.kind.as_ref() != REL_HYPERLINK {
        return None;
    }
    match &relationship.target {
        Target::External { raw, .. } if !raw.is_empty() => Some(Arc::clone(raw)),
        _ => None,
    }
}

fn anchor_ok(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 255
        && !value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '\''))
}

/// Runs of text and tabs only. A break, drawing or other child stays preserved.
fn simple_runs(element: &Element) -> bool {
    let mut runs = 0usize;
    for child in element.elements() {
        if !child.is(W, "r") {
            return false;
        }
        runs += 1;
        for inner in child.elements() {
            match &*inner.local {
                "rPr" if inner.namespace.as_deref() == Some(W) => {}
                "t" if inner.namespace.as_deref() == Some(W)
                    && inner.elements().next().is_none() =>
                {
                    if inner.text().contains(joyword_model::OBJECT_MARKER) {
                        return false;
                    }
                }
                "tab"
                    if inner.namespace.as_deref() == Some(W)
                        && inner.attributes.is_empty()
                        && inner.elements().next().is_none() => {}
                _ => return false,
            }
        }
    }
    runs > 0
}
