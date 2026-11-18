//! Preserve a custom geometry independently of its editable, flattened preview.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use joyword_model::connector::GeometrySource;
use joyword_model::freeform::Freeform;
use quick_xml::events::BytesStart;
use quick_xml::name::{PrefixDeclaration, QName, ResolveResult};
use quick_xml::reader::NsReader;

use crate::package::MAX_EMBEDDED_XML;
use crate::xml::{El, MAX_ATTRS, Node, W};

pub(crate) const PART_BUDGET: usize = 16 << 20;
pub(crate) const WRITE_BUDGET: usize = 128 << 20;

// The range is the namespace declarations added to make the fragment standalone.
// Removing just those declarations gives the original bytes in the source part.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Fragment {
    xml: Arc<str>,
    declarations: Range<usize>,
}

const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";

/// Markup-compatibility declarations belong to the URI that the prefix
/// designated at the declaring element. A deeper element can reuse its prefix.
pub(crate) struct Compatibility {
    pub depth: usize,
    ignored: Option<Vec<String>>,
}

impl Compatibility {
    pub fn at(reader: &NsReader<&[u8]>, el: &El, depth: usize) -> Option<Self> {
        if !el.attrs.iter().any(|(key, _)| key.starts_with("mc:")) {
            return None;
        }
        let ignored = || {
            let mut uris = Vec::new();
            for (key, value) in &el.attrs {
                if !key.starts_with("mc:") {
                    continue;
                }
                if key != "mc:Ignorable" {
                    return None;
                }
                for prefix in value.split_whitespace() {
                    if uris.len() >= 64 || prefix.len() > 128 {
                        return None;
                    }
                    let name = format!("{prefix}:scope");
                    let ResolveResult::Bound(uri) =
                        reader.resolver().resolve_element(QName(name.as_bytes())).0
                    else {
                        return None;
                    };
                    let uri = std::str::from_utf8(uri.into_inner()).ok()?;
                    if uri.len() > 4096 {
                        return None;
                    }
                    uris.push(quick_xml::escape::unescape(uri).ok()?.into_owned());
                }
            }
            Some(uris)
        };
        Some(Self {
            depth,
            ignored: ignored(),
        })
    }
}

pub(crate) struct Capture {
    pub depth: usize,
    start: usize,
    name_end: usize,
    declarations: Option<(String, String)>,
}

impl Capture {
    pub fn begin(
        reader: &NsReader<&[u8]>,
        tag: &BytesStart<'_>,
        start: usize,
        depth: usize,
        budget: usize,
        scopes: &[Compatibility],
    ) -> Self {
        let declarations = (budget > 0)
            .then(|| inherited(reader, tag, budget.min(MAX_EMBEDDED_XML), scopes))
            .flatten();
        Self {
            depth,
            start,
            name_end: 1 + tag.name().as_ref().len(),
            declarations,
        }
    }

    pub fn finish(self, part: &[u8], end: usize, budget: &mut usize) -> Option<Fragment> {
        let raw = part.get(self.start..end)?;
        let (extra, merge) = self.declarations?;
        let total = raw
            .len()
            .checked_add(extra.len())?
            .checked_add(merge.len())?;
        if total > MAX_EMBEDDED_XML || total > *budget {
            return None;
        }
        let (name, rest) = std::str::from_utf8(raw)
            .ok()?
            .split_at_checked(self.name_end)?;
        let mut xml = String::with_capacity(total);
        xml.push_str(name);
        xml.push_str(&extra);
        xml.push_str(rest);
        if !merge.is_empty() {
            let pos = {
                let tree = roxmltree::Document::parse(&xml).ok()?;
                tree.root_element()
                    .attribute_node((MC, "Ignorable"))?
                    .range_value()
                    .end
            };
            xml.insert_str(pos, &merge);
        }
        *budget -= total;
        Some(Fragment {
            xml: xml.into(),
            declarations: self.name_end..self.name_end + extra.len(),
        })
    }
}

