//! PPTX import: package → [`Presentation`].

pub(crate) mod chart;
pub(crate) mod dml;
pub(super) mod shapes;
use crate::theme_part as theme;
pub(crate) use shapes::preset_geom;
mod timing;

pub(crate) use crate::chartex_part::{chartex_from, read_chartex_xml};
pub(crate) use chart::{chart_from as chart_from_el, read_chart_xml};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use joydeck_geom::{Size, emu_to_pt};
use joydeck_model::{
    CustomShow, EmbeddedFont, Geom, HeaderFooter, LayoutId, MediaId, MediaItem, PhType,
    Presentation, Properties, Section, ShowSettings, Slide, SlideId, defaults,
};

use crate::opc::{self, Package, Rels};
use crate::xml::{Doc, El, Node};
use crate::{PptxError, master_part};

/// Import state shared by all parts.
pub struct Imp<'a> {
    pub pkg: &'a Package,
    pub media: Vec<MediaItem>,
    pub media_by_part: HashMap<String, MediaId>,
    pub media_by_uri: HashMap<String, MediaId>,
    pub next: u32,
    pub slide_by_part: HashMap<String, SlideId>,
    pub custom_show_names: HashMap<String, String>,
    /// GUIDs (upper case) of the table styles the file defines.
    pub table_style_ids: Vec<String>,
    /// Per legacy VML drawing part: shape id → preview picture part (see `shapes::vml_preview`).
    pub vml_previews: HashMap<String, HashMap<String, String>>,
    /// Aggregate byte count of retained chart sidecars across the presentation.
    pub kept_sidecar_bytes: usize,
    /// Cached kept sidecars by normalized chart part name.
    pub kept_charts: HashMap<String, joydeck_model::chart::ChartKept>,
}

impl Imp<'_> {
    pub fn alloc(&mut self) -> u32 {
        self.next = self.next.saturating_add(1);
        self.next
    }
}

/// One part being read: its name, relationships and root namespace declarations.
pub struct Part {
    pub name: String,
    pub rels: Rels,
    pub ns: Vec<(String, String)>,
    /// Prefixes bound below the root only (on a wrapper such as `mc:Choice`), each to one namespace.
    pub inner_ns: Vec<(String, String)>,
}

impl Part {
    pub fn new(pkg: &Package, name: &str, doc: &Doc) -> Self {
        let ns = doc.ns_decls();
        Part {
            name: name.to_string(),
            rels: pkg.rels(name),
            inner_ns: inner_decls(&doc.root, &ns),
            ns,
        }
    }
    /// Serialize an element with the namespace declarations it needs from the part root.
    pub fn keep(&self, el: &El) -> String {
        keep_xml(el, &self.ns)
    }
    /// Like [`Part::keep`], also declaring prefixes that only an enclosing wrapper binds.
    pub fn keep_inner(&self, el: &El) -> String {
        let ns: Vec<(String, String)> = self.ns.iter().chain(&self.inner_ns).cloned().collect();
        keep_xml(el, &ns)
    }
}

/// Prefixes declared below the root but not on it, skipping any bound to different namespaces.
fn inner_decls(root: &El, ns: &[(String, String)]) -> Vec<(String, String)> {
    fn rec(
        e: &El,
        out: &mut Vec<(String, String)>,
        out_map: &mut HashMap<String, String>,
        clash: &mut HashSet<String>,
        depth: usize,
    ) {
        if depth > crate::xml::MAX_DEPTH {
            return;
        }
        for (k, v) in e.attrs.iter().filter(|(k, _)| k.starts_with("xmlns:")) {
            if let Some(existing) = out_map.get(k) {
                if existing != v {
                    clash.insert(k.clone());
                }
            } else if out.len() < 1000 {
                out_map.insert(k.clone(), v.clone());
                out.push((k.clone(), v.clone()));
            }
        }
        for c in e.elements() {
            rec(c, out, out_map, clash, depth + 1);
        }
    }
    let mut out = vec![];
    let mut out_map = HashMap::new();
    let mut clash = HashSet::new();
    let root_ns: HashSet<&str> = ns.iter().map(|(r, _)| r.as_str()).collect();
    for c in root.elements() {
        rec(c, &mut out, &mut out_map, &mut clash, 1);
    }
    out.retain(|(k, _)| !clash.contains(k) && !root_ns.contains(k.as_str()));
    out
}

/// Serialize `el`, declaring every prefix it uses that the source root declared.
pub fn keep_xml(el: &El, ns: &[(String, String)]) -> String {
    let mut used = HashSet::new();
    fn rec(e: &El, used: &mut HashSet<String>, depth: usize) {
        if depth > crate::xml::MAX_DEPTH {
            return;
        }
        if let Some((p, _)) = e.name.split_once(':') {
            used.insert(p.to_string());
        }
        for (k, _) in &e.attrs {
            if let Some((p, _)) = k.split_once(':')
                && p != "xmlns"
                && p != "xml"
            {
                used.insert(p.to_string());
            }
        }
        for c in e.elements() {
            rec(c, used, depth + 1);
        }
    }
    rec(el, &mut used, 0);
    let mut el = el.clone();
    for (k, v) in ns {
        let p = k.strip_prefix("xmlns:").unwrap_or("");
        if !p.is_empty() && used.contains(p) && !el.attrs.iter().any(|(a, _)| a == k) {
            el.attrs.push((k.clone(), v.clone()));
        }
    }
    el.to_xml()
}

