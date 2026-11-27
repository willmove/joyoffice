//! Targeted R5 XML updates. Unknown surrounding XML and untouched parts are retained.
use super::*;
use joydeck_model::{Shape, ShapeKind};
use std::{
    collections::HashMap,
    hash::{BuildHasher, RandomState},
};

pub(super) fn globals(
    pkg: &opc::Package,
    generated: &opc::Package,
    base: &Presentation,
    current: &Presentation,
    parts: &mut PartEdits<'_>,
    pres: &mut El,
) -> Result<(), PptxError> {
    if base.slide_size != current.slide_size {
        let new = generated
            .xml("ppt/presentation.xml")
            .and_then(|d| d.root.child("sldSz").cloned())
            .ok_or_else(|| error("generated slide size missing"))?;
        replace(pres, "sldSz", Some(new));
    }
    if base.masters.len() != current.masters.len() {
        return Err(error("adding/removing masters requires .joydeck Save As"));
    }
    // Minimal public PPTX files may have no master parts. Do not require them when unchanged.
    let masters = if base.masters != current.masters {
        placeholders::master_paths(pkg)?
    } else {
        vec![]
    };
    for (index, (old, new)) in base.masters.iter().zip(&current.masters).enumerate() {
        if old == new {
            continue;
        }
        let part = masters
            .get(index)
            .ok_or_else(|| error("master binding missing"))?;
        if old.background != new.background {
            let mut root = pkg
                .xml(part)
                .ok_or_else(|| error("master XML missing"))?
                .root;
            namespaces(&mut root)?;
            let generated_root = generated
                .xml(&format!("ppt/slideMasters/slideMaster{}.xml", index + 1))
                .ok_or_else(|| error("generated master missing"))?
                .root;
            let c = child_mut(&mut root, "cSld").ok_or_else(|| error("master common missing"))?;
            replace(
                c,
                "bg",
                generated_root
                    .child("cSld")
                    .and_then(|c| c.child("bg"))
                    .cloned(),
            );
            if let Some(at) = c
                .children
                .iter()
                .position(|n| matches!(n,Node::El(e) if e.is("bg")))
            {
                let bg = c.children.remove(at);
                c.children.insert(0, bg);
            }
            parts.insert(part.clone(), root.to_xml().into_bytes());
        }
        if old.theme != new.theme {
            let theme = pkg
                .rels(part)
                .items
                .into_iter()
                .find(|r| r.kind == "theme")
                .ok_or_else(|| error("theme relationship missing"))?
                .target;
            // A shared theme part cannot be changed for just one master without rebinding it.
            for (other, m) in masters.iter().enumerate() {
                if other != index
                    && pkg
                        .rels(m)
                        .items
                        .iter()
                        .any(|r| r.kind == "theme" && r.target == theme)
                    && current.masters[other].theme != new.theme
                {
                    return Err(error("shared theme needs .joydeck Save As"));
                }
            }
            let mut root = pkg
                .xml(&theme)
                .ok_or_else(|| error("theme XML missing"))?
                .root;
            let gen_root = generated
                .xml(&format!("ppt/theme/theme{}.xml", index + 1))
                .ok_or_else(|| error("generated theme missing"))?
                .root;
            if old.theme.name != new.theme.name {
                root.set_attr("name", &new.theme.name);
            }
            let target = child_mut(&mut root, "themeElements")
                .ok_or_else(|| error("theme elements missing"))?;
            let source = gen_root
                .child("themeElements")
                .ok_or_else(|| error("generated theme elements missing"))?;
            if old.theme.colors != new.theme.colors {
                let original = child_mut(target, "clrScheme")
                    .ok_or_else(|| error("original color scheme missing"))?;
                if old.theme.colors.name != new.theme.colors.name {
                    original.set_attr("name", &new.theme.colors.name);
                }
                let generated = source
                    .child("clrScheme")
                    .ok_or_else(|| error("generated color scheme missing"))?;
                for slot in [
                    "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4",
                    "accent5", "accent6", "hlink", "folHlink",
                ] {
                    if let Some(color_slot) = joydeck_model::SchemeSlot::from_xml(slot)
                        && old.theme.colors.get(color_slot) != new.theme.colors.get(color_slot)
                    {
                        replace(original, slot, generated.child(slot).cloned());
                    }
                }
            }
            if old.theme.fonts != new.theme.fonts {
                let original = child_mut(target, "fontScheme")
                    .ok_or_else(|| error("original font scheme missing"))?;
                if old.theme.fonts.name != new.theme.fonts.name {
                    original.set_attr("name", &new.theme.fonts.name);
                }
                let generated = source
                    .child("fontScheme")
                    .ok_or_else(|| error("generated font scheme missing"))?;
                for (name, changed) in [
                    (
                        "majorFont",
                        old.theme.fonts.major.latin != new.theme.fonts.major.latin,
                    ),
                    (
                        "minorFont",
                        old.theme.fonts.minor.latin != new.theme.fonts.minor.latin,
                    ),
                ] {
                    if changed {
                        let target = child_mut(original, name)
                            .ok_or_else(|| error("original font set missing"))?;
                        replace(
                            target,
                            "latin",
                            generated
                                .child(name)
                                .and_then(|s| s.child("latin"))
                                .cloned(),
                        );
                    }
                }
            }
            parts.insert(theme, root.to_xml().into_bytes());
        }
    }
    if base.props != current.props {
        let part = pkg
            .rels("")
            .items
            .into_iter()
            .find(|r| r.ty == opc::RT_CORE)
            .map(|r| r.target)
            .unwrap_or_else(|| "docProps/core.xml".into());
        let mut root = pkg
            .xml(&part)
            .ok_or_else(|| error("core properties missing"))?
            .root;
        let gen_root = generated
            .xml("docProps/core.xml")
            .ok_or_else(|| error("generated properties missing"))?
            .root;
        for local in ["title", "creator"] {
            replace(&mut root, local, gen_root.child(local).cloned());
        }
        parts.insert(part, root.to_xml().into_bytes());
    }
    Ok(())
}
pub(super) fn new_name(parts: &PartEdits<'_>, source: &str) -> String {
    let (folder, file) = source.rsplit_once('/').unwrap_or(("ppt/media", source));
    let mut n = 1;
    loop {
        let path = format!("{folder}/joydeck-{n}-{file}");
        if !parts.contains_key(&path) {
            return path;
        }
        n += 1;
    }
}
fn import_part(
    pkg: &opc::Package,
    source: &str,
    parts: &mut PartEdits<'_>,
    ct: &mut El,
    imported: &mut BTreeMap<String, String>,
    depth: usize,
) -> Result<String, PptxError> {
    if let Some(part) = imported.get(source) {
        return Ok(part.clone());
    }
    if depth > 16 || imported.len() >= 256 {
        return Err(error("generated dependency graph budget exceeded"));
    }
    let bytes = pkg
        .get(source)
        .ok_or_else(|| error("generated dependency missing"))?;
    if bytes.len() > 32 << 20 {
        return Err(error("generated part exceeds 32 MiB"));
    }
    let target = new_name(parts, source);
    imported.insert(source.into(), target.clone());
    parts.insert(target.clone(), bytes.to_vec());
    if let Some(kind) = pkg.content_type(source) {
        add_override(ct, &target, &kind);
    }
    let rels = pkg.rels(source);
    if !rels.items.is_empty() {
        let mut root = pkg
            .xml(&opc::rels_path(source))
            .ok_or_else(|| error("generated dependency relationships missing"))?
            .root;
        for rel in rels.items {
            if rel.external {
                return Err(error("edited external resource requires .joydeck Save As"));
            }
            let destination = import_part(pkg, &rel.target, parts, ct, imported, depth + 1)?;
            let node = root
                .elements_mut()
                .find(|e| e.attr("Id") == Some(&rel.id))
                .ok_or_else(|| error("generated relationship missing"))?;
            set(
                node,
                "Target",
                crate::relationships::rebase(&target, &destination, &rel.target_raw),
            );
        }
        parts.insert(opc::rels_path(&target), root.to_xml().into_bytes());
    }
    Ok(target)
}

