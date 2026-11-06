//! Patch field-bearing or linked paragraphs, retaining original XML for unchanged runs.
use super::*;
use joydeck_model::text::{Action, Paragraph, Run, RunKind, RunProps};
use std::collections::{BTreeSet, VecDeque};

pub(super) struct Ids<'a> {
    part: &'a str,
    used: BTreeSet<String>,
    sequence: usize,
}

impl<'a> Ids<'a> {
    pub(super) fn new(root: &El, part: &'a str) -> Self {
        let mut fields = vec![];
        root.find_all("fld", &mut fields);
        Self {
            part,
            used: fields
                .into_iter()
                .filter_map(|e| e.attr("id"))
                .map(str::to_ascii_uppercase)
                .collect(),
            sequence: 0,
        }
    }

    fn assign(&mut self, element: &mut El) {
        loop {
            self.sequence += 1;
            let id =
                crate::tables::guid(&format!("{}:preserved-field:{}", self.part, self.sequence));
            if self.used.insert(id.clone()) {
                element.set_attr("id", &id);
                return;
            }
        }
    }

    pub(super) fn reassign_tree(&mut self, element: &mut El) {
        if element.prefix() == "a" && element.is("fld") {
            self.assign(element);
        }
        if !element.is("extLst") {
            for child in element.elements_mut() {
                self.reassign_tree(child);
            }
        }
    }
}

fn has_field_or_link(body: &TextBody) -> bool {
    body.paragraphs.iter().flat_map(|p| &p.runs).any(|r| {
        matches!(r.kind, RunKind::Field { .. }) || r.props.link.is_some() || r.props.hover.is_some()
    })
}

fn field_key(run: &Run) -> Option<(&str, &str)> {
    if let RunKind::Field { field } = &run.kind {
        Some((field.as_str(), run.text.as_str()))
    } else {
        None
    }
}

fn raw_runs(para: &El) -> Vec<(usize, &El)> {
    para.children
        .iter()
        .enumerate()
        .filter_map(|(i, node)| match node {
            Node::El(e)
                if (e.prefix() == "a" || e.prefix() == para.prefix())
                    && matches!(e.local(), "r" | "br" | "fld") =>
            {
                Some((i, e))
            }
            _ => None,
        })
        .collect()
}

fn agrees(nodes: &[(usize, &El)], runs: &[Run]) -> bool {
    nodes.len() == runs.len()
        && nodes.iter().zip(runs).all(|((_, node), run)| {
            let text = || node.child("t").map(El::text).unwrap_or_default();
            match &run.kind {
                RunKind::Text => node.is("r") && text() == run.text,
                RunKind::Break => node.is("br"),
                RunKind::Field { field } => {
                    node.is("fld")
                        && node.attr("type").unwrap_or_default() == field
                        && text() == run.text
                }
                RunKind::Math { .. } => false,
            }
        })
}

fn attrs_in(element: &El, allowed: &[&str]) -> bool {
    element
        .attrs
        .iter()
        .all(|(key, _)| allowed.contains(&key.as_str()))
}

fn simple_props(props: &El, model: &RunProps) -> bool {
    attrs_in(
        props,
        &[
            "lang", "sz", "b", "i", "u", "strike", "baseline", "cap", "spc", "kern", "noProof",
            "dirty", "smtClean",
        ],
    ) && props.elements().all(|style| {
        if style.prefix() != props.prefix() {
            return false;
        }
        match style.local() {
            "latin" | "ea" | "cs" | "sym" => {
                attrs_in(style, &["typeface"]) && style.elements().next().is_none()
            }
            "solidFill" => {
                style.attrs.is_empty()
                    && style.elements().count() == 1
                    && style.elements().all(|color| {
                        color.prefix() == style.prefix()
                            && matches!(color.local(), "srgbClr" | "schemeClr")
                            && attrs_in(color, &["val"])
                            && color.elements().next().is_none()
                    })
            }
            "hlinkClick" | "hlinkMouseOver" => {
                let link = if style.is("hlinkClick") {
                    &model.link
                } else {
                    &model.hover
                };
                link.as_ref().is_some_and(|link| {
                    matches!(link.action, Action::Url { .. } | Action::Slide { .. })
                        && link.sound.is_none()
                        && link.zoom_return.is_none()
                }) && attrs_in(
                    style,
                    &["r:id", "action", "tooltip", "highlightClick", "endSnd"],
                ) && style.elements().next().is_none()
            }
            _ => false,
        }
    })
}

fn simple_run(run: &El, model: &Run) -> bool {
    // Reuse opaque runs when unchanged. Rewriting a changed run is allowed only
    // when the model can carry all of its formatting.
    attrs_in(run, if run.is("fld") { &["id", "type"] } else { &[] })
        && run.elements().all(|child| {
            child.prefix() == run.prefix()
                && match child.local() {
                    "t" => attrs_in(child, &["xml:space"]) && child.elements().next().is_none(),
                    "rPr" => simple_props(child, &model.props),
                    _ => false,
                }
        })
}

