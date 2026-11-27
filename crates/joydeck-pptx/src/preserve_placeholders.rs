//! Preserve placeholder identity and implicit layout/master inheritance on edited slides.

use std::collections::HashSet;

use joydeck_model::{LayoutId, Presentation, Shape};

use super::{El, Node, PptxError, child_mut, error, opc};

const IDENTITY: &[&str] = &["type", "idx", "orient", "sz", "hasCustomPrompt"];

pub(super) fn master_paths(package: &opc::Package) -> Result<Vec<String>, PptxError> {
    let part = package.presentation_path()?;
    let doc = crate::read::load(package, &part).ok_or_else(|| error("presentation missing"))?;
    Ok(
        crate::presentation_parts::master_paths(&doc.root, &package.rels(&part))
            .into_iter()
            .filter(|path| crate::read::load(package, path).is_some())
            .collect(),
    )
}

pub(super) fn rebind(
    relationships: &mut El,
    part: &str,
    package: &opc::Package,
    base: &Presentation,
    id: LayoutId,
) -> Result<(), PptxError> {
    let mut seen = HashSet::new();
    for (master, path) in base.masters.iter().zip(master_paths(package)?) {
        let doc =
            crate::read::load(package, &path).ok_or_else(|| error("invalid source master"))?;
        let edges = package.rels(&path);
        let candidates = crate::master_part::layout_paths(&doc.root, &edges).filter(|path| {
            crate::read::load(package, path).is_some() && seen.insert(opc::norm(path))
        });
        if let Some((_, target)) = master
            .layouts
            .iter()
            .zip(candidates)
            .find(|(layout, _)| layout.id == id)
        {
            let mut matching = relationships.elements_mut().filter(|edge| {
                edge.is("Relationship")
                    && edge
                        .attr("Type")
                        .is_some_and(|value| value.rsplit('/').next() == Some("slideLayout"))
                    && edge.attr("TargetMode") != Some("External")
            });
            let edge = matching
                .next()
                .ok_or_else(|| error("layout relationship missing"))?;
            if matching.next().is_some() {
                return Err(error("ambiguous slide layout relationships"));
            }
            edge.set_attr("Target", &opc::relative(part, target));
            return Ok(());
        }
    }
    Err(error("layout has no source part; save .joydeck instead"))
}

fn nonvisual(shape: &El) -> Option<&El> {
    shape
        .elements()
        .find(|node| node.local().starts_with("nv"))?
        .child("nvPr")
}

pub(super) fn edit(
    shape: &mut El,
    generated: &El,
    previous: &Shape,
    current: &Shape,
) -> Result<(), PptxError> {
    if previous.ph != current.ph {
        let target = shape
            .elements_mut()
            .find(|node| node.local().starts_with("nv"))
            .and_then(|node| child_mut(node, "nvPr"))
            .ok_or_else(|| error("placeholder nonvisual properties missing"))?;
        let generated = nonvisual(generated).and_then(|node| node.child("ph"));
        let index = target
            .children
            .iter()
            .position(|node| matches!(node, Node::El(e) if e.is("ph")));
        match (index, generated) {
            (Some(index), Some(generated)) => {
                let Some(Node::El(existing)) = target.children.get_mut(index) else {
                    return Err(error("placeholder element missing"));
                };
                existing
                    .attrs
                    .retain(|(key, _)| !IDENTITY.contains(&key.as_str()));
                existing.attrs.extend(
                    generated
                        .attrs
                        .iter()
                        .filter(|(key, _)| IDENTITY.contains(&key.as_str()))
                        .cloned(),
                );
            }
            (None, Some(generated)) => target.children.insert(0, Node::El(generated.clone())),
            (Some(index), None) => {
                let Some(Node::El(existing)) = target.children.get(index) else {
                    return Err(error("placeholder element missing"));
                };
                if !plain(existing, IDENTITY) {
                    return Err(error(
                        "placeholder has unrecognized properties; save .joydeck instead",
                    ));
                }
                target.children.remove(index);
            }
            (None, None) => {}
        }
    }
    if current.ph.is_some() && previous.xfrm.is_some() && current.xfrm.is_none() {
        remove_override(shape)?;
    }
    Ok(())
}

fn plain(element: &El, names: &[&str]) -> bool {
    element
        .attrs
        .iter()
        .all(|(key, _)| names.contains(&key.as_str()))
        && element
            .children
            .iter()
            .all(|node| matches!(node, Node::Text(text) if text.trim().is_empty()))
}

fn remove_override(shape: &mut El) -> Result<(), PptxError> {
    let props = child_mut(shape, "spPr").ok_or_else(|| error("placeholder properties missing"))?;
    let index = props
        .children
        .iter()
        .position(|node| matches!(node, Node::El(element) if element.is("xfrm")));
    let Some(index) = index else { return Ok(()) };
    let Some(Node::El(transform)) = props.children.get(index) else {
        return Err(error("placeholder transform missing"));
    };
    let known = transform
        .attrs
        .iter()
        .all(|(key, _)| matches!(key.as_str(), "rot" | "flipH" | "flipV"))
        && transform.children.iter().all(|child| match child {
            Node::Text(text) => text.trim().is_empty(),
            Node::El(element) if element.prefix() == transform.prefix() => match element.local() {
                "off" => plain(element, &["x", "y"]),
                "ext" => plain(element, &["cx", "cy"]),
                _ => false,
            },
            _ => false,
        });
    if !known {
        return Err(error(
            "placeholder transform contains unrecognized XML; save .joydeck instead",
        ));
    }
    props.children.remove(index);
    Ok(())
}
