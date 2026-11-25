//! PresentationML transition attributes, sound ownership and compatible markup branches.

use joydeck_model::Transition;

use crate::opc::{NS_MC, NS_P14, NS_P15, NS_P159};
use crate::read::{Imp, Part, dml};
use crate::write::{Exp, Out};
use crate::xml::{A, El, W};

const DAY_MS: u32 = 86_400_000;

pub(super) fn milliseconds(node: &El, attr: &str) -> Option<u32> {
    node.f64(attr)
        .map(|millis| millis.clamp(0.0, f64::from(DAY_MS)) as u32)
}

pub(super) fn read(imp: &mut Imp, part: &Part, element: &El) -> Option<Transition> {
    let mut value = Transition {
        advance_on_click: element.bool("advClick").unwrap_or(true),
        advance_after_ms: milliseconds(element, "advTm"),
        ..Transition::default()
    };
    if let Some(effect) = effect(element) {
        if let Some((kind, option)) = crate::maps::transition_from_xml(effect) {
            value.kind = kind;
            value.option = option;
        } else {
            value.kind = effect.local().into();
            value.raw = Some(part.keep(element));
        }
    } else {
        value.kind = "none".into();
    }
    value.duration_ms = milliseconds(element, "dur").unwrap_or_else(|| match element.attr("spd") {
        Some("med") => 750,
        Some("slow") => joydeck_model::anim::TRANSITIONS
            .iter()
            .find(|record| record.0 == value.kind)
            .map_or(1000, |record| record.3.max(1000)),
        _ => 500,
    });
    if let Some(sound) = element.path(&["sndAc", "stSnd"])
        && let Some(audio) = sound.child("snd")
    {
        value.sound = dml::media_ref(imp, part, audio);
        value.sound_loop = matches!(sound.attr("loop"), Some("1" | "true"));
    }
    value.stop_sound = element.path(&["sndAc", "endSnd"]).is_some();
    let active = value.kind != "none"
        || !value.advance_on_click
        || value.advance_after_ms.is_some()
        || value.sound.is_some()
        || value.stop_sound;
    active.then_some(value)
}

fn effect(node: &El) -> Option<&El> {
    node.elements()
        .find(|child| !matches!(child.local(), "sndAc" | "extLst"))
}

pub(super) fn write(xml: &mut W, export: &mut Exp, owner: &mut Out, transition: &Transition) {
    let (active, namespace) = encode_effect(transition);
    let silence = transition.stop_sound;
    let sound = transition.sound.and_then(|id| {
        let media = export.p.media(id)?;
        let name = if media.name.is_empty() {
            "sound.wav".into()
        } else {
            media.name.clone()
        };
        export.media_rel(owner, id, "audio").map(|rid| (rid, name))
    });
    let spd = match transition.duration_ms {
        0..=600 => "fast",
        601..=900 => "med",
        _ => "slow",
    };
    let fallback = if namespace == "p" {
        active.as_str()
    } else {
        "<p:fade/>"
    };
    let choice = match namespace {
        "p15" => A::new().a("xmlns:p15", NS_P15).a("Requires", "p15"),
        "p159" => A::new().a("xmlns:p159", NS_P159).a("Requires", "p159"),
        _ => A::new().a("Requires", "p14"),
    }
    .a("xmlns:p14", NS_P14);
    let mut choice = Some(choice);
    let has_effect = transition.kind != "none";
    xml.open("mc:AlternateContent", A::new().a("xmlns:mc", NS_MC));
    for modern in [true, false] {
        let branch = if modern { "mc:Choice" } else { "mc:Fallback" };
        if let Some(attributes) = choice.take() {
            xml.open(branch, attributes);
        } else {
            xml.open0(branch);
        }
        xml.open(
            "p:transition",
            A::new()
                .a("spd", spd)
                .o(
                    "p14:dur",
                    (modern && has_effect).then_some(transition.duration_ms.min(60_000)),
                )
                .o("advClick", (!transition.advance_on_click).then_some("0"))
                .o(
                    "advTm",
                    transition.advance_after_ms.map(|ms| ms.min(DAY_MS)),
                ),
        );
        if has_effect {
            xml.raw(if modern { &active } else { fallback });
        }
        if let Some((relationship, name)) = &sound {
            xml.open0("p:sndAc");
            xml.open("p:stSnd", A::new().t("loop", transition.sound_loop));
            xml.empty("p:snd", A::new().a("r:embed", relationship).a("name", name));
            xml.close("p:stSnd");
            xml.close("p:sndAc");
        } else if silence {
            xml.open0("p:sndAc");
            xml.empty0("p:endSnd");
            xml.close("p:sndAc");
        }
        xml.close("p:transition");
        xml.close(branch);
    }
    xml.close("mc:AlternateContent");
}

fn encode_effect(transition: &Transition) -> (String, &'static str) {
    if transition.kind == "none" {
        return (String::new(), "p");
    }
    let mut xml = W::frag();
    if let Some(prefix) =
        crate::maps::transition_to_xml(&mut xml, &transition.kind, &transition.option)
    {
        return (xml.s, prefix);
    }
    if let Some(raw) = &transition.raw
        && let Ok(doc) = crate::xml::parse(raw.as_bytes())
        && let Some(node) = effect(&doc.root)
        && !crate::read::has_rel_refs(node)
    {
        let namespace = match node.prefix() {
            "p" => Some("p"),
            "p14" => Some("p14"),
            "p15" => Some("p15"),
            "p159" => Some("p159"),
            _ => None,
        };
        if let Some(namespace) = namespace {
            let mut node = node.clone();
            let decl = format!("xmlns:{namespace}");
            if let Some((key, value)) = doc.ns_decls().into_iter().find(|(key, _)| key == &decl)
                && !node.attrs.iter().any(|(name, _)| name == &key)
            {
                node.attrs.push((key, value));
            }
            return (node.to_xml(), namespace);
        }
    }
    ("<p:fade/>".into(), "p")
}