/// Namespace prefixes whose `mc:Choice` content we understand.
const SUPPORTED_MC: [&str; 10] = [
    "p14", "p15", "p159", "a14", "a16", "p1510", "a15", "c14", "p188", "cx1",
];

/// Office 2016 charts sit in a `cx1` choice; take it only for a layout DeckCraft reads (the
/// fallback is a picture or "this chart isn't available" text). The part is checked later, when
/// the frame is read, so here only the namespace counts.
fn chartex_readable(ch: &El) -> bool {
    let mut found = vec![];
    ch.find_all("graphicData", &mut found);
    found
        .iter()
        .any(|g| g.attr("uri").is_some_and(|u| u.ends_with("/chartex")))
}

/// Add-in tags named by `holder`'s `p:custDataLst`: the name/value pairs of each tags part.
pub fn tags(imp: &Imp, part: &Part, holder: &El) -> Vec<(String, String)> {
    let Some(list) = holder.child("custDataLst") else {
        return vec![];
    };
    let mut out = Vec::new();
    for t in list.children_named("tags").take(100) {
        let Some(doc) = t
            .attr("r:id")
            .and_then(|id| part.rels.get(id))
            .and_then(|r| imp.pkg.xml(&r.target))
        else {
            continue;
        };
        for tag in doc.root.children_named("tag").take(10_000) {
            if let Some(name) = tag.attr("name") {
                out.push((name.to_string(), tag.attr("val").unwrap_or("").to_string()));
            }
        }
    }
    out
}

/// Relationship types whose parts are kept verbatim, and whether they hang off the package root.
const KEPT_RELS: [(&str, bool); 3] = [
    (
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml",
        false,
    ),
    (
        "http://schemas.microsoft.com/office/2020/02/relationships/classificationlabels",
        true,
    ),
    (
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties",
        true,
    ),
];

/// A part we may keep: outside `ppt/` and the properties the writer makes itself.
fn keepable(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    !n.is_empty()
        && !n.contains("..")
        && !n.starts_with("ppt/")
        && n != "docprops/core.xml"
        && n != "docprops/app.xml"
        && !n.starts_with("[content_types]")
}

/// Custom XML data, sensitivity labels and custom properties, with the parts they relate to
/// (`itemProps`) and their relationships parts, as written.
fn kept_parts(pkg: &Package, pres_path: &str) -> Vec<joydeck_model::KeptPart> {
    use joydeck_model::KeptPart;
    let mut out: Vec<KeptPart> = Vec::new();
    let text = |name: &str| {
        pkg.get(name)
            .and_then(|b| std::str::from_utf8(b).ok())
            .map(str::to_string)
    };
    for (holder, from_root) in [("", true), (pres_path, false)] {
        let Some(doc) = pkg.xml(&opc::rels_path(holder)) else {
            continue;
        };
        let base = opc::dir_of(holder);
        for r in doc.root.children_named("Relationship") {
            let (Some(ty), Some(target)) = (r.attr("Type"), r.attr("Target")) else {
                continue;
            };
            if r.attr("TargetMode")
                .is_some_and(|m| m.eq_ignore_ascii_case("external"))
                || !KEPT_RELS
                    .iter()
                    .any(|(t, root)| *t == ty && *root == from_root)
            {
                continue;
            }
            let name = opc::resolve(&base, target);
            if !keepable(&name)
                || out.iter().any(|k| k.name.eq_ignore_ascii_case(&name))
                || out.len() >= 400
            {
                continue;
            }
            let Some(xml) = text(&name) else { continue };
            out.push(KeptPart {
                content_type: pkg.content_type(&name).unwrap_or_default(),
                name: name.clone(),
                xml,
                rel_type: ty.to_string(),
                from_root,
            });
            // The part's own relationships (customXml item → itemProps) and their targets.
            let rp = opc::rels_path(&name);
            if let Some(rels_xml) = text(&rp) {
                for sub in pkg
                    .rels(&name)
                    .items
                    .iter()
                    .filter(|s| !s.external && keepable(&s.target))
                {
                    if let Some(x) = text(&sub.target)
                        && !out.iter().any(|k| k.name.eq_ignore_ascii_case(&sub.target))
                    {
                        out.push(KeptPart {
                            content_type: pkg.content_type(&sub.target).unwrap_or_default(),
                            name: sub.target.clone(),
                            xml: x,
                            ..Default::default()
                        });
                    }
                }
                out.push(KeptPart {
                    name: rp,
                    xml: rels_xml,
                    ..Default::default()
                });
            }
        }
    }
    out
}

