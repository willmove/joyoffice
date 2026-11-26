//! MS-ODRAWXML model3d graphic frame payloads and embedded-part bindings.

use joydeck_geom::Xfrm;
use joydeck_model::style::Effects;
use joydeck_model::{MediaId, Shape};

use crate::opc::{NS_R, RT_MODEL3D, Rel, Rels};
use crate::read::{Imp, Part, dml::media_ref};
use crate::write::dml::emu_pos;
use crate::xml::{A, El, W};

pub(crate) const NS: &str = "http://schemas.microsoft.com/office/drawing/2017/model3d";

pub(crate) fn read_surface(part: &Part, properties: &El, effects: &mut Option<Effects>) {
    let scene = properties.child("scene3d");
    let surface = properties.child("sp3d");
    if scene.is_none() && surface.is_none() {
        return;
    }
    let effect = effects.get_or_insert_with(Effects::default);
    effect.raw3d = Some(
        scene
            .into_iter()
            .chain(surface)
            .map(|node| part.keep_inner(node))
            .collect(),
    );
    effect.bevel = surface
        .and_then(|node| node.child("bevelT"))
        .map(|bevel| bevel.attr("prst").unwrap_or("circle").to_owned());
}

pub(crate) fn write_surface(xml: &mut W, effects: &Effects) {
    if let Some(raw) = &effects.raw3d {
        xml.raw(raw);
        return;
    }
    let Some(bevel) = effects.bevel.as_deref().filter(|name| {
        !name.is_empty()
            && name.len() <= 32
            && name.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }) else {
        return;
    };
    xml.open0("a:sp3d");
    xml.empty("a:bevelT", A::new().a("prst", bevel));
    xml.close("a:sp3d");
}

pub(crate) fn payload(frame: &El) -> Option<&El> {
    let data = frame.path(&["graphic", "graphicData"])?;
    (data.attr("uri") == Some(NS)).then_some(())?;
    data.child("model3d")
}

pub(crate) fn relationship<'a>(shape: &El, rels: &'a Rels) -> Option<&'a Rel> {
    let id = if shape.is("contentPart") {
        shape.attr("r:id")?
    } else {
        payload(shape)?.attr("r:embed")?
    };
    rels.get(id).filter(|edge| {
        !edge.external
            && (edge.ty == RT_MODEL3D || edge.target.to_ascii_lowercase().ends_with(".glb"))
    })
}

pub(crate) fn recognizes_choice(choice: &El, rels: &Rels) -> bool {
    choice
        .elements()
        .any(|shape| relationship(shape, rels).is_some())
}

pub(crate) fn load_payload(
    imp: &mut Imp,
    part: &Part,
    frame: &El,
) -> Option<(MediaId, Option<MediaId>, Option<String>)> {
    let edge = relationship(frame, &part.rels)?;
    let media = imp.media_for(&edge.target)?;
    let Some(model) = payload(frame) else {
        return Some((media, None, None));
    };
    let poster = model
        .path(&["raster", "blip"])
        .and_then(|blip| media_ref(imp, part, blip));
    let mut model = model.clone();
    detach_relationships(&mut model, &part.ns, &part.inner_ns);
    Some((media, poster, Some(part.keep_inner(&model))))
}

fn detach_relationships(element: &mut El, outer: &[(String, String)], inner: &[(String, String)]) {
    let mut removed = false;
    element.attrs.retain(|(name, _)| {
        let Some((prefix, _)) = name.split_once(':') else {
            return true;
        };
        let binding = format!("xmlns:{prefix}");
        let bound = prefix == "r"
            || outer
                .iter()
                .chain(inner)
                .any(|(name, ns)| name == &binding && ns == NS_R);
        removed |= bound;
        !bound
    });
    if removed && !matches!(element.local(), "model3d" | "blip") {
        log::warn!(
            "pptx: unmodelled relationship on embedded 3-D {} will require source preservation",
            element.local()
        );
    }
    for child in element.elements_mut() {
        detach_relationships(child, outer, inner);
    }
}

pub(crate) fn write_payload(xml: &mut W, shape: &Shape, model_id: &str, image_id: Option<&str>) {
    xml.open("a:graphicData", A::new().a("uri", NS));
    let source = shape
        .raw_ext
        .as_deref()
        .and_then(|value| crate::xml::parse(value.as_bytes()).ok())
        .map(|document| document.root)
        .filter(|element| element.is("model3d") && element.child("camera").is_some());
    if let Some(mut model) = source {
        model.set_attr("r:embed", model_id);
        if let Some(raster) = model.elements_mut().find(|child| child.is("raster"))
            && let Some(blip) = raster.elements_mut().find(|child| child.is("blip"))
            && let Some(image_id) = image_id
        {
            blip.set_attr("r:embed", image_id);
        }
        if image_id.is_none() {
            model
                .children
                .retain(|node| !matches!(node, crate::xml::Node::El(child) if child.is("raster")));
        }
        xml.raw(&model.to_xml());
    } else {
        write_default(xml, shape.xfrm.unwrap_or_default(), model_id, image_id);
    }
    xml.close("a:graphicData");
}

fn write_default(xml: &mut W, frame: Xfrm, model_id: &str, image_id: Option<&str>) {
    xml.open("am3d:model3d", A::new().a("r:embed", model_id));
    xml.open0("am3d:spPr");
    crate::drawing_geometry::write_transform(
        xml,
        "a:xfrm",
        &Xfrm::new(0.0, 0.0, frame.w, frame.h),
        None,
    );
    crate::drawing_geometry::geometry(xml, &joydeck_model::Geom::preset("rect"));
    xml.close("am3d:spPr");
    xml.open0("am3d:camera");
    xml.empty("am3d:pos", A::new().a("x", 0).a("y", 0).a("z", 108_000_000));
    xml.empty(
        "am3d:up",
        A::new().a("dx", 0).a("dy", 36_000_000).a("dz", 0),
    );
    xml.empty("am3d:lookAt", A::new().a("x", 0).a("y", 0).a("z", 0));
    xml.empty("am3d:perspective", A::new().a("fov", 2_700_000));
    xml.close("am3d:camera");
    xml.empty0("am3d:trans");
    if let Some(image_id) = image_id {
        xml.open(
            "am3d:raster",
            A::new()
                .a("rName", "JoyDeck")
                .a("rVer", env!("CARGO_PKG_VERSION")),
        );
        xml.empty("am3d:blip", A::new().a("r:embed", image_id));
        xml.close("am3d:raster");
    }
    xml.empty(
        "am3d:objViewport",
        A::new().a("viewportSz", emu_pos(frame.w.max(frame.h)).max(1)),
    );
    xml.close("am3d:model3d");
}
