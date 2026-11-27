//! PresentationML master and layout parts, including their inherited formatting.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use joydeck_color::SchemeSlot;
use joydeck_model::text::ListStyle;
use joydeck_model::theme::Theme;
use joydeck_model::{
    Background, ColorRef, Fill, Layout, LayoutId, LayoutType, Master, MasterId,
    PlaceholderVisibility, Presentation, Shape, defaults,
};

use crate::opc::{self, NS_P};
use crate::presentation_parts;
use crate::read::{self, Imp, Part, dml as decode, shapes};
use crate::theme_part;
use crate::write::{self, Exp, Out, dml as encode};
use crate::xml::{A, Doc, El, Node, W};

const MAX_LAYOUTS_PER_MASTER: usize = 1000;

pub(super) fn layout_paths<'a>(
    root: &'a El,
    relationships: &'a opc::Rels,
) -> impl Iterator<Item = &'a str> {
    let declared = root
        .child("sldLayoutIdLst")
        .into_iter()
        .flat_map(|list| list.children_named("sldLayoutId"))
        .filter_map(|node| presentation_parts::target(node, relationships, "slideLayout"));
    let extras = relationships
        .all_of("slideLayout")
        .filter(|edge| !edge.external)
        .map(|edge| edge.target.as_str());
    let mut distinct = HashSet::new();
    declared
        .chain(extras)
        .filter(move |path| distinct.insert(opc::norm(path)))
        .take(MAX_LAYOUTS_PER_MASTER)
}

pub(super) fn read_graph(
    reader: &mut Imp,
    model: &mut Presentation,
    paths: &[String],
    documents: &HashMap<String, Doc>,
) -> HashMap<String, LayoutId> {
    let mut layouts = HashMap::new();
    for path in paths {
        let Some(doc) = documents.get(&opc::norm(path)) else {
            log::warn!("pptx: master {path} missing; skipped");
            continue;
        };
        let part = Part::new(reader.pkg, path, doc);
        let root = &doc.root;
        let theme = related_theme(reader, &part).unwrap_or_default();
        let id = MasterId(reader.alloc());
        let Common { name, bg, shapes } = Common::read(reader, &part, root);
        let style = |reader: &mut Imp, name| {
            root.path(&["txStyles", name])
                .map(|node| decode::list_style(reader, &part, node))
                .unwrap_or_default()
        };
        let mut master = Master {
            id,
            name: if name.is_empty() {
                theme.name.clone()
            } else {
                name
            },
            theme,
            color_map: root.child("clrMap").map(read_mapping).unwrap_or_default(),
            placeholder_visibility: read_visibility(root),
            preserve: root.bool("preserve").unwrap_or(false),
            background: bg,
            shapes,
            title_style: style(reader, "titleStyle"),
            body_style: style(reader, "bodyStyle"),
            other_style: style(reader, "otherStyle"),
            ..Default::default()
        };
        for layout_path in layout_paths(root, &part.rels) {
            let key = opc::norm(layout_path);
            if layouts.contains_key(&key) {
                continue;
            }
            let Some(layout_doc) = documents.get(&key) else {
                continue;
            };
            let layout = read_layout(reader, layout_path, layout_doc);
            layouts.insert(key, layout.id);
            master.layouts.push(layout);
        }
        if master.layouts.is_empty() {
            master.layouts.push(Layout {
                id: LayoutId(reader.alloc()),
                name: "Blank".into(),
                kind: LayoutType::Blank,
                show_master_shapes: true,
                ..Default::default()
            });
        }
        master.raw_ext = read::extensions(&part, root);
        model.masters.push(Arc::new(master));
    }
    if model.masters.is_empty() {
        log::warn!("pptx: no readable slide master; using a default one");
        let (master, _) = defaults::build_master(model.slide_size, Default::default(), reader.next);
        reader.next = reader.next.saturating_add(200);
        model.masters.push(Arc::new(master));
    }
    layouts
}

struct Common {
    name: String,
    bg: Option<Background>,
    shapes: Vec<Shape>,
}