/// Replace every `mc:AlternateContent` by the children of the branch we can read best.
/// A 3-D model choice is left in place (even when its `Requires` prefix is not one we otherwise
/// understand) so the shape reader can keep the model and its fallback picture. Ink content parts
/// still fall through to the fallback picture.
pub fn resolve_mc(el: &mut El, depth: usize) {
    resolve_mc_rels(el, depth, None);
}

fn resolve_mc_rels(el: &mut El, depth: usize, rels: Option<&Rels>) {
    if depth > crate::xml::MAX_DEPTH {
        return;
    }
    if el
        .children
        .iter()
        .any(|c| matches!(c, Node::El(e) if e.is("AlternateContent")))
    {
        let mut out = Vec::with_capacity(el.children.len());
        for c in std::mem::take(&mut el.children) {
            match c {
                Node::El(ac) if ac.is("AlternateContent") => {
                    if rels.is_some_and(|rels| {
                        ac.children_named("Choice")
                            .any(|ch| crate::model3d_part::recognizes_choice(ch, rels))
                    }) {
                        out.push(Node::El(ac));
                        continue;
                    }
                    let choice = ac.children_named("Choice").find(|ch| {
                        let req_ok = ch
                            .attr("Requires")
                            .unwrap_or("")
                            .split_whitespace()
                            .all(|r| SUPPORTED_MC.contains(&r));
                        // Ink content parts are not modelled; prefer their fallback picture.
                        let modelled = !ch.elements().any(|e| e.is("contentPart"))
                            && (!ch.attr("Requires").unwrap_or("").contains("cx1")
                                || chartex_readable(ch));
                        req_ok && modelled
                    });
                    let branch = choice.or_else(|| ac.child("Fallback"));
                    if let Some(b) = branch {
                        for n in &b.children {
                            if let Node::El(e) = n {
                                let mut e = e.clone();
                                // Carry namespace declarations from the branch onto its content.
                                for (k, v) in b.attrs.iter().chain(ac.attrs.iter()) {
                                    if k.starts_with("xmlns")
                                        && !e.attrs.iter().any(|(a, _)| a == k)
                                    {
                                        e.attrs.push((k.clone(), v.clone()));
                                    }
                                }
                                out.push(Node::El(e));
                            }
                        }
                    }
                }
                other => out.push(other),
            }
        }
        el.children = out;
    }
    for c in el.elements_mut() {
        resolve_mc_rels(c, depth + 1, rels);
    }
}

pub(super) fn load(pkg: &Package, name: &str) -> Option<Doc> {
    let mut d = pkg.xml(name)?;
    let rels = pkg.rels(name);
    resolve_mc_rels(&mut d.root, 0, Some(&rels));
    Some(d)
}

pub(super) fn max_cnvpr(el: &El, m: &mut u32) {
    let mut v = vec![];
    el.find_all("cNvPr", &mut v);
    for c in v {
        if let Some(id) = c.u32("id")
            && id < shapes::MAX_KEPT_ID
        {
            *m = (*m).max(id);
        }
    }
}

/// The theme of a `.thmx` theme file (its `officeDocument` is a theme part), or of the first
/// master of a `.pptx` / `.potx`, with the media its format scheme uses (ids from 1).
pub fn import_theme(
    bytes: &[u8],
) -> Result<(joydeck_model::theme::Theme, Vec<MediaItem>), PptxError> {
    let pkg = Package::open(bytes)?;
    let target = pkg
        .rels("")
        .first_of("officeDocument")
        .map(|r| r.target.clone());
    let theme_path = match &target {
        Some(t) if pkg.xml(t).is_some_and(|d| d.root.is("theme")) => t.clone(),
        _ => {
            // A presentation: its first master's theme.
            let pres = target.unwrap_or_else(|| "ppt/presentation.xml".into());
            let pres_doc = load(&pkg, &pres)
                .ok_or_else(|| PptxError::NotPptx("neither a theme nor a presentation".into()))?;
            let pres_part = Part::new(&pkg, &pres, &pres_doc);
            let master = pres_doc
                .root
                .child("sldMasterIdLst")
                .and_then(|l| l.children_named("sldMasterId").next())
                .and_then(|m| {
                    pres_part
                        .rels
                        .target(m.attr("r:id").unwrap_or(""))
                        .map(String::from)
                })
                .or_else(|| {
                    pres_part
                        .rels
                        .all_of("slideMaster")
                        .next()
                        .map(|r| r.target.clone())
                })
                .ok_or_else(|| PptxError::NotPptx("the presentation has no slide master".into()))?;
            pkg.rels(&master)
                .first_of("theme")
                .map(|r| r.target.clone())
                .ok_or_else(|| PptxError::NotPptx("the slide master has no theme".into()))?
        }
    };
    let doc = load(&pkg, &theme_path)
        .ok_or_else(|| PptxError::NotPptx(format!("missing or unreadable {theme_path}")))?;
    if !doc.root.is("theme") {
        return Err(PptxError::NotPptx(format!(
            "{theme_path} is not a theme part"
        )));
    }
    let mut imp = Imp {
        pkg: &pkg,
        media: vec![],
        media_by_part: HashMap::new(),
        media_by_uri: HashMap::new(),
        next: 0,
        slide_by_part: HashMap::new(),
        custom_show_names: HashMap::new(),
        table_style_ids: Vec::new(),
        vml_previews: HashMap::new(),
        kept_sidecar_bytes: 0,
        kept_charts: HashMap::new(),
    };
    let part = Part::new(&pkg, &theme_path, &doc);
    let theme = theme::read_theme(&mut imp, &part, &doc.root);
    Ok((theme, imp.media))
}