fn shareable_media(mime: &str) -> bool {
    ["image/", "audio/", "video/"]
        .iter()
        .any(|prefix| mime.starts_with(prefix))
}

pub(super) struct Pictures<'a> {
    source: &'a opc::Package,
    fingerprint: RandomState,
    parts: Option<HashMap<(String, u64), Vec<String>>>,
}

impl<'a> Pictures<'a> {
    pub(super) fn new(source: &'a opc::Package) -> Self {
        Self {
            source,
            fingerprint: RandomState::new(),
            parts: None,
        }
    }

    pub(super) fn target(
        &mut self,
        generated: &opc::Package,
        source: &str,
        parts: &mut PartEdits<'_>,
        ct: &mut El,
    ) -> Result<String, PptxError> {
        let Some((kind, bytes)) = generated.content_type(source).zip(generated.get(source)) else {
            return Err(error(
                "generated media part is missing data or a content type",
            ));
        };
        if !shareable_media(&kind)
            || bytes.len() > 32 << 20
            || generated.get(&opc::rels_path(source)).is_some()
        {
            return import_part(generated, source, parts, ct, &mut BTreeMap::new(), 0);
        }
        // Index once per save, on the first generated resource. Only independent,
        // byte-identical media of the same declared type can share a part.
        let index = self.parts.get_or_insert_with(|| {
            let mut index = HashMap::<_, Vec<_>>::new();
            for (path, data) in parts.iter() {
                if data.len() <= 32 << 20
                    && self.source.original_name(path) == Some(path)
                    && let Some(kind) = self.source.content_type(path)
                    && shareable_media(&kind)
                    && self.source.get(&opc::rels_path(path)).is_none()
                {
                    let key = (kind, self.fingerprint.hash_one(data));
                    index.entry(key).or_default().push(path.to_owned());
                }
            }
            index
        });
        let candidates = index
            .entry((kind, self.fingerprint.hash_one(bytes)))
            .or_default();
        if let Some(path) = candidates
            .iter()
            .find(|path| parts.get(path).is_some_and(|data| data == bytes))
        {
            return Ok(path.clone());
        }
        let target = import_part(generated, source, parts, ct, &mut BTreeMap::new(), 0)?;
        candidates.push(target.clone());
        Ok(target)
    }
}