impl Common {
    fn read(reader: &mut Imp, part: &Part, root: &El) -> Self {
        let content = root.child("cSld");
        Self {
            name: content
                .and_then(|node| node.attr("name"))
                .unwrap_or_default()
                .into(),
            bg: content.and_then(|node| read_background(reader, part, node)),
            shapes: content
                .and_then(|node| node.child("spTree"))
                .map(|tree| shapes::sp_tree(reader, part, &mut shapes::IdCtx::default(), tree, 0))
                .unwrap_or_default(),
        }
    }
}

fn related_theme(reader: &mut Imp, owner: &Part) -> Option<Theme> {
    let rel = owner.rels.first_of("theme")?;
    let doc = reader.pkg.xml(&rel.target)?;
    let part = Part::new(reader.pkg, &rel.target, &doc);
    Some(theme_part::read_theme(reader, &part, &doc.root))
}

fn read_layout(reader: &mut Imp, path: &str, doc: &Doc) -> Layout {
    let root = &doc.root;
    let part = Part::new(reader.pkg, path, doc);
    let id = LayoutId(reader.alloc());
    let Common { name, bg, shapes } = Common::read(reader, &part, root);
    Layout {
        id,
        name,
        background: bg,
        shapes,
        kind: LayoutType::from_xml(root.attr("type").unwrap_or("cust")),
        show_master_shapes: root.bool("showMasterSp").unwrap_or(true),
        preserve: root.bool("preserve").unwrap_or(false),
        color_map: read_override(root),
        placeholder_visibility: read_visibility(root),
        raw_ext: read::extensions(&part, root),
    }
}

pub(super) fn read_aux(reader: &mut Imp, model: &mut Presentation, presentation: &Part) {
    for (relation, default_name) in [
        ("notesMaster", "Notes Master"),
        ("handoutMaster", "Handout Master"),
    ] {
        let Some(rel) = presentation.rels.first_of(relation) else {
            continue;
        };
        let Some(doc) = read::load(reader.pkg, &rel.target) else {
            continue;
        };
        let part = Part::new(reader.pkg, &rel.target, &doc);
        let id = MasterId(reader.alloc());
        let notes = relation == "notesMaster";
        let theme = related_theme(reader, &part)
            .or_else(|| {
                model
                    .masters
                    .first()
                    .filter(|_| !notes)
                    .map(|m| m.theme.clone())
            })
            .unwrap_or_default();
        let Common { name, bg, shapes } = Common::read(reader, &part, &doc.root);
        let master = Master {
            id,
            theme,
            name: if notes || name.is_empty() {
                default_name.into()
            } else {
                name
            },
            background: bg,
            shapes,
            color_map: doc
                .root
                .child("clrMap")
                .map(read_mapping)
                .unwrap_or_default(),
            body_style: if notes {
                doc.root
                    .child("notesStyle")
                    .map(|node| decode::list_style(reader, &part, node))
                    .unwrap_or_default()
            } else {
                ListStyle::default()
            },
            ..Default::default()
        };
        let slot = if notes {
            &mut model.notes_master
        } else {
            &mut model.handout_master
        };
        *slot = Some(Arc::new(master));
    }
}

pub(super) fn read_background(reader: &mut Imp, part: &Part, content: &El) -> Option<Background> {
    let background = content.child("bg")?;
    if let Some(properties) = background.child("bgPr") {
        let fill = decode::fill_of(reader, part, properties).unwrap_or(Fill::None);
        Some(Background::Fill { fill })
    } else {
        let reference = background.child("bgRef")?;
        Some(Background::Ref {
            idx: reference.u32("idx").unwrap_or(1001),
            color: decode::color(reference).unwrap_or_else(|| ColorRef::scheme(SchemeSlot::Bg1)),
        })
    }
}

fn read_mapping(node: &El) -> Vec<(String, String)> {
    node.attrs
        .iter()
        .filter(|(name, _)| !name.starts_with("xmlns"))
        .cloned()
        .collect()
}

pub(super) fn read_override(root: &El) -> Option<Vec<(String, String)>> {
    root.path(&["clrMapOvr", "overrideClrMapping"])
        .map(read_mapping)
}