pub fn import(bytes: &[u8]) -> Result<Presentation, PptxError> {
    import_checked(bytes).map(|(p, _)| p)
}

/// [`import`], plus the parts of the slide list the file names but that couldn't be read (a
/// missing or truncated slide): those slides are left out, so a caller can say so (#203).
pub fn import_checked(bytes: &[u8]) -> Result<(Presentation, Vec<String>), PptxError> {
    import_package(Package::open(bytes)?)
}

pub(super) fn import_package(pkg: Package) -> Result<(Presentation, Vec<String>), PptxError> {
    let mut lost_slides: Vec<String> = vec![];
    let root_rels = pkg.rels("");
    let pres_path = pkg.presentation_path()?;
    let pres_doc = load(&pkg, &pres_path)
        .ok_or_else(|| PptxError::NotPptx(format!("missing or unreadable {pres_path}")))?;
    let pres_el = &pres_doc.root;
    if !pres_el.is("presentation") {
        return Err(PptxError::NotPptx(format!(
            "{pres_path} is not a presentation part"
        )));
    }
    let pres_part = Part::new(&pkg, &pres_path, &pres_doc);

    let crate::presentation_parts::Inventory {
        masters: master_paths,
        slides: slide_entries,
        documents: docs,
        max_shape_id: max_id,
    } = crate::presentation_parts::Inventory::read(&pkg, pres_el, &pres_part.rels);

    let mut imp = Imp {
        pkg: &pkg,
        media: vec![],
        media_by_part: HashMap::new(),
        media_by_uri: HashMap::new(),
        next: max_id.max(1),
        slide_by_part: HashMap::new(),
        custom_show_names: HashMap::new(),
        table_style_ids: Vec::new(),
        vml_previews: HashMap::new(),
        kept_sidecar_bytes: 0,
        kept_charts: HashMap::new(),
    };
    // Slide ids first so hyperlinks anywhere can point at slides.
    let slide_ids = register_slides(&mut imp, &docs, &slide_entries, &mut lost_slides);
    read_custom_show_names(&mut imp, pres_el);

    let mut p = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
    p.masters.clear();
    p.media.clear();
    p.default_text_style = Default::default();

    read_presentation_settings(&mut imp, &mut p, &pres_part, pres_el);
    let layout_by_part = master_part::read_graph(&mut imp, &mut p, &master_paths, &docs);
    let file_slide_ids = read_slides(
        &mut imp,
        &mut p,
        &pres_part,
        &docs,
        &slide_ids,
        &layout_by_part,
    );
    read_sections(&mut p, &pres_part, pres_el, &file_slide_ids);
    p.tags = tags(&imp, &pres_part, pres_el);
    read_custom_shows(&mut p, &imp, &pres_part, pres_el);
    read_guides_and_show(&mut p, &imp, &pres_part);
    master_part::read_aux(&mut imp, &mut p, &pres_part);
    read_document_properties(&mut p, &pkg, &root_rels);

    p.kept_parts = kept_parts(&pkg, &pres_path);

    inherit_placeholder_geometry(&mut p);
    default_text_anchor(&mut p);
    p.header_footer = header_footer(&p);
    p.embedded_fonts = read_embedded_fonts(&mut imp, &pres_part, pres_el);
    p.media = std::mem::take(&mut imp.media);
    p.next_id = imp.next;
    p.fix_next_id();
    Ok((p, lost_slides))
}

/// Allocates an id for every readable slide; slides that are missing or unreadable are recorded in
/// `lost_slides` and left out.
fn register_slides(
    imp: &mut Imp,
    docs: &HashMap<String, Doc>,
    slide_entries: &[(u32, String)],
    lost_slides: &mut Vec<String>,
) -> Vec<(u32, String, SlideId)> {
    let mut slide_ids: Vec<(u32, String, SlideId)> = vec![];
    for (file_id, path) in slide_entries {
        if !docs.contains_key(&opc::norm(path)) {
            log::warn!("pptx: slide {path} is missing or unreadable; skipped");
            lost_slides.push(path.clone());
            continue;
        }
        let id = SlideId(imp.alloc());
        imp.slide_by_part.insert(opc::norm(path), id);
        slide_ids.push((*file_id, path.clone(), id));
    }
    slide_ids
}

fn read_custom_show_names(imp: &mut Imp, pres_el: &El) {
    if let Some(l) = pres_el.child("custShowLst") {
        for cs in l.children_named("custShow") {
            if let (Some(id), Some(name)) = (cs.attr("id"), cs.attr("name")) {
                imp.custom_show_names
                    .insert(id.to_string(), name.to_string());
            }
        }
    }
}