/// A leader drag replaces two literal adjustments. Keep the source preset,
/// including non-adjust guides, extension elements and qualified attributes.
fn update_callout(
    props: &mut El,
    old: &joydeck_model::Geom,
    new: &joydeck_model::Geom,
) -> Result<bool, PptxError> {
    use joydeck_model::Geom;
    let (
        Geom::Preset { name, adj: before },
        Geom::Preset {
            name: updated,
            adj: after,
        },
    ) = (old, new)
    else {
        return Ok(false);
    };
    if name != updated || joydeck_geom::preset::LineCallout::named(name).is_none() {
        return Ok(false);
    }
    let geom = child_mut(props, "prstGeom")
        .filter(|g| g.attr("prst") == Some(name.as_str()))
        .ok_or_else(|| error("source callout geometry is missing"))?;
    let prefix = geom.prefix();
    let qualified = |local: &str| {
        if prefix.is_empty() {
            local.to_owned()
        } else {
            format!("{prefix}:{local}")
        }
    };
    let list_name = qualified("avLst");
    if geom.child("avLst").is_none() {
        geom.children.insert(
            0,
            Node::El(El {
                name: list_name,
                ..El::default()
            }),
        );
    }
    let list = child_mut(geom, "avLst").ok_or_else(|| error("callout guides missing"))?;
    let prefix = list.prefix().to_owned();
    let guide_name = if prefix.is_empty() {
        "gd".to_owned()
    } else {
        format!("{prefix}:gd")
    };
    let defaults = joydeck_geom::preset::adjustment_defaults(name);
    for (i, &default) in defaults.iter().enumerate() {
        let from = before.get(i).copied().unwrap_or(default);
        let to = after.get(i).copied().unwrap_or(default);
        if from == to {
            continue;
        }
        let key = format!("adj{}", i + 1);
        let mut positions = list
            .children
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let Node::El(g) = node else { return None };
                (g.name == guide_name
                    && g.attr("name")
                        .is_some_and(|n| n == key || (i == 0 && n == "adj")))
                .then_some(index)
            });
        let position = positions.next();
        if positions.next().is_some() {
            return Err(error(
                "multiple guides set the same callout vertex; save .joydeck to preserve them",
            ));
        }
        let literal = format!("val {to:.0}");
        if let Some(Node::El(g)) = position.and_then(|i| list.children.get_mut(i)) {
            let value = g
                .attr("fmla")
                .and_then(|f| f.trim().strip_prefix("val "))
                .and_then(|v| v.trim().parse::<f64>().ok());
            if value != Some(from) {
                return Err(error(
                    "callout vertex has a computed guide; save .joydeck to preserve it",
                ));
            }
            g.set_attr("fmla", &literal);
        } else {
            list.children.insert(
                0,
                Node::El(El {
                    name: guide_name.clone(),
                    attrs: vec![("name".into(), key), ("fmla".into(), literal)],
                    children: vec![],
                }),
            );
        }
    }
    // The existing reader inspects a bounded number of guide entries. A crowded
    // list must never produce a successful save that reopens with different tips.
    let Geom::Preset { adj: actual, .. } = crate::read::preset_geom(geom) else {
        return Err(error("unreadable callout geometry"));
    };
    if actual != *after && !(actual.is_empty() && after.as_slice() == defaults) {
        return Err(error(
            "callout adjustments exceed the PPTX reader's guide budget; save .joydeck to preserve them",
        ));
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn shape(
    target: &mut El,
    generated: &El,
    old: Option<&Shape>,
    new: &Shape,
    pkg: &opc::Package,
    gen_slide: &str,
    slide: &str,
    parts: &mut PartEdits<'_>,
    ct: &mut El,
    rels: &mut Vec<El>,
    assets: &mut assets::Assets<'_>,
    preserved_chart: bool,
) -> Result<(), PptxError> {
    if let Some(old) = old
        && (new.fill != old.fill || new.line != old.line || new.geom != old.geom)
    {
        let source = generated
            .child("spPr")
            .ok_or_else(|| error("generated shape properties missing"))?;
        let props =
            child_mut(target, "spPr").ok_or_else(|| error("source shape properties missing"))?;
        if new.fill != old.fill {
            props.children.retain(|n|!matches!(n,Node::El(e) if matches!(e.local(),"solidFill"|"gradFill"|"pattFill"|"noFill"|"blipFill"|"grpFill")));
            if let Some(fill) = source.elements().find(|e| {
                matches!(
                    e.local(),
                    "solidFill" | "gradFill" | "pattFill" | "noFill" | "blipFill" | "grpFill"
                )
            }) {
                props.children.push(Node::El(fill.clone()));
            }
        }
        if new.line != old.line {
            replace(props, "ln", source.child("ln").cloned());
        }
        if new.geom != old.geom && !update_callout(props, &old.geom, &new.geom)? {
            props
                .children
                .retain(|n| !matches!(n,Node::El(e) if matches!(e.local(),"prstGeom"|"custGeom")));
            if let Some(g) = source
                .elements()
                .find(|e| matches!(e.local(), "prstGeom" | "custGeom"))
            {
                props.children.push(Node::El(g.clone()));
            }
        }
        // Keep schema ordering: xfrm, geometry, fill, line, effects, extension.
        props.children.sort_by_key(|n| match n {
            Node::El(e) => match e.local() {
                "xfrm" => 0,
                "prstGeom" | "custGeom" => 1,
                "noFill" | "solidFill" | "gradFill" | "pattFill" | "blipFill" | "grpFill" => 2,
                "ln" => 3,
                "effectLst" | "effectDag" => 4,
                _ => 5,
            },
            _ => 5,
        });
    }
    let changed = old.is_none_or(|old| old.kind != new.kind);
    if !changed || preserved_chart {
        return Ok(());
    }
    if let Some(Shape {
        kind: ShapeKind::Chart(c),
        ..
    }) = old
        && (c.raw.as_deref().is_some_and(|raw| raw.contains("extLst"))
            || c.kept
                .rels
                .iter()
                .any(|r| r.external || !r.rel_type.ends_with("/package")))
    {
        return Err(error(
            "edited chart has extensions/styles/external dependencies; save .joydeck to preserve them",
        ));
    }
    let resource = match new.kind {
        ShapeKind::Picture { .. } if old.is_some() => "blipFill",
        ShapeKind::Chart(_) => "graphic",
        _ => return Ok(()),
    };
    let mut replacement = generated
        .child(resource)
        .cloned()
        .ok_or_else(|| error("generated content missing"))?;
    let local = if resource == "blipFill" {
        "blip"
    } else {
        "chart"
    };
    let node = find_mut(&mut replacement, local)
        .ok_or_else(|| error("generated content relationship missing"))?;
    let attr = if local == "blip" { "r:embed" } else { "r:id" };
    let rid = node
        .attr(attr)
        .ok_or_else(|| error("generated resource ID missing"))?;
    let relationship = pkg
        .rels(gen_slide)
        .items
        .into_iter()
        .find(|r| r.id == rid)
        .ok_or_else(|| error("generated relationship missing"))?;
    if relationship.external {
        return Err(error("edited linked resources need .joydeck Save As"));
    }
    let destination = if local == "blip" && relationship.kind == "image" {
        assets.picture(pkg, &relationship.target, parts, ct)?
    } else {
        import_part(
            pkg,
            &relationship.target,
            parts,
            ct,
            &mut BTreeMap::new(),
            0,
        )?
    };
    let mut id = format!("rIdJoyDeckContent{}", new.id.0);
    while rels.iter().any(|r| r.attr("Id") == Some(&id))
        || parts
            .get(&opc::rels_path(slide))
            .and_then(|b| crate::xml::parse(b).ok())
            .is_some_and(|d| d.root.elements().any(|r| r.attr("Id") == Some(&id)))
    {
        id.push('x');
    }
    set(node, attr, id.clone());
    if resource == "graphic"
        && old.is_some()
        && let Some(data) =
            child_mut(target, "graphic").and_then(|graphic| child_mut(graphic, "graphicData"))
        && data.attr("uri") == Some(opc::NS_C)
        && let Some(chart) = child_mut(data, "chart")
    {
        if let Some((_, previous_id)) = chart.attrs.iter_mut().find(|(key, _)| key.ends_with(":id"))
        {
            *previous_id = id.clone();
        } else {
            return Err(error("original chart relationship missing"));
        }
    } else {
        replace(target, resource, Some(replacement));
    }
    rels.push(rel_element(
        &id,
        &relationship.kind,
        &opc::relative(slide, &destination),
    ));
    Ok(())
}

#[cfg(test)]
mod callout_tests {
    use super::*;
    use joydeck_model::Geom;

    #[test]
    fn preserving_literal_updates_respects_foreign_guides_and_reader_bounds()
    -> Result<(), Box<dyn std::error::Error>> {
        let previous = Geom::preset("callout1");
        let mut next = previous.clone();
        assert!(next.set_callout_vertex(0, -35000, 63000));
        let source = |guides: &str| {
            crate::xml::parse(format!(
            r#"<p:spPr xmlns:p="p" xmlns:q="a" xmlns:ext="extra"><q:prstGeom prst="callout1"><q:avLst ext:attr="keep">{guides}</q:avLst></q:prstGeom></p:spPr>"#
        ).as_bytes()).map(|d|d.root)
        };
        let foreign = r#"<ext:gd name="userGuide" fmla="val 848"/><ext:item keep="yes"/>"#;
        let mut doc = source(foreign)?;
        assert!(update_callout(&mut doc, &previous, &next)?);
        let list = doc
            .child("prstGeom")
            .and_then(|g| g.child("avLst"))
            .ok_or("avLst missing")?;
        assert!(
            list.elements()
                .any(|e| e.name == "ext:gd" && e.attr("fmla") == Some("val 848"))
        );
        assert!(
            list.elements()
                .any(|e| e.name == "ext:item" && e.attr("keep") == Some("yes"))
        );
        for guides in [
            r#"<q:gd name="adj1" fmla="*/ h 1 2"/>"#.to_owned(),
            r#"<ext:gd name="adj1" fmla="val 848"/>"#.to_owned(),
            format!(
                "{}{}",
                r#"<q:gd name="extraGuide" fmla="val 0"/>"#.repeat(16),
                r#"<q:gd name="adj1" fmla="val 18750"/>"#
            ),
            r#"<q:gd name="adj1" fmla="val 18750"/><q:gd name="adj" fmla="val 18750"/>"#.to_owned(),
        ] {
            assert!(update_callout(&mut source(&guides)?, &previous, &next).is_err());
        }
        Ok(())
    }
}