fn patch_para(
    raw: &mut El,
    generated: &El,
    old: &Paragraph,
    new: &Paragraph,
    ids: &mut Ids<'_>,
    relationships: &mut links::Relationships<'_>,
) -> Result<(), PptxError> {
    if old.level != new.level || old.props != new.props || old.def_props != new.def_props {
        return Err(error(
            "combined field and paragraph formatting edits require .joydeck Save As",
        ));
    }
    let end_props = if old.end_props != new.end_props {
        if raw
            .child("endParaRPr")
            .is_some_and(|p| !simple_props(p, &old.end_props))
        {
            return Err(error(
                "field edit would rewrite opaque end formatting; save .joydeck instead",
            ));
        }
        let mut end = generated.child("endParaRPr").cloned();
        if let Some(end) = &mut end {
            relationships.rebind_generated(end)?;
        }
        Some(end)
    } else {
        None
    };
    let existing = raw_runs(raw);
    let generated = raw_runs(generated);
    if !agrees(&existing, &old.runs) || !agrees(&generated, &new.runs) {
        return Err(error(
            "cannot map the original field paragraph; save .joydeck instead",
        ));
    }
    let prefix = old
        .runs
        .iter()
        .zip(&new.runs)
        .take_while(|(a, b)| a == b)
        .count();
    let tail = old.runs[prefix..]
        .iter()
        .rev()
        .zip(new.runs[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_end = old.runs.len() - tail;
    let new_end = new.runs.len() - tail;
    let mut kept = BTreeSet::new();
    let mut fields: BTreeMap<_, VecDeque<_>> = BTreeMap::new();
    for (i, run) in old.runs.iter().enumerate().take(old_end).skip(prefix) {
        if let Some(key) = field_key(run) {
            fields.entry(key).or_default().push_back((i, run));
        }
    }
    let mut replacement = Vec::new();
    for (i, run) in new.runs.iter().enumerate().take(new_end).skip(prefix) {
        let reuse = field_key(run)
            .and_then(|key| fields.get_mut(&key))
            .and_then(|candidates| {
                let found = candidates.iter().position(|(_, prev)| *prev == run)?;
                candidates.remove(found).map(|(i, _)| i)
            });
        if let Some(i) = reuse {
            kept.insert(i);
            replacement.push(Node::El(existing[i].1.clone()));
        } else {
            let mut element = generated[i].1.clone();
            relationships.rebind_generated(&mut element)?;
            if element.is("fld") {
                ids.assign(&mut element);
            }
            replacement.push(Node::El(element));
        }
    }
    if existing
        .iter()
        .enumerate()
        .take(old_end)
        .skip(prefix)
        .any(|(i, (_, node))| !kept.contains(&i) && !simple_run(node, &old.runs[i]))
    {
        return Err(error(
            "field edit would rewrite opaque text formatting; save .joydeck instead",
        ));
    }
    let removed: BTreeSet<_> = existing[prefix..old_end].iter().map(|(i, _)| *i).collect();
    let at = existing
        .get(prefix)
        .map(|(i, _)| *i)
        .or_else(|| existing.last().map(|(i, _)| i + 1))
        .or_else(|| {
            raw.children
                .iter()
                .position(|node| matches!(node, Node::El(e) if e.is("endParaRPr")))
        })
        .unwrap_or(raw.children.len());
    let mut result = Vec::new();
    for (i, node) in std::mem::take(&mut raw.children).into_iter().enumerate() {
        if i == at {
            result.append(&mut replacement);
        }
        if !removed.contains(&i) {
            result.push(node);
        }
    }
    result.append(&mut replacement);
    raw.children = result;
    if let Some(end_props) = end_props {
        replace(raw, "endParaRPr", end_props);
    }
    Ok(())
}

pub(super) fn patch(
    source: &mut El,
    generated: &El,
    old: &TextBody,
    new: &TextBody,
    ids: &mut Ids<'_>,
    relationships: &mut links::Relationships<'_>,
) -> Result<bool, PptxError> {
    if !has_field_or_link(old) && !has_field_or_link(new) {
        return Ok(false);
    }
    if old.list_style != new.list_style || old.paragraphs.len() != new.paragraphs.len() {
        return Err(error(
            "changing field paragraph structure requires .joydeck Save As",
        ));
    }
    let Some(raw) = child_mut(source, "txBody") else {
        return Err(error("missing original field text body"));
    };
    let updated = generated
        .child("txBody")
        .ok_or_else(|| error("missing generated field text body"))?;
    let source_paras: Vec<_> = raw.elements_mut().filter(|e| e.is("p")).collect();
    let generated_paras: Vec<_> = updated.elements().filter(|e| e.is("p")).collect();
    if source_paras.len() != old.paragraphs.len() || generated_paras.len() != new.paragraphs.len() {
        return Err(error(
            "cannot map the original field paragraphs; save .joydeck instead",
        ));
    }
    for ((raw, generated), (old, new)) in source_paras
        .into_iter()
        .zip(generated_paras)
        .zip(old.paragraphs.iter().zip(&new.paragraphs))
    {
        if old != new {
            patch_para(raw, generated, old, new, ids, relationships)?;
        }
    }
    Ok(true)
}