/// Slide and notes size, first slide number, default text style and table styles.
fn read_presentation_settings(imp: &mut Imp, p: &mut Presentation, pres_part: &Part, pres_el: &El) {
    if let Some(sz) = pres_el.child("sldSz") {
        let w = sz.i64("cx").map(emu_to_pt).unwrap_or(960.0);
        let h = sz.i64("cy").map(emu_to_pt).unwrap_or(540.0);
        p.slide_size = Size::new(sane_len(w, 960.0), sane_len(h, 540.0));
    }
    if let Some(sz) = pres_el.child("notesSz") {
        let w = sz.i64("cx").map(emu_to_pt).unwrap_or(540.0);
        let h = sz.i64("cy").map(emu_to_pt).unwrap_or(720.0);
        p.notes_size = Size::new(sane_len(w, 540.0), sane_len(h, 720.0));
    }
    p.first_slide_number = pres_el.u32("firstSlideNum").unwrap_or(1).min(9999);
    if let Some(dts) = pres_el.child("defaultTextStyle") {
        p.default_text_style = dml::list_style(imp, pres_part, dts);
    }
    if let Some(r) = pres_part.rels.first_of("tableStyles")
        && let Some(d) = load(imp.pkg, &r.target)
    {
        let part = Part::new(imp.pkg, &r.target, &d);
        p.table_styles = crate::table_part::decode_styles(imp, &part, &d.root);
        imp.table_style_ids = p.table_styles.iter().map(|s| s.id.clone()).collect();
    }
}

/// Reads every registered slide with its notes and comments. Returns the slide ids by file id.
fn read_slides(
    imp: &mut Imp,
    p: &mut Presentation,
    pres_part: &Part,
    docs: &HashMap<String, Doc>,
    slide_ids: &[(u32, String, SlideId)],
    layout_by_part: &HashMap<String, LayoutId>,
) -> HashMap<u32, SlideId> {
    let pkg = imp.pkg;
    let fallback_layout = p
        .masters
        .first()
        .and_then(|m| m.layouts.first())
        .map(|l| l.id)
        .unwrap_or_default();
    let mut file_slide_ids: HashMap<u32, SlideId> = HashMap::new();
    let mut review = crate::review_part::Reader::new(pres_part);
    for (file_id, path, id) in slide_ids {
        let Some(doc) = docs.get(&opc::norm(path)) else {
            continue;
        };
        let part = Part::new(pkg, path, doc);
        let layout = part
            .rels
            .first_of("slideLayout")
            .and_then(|r| layout_by_part.get(&opc::norm(&r.target)).copied())
            .unwrap_or(fallback_layout);
        let mut slide = read_slide(imp, &part, doc, *id, layout);
        review.attach(imp, &part, &mut slide);
        file_slide_ids.insert(*file_id, *id);
        p.slides.push(Arc::new(slide));
    }
    file_slide_ids
}

/// Sections, and the extension elements the model does not understand (kept verbatim).
fn read_sections(
    p: &mut Presentation,
    pres_part: &Part,
    pres_el: &El,
    file_slide_ids: &HashMap<u32, SlideId>,
) {
    let Some(ext) = pres_el.child("extLst") else {
        return;
    };
    if let Some(sl) = ext.find("sectionLst") {
        for s in sl.children_named("section").take(10_000) {
            let slides = s
                .child("sldIdLst")
                .map(|l| {
                    l.children_named("sldId")
                        .filter_map(|x| x.u32("id").and_then(|i| file_slide_ids.get(&i).copied()))
                        .collect()
                })
                .unwrap_or_default();
            p.sections.push(Section {
                name: s.attr("name").unwrap_or("").to_string(),
                slides,
            });
        }
    }
    let kept: String = ext
        .children_named("ext")
        .filter(|e| e.attr("uri") != Some(crate::SECTION_EXT_URI) && !has_rel_refs(e))
        .map(|e| pres_part.keep(e))
        .collect();
    if !kept.is_empty() {
        p.raw_ext = Some(kept);
    }
}

fn read_custom_shows(p: &mut Presentation, imp: &Imp, pres_part: &Part, pres_el: &El) {
    let Some(l) = pres_el.child("custShowLst") else {
        return;
    };
    for cs in l.children_named("custShow") {
        let slides = cs
            .child("sldLst")
            .map(|l| {
                l.children_named("sld")
                    .filter_map(|s| pres_part.rels.target(s.attr("r:id").unwrap_or("")))
                    .filter_map(|t| imp.slide_by_part.get(&opc::norm(t)).copied())
                    .collect()
            })
            .unwrap_or_default();
        p.custom_shows.push(CustomShow {
            name: cs.attr("name").unwrap_or("").to_string(),
            slides,
        });
    }
}