fn inherited(
    reader: &NsReader<&[u8]>,
    tag: &BytesStart<'_>,
    limit: usize,
    scopes: &[Compatibility],
) -> Option<(String, String)> {
    let bindings = reader.resolver().bindings();
    if bindings.size_hint().1? > MAX_ATTRS {
        return None;
    }
    let mut own = HashSet::new();
    let mut own_ignorable = None;
    for attr in tag.attributes() {
        let attr = attr.ok()?;
        let name = attr.key;
        let (ns, local) = reader.resolver().resolve_attribute(name);
        if local.as_ref() == b"Ignorable"
            && matches!(ns, ResolveResult::Bound(ns) if ns.as_ref()==MC.as_bytes())
        {
            own_ignorable = Some(
                attr.normalized_value(quick_xml::XmlVersion::Explicit1_0)
                    .ok()?
                    .into_owned(),
            );
        }
        let key = name.into_inner();
        if key == b"xmlns" {
            own.insert(&b""[..]);
        }
        if let Some(prefix) = key.strip_prefix(b"xmlns:") {
            own.insert(prefix);
        }
    }
    let mut extra = String::new();
    let mut names = HashMap::new();
    for (prefix, uri) in bindings {
        let prefix = match prefix {
            PrefixDeclaration::Default => &b""[..],
            PrefixDeclaration::Named(s) => s,
        };
        if prefix == b"xml" {
            continue;
        }
        // quick-xml's namespace resolver returns the lexical (escaped) URI.
        let uri = quick_xml::escape::unescape(std::str::from_utf8(uri.as_ref()).ok()?).ok()?;
        if !prefix.is_empty() {
            names.insert(
                std::str::from_utf8(prefix).ok()?.to_string(),
                uri.to_string(),
            );
        }
        if own.contains(prefix) {
            continue;
        }
        if prefix.len() + uri.len() + extra.len() + 12 > limit {
            return None;
        }
        extra.push_str(" xmlns");
        if !prefix.is_empty() {
            extra.push(':');
            extra.push_str(std::str::from_utf8(prefix).ok()?);
        }
        extra.push_str("=\"");
        extra.push_str(&crate::xml::esc(&uri));
        extra.push('"');
        if extra.len() > limit {
            return None;
        }
    }
    let mut needed = Vec::new();
    for scope in scopes {
        for uri in scope.ignored.as_ref()? {
            let already = own_ignorable
                .as_ref()
                .is_some_and(|value| value.split_whitespace().any(|p| names.get(p) == Some(uri)));
            if !already && !needed.contains(&uri.as_str()) {
                if needed.len() >= 64 {
                    return None;
                }
                needed.push(uri.as_str());
            }
        }
    }
    if needed.is_empty() {
        return Some((extra, String::new()));
    }
    let mut serial = 0;
    let mut prefix_for = |uri: &str| -> Option<String> {
        if let Some(p) = names
            .iter()
            .filter(|(_, u)| u.as_str() == uri)
            .map(|(p, _)| p)
            .min()
        {
            return Some(p.clone());
        }
        let p = loop {
            let p = format!("joyGeom{serial}");
            serial += 1;
            if !names.contains_key(&p) {
                break p;
            }
        };
        extra.push_str(&format!(" xmlns:{p}=\"{}\"", crate::xml::esc(uri)));
        if extra.len() > limit {
            return None;
        }
        names.insert(p.clone(), uri.to_string());
        Some(p)
    };
    let values = needed
        .into_iter()
        .map(&mut prefix_for)
        .collect::<Option<Vec<_>>>()?
        .join(" ");
    if own_ignorable.is_some() {
        return Some((extra, format!(" {values}")));
    }
    let mc = prefix_for(MC)?;
    extra.push_str(&format!(" {mc}:Ignorable=\"{values}\""));
    (extra.len() <= limit).then_some((extra, String::new()))
}

pub(crate) fn origin(geometry: &El, preview: Option<&Freeform>) -> Option<GeometrySource> {
    let raw = geometry.kids.iter().find_map(|n| match n {
        Node::GeometrySource(Some(fragment)) => Some(fragment),
        _ => None,
    });
    let Some(raw) = raw else {
        return Some(GeometrySource::Unavailable);
    };
    // Our native ink and polyline syntax is regenerated exactly from the model.
    // Keep imported guides, curves, comments, extensions and spelling otherwise.
    let rest = &raw.xml[raw.declarations.end..];
    if rest.starts_with("><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect ")
        && let Some(f) = preview
    {
        let mut native = W::default();
        native.freeform_geometry(f, f.w, f.h);
        if native.s.strip_prefix(&raw.xml[..raw.declarations.start]) == Some(rest) {
            return None;
        }
    }
    Some(GeometrySource::Xml(raw.xml.clone()))
}

