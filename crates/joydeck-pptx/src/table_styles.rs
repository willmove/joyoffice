//! DrawingML table style identifiers and style-list serialization.

use std::collections::HashSet;

use joydeck_model::table::{TableStyleDef, table_styles};

use crate::opc::NS_A;
use crate::xml::{A, W};

const DEFAULT: &str = "{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}";
const BUILTINS: &[(&str, &str)] = &[
    ("none", "{2D5ABB26-0587-4C30-8999-92F81FD0307C}"),
    ("grid", "{5940675A-B579-460E-94D1-54222C63F5DA}"),
    ("medium2-accent1", DEFAULT),
    ("medium2-tx1", "{073A0DAA-6AF3-43AB-8588-CEC1D06C72B9}"),
    ("medium2-accent2", "{21E4AEA4-8DFA-4A89-87EB-49C32662AFE8}"),
    ("medium2-accent3", "{F5AB1C69-6EDB-4FF4-983F-18BD219EF322}"),
    ("medium2-accent4", "{00A15C55-8517-42AA-B614-E9B94910E393}"),
    ("medium2-accent5", "{7DF18680-E054-41AD-8BC1-D1AEF772440D}"),
    ("medium2-accent6", "{93296810-A885-4BE3-A3E7-6D5BEEA58F35}"),
];

/// Existing documents and field bindings depend on these deterministic identifiers.
pub fn guid(text: &str) -> String {
    let hash = |salt: u64| {
        text.bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64 ^ salt, |acc, octet| {
                (acc ^ u64::from(octet)).wrapping_mul(0x0100_0000_01b3)
            })
    };
    let hi = hash(0x5163_7261_6674);
    let lo = hash(0x7461_626c_6573);
    format!(
        "{{{:08X}-{:04X}-4{:03X}-8{:03X}-{:012X}}}",
        hi >> 32,
        (hi >> 16) & 0xffff,
        hi & 0xfff,
        (lo >> 48) & 0xfff,
        lo & 0xffff_ffff_ffff
    )
}

fn catalog_guid(name: &str) -> String {
    guid(&format!("joydeck-table-style:{name}"))
}

pub fn is_own_guid(value: &str) -> bool {
    table_styles()
        .iter()
        .any(|(name, _)| catalog_guid(name).eq_ignore_ascii_case(value))
}

pub fn style_from_guid(value: &str) -> String {
    if value.is_empty() {
        return "none".into();
    }
    if let Some((name, _)) = BUILTINS
        .iter()
        .find(|(_, guid)| guid.eq_ignore_ascii_case(value))
    {
        return (*name).into();
    }
    table_styles()
        .into_iter()
        .find(|(name, _)| catalog_guid(name).eq_ignore_ascii_case(value))
        .map_or_else(|| value.into(), |(name, _)| name)
}

pub fn guid_for_style(name: &str) -> (String, bool) {
    if name.starts_with('{') {
        return (name.into(), false);
    }
    if let Some((_, value)) = BUILTINS[..3].iter().find(|(id, _)| *id == name) {
        return ((*value).into(), false);
    }
    if table_styles().iter().any(|(id, _)| id == name) {
        (catalog_guid(name), true)
    } else {
        (DEFAULT.into(), false)
    }
}

#[derive(Clone, Copy)]
enum Tone {
    Plain,
    Tint(u32),
    Shade(u32),
}

#[derive(Clone, Copy)]
struct Color<'a>(&'a str, Tone);

#[derive(Default, Clone, Copy)]
struct Area<'a> {
    text: Option<&'a str>,
    bold: bool,
    fill: Option<Color<'a>>,
    border: Option<(&'a str, u32)>,
}

