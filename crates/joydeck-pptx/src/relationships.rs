//! OPC relationship targets used by both the reader and preserving writeback.

use std::{collections::HashSet, fmt::Write as _};

use crate::xml::{A, El, W};

#[derive(Default)]
pub struct RelsOut {
    edges: Vec<OutputEdge>,
    used: HashSet<String>,
}

struct OutputEdge {
    id: String,
    ty: String,
    target: String,
    external: bool,
}

impl RelsOut {
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn retain_internal_kind(&mut self, kind: &str) -> bool {
        let previous_count = self.edges.len();
        self.edges.retain(|edge| edge.ty != kind || !edge.external);
        if self.edges.len() != previous_count {
            self.used = self.edges.iter().map(|edge| edge.id.clone()).collect();
        }
        self.edges.iter().any(|edge| edge.ty == kind)
    }

    pub fn carry(&mut self, id: String, ty: String, target: String, external: bool) {
        self.used.insert(id.clone());
        self.edges.push(OutputEdge {
            id,
            ty,
            target,
            external,
        });
    }

    pub fn add(&mut self, kind: &str, target: &str, external: bool) -> String {
        let ty = relationship_type(kind);
        for edge in &self.edges {
            if edge.ty == ty && edge.target == target && edge.external == external {
                return edge.id.clone();
            }
        }
        self.allocate(ty, target, external)
    }

    pub fn add_new(&mut self, kind: &str, target: &str, external: bool) -> String {
        self.allocate(relationship_type(kind), target, external)
    }

    fn allocate(&mut self, ty: String, target: &str, external: bool) -> String {
        // Carried chart relationships retain the IDs referenced by the original chart XML.
        let mut id = format!("rId{}", self.edges.len() + 1);
        while self.used.contains(&id) {
            id.push('x');
        }
        self.carry(id.clone(), ty, target.into(), external);
        id
    }

    pub fn to_xml(&self) -> Vec<u8> {
        let mut output = W::new();
        output.open(
            "Relationships",
            A::new().a("xmlns", joyoffice_opc::RELATIONSHIPS_NAMESPACE),
        );
        for edge in &self.edges {
            output.empty(
                "Relationship",
                A::new()
                    .a("Id", &edge.id)
                    .a("Type", &edge.ty)
                    .a("Target", &edge.target)
                    .o("TargetMode", edge.external.then_some("External")),
            );
        }
        output.close("Relationships");
        output.finish()
    }
}

