//! DrawingML theme parts and their three style lists.

use joydeck_color::SchemeSlot;
use joydeck_model::style::{ColorBase, ColorRef, Effects, Fill};
use joydeck_model::theme::{FontScheme, FontSet, FormatScheme, ScriptFont, Theme};

use crate::opc::{NS_A, NS_R};
use crate::read::{Imp, Part, dml as input};
use crate::write::{Exp, Out, dml as output};
use crate::xml::{A, El, W};

const STYLE_LIMIT: usize = 16;
const FONT_LIMIT: usize = 256;

fn typefaces(node: Option<&El>) -> FontSet {
    let Some(node) = node else {
        return FontSet::default();
    };
    let face = |kind| {
        node.child(kind)
            .and_then(|font| font.attr("typeface"))
            .unwrap_or("")
            .into()
    };
    FontSet {
        latin: face("latin"),
        ea: face("ea"),
        cs: face("cs"),
        scripts: node
            .children_named("font")
            .filter_map(|font| {
                Some(ScriptFont {
                    script: font.attr("script")?.into(),
                    typeface: font.attr("typeface").unwrap_or("").into(),
                })
            })
            .take(FONT_LIMIT)
            .collect(),
    }
}

fn name(node: &El) -> String {
    node.attr("name").unwrap_or_default().into()
}

pub(super) fn read_theme(imp: &mut Imp, part: &Part, root: &El) -> Theme {
    let mut theme = Theme {
        name: root.attr("name").unwrap_or("Theme").into(),
        ..Theme::default()
    };
    let Some(elements) = root.child("themeElements") else {
        return theme;
    };
    if let Some(scheme) = elements.child("clrScheme") {
        theme.colors.name = name(scheme);
        for slot in SchemeSlot::THEME {
            if let Some(mut color) = scheme.child(slot.xml_name()).and_then(input::color)
                && !matches!(color.base, ColorBase::Scheme { .. })
            {
                // Keep the stored swatch; shape references carry their own transforms.
                color.mods.clear();
                theme.colors.set(slot, color.resolve(&theme.colors, None));
            }
        }
    }
    if let Some(fonts) = elements.child("fontScheme") {
        theme.fonts = FontScheme {
            name: name(fonts),
            major: typefaces(fonts.child("majorFont")),
            minor: typefaces(fonts.child("minorFont")),
        };
    }
    if let Some(styles) = elements.child("fmtScheme") {
        theme.format = read_matrix(imp, part, styles);
    }
    let extras: String = root
        .elements()
        .filter(|node| {
            matches!(node.local(), "objectDefaults" | "extraClrSchemeLst")
                && !crate::read::has_rel_refs(node)
        })
        .map(|node| part.keep(node))
        .collect();
    theme.raw_extra = (!extras.is_empty()).then_some(extras);
    theme
}

fn read_matrix(imp: &mut Imp, part: &Part, styles: &El) -> FormatScheme {
    let defaults = joydeck_model::theme::default_format();
    let fills = |imp: &mut Imp, which, backup: &[Fill]| {
        let mut list: Vec<_> = styles
            .child(which)
            .into_iter()
            .flat_map(El::elements)
            .take(STYLE_LIMIT)
            .filter_map(|node| input::fill_el(imp, part, node))
            .collect();
        list.extend(backup.iter().take(3).skip(list.len()).cloned());
        list
    };
    let shape_fills = fills(imp, "fillStyleLst", &defaults.fills);
    let mut lines: Vec<_> = styles
        .child("lnStyleLst")
        .into_iter()
        .flat_map(|list| list.children_named("ln"))
        .take(STYLE_LIMIT)
        .map(|node| input::line(imp, part, node))
        .collect();
    lines.extend(defaults.lines.iter().take(3).skip(lines.len()).cloned());
    let mut effects: Vec<_> = styles
        .child("effectStyleLst")
        .into_iter()
        .flat_map(|list| list.children_named("effectStyle"))
        .take(STYLE_LIMIT)
        .map(|node| read_effect(part, node))
        .collect();
    effects.resize_with(effects.len().max(3), Effects::default);
    let backgrounds = fills(imp, "bgFillStyleLst", &defaults.bg_fills);
    FormatScheme {
        name: name(styles),
        fills: shape_fills,
        lines,
        effects,
        bg_fills: backgrounds,
    }
}

fn read_effect(part: &Part, node: &El) -> Effects {
    let mut effect = node
        .child("effectLst")
        .map(input::effects)
        .unwrap_or_default();
    effect.bevel = node
        .path(&["sp3d", "bevelT"])
        .map(|bevel| bevel.attr("prst").unwrap_or("circle").into());
    let extras: String = node
        .elements()
        .filter(|node| matches!(node.local(), "scene3d" | "sp3d"))
        .map(|node| part.keep(node))
        .collect();
    effect.raw3d = (!extras.is_empty()).then_some(extras);
    effect
}