/// Recovery files and clipboard input can also supply this field. Validate its
/// exact outer boundary, relationship ownership and preview before writing raw XML.
pub(crate) fn verified<'a>(
    source: &'a GeometrySource,
    current: Option<&Freeform>,
    size: [f32; 2],
) -> Result<&'a str, &'static str> {
    let GeometrySource::Xml(xml) = source else {
        return Err("Custom geometry exceeds the original XML preservation budget");
    };
    let invalid = "Invalid preserved DrawingML custom geometry";
    if xml.len() > MAX_EMBEDDED_XML {
        return Err(invalid);
    }
    let parsed = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(|_| invalid)?;
    let root = parsed.root_element();
    if root.range() != (0..xml.len())
        || root.tag_name().name() != "custGeom"
        || !matches!(
            root.tag_name().namespace(),
            Some(
                "http://schemas.openxmlformats.org/drawingml/2006/main"
                    | "http://purl.oclc.org/ooxml/drawingml/main"
            )
        )
    {
        return Err(invalid);
    }
    if root.descendants().any(|node| {
        node.attributes().any(|a| {
            matches!(
                a.namespace(),
                Some(
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                        | "http://purl.oclc.org/ooxml/officeDocument/relationships"
                )
            ) || (a.namespace() == Some("urn:schemas-microsoft-com:office:office")
                && a.name() == "relid")
        })
    }) {
        return Err("Cannot safely reattach a relationship in custom geometry");
    }
    let geometry = crate::xml::parse_bounded(xml.as_bytes(), crate::xml::MAX_DEPTH, 250_000)
        .map_err(|_| invalid)?;
    // A resize changes the containing frame. Compare in the preview's original
    // coordinate space, which is also what Freeform::placed uses for scaling.
    let [w, h] = current.map_or(size, |f| [f.w, f.h]);
    let original = crate::read::freeform::paths(&geometry, w, h);
    if original.as_ref().map(|f| &f.paths) != current.map(|f| &f.paths) {
        return Err("Edited freeform points conflict with preserved DrawingML geometry");
    }
    Ok(xml)
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

    #[test]
    fn inherited_aliases_and_extension_bytes_survive_without_replaying_edited_points() {
        let body = r#"<!--unchanged--><d:avLst><d:gd name="adj" fmla="val 20"/></d:avLst><d:gdLst><d:gd name="px" fmla="*/ wd2 adj 10"/></d:gdLst><d:pathLst><d:path w="120" h="80"><d:moveTo><d:pt x="px" y="vc"/></d:moveTo><d:arcTo wR="hd2" hR="hd4" stAng="0" swAng="cd4"/></d:path></d:pathLst><d:extLst><d:ext uri="urn:fixture"><x:extra x:expression="x:q" note="&amp;"/></d:ext></d:extLst>"#;
        let input = format!(
            r#"<outer xmlns:d="{A}" xmlns:x="urn:saved"><d:custGeom>{body}</d:custGeom></outer>"#
        );
        let tree = crate::xml::parse(input.as_bytes()).unwrap();
        let geom = tree.child("a:custGeom").unwrap();
        let mut preview = crate::read::freeform::paths(geom, 120., 80.).unwrap();
        let p = &preview.paths[0].pts;
        assert_eq!(p.first(), Some(&[120., 40.]));
        assert_eq!(p.last(), Some(&[80., 60.]));
        let source = origin(geom, Some(&preview)).unwrap();
        let xml = verified(&source, Some(&preview), [240., 40.]).unwrap();
        assert!(xml.contains(body));
        let parsed = roxmltree::Document::parse(xml).unwrap();
        assert!(
            parsed
                .descendants()
                .any(|n| n.has_tag_name(("urn:saved", "extra")))
        );
        let second = crate::xml::parse(xml.as_bytes()).unwrap();
        assert_eq!(origin(&second, Some(&preview)), Some(source.clone()));
        preview.paths[0].pts[0][1] += 1.;
        assert!(verified(&source, Some(&preview), [120., 80.]).is_err());
    }

    #[test]
    fn inherited_compatibility_uses_uris_and_merges_with_local_declarations() {
        let input = format!(
            r#"<outer xmlns:d="{A}" xmlns:compat="{MC}" xmlns:x="urn:old" compat:Ignorable="x"><nested xmlns:x="urn:new" xmlns:y="urn:old"><d:custGeom compat:Ignorable="x"><d:pathLst/><y:future/><x:another/></d:custGeom></nested></outer>"#
        );
        let tree = crate::xml::parse(input.as_bytes()).unwrap();
        let source = origin(tree.find("a:custGeom").unwrap(), None).unwrap();
        let xml = verified(&source, None, [1., 1.]).unwrap();
        let parsed = roxmltree::Document::parse(xml).unwrap();
        let root = parsed.root_element();
        let uris = root
            .attribute((MC, "Ignorable"))
            .unwrap()
            .split_whitespace()
            .filter_map(|p| root.lookup_namespace_uri(Some(p)))
            .collect::<HashSet<_>>();
        assert_eq!(uris, HashSet::from(["urn:old", "urn:new"]));
        assert!(xml.contains("<y:future/><x:another/>"));
    }

    #[test]
    fn recovered_geometry_checks_bounds_ownership_and_exact_xml_root() {
        for raw in [
            format!(r#"<d:custGeom xmlns:d="{A}"/><outside/>"#),
            format!(r#"<!--outside--><d:custGeom xmlns:d="{A}"/>"#),
            format!(r#"<!DOCTYPE x [<!ENTITY e "value">]><d:custGeom xmlns:d="{A}"/>"#),
            format!(
                r#"<d:custGeom xmlns:d="{A}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><vendor r:id="rId4"/></d:custGeom>"#
            ),
            "<foreign/>".to_string(),
            "a".repeat(MAX_EMBEDDED_XML + 1),
        ] {
            assert!(verified(&GeometrySource::Xml(raw.into()), None, [1., 1.]).is_err());
        }
        assert!(verified(&GeometrySource::Unavailable, None, [1., 1.]).is_err());
    }
}
