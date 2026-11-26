//! ECMA-376 diagram data model. The outline follows parOf connections in source order;
//! the presentation points contribute labels and picture bindings to their associated nodes.

use std::collections::{HashMap, HashSet};

use joydeck_model::DiagramNode;

use crate::xml::El;

pub(crate) const MAX_NODES: usize = 2_000;
const MAX_POINTS: usize = MAX_NODES * 8;
const MAX_CONNECTIONS: usize = MAX_NODES * 4;

#[derive(Default)]
pub(crate) struct Outline {
    pub(crate) nodes: Vec<DiagramNode>,
    pub(crate) pictures: Vec<(usize, String)>,
}

#[derive(Default)]
struct PresentationPoint<'a> {
    title: Option<&'a El>,
    picture: Option<&'a str>,
}

pub(crate) fn read(model: &El) -> Outline {
    let Some(points) = model.child("ptLst") else {
        return Outline::default();
    };
    let mut root = None;
    let mut nodes = HashMap::new();
    let mut presentation: HashMap<&str, PresentationPoint<'_>> = HashMap::new();
    for point in points.children_named("pt").take(MAX_POINTS) {
        match point.attr("type").unwrap_or("node") {
            "doc" if root.is_none() => root = point.attr("modelId"),
            "node" | "asst" => {
                if let Some(id) = point.attr("modelId") {
                    nodes.insert(id, point);
                }
            }
            "pres" => {
                let Some(properties) = point.child("prSet") else {
                    continue;
                };
                let Some(owner) = properties.attr("presAssocID") else {
                    continue;
                };
                let entry = presentation.entry(owner).or_default();
                if properties
                    .attr("presName")
                    .is_some_and(|label| label.starts_with("titleText"))
                {
                    entry.title = point.child("t").or(entry.title);
                }
                if entry.picture.is_none() {
                    entry.picture = point
                        .path(&["spPr", "blipFill", "blip"])
                        .and_then(|image| image.attr("r:embed"));
                }
            }
            _ => {}
        }
    }
    let Some(root) = root else {
        return Outline::default();
    };
    let mut children = HashMap::<&str, Vec<(i64, &str)>>::new();
    if let Some(connections) = model.child("cxnLst") {
        for edge in connections.children_named("cxn").take(MAX_CONNECTIONS) {
            if edge.attr("type").is_some_and(|kind| kind != "parOf") {
                continue;
            }
            if let (Some(from), Some(to)) = (edge.attr("srcId"), edge.attr("destId"))
                && (from == root || nodes.contains_key(from))
                && nodes.contains_key(to)
            {
                children
                    .entry(from)
                    .or_default()
                    .push((edge.i64("srcOrd").unwrap_or(0), to));
            }
        }
    }
    for siblings in children.values_mut() {
        siblings.sort_by_key(|&(order, _)| order);
    }
    let mut output = Outline::default();
    let mut visited = HashSet::from([root]);
    let mut pending = Vec::new();
    push_children(&mut pending, children.get(root), 0);
    while let Some((id, depth)) = pending.pop() {
        if output.nodes.len() == MAX_NODES {
            break;
        }
        if !visited.insert(id) {
            continue;
        }
        let Some(point) = nodes.get(id) else {
            continue;
        };
        let decorations = presentation.get(id);
        if let Some(picture) = decorations.and_then(|value| value.picture) {
            output
                .pictures
                .push((output.nodes.len(), picture.to_owned()));
        }
        output.nodes.push(DiagramNode {
            text: body_text(point.child("t")),
            level: depth,
            picture: None,
            assistant: point.attr("type") == Some("asst"),
            title: body_text(decorations.and_then(|value| value.title)),
        });
        push_children(&mut pending, children.get(id), (depth + 1).min(8));
    }
    output
}

fn push_children<'a>(
    pending: &mut Vec<(&'a str, u8)>,
    children: Option<&Vec<(i64, &'a str)>>,
    depth: u8,
) {
    if let Some(children) = children {
        pending.extend(children.iter().rev().map(|&(_, id)| (id, depth)));
    }
}

fn body_text(body: Option<&El>) -> String {
    body.into_iter()
        .flat_map(|node| node.children_named("p"))
        .map(|paragraph| {
            let mut spans = Vec::new();
            paragraph.find_all("t", &mut spans);
            spans.into_iter().map(El::text).collect::<String>()
        })
        .filter(|paragraph| !paragraph.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