fn read_visibility(root: &El) -> Option<PlaceholderVisibility> {
    let mut matches = root.children_named("hf");
    let node = matches.next()?;
    // Only attribute-only PresentationML flags may authorize creation of footer placeholders.
    if matches.next().is_some()
        || node.name != "p:hf"
        || node
            .attrs
            .iter()
            .chain(&root.attrs)
            .find(|(name, _)| name == "xmlns:p")
            .is_none_or(|(_, ns)| ns != NS_P)
        || !node
            .children
            .iter()
            .all(|child| matches!(child, Node::Text(text) if text.trim().is_empty()))
    {
        return None;
    }
    let mut visibility = PlaceholderVisibility::default();
    for (name, value) in &node.attrs {
        let flag = match name.as_str() {
            "dt" => &mut visibility.date,
            "ftr" => &mut visibility.footer,
            "hdr" => &mut visibility.header,
            "sldNum" => &mut visibility.slide_number,
            "xmlns" => continue,
            other if other.starts_with("xmlns:") => continue,
            _ => return None,
        };
        if flag.is_some() {
            return None;
        }
        *flag = Some(match value.trim() {
            "0" | "false" => false,
            "1" | "true" => true,
            _ => return None,
        });
    }
    Some(visibility)
}

pub(super) fn write_background(writer: &mut W, output: &mut Exp, owner: &mut Out, bg: &Background) {
    writer.open0("p:bg");
    match bg {
        Background::Fill { fill } => {
            writer.open0("p:bgPr");
            encode::fill(
                writer,
                output,
                owner,
                match fill {
                    Fill::Background | Fill::Group => &Fill::None,
                    fill => fill,
                },
            );
            writer.empty0("a:effectLst");
            writer.close("p:bgPr");
        }
        Background::Ref { idx, color } => {
            let index = match *idx {
                1..=3 | 1001..=1003 => *idx,
                _ => 1001,
            };
            writer.open("p:bgRef", A::new().a("idx", index));
            encode::color(writer, color);
            writer.close("p:bgRef");
        }
    }
    writer.close("p:bg");
}

fn mapping_attrs(map: &[(String, String)]) -> A {
    let keys = [
        "bg1", "tx1", "bg2", "tx2", "accent1", "accent2", "accent3", "accent4", "accent5",
        "accent6", "hlink", "folHlink",
    ];
    let defaults = [
        "lt1", "dk1", "lt2", "dk2", "accent1", "accent2", "accent3", "accent4", "accent5",
        "accent6", "hlink", "folHlink",
    ];
    keys.into_iter()
        .zip(defaults)
        .fold(A::new(), |attrs, (key, default)| {
            let value = map
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.as_str())
                .filter(|value| {
                    SchemeSlot::from_xml(value)
                        .is_some_and(|slot| SchemeSlot::THEME.contains(&slot))
                })
                .unwrap_or(default);
            attrs.a(key, value)
        })
}

pub(super) fn write_mapping(writer: &mut W, mapping: &[(String, String)]) {
    writer.empty("p:clrMap", mapping_attrs(mapping));
}

pub(super) fn write_override(writer: &mut W, mapping: Option<&[(String, String)]>) {
    writer.open0("p:clrMapOvr");
    if let Some(mapping) = mapping {
        writer.empty("a:overrideClrMapping", mapping_attrs(mapping));
    } else {
        writer.empty0("a:masterClrMapping");
    }
    writer.close("p:clrMapOvr");
}

fn write_visibility(writer: &mut W, visibility: Option<&PlaceholderVisibility>) {
    if let Some(visibility) = visibility {
        let attrs = [
            ("dt", visibility.date),
            ("ftr", visibility.footer),
            ("hdr", visibility.header),
            ("sldNum", visibility.slide_number),
        ];
        writer.empty(
            "p:hf",
            attrs.into_iter().fold(A::new(), |attrs, (key, value)| {
                attrs.o(key, value.map(u8::from))
            }),
        );
    }
}

pub(super) struct Written {
    pub ids: Vec<(u64, String)>,
    pub layouts: HashMap<LayoutId, String>,
}