fn defaulted<'a>(name: &'a str, fallback: &'a str) -> &'a str {
    if name.is_empty() { fallback } else { name }
}

pub(super) fn theme_xml(export: &mut Exp, out: &mut Out, theme: &Theme) -> Vec<u8> {
    let mut xml = W::new();
    xml.open(
        "a:theme",
        A::new()
            .a("xmlns:a", NS_A)
            .a("xmlns:r", NS_R)
            .a("name", defaulted(&theme.name, "Theme")),
    );
    xml.open0("a:themeElements");
    xml.open(
        "a:clrScheme",
        A::new().a("name", defaulted(&theme.colors.name, &theme.name)),
    );
    for slot in SchemeSlot::THEME {
        let element = format!("a:{}", slot.xml_name());
        xml.open0(&element);
        xml.val("a:srgbClr", theme.colors.get(slot).hex());
        xml.close(&element);
    }
    xml.close("a:clrScheme");
    xml.open(
        "a:fontScheme",
        A::new().a("name", defaulted(&theme.fonts.name, &theme.name)),
    );
    for (element, set) in [
        ("a:majorFont", &theme.fonts.major),
        ("a:minorFont", &theme.fonts.minor),
    ] {
        xml.open0(element);
        for (tag, face) in [
            ("a:latin", defaulted(&set.latin, "Arial")),
            ("a:ea", &set.ea),
            ("a:cs", &set.cs),
        ] {
            xml.empty(tag, A::new().a("typeface", face));
        }
        for font in &set.scripts {
            xml.empty(
                "a:font",
                A::new()
                    .a("script", &font.script)
                    .a("typeface", &font.typeface),
            );
        }
        xml.close(element);
    }
    xml.close("a:fontScheme");
    write_matrix(&mut xml, export, out, &theme.format, &theme.name);
    xml.close("a:themeElements");
    if let Some(extras) = &theme.raw_extra {
        if !extras.contains("objectDefaults") {
            xml.empty0("a:objectDefaults");
        }
        xml.raw(extras);
    } else {
        xml.empty0("a:objectDefaults");
        xml.empty0("a:extraClrSchemeLst");
    }
    xml.close("a:theme");
    xml.finish()
}

fn write_matrix(xml: &mut W, export: &mut Exp, out: &mut Out, matrix: &FormatScheme, name: &str) {
    let defaults = joydeck_model::theme::default_format();
    let placeholder = Fill::solid(ColorRef::scheme(SchemeSlot::PhClr));
    xml.open(
        "a:fmtScheme",
        A::new().a("name", defaulted(&matrix.name, name)),
    );
    write_fills(
        xml,
        export,
        out,
        "a:fillStyleLst",
        &matrix.fills,
        &defaults.fills,
        &placeholder,
    );
    xml.open0("a:lnStyleLst");
    for index in 0..matrix.lines.len().max(3) {
        let mut line = matrix
            .lines
            .get(index)
            .or_else(|| defaults.lines.get(index))
            .cloned()
            .unwrap_or_default();
        if matches!(line.fill, None | Some(Fill::Background | Fill::Group)) {
            line.fill = Some(placeholder.clone());
        }
        output::line(xml, export, out, "a:ln", &line);
    }
    xml.close("a:lnStyleLst");
    xml.open0("a:effectStyleLst");
    let empty = Effects::default();
    for index in 0..matrix.effects.len().max(3) {
        let effect = matrix.effects.get(index).unwrap_or(&empty);
        xml.open0("a:effectStyle");
        output::effects(xml, effect);
        if let Some(fragment) = effect.raw3d.as_deref() {
            xml.raw(fragment);
        } else if let Some(fragment) = effect.bevel.as_deref().and_then(output::bevel_sp3d) {
            xml.raw(&fragment);
        }
        xml.close("a:effectStyle");
    }
    xml.close("a:effectStyleLst");
    write_fills(
        xml,
        export,
        out,
        "a:bgFillStyleLst",
        &matrix.bg_fills,
        &defaults.bg_fills,
        &placeholder,
    );
    xml.close("a:fmtScheme");
}

fn write_fills(
    xml: &mut W,
    export: &mut Exp,
    out: &mut Out,
    tag: &str,
    fills: &[Fill],
    defaults: &[Fill],
    placeholder: &Fill,
) {
    xml.open0(tag);
    for index in 0..fills.len().max(3) {
        let fill = fills
            .get(index)
            .or_else(|| defaults.get(index))
            .unwrap_or(placeholder);
        output::fill(
            xml,
            export,
            out,
            if matches!(fill, Fill::Background | Fill::Group) {
                placeholder
            } else {
                fill
            },
        );
    }
    xml.close(tag);
}