/// Drawing guides (eighths of a point) and slide show settings.
fn read_guides_and_show(p: &mut Presentation, imp: &Imp, pres_part: &Part) {
    let pkg = imp.pkg;
    if let Some(r) = pres_part.rels.first_of("viewProps")
        && let Some(d) = pkg.xml(&r.target)
        && let Some(lst) = d.root.find("slideViewPr").and_then(|v| v.find("guideLst"))
    {
        p.guides = lst
            .children_named("guide")
            .filter_map(|g| {
                let pos = g
                    .attr("pos")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0)
                    / 8.0;
                pos.is_finite().then_some(joydeck_model::DrawGuide {
                    vertical: g.attr("orient") != Some("horz"),
                    pos,
                })
            })
            .take(1000)
            .collect();
    }
    if let Some(r) = pres_part.rels.first_of("presProps")
        && let Some(d) = pkg.xml(&r.target)
        && let Some(sp) = d.root.child("showPr")
    {
        p.show = read_show(sp, &imp.custom_show_names);
    }
}

fn read_document_properties(p: &mut Presentation, pkg: &Package, root_rels: &Rels) {
    if let Some(r) = root_rels.first_of("core-properties")
        && let Some(d) = pkg.xml(&r.target)
    {
        read_core(&d.root, &mut p.props);
    }
    if let Some(r) = root_rels.first_of("extended-properties")
        && let Some(d) = pkg.xml(&r.target)
    {
        p.props.company = d
            .root
            .child("Company")
            .map(|c| c.text())
            .unwrap_or_default();
    }
}

/// Embedded font faces in one file. Past this, further typefaces are dropped.
const MAX_EMBEDDED_FONTS: usize = 64;
/// One font part. A larger part is skipped; the rest of the file is still read.
const MAX_FONT_BYTES: usize = 32 * 1024 * 1024;

/// `p:embeddedFontLst`: each typeface and the font parts its four face slots point at.
fn read_embedded_fonts(imp: &mut Imp, pres: &Part, root: &El) -> Vec<EmbeddedFont> {
    let Some(list) = root.child("embeddedFontLst") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in list.children_named("embeddedFont").take(MAX_EMBEDDED_FONTS) {
        let Some(font) = e.child("font") else {
            continue;
        };
        let Some(typeface) = font
            .attr("typeface")
            .map(str::trim)
            .filter(|t| !t.is_empty())
        else {
            continue;
        };
        let typeface: String = typeface.chars().take(255).collect();
        let slot = |imp: &mut Imp, name: &str| -> Option<MediaId> {
            let id = e.child(name)?.attr("r:id")?;
            let part = pres.rels.target(id)?;
            let data = imp.pkg.get(part)?;
            if data.is_empty() || data.len() > MAX_FONT_BYTES {
                log::warn!("pptx: embedded font {part} skipped ({} bytes)", data.len());
                return None;
            }
            imp.media_for(part)
        };
        let regular = slot(imp, "regular");
        let bold = slot(imp, "bold");
        let italic = slot(imp, "italic");
        let bold_italic = slot(imp, "boldItalic");
        if regular.is_none() && bold.is_none() && italic.is_none() && bold_italic.is_none() {
            continue;
        }
        out.push(EmbeddedFont {
            typeface,
            panose: panose_attr(font),
            pitch_family: byte_attr(font, "pitchFamily"),
            charset: byte_attr(font, "charset"),
            regular,
            bold,
            italic,
            bold_italic,
        });
    }
    out
}

fn byte_attr(el: &El, name: &str) -> Option<u8> {
    u8::try_from(el.u32(name)?).ok()
}

/// `panose` is 20 hex digits (ST_Panose). Anything else is dropped.
fn panose_attr(el: &El) -> Option<String> {
    let v = el.attr("panose")?.trim();
    (v.len() == 20 && v.chars().all(|c| c.is_ascii_hexdigit())).then(|| v.to_string())
}

fn sane_len(v: f64, dflt: f64) -> f64 {
    if v.is_finite() && (1.0..=20_000.0).contains(&v) {
        v
    } else {
        dflt
    }
}

/// Does the element (or a descendant) reference a relationship?
pub fn has_rel_refs(el: &El) -> bool {
    !rel_refs(el).is_empty()
}

/// Is this attribute a relationship id (`r:id`, `r:embed`, `r:dm`…)?
pub fn is_rel_attr(k: &str, v: &str) -> bool {
    !k.starts_with("xmlns")
        && k.contains(':')
        && matches!(
            k.rsplit(':').next(),
            Some("id" | "embed" | "link" | "dm" | "lo" | "qs" | "cs" | "pict" | "href")
        )
        && !v.is_empty()
}

/// Every relationship id `el` and its descendants use, sorted and without repeats.
pub fn rel_refs(el: &El) -> Vec<String> {
    fn rec(e: &El, out: &mut Vec<String>, depth: usize) {
        if depth > crate::xml::MAX_DEPTH {
            return;
        }
        out.extend(
            e.attrs
                .iter()
                .filter(|(k, v)| is_rel_attr(k, v))
                .map(|(_, v)| v.clone()),
        );
        for c in e.elements() {
            rec(c, out, depth + 1);
        }
    }
    let mut out = vec![];
    rec(el, &mut out, 0);
    out.sort();
    out.dedup();
    out
}