fn relationship_type(kind: &str) -> String {
    if kind.starts_with("http") || kind.contains(':') {
        kind.into()
    } else {
        format!("{}{kind}", crate::opc::RT_BASE)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Rel {
    pub id: String,
    pub kind: String,
    pub ty: String,
    pub target_raw: String,
    pub target: String,
    pub external: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Rels {
    pub items: Vec<Rel>,
}

impl Rels {
    pub fn get(&self, id: &str) -> Option<&Rel> {
        self.items.iter().find(|relationship| relationship.id == id)
    }

    pub fn target(&self, id: &str) -> Option<&str> {
        Some(&self.get(id)?.target)
    }

    pub fn first_of(&self, kind: &str) -> Option<&Rel> {
        self.items
            .iter()
            .find(|relationship| relationship.kind == kind)
    }

    pub fn all_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Rel> + 'a {
        self.items
            .iter()
            .filter(move |relationship| relationship.kind == kind)
    }
}

pub(super) fn read(root: &El, owner: &str) -> Rels {
    let (parent, _) = part_path(owner);
    let items = root
        .children_named("Relationship")
        .filter_map(|node| {
            let id = node.attr("Id")?.to_owned();
            let ty = node.attr("Type")?.to_owned();
            let raw = node.attr("Target")?;
            let external = node
                .attr("TargetMode")
                .is_some_and(|value| value.eq_ignore_ascii_case("External"));
            let target = match (external, raw.starts_with('#')) {
                (true, _) => raw.to_owned(),
                (false, true) if !owner.is_empty() => owner.trim_start_matches('/').to_owned(),
                _ => resolve(parent, raw),
            };
            Some(Rel {
                id,
                kind: file_of(&ty).to_owned(),
                ty,
                target_raw: raw.to_owned(),
                target,
                external,
            })
        })
        .collect();
    Rels { items }
}

fn part_path(name: &str) -> (&str, &str) {
    let name = name.trim_start_matches('/');
    name.rsplit_once('/').unwrap_or(("", name))
}

pub fn dir_of(name: &str) -> String {
    part_path(name).0.to_owned()
}

pub fn file_of(name: &str) -> &str {
    name.rsplit_once('/').map_or(name, |(_, file)| file)
}

pub fn rels_path(owner: &str) -> String {
    let (parent, filename) = part_path(owner);
    if parent.is_empty() {
        format!("_rels/{filename}.rels")
    } else {
        format!("{parent}/_rels/{filename}.rels")
    }
}

/// Decode URI octets while keeping malformed escapes readable in legacy files.
fn decode_path(path: &str) -> String {
    let mut pending = path.as_bytes();
    let mut decoded = Vec::with_capacity(pending.len());
    while let Some((&byte, remainder)) = pending.split_first() {
        if byte == b'%'
            && let [hi, lo, tail @ ..] = remainder
            && let (Some(hi), Some(lo)) =
                (char::from(*hi).to_digit(16), char::from(*lo).to_digit(16))
        {
            decoded.push((16 * hi + lo) as u8);
            pending = tail;
        } else {
            decoded.push(byte);
            pending = remainder;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Resolve an internal URI to a package member. Historical PowerPoint inputs
/// also use backslashes and redundant root-level parent segments.
pub fn resolve(parent: &str, reference: &str) -> String {
    let path = reference
        .split_once('#')
        .map_or(reference, |(path, _)| path);
    let path = decode_path(path);
    let is_separator = |c| matches!(c, '/' | '\\');
    let mut segments = Vec::new();
    if !path.starts_with(is_separator) {
        segments.extend(parent.split('/').filter(|part| !part.is_empty()));
    }
    for segment in path.split(is_separator) {
        match segment {
            "" | "." => (),
            ".." => {
                segments.pop();
            }
            part => segments.push(part),
        }
    }
    segments.join("/")
}

/// Form a relationship URI from the owner part to a decoded member name.
pub fn relative(owner: &str, destination: &str) -> String {
    let mut from = part_path(owner)
        .0
        .split('/')
        .filter(|s| !s.is_empty())
        .peekable();
    let (parent, file) = part_path(destination);
    let mut to = parent.split('/').filter(|s| !s.is_empty()).peekable();
    while from.peek().is_some() && from.peek() == to.peek() {
        from.next();
        to.next();
    }
    let mut uri = "../".repeat(from.count());
    for c in to
        .flat_map(|dir| dir.chars().chain(['/']))
        .chain(file.chars())
    {
        if matches!(c, '%' | '#' | '?' | ' ') || c.is_ascii_control() {
            let _ = write!(uri, "%{:02X}", u32::from(c));
        } else {
            uri.push(c);
        }
    }
    uri
}

/// Relocate a relationship without dropping a subresource fragment.
pub(super) fn rebase(owner: &str, destination: &str, original: &str) -> String {
    if original.starts_with('#') {
        return original.to_owned();
    }
    let mut target = relative(owner, destination);
    if let Some((_, fragment)) = original.split_once('#') {
        target.push('#');
        target.push_str(fragment);
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_edges_reuse_only_equal_relationships_and_keep_carried_ids()
    -> Result<(), crate::PptxError> {
        let target = "https://example.invalid/中文?a=1&b=2#view";
        let mut output = RelsOut::default();
        output.carry(
            "rId2".into(),
            "urn:vendor:sidecar".into(),
            "../vendor.xml".into(),
            false,
        );
        assert_eq!(output.add("hyperlink", target, true), "rId2x");
        assert_eq!(output.add("hyperlink", target, true), "rId2x");
        assert_eq!(output.add_new("hyperlink", target, true), "rId3");
        assert_eq!(output.add("hyperlink", target, false), "rId4");
        let parsed = crate::xml::parse(&output.to_xml())?;
        let edges = read(&parsed.root, "ppt/slides/slide1.xml").items;
        assert_eq!(edges.len(), 4);
        assert_eq!(edges[0].id, "rId2");
        assert_eq!(edges[0].ty, "urn:vendor:sidecar");
        assert_eq!(edges[0].target, "ppt/vendor.xml");
        assert_eq!(edges[1].target_raw, target);
        assert_eq!(edges[1].target, target);
        assert!(edges[1].external);
        assert!(!edges[3].external);
        Ok(())
    }

    #[test]
    fn a_relocated_member_keeps_literal_delimiters_and_a_fragment() {
        let from = "ppt/slides/copy.xml";
        let part = "vendor/文件 #2%.xml";
        let target = rebase(from, part, "../old.xml#item%201");
        assert_eq!(target, "../../vendor/文件%20%232%25.xml#item%201");
        assert_eq!(resolve(&dir_of(from), &target), part);
        assert_eq!(rebase(from, part, "#local"), "#local");
    }
}
