//! ECMA-376 DrawingML presets, adjustment guides and shape coordinate boxes.

use joydeck_geom::{Xfrm, emu_to_pt, preset};
use joydeck_model::Geom;

use crate::write::dml::{ang, emu, emu_pos};
use crate::xml::{A, El, W};

pub(crate) fn adjustments(node: &El, defaults: &[f64], limit: usize) -> Vec<f64> {
    let mut overrides = [None; 16];
    let Some(list) = node.child("avLst") else {
        return Vec::new();
    };
    for guide in list.children_named("gd").take(overrides.len()) {
        let name = guide.attr("name").unwrap_or_default();
        let slot = if name == "adj" {
            Some(0)
        } else {
            name.strip_prefix("adj")
                .and_then(|suffix| suffix.parse::<usize>().ok())
                .and_then(|number| number.checked_sub(1))
        };
        let Some(slot) = slot.filter(|slot| *slot < limit) else {
            continue;
        };
        let Some(value) = guide.attr("fmla").and_then(adjustment_literal) else {
            continue;
        };
        overrides[slot] = Some(value);
    }
    let Some(last) = overrides.iter().rposition(Option::is_some) else {
        return Vec::new();
    };
    (0..=last)
        .map(|i| {
            overrides[i]
                .or_else(|| defaults.get(i).copied())
                .unwrap_or(0.0)
        })
        .collect()
}

fn adjustment_literal(formula: &str) -> Option<f64> {
    let mut words = formula.split_ascii_whitespace();
    if words.next()? != "val" {
        return None;
    }
    let number = words.next()?.parse::<f64>().ok()?;
    (number.is_finite() && words.next().is_none()).then_some(number)
}

pub(crate) fn preset_geometry(node: &El) -> Geom {
    let name = node.attr("prst").unwrap_or("rect");
    let defaults = preset::adjustment_defaults(name);
    let mut adj = adjustments(node, defaults, 16);
    if !adj.is_empty() && adj.len() < defaults.len() {
        adj.extend_from_slice(&defaults[adj.len()..]);
    }
    Geom::Preset {
        name: name.to_owned(),
        adj,
    }
}

pub(crate) fn read_geometry(properties: &El, bounds: Option<Xfrm>) -> Option<Geom> {
    if let Some(node) = properties.child("prstGeom") {
        return Some(preset_geometry(node));
    }
    let geometry = properties.child("custGeom")?;
    let (width, height) = bounds.map_or((21600.0, 21600.0), |bounds| {
        (
            joydeck_geom::pt_to_emu(bounds.w) as f64,
            joydeck_geom::pt_to_emu(bounds.h) as f64,
        )
    });
    let paths = crate::custgeom::read_cust_geom(geometry, width, height);
    (!paths.is_empty()).then_some(Geom::Custom { paths })
}

pub(crate) fn geometry(xml: &mut W, geometry: &Geom) {
    if let Geom::Custom { paths } = geometry
        && !paths.is_empty()
    {
        crate::custgeom::write_cust_geom(xml, paths);
        return;
    }
    let (name, adjustments) = match geometry {
        Geom::Preset { name, adj } => (name.as_str(), adj.as_slice()),
        Geom::Custom { .. } => ("rect", &[][..]),
    };
    let name = if name.is_empty()
        || name == "textBox"
        || !name.bytes().all(|ch| ch.is_ascii_alphanumeric())
    {
        "rect"
    } else {
        name
    };
    xml.open("a:prstGeom", A::new().a("prst", name));
    if adjustments.is_empty() {
        xml.empty0("a:avLst");
    } else {
        write_guides(xml, name, adjustments);
    }
    xml.close("a:prstGeom");
}

fn write_guides(xml: &mut W, name: &str, values: &[f64]) {
    let defaults = preset::adjustment_defaults(name);
    let count = values.len().min(16).max(defaults.len());
    let numbered = matches!(
        name,
        "bentConnector3" | "curvedConnector3" | "frame" | "mathPlus" | "mathMinus" | "mathMultiply"
    ) || if defaults.is_empty() {
        count != 1
    } else {
        defaults.len() != 1
    };
    let limit = if name == "borderCallout1" {
        f64::from(i32::MAX)
    } else {
        2.0e9
    };
    xml.open0("a:avLst");
    for i in 0..count {
        let key = if i == 0 && !numbered {
            "adj".to_owned()
        } else {
            format!("adj{}", i + 1)
        };
        let value = values
            .get(i)
            .or_else(|| defaults.get(i))
            .copied()
            .unwrap_or(0.0);
        let value = if value.is_finite() {
            value.round().clamp(-limit, limit) as i64
        } else {
            0
        };
        guide(xml, &key, value);
    }
    for &(key, value) in match name {
        "star5" | "pentagon" => &[("hf", 105_146), ("vf", 110_557)][..],
        "star7" | "heptagon" => &[("hf", 102_572), ("vf", 105_210)],
        "star6" => &[("hf", 115_470)],
        "star10" => &[("hf", 105_146)],
        "hexagon" => &[("vf", 115_470)],
        "decagon" => &[("vf", 105_146)],
        _ => &[],
    } {
        guide(xml, key, value);
    }
    xml.close("a:avLst");
}

fn guide(xml: &mut W, key: &str, value: i64) {
    xml.empty(
        "a:gd",
        A::new().a("name", key).a("fmla", format!("val {value}")),
    );
}

pub(crate) fn read_transform(node: &El) -> (Xfrm, Option<Xfrm>) {
    let mut transform = box_from(node.child("off"), node.child("ext"));
    transform.rot = node.f64("rot").unwrap_or(0.0) / 60_000.0;
    transform.flip_h = node.bool("flipH").unwrap_or_default();
    transform.flip_v = node.bool("flipV").unwrap_or_default();
    let child = node
        .child("chOff")
        .zip(node.child("chExt"))
        .map(|(origin, size)| box_from(Some(origin), Some(size)));
    (transform, child)
}

fn box_from(origin: Option<&El>, size: Option<&El>) -> Xfrm {
    let coordinate =
        |node: Option<&El>, key| emu_to_pt(node.and_then(|node| node.i64(key)).unwrap_or_default());
    Xfrm::new(
        coordinate(origin, "x"),
        coordinate(origin, "y"),
        coordinate(size, "cx").max(0.0),
        coordinate(size, "cy").max(0.0),
    )
}

pub(crate) fn write_transform(xml: &mut W, tag: &str, transform: &Xfrm, child: Option<&Xfrm>) {
    let transform = finite_box(transform);
    let rotation = ang(transform.rot);
    xml.open(
        tag,
        A::new()
            .o("rot", (rotation != 0).then_some(rotation))
            .t("flipH", transform.flip_h)
            .t("flipV", transform.flip_v),
    );
    write_box(xml, "a:off", "a:ext", transform);
    if let Some(child) = child {
        write_box(xml, "a:chOff", "a:chExt", finite_box(child));
    }
    xml.close(tag);
}

fn finite_box(transform: &Xfrm) -> Xfrm {
    if transform.is_finite() {
        *transform
    } else {
        Xfrm::default()
    }
}

fn write_box(xml: &mut W, off: &str, ext: &str, shape: Xfrm) {
    xml.empty(off, A::new().a("x", emu(shape.x)).a("y", emu(shape.y)));
    xml.empty(
        ext,
        A::new().a("cx", emu_pos(shape.w)).a("cy", emu_pos(shape.h)),
    );
}