pub(super) fn write_graph(writer: &mut Exp, presentation: &mut Out) -> Written {
    let mut written = Written {
        ids: Vec::new(),
        layouts: HashMap::new(),
    };
    let mut layout_count = 0;
    // Slide master and layout IDs occupy the top half of the ST_SlideMasterId range.
    let mut next_file_id = 2_147_483_648_u64;
    for (index, master) in writer.p.masters.iter().enumerate() {
        let mut part = Out::new(&format!("ppt/slideMasters/slideMaster{}.xml", index + 1));
        let master_id = next_file_id;
        next_file_id += 1;
        let mut layouts = Vec::new();
        for layout in &master.layouts {
            layout_count += 1;
            let name = format!("ppt/slideLayouts/slideLayout{layout_count}.xml");
            let id = part
                .rels
                .add("slideLayout", &opc::relative(&part.name, &name), false);
            layouts.push((next_file_id, id));
            next_file_id += 1;
            write_layout(writer, &part.name, &name, layout);
            written.layouts.insert(layout.id, name);
        }
        let theme = format!("ppt/theme/theme{}.xml", index + 1);
        write_theme(writer, &mut part, &theme, &master.theme);
        let xml = write_master(writer, &mut part, master, &layouts);
        writer.pkg.xml_part(&part.name, write::CT_MASTER, xml);
        writer.pkg.rels(&part.name, &part.rels);
        let id = presentation.rels.add(
            "slideMaster",
            &opc::relative(&presentation.name, &part.name),
            false,
        );
        written.ids.push((master_id, id));
    }
    written
}

pub(super) fn write_theme(writer: &mut Exp, owner: &mut Out, path: &str, theme: &Theme) {
    owner
        .rels
        .add("theme", &opc::relative(&owner.name, path), false);
    let mut part = Out::new(path);
    let contents = theme_part::theme_xml(writer, &mut part, theme);
    writer.pkg.xml_part(path, write::CT_THEME, contents);
    writer.pkg.rels(path, &part.rels);
}

fn write_layout(writer: &mut Exp, master: &str, path: &str, layout: &Layout) {
    let mut part = Out::new(path);
    part.rels
        .add("slideMaster", &opc::relative(path, master), false);
    let mut xml = W::new();
    write::root_open(
        &mut xml,
        "p:sldLayout",
        A::new()
            .o("showMasterSp", (!layout.show_master_shapes).then_some("0"))
            .a("type", layout.kind.xml())
            .t("preserve", layout.preserve),
    );
    write::csld(
        &mut xml,
        writer,
        &mut part,
        &layout.name,
        layout.background.as_ref(),
        &layout.shapes,
        &[],
    );
    write_override(&mut xml, layout.color_map.as_deref());
    write_visibility(&mut xml, layout.placeholder_visibility.as_ref());
    write::ext_list(
        &mut xml,
        write::ext_children(layout.raw_ext.as_deref()).as_deref(),
    );
    xml.close("p:sldLayout");
    writer.pkg.xml_part(path, write::CT_LAYOUT, xml.finish());
    writer.pkg.rels(path, &part.rels);
}

fn write_master(
    writer: &mut Exp,
    owner: &mut Out,
    master: &Master,
    layouts: &[(u64, String)],
) -> Vec<u8> {
    let mut xml = W::new();
    write::root_open(
        &mut xml,
        "p:sldMaster",
        A::new().t("preserve", master.preserve),
    );
    write::csld(
        &mut xml,
        writer,
        owner,
        &master.name,
        master.background.as_ref(),
        &master.shapes,
        &[],
    );
    write_mapping(&mut xml, &master.color_map);
    xml.open0("p:sldLayoutIdLst");
    for (id, rid) in layouts {
        xml.empty("p:sldLayoutId", A::new().a("id", id).a("r:id", rid));
    }
    xml.close("p:sldLayoutIdLst");
    write_visibility(&mut xml, master.placeholder_visibility.as_ref());
    xml.open0("p:txStyles");
    let (title, body, other) = defaults::master_styles();
    for (tag, value, default) in [
        ("p:titleStyle", &master.title_style, title),
        ("p:bodyStyle", &master.body_style, body),
        ("p:otherStyle", &master.other_style, other),
    ] {
        let style = if value.is_empty() {
            Cow::Owned(default)
        } else {
            Cow::Borrowed(value)
        };
        encode::list_style(&mut xml, writer, owner, tag, &style);
    }
    xml.close("p:txStyles");
    write::ext_list(
        &mut xml,
        write::ext_children(master.raw_ext.as_deref()).as_deref(),
    );
    xml.close("p:sldMaster");
    xml.finish()
}