/// A shape's text without an anchor of its own sits at the top, as the standard has it and as
/// PowerPoint draws it. The theme's shape defaults (`a:objectDefaults/a:spDef/a:bodyPr`, centred in
/// Office's themes) are for shapes inserted later, not for the file's own shapes: PowerPoint for
/// Mac draws `<a:bodyPr/>` at the top under an `spDef` with `anchor="ctr"`. The model's own default
/// for shapes is the middle, so a top or bottom anchor is made explicit here and a middle one left
/// unset (placeholders and text boxes keep inheriting).
fn default_text_anchor(p: &mut Presentation) {
    fn fix(shapes: &mut [joydeck_model::Shape]) {
        walk_shapes_mut(shapes, &mut |s| {
            if s.ph.is_none()
                && !s.text_box
                && !matches!(s.kind, joydeck_model::ShapeKind::Table(_))
                && let Some(t) = &mut s.text
            {
                // Middle is the model's own default for shapes, so it is kept as unset.
                let got = t.body.anchor.unwrap_or(joydeck_model::text::Anchor::Top);
                t.body.anchor = (got != joydeck_model::text::Anchor::Middle).then_some(got);
            }
        });
    }
    for m in &mut p.masters {
        let m = Arc::make_mut(m);
        fix(&mut m.shapes);
        for l in &mut m.layouts {
            fix(&mut l.shapes);
        }
    }
    for s in &mut p.slides {
        fix(&mut Arc::make_mut(s).shapes);
    }
}

/// Placeholders read without a geometry (an empty custom geometry, see `shapes::sp`) take the
/// geometry of the layout or master placeholder they inherit from, as PowerPoint draws them: a
/// layout's picture placeholder cut into a shape keeps that shape on the slide. Anything left
/// is a rectangle.
fn inherit_placeholder_geometry(p: &mut Presentation) {
    let unset = |g: &Geom| matches!(g, Geom::Custom { paths } if paths.is_empty());
    // Masters first, then layouts (from their masters), then slides (from layouts and masters).
    for m in &mut p.masters {
        let m = Arc::make_mut(m);
        walk_shapes_mut(&mut m.shapes, &mut |s| {
            if unset(&s.geom) {
                s.geom = Geom::default();
            }
        });
    }
    let mut layout_geoms: Vec<(LayoutId, usize, Geom)> = vec![];
    for m in &p.masters {
        for l in &m.layouts {
            let ctx = joydeck_model::resolve::Ctx::for_layout(p, m, l);
            for (i, s) in l.shapes.iter().enumerate() {
                if unset(&s.geom) {
                    let (_, mp) = joydeck_model::resolve::parents(&ctx, s);
                    layout_geoms.push((
                        l.id,
                        i,
                        mp.map(|m| m.geom.clone())
                            .filter(|g| !unset(g))
                            .unwrap_or_default(),
                    ));
                }
            }
        }
    }
    for m in &mut p.masters {
        let m = Arc::make_mut(m);
        for l in &mut m.layouts {
            for (id, i, g) in layout_geoms.iter().filter(|(id, _, _)| *id == l.id) {
                let _ = id;
                if let Some(s) = l.shapes.get_mut(*i) {
                    s.geom = g.clone();
                }
            }
        }
    }
    let mut slide_geoms: Vec<(usize, usize, Geom)> = vec![];
    for (si, sl) in p.slides.iter().enumerate() {
        let Some(ctx) = joydeck_model::resolve::Ctx::for_slide(p, sl) else {
            continue;
        };
        for (i, s) in sl.shapes.iter().enumerate() {
            if unset(&s.geom) {
                let (lp, mp) = joydeck_model::resolve::parents(&ctx, s);
                let g = lp
                    .map(|l| l.geom.clone())
                    .filter(|g| !unset(g))
                    .or_else(|| mp.map(|m| m.geom.clone()).filter(|g| !unset(g)));
                slide_geoms.push((si, i, g.unwrap_or_default()));
            }
        }
    }
    for (si, i, g) in slide_geoms {
        if let Some(sl) = p.slides.get_mut(si)
            && let Some(s) = Arc::make_mut(sl).shapes.get_mut(i)
        {
            s.geom = g;
        }
    }
    // Placeholders inside groups (rare) and anything still unset: rectangles.
    for sl in &mut p.slides {
        walk_shapes_mut(&mut Arc::make_mut(sl).shapes, &mut |s| {
            if unset(&s.geom) {
                s.geom = Geom::default();
            }
        });
    }
    for m in &mut p.masters {
        for l in &mut Arc::make_mut(m).layouts {
            walk_shapes_mut(&mut l.shapes, &mut |s| {
                if unset(&s.geom) {
                    s.geom = Geom::default();
                }
            });
        }
    }
}

/// Every shape, groups included.
fn walk_shapes_mut(
    shapes: &mut [joydeck_model::Shape],
    f: &mut dyn FnMut(&mut joydeck_model::Shape),
) {
    for s in shapes {
        f(s);
        if let joydeck_model::ShapeKind::Group { children, .. } = &mut s.kind {
            walk_shapes_mut(children, f);
        }
    }
}