fn style_areas<'a>(kind: &str, accent: &'a str) -> [Option<Area<'a>>; 7] {
    let plain = |slot| Color(slot, Tone::Plain);
    let tint = |value| Color(accent, Tone::Tint(value));
    let shade = |value| Color(accent, Tone::Shade(value));
    let text = |color| Area {
        text: Some(color),
        ..Area::default()
    };
    let bold = Area {
        bold: true,
        ..Area::default()
    };
    let filled = |color| Area {
        fill: Some(color),
        ..Area::default()
    };
    let white_bold = Area {
        text: Some("lt1"),
        fill: Some(plain(accent)),
        ..bold
    };
    // wholeTbl, band1H, band1V, lastCol, firstCol, lastRow, firstRow.
    match kind {
        "grid" => [
            Some(Area {
                border: Some(("dk1", 9525)),
                ..text("dk1")
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        "light1" => [
            Some(text("dk1")),
            Some(filled(tint(20000))),
            Some(filled(tint(20000))),
            Some(bold),
            Some(bold),
            Some(bold),
            Some(Area {
                border: Some((accent, 12700)),
                ..bold
            }),
        ],
        "light2" => [
            Some(Area {
                border: Some((accent, 12700)),
                ..text("dk1")
            }),
            None,
            None,
            None,
            Some(bold),
            Some(bold),
            Some(Area {
                border: Some((accent, 12700)),
                ..white_bold
            }),
        ],
        "dark1" => [
            Some(Area {
                fill: Some(shade(75000)),
                border: Some(("lt1", 12700)),
                ..text("lt1")
            }),
            Some(filled(shade(60000))),
            Some(filled(shade(60000))),
            None,
            Some(bold),
            Some(bold),
            Some(Area {
                fill: Some(plain("dk1")),
                ..white_bold
            }),
        ],
        "medium4" => [
            Some(Area {
                fill: Some(tint(20000)),
                border: Some((accent, 9525)),
                ..text("dk1")
            }),
            Some(filled(tint(40000))),
            Some(filled(tint(40000))),
            Some(bold),
            Some(bold),
            Some(bold),
            Some(Area {
                fill: Some(tint(40000)),
                ..bold
            }),
        ],
        _ => [
            Some(Area {
                fill: Some(tint(20000)),
                border: Some(("lt1", 12700)),
                ..text("dk1")
            }),
            Some(filled(tint(40000))),
            Some(filled(tint(40000))),
            Some(white_bold),
            Some(white_bold),
            Some(white_bold),
            Some(white_bold),
        ],
    }
}

fn area(xml: &mut W, tag: &str, props: Area<'_>) {
    xml.open0(tag);
    if props.bold || props.text.is_some() {
        xml.open("a:tcTxStyle", A::new().o("b", props.bold.then_some("on")));
        xml.open("a:fontRef", A::new().a("idx", "minor"));
        xml.val("a:prstClr", "black");
        xml.close("a:fontRef");
        xml.val("a:schemeClr", props.text.unwrap_or("dk1"));
        xml.close("a:tcTxStyle");
    }
    xml.open0("a:tcStyle");
    xml.open0("a:tcBdr");
    if let Some((slot, width)) = props.border {
        for side in [
            "a:left",
            "a:right",
            "a:top",
            "a:bottom",
            "a:insideH",
            "a:insideV",
        ] {
            xml.open0(side);
            xml.open("a:ln", A::new().a("w", width).a("cmpd", "sng"));
            color(xml, Color(slot, Tone::Plain));
            xml.close("a:ln");
            xml.close(side);
        }
    }
    xml.close("a:tcBdr");
    if let Some(fill) = props.fill {
        xml.open0("a:fill");
        color(xml, fill);
        xml.close("a:fill");
    }
    xml.close("a:tcStyle");
    xml.close(tag);
}

fn color(xml: &mut W, Color(slot, tone): Color<'_>) {
    xml.open0("a:solidFill");
    if let Tone::Plain = tone {
        xml.val("a:schemeClr", slot);
    } else {
        xml.open("a:schemeClr", A::new().a("val", slot));
        match tone {
            Tone::Plain => {}
            Tone::Tint(amount) => xml.val("a:tint", amount),
            Tone::Shade(amount) => xml.val("a:shade", amount),
        }
        xml.close("a:schemeClr");
    }
    xml.close("a:solidFill");
}

fn definition(xml: &mut W, id: &str, name: &str) {
    let (kind, selected) = id.split_once('-').unwrap_or((id, "accent1"));
    let accent = match selected {
        "tx1" => "dk1",
        slot if slot.starts_with("accent") => slot,
        _ => "accent1",
    };
    xml.open(
        "a:tblStyle",
        A::new()
            .a("styleId", catalog_guid(id))
            .a("styleName", format!("DeckCraft {name}")),
    );
    for (tag, props) in [
        "a:wholeTbl",
        "a:band1H",
        "a:band1V",
        "a:lastCol",
        "a:firstCol",
        "a:lastRow",
        "a:firstRow",
    ]
    .into_iter()
    .zip(style_areas(kind, accent))
    {
        if let Some(props) = props {
            area(xml, tag, props);
        }
    }
    xml.close("a:tblStyle");
}

pub fn table_styles_xml(used: &[String], definitions: &[TableStyleDef]) -> Vec<u8> {
    let mut xml = W::new();
    xml.open(
        "a:tblStyleLst",
        A::new().a("xmlns:a", NS_A).a("def", DEFAULT),
    );
    let names = table_styles();
    let mut written = HashSet::new();
    let parse = |definition: &TableStyleDef| {
        crate::xml::parse(definition.raw.as_bytes()).is_ok_and(|doc| doc.root.is("tblStyle"))
    };
    for id in used {
        if !written.insert(id.to_ascii_uppercase()) {
            continue;
        }
        if let Some(definition) = definitions
            .iter()
            .find(|definition| definition.id.eq_ignore_ascii_case(id) && parse(definition))
        {
            xml.raw(&definition.raw);
        } else if guid_for_style(id).1
            && let Some((_, name)) = names.iter().find(|(name, _)| name == id)
        {
            definition(&mut xml, id, name);
        }
    }
    for definition in definitions {
        if parse(definition) && written.insert(definition.id.to_ascii_uppercase()) {
            xml.raw(&definition.raw);
        }
    }
    xml.close("a:tblStyleLst");
    xml.finish()
}

#[cfg(test)]
#[path = "table_style_tests.rs"]
mod tests;