fn read_slide(imp: &mut Imp, part: &Part, doc: &Doc, id: SlideId, layout: LayoutId) -> Slide {
    let root = &doc.root;
    let mut s = Slide {
        id,
        layout,
        hidden: root.bool("show") == Some(false),
        show_master_shapes: root.bool("showMasterSp").unwrap_or(true),
        color_map: master_part::read_override(root),
        ..Default::default()
    };
    let mut ctx = shapes::IdCtx::default();
    if let Some(csld) = root.child("cSld") {
        s.name = csld.attr("name").unwrap_or("").to_string();
        s.tags = tags(imp, part, csld);
        s.background = master_part::read_background(imp, part, csld);
        if let Some(tree) = csld.child("spTree") {
            s.shapes = shapes::sp_tree(imp, part, &mut ctx, tree, 0);
        }
        if let Some(c) = csld.child("controls") {
            let pics = shapes::controls(imp, part, &mut ctx, c);
            s.shapes.extend(pics);
        }
    }
    if let Some(t) = root.child("transition") {
        s.transition = crate::transition_part::read(imp, part, t);
    }
    if let Some(t) = root.child("timing") {
        s.animations = timing::animations(imp, part, &ctx, t, &s.shapes);
        timing::media_settings(t, &ctx, &mut s.shapes);
    }
    s.raw_ext = extensions(part, root);
    s
}

pub(super) fn extensions(part: &Part, root: &El) -> Option<String> {
    let node = root.child("extLst").filter(|node| !has_rel_refs(node))?;
    Some(node.elements().map(|child| part.keep(child)).collect())
}

fn read_show(sp: &El, shows: &HashMap<String, String>) -> ShowSettings {
    let mut s = ShowSettings {
        loop_until_esc: sp.bool("loop").unwrap_or(false),
        without_narration: sp.bool("showNarration") == Some(false),
        without_animation: sp.bool("showAnimation") == Some(false),
        use_timings: sp.bool("useTimings").unwrap_or(true),
        ..Default::default()
    };
    if sp.child("browse").is_some() {
        s.show_type = "browsed".into();
    } else if sp.child("kiosk").is_some() {
        s.show_type = "kiosk".into();
    }
    if let Some(r) = sp.child("sldRg") {
        s.range = Some((
            r.u32("st").unwrap_or(1).max(1),
            r.u32("end").unwrap_or(1).max(1),
        ));
    }
    if let Some(c) = sp.child("custShow") {
        let id = c.attr("id").unwrap_or("");
        s.custom_show = Some(shows.get(id).cloned().unwrap_or_else(|| id.to_string()));
    }
    if let Some(c) = sp.child("penClr").and_then(dml::color)
        && let joydeck_model::ColorBase::Rgb { rgb } = c.base
    {
        s.pen_color = rgb;
    }
    s
}

fn read_core(root: &El, p: &mut Properties) {
    for e in root.elements() {
        let t = e.text();
        match e.local() {
            "title" => p.title = t,
            "subject" => p.subject = t,
            "creator" => p.author = t,
            "keywords" => p.keywords = t,
            "description" => p.comments = t,
            "category" => p.category = t,
            "lastModifiedBy" => p.last_modified_by = t,
            "revision" => p.revision = t.trim().parse().unwrap_or(0),
            "created" => p.created = t,
            "modified" => p.modified = t,
            _ => {}
        }
    }
}

/// Summary of slide-local placeholders for dialog defaults, not an instruction
/// to replace each slide's own text or fields. Mixed decks retain their content.
fn header_footer(p: &Presentation) -> HeaderFooter {
    let mut hf = HeaderFooter::default();
    for s in &p.slides {
        for sh in &s.shapes {
            match sh.ph_type() {
                Some(PhType::SlideNum) => hf.slide_number = true,
                Some(PhType::Header) => hf.header = true,
                Some(PhType::Date) => {
                    hf.date = true;
                    if hf.date_format.is_empty()
                        && let Some(t) = &sh.text
                        && let Some(field) =
                            t.paragraphs
                                .iter()
                                .flat_map(|p| &p.runs)
                                .find_map(|r| match &r.kind {
                                    joydeck_model::text::RunKind::Field { field }
                                        if field.starts_with("datetime") =>
                                    {
                                        Some(field)
                                    }
                                    _ => None,
                                })
                    {
                        hf.date_format.clone_from(field);
                    }
                    if let Some(t) = &sh.text
                        && !t
                            .paragraphs
                            .iter()
                            .flat_map(|p| &p.runs)
                            .any(|r| matches!(r.kind, joydeck_model::text::RunKind::Field { .. }))
                        && hf.date_text.is_empty()
                    {
                        hf.date_text = t.text();
                    }
                }
                Some(PhType::Footer) => {
                    hf.footer = true;
                    if hf.footer_text.is_empty()
                        && let Some(t) = &sh.text
                    {
                        hf.footer_text = t.text();
                    }
                }
                _ => {}
            }
        }
    }
    hf
}
