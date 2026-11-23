//! Slide transition dispatcher routing every PresentationML transition kind to
//! its first-party JoyOffice layer generator.
use crate::transition::{Layer, Source};
use crate::transition_surface::{smoothstep, unit_hash};

const RANDOM_KINDS: &[&str] = &[
    "fade",
    "push",
    "wipe",
    "split",
    "reveal",
    "randomBar",
    "shape",
    "uncover",
    "cover",
    "fallOver",
    "drape",
    "curtains",
    "wind",
    "prestige",
    "fracture",
    "crush",
    "peelOff",
    "pageCurlDouble",
    "airplane",
    "origami",
    "dissolve",
    "checker",
    "blinds",
    "clock",
    "ripple",
    "honeycomb",
    "glitter",
    "vortex",
    "shred",
    "switch",
    "flip",
    "gallery",
    "cube",
    "doors",
    "box",
    "comb",
    "zoom",
    "pan",
    "ferris",
    "conveyor",
    "rotate",
    "window",
    "orbit",
    "flythrough",
];

fn choose_random(seed_text: &str) -> (&'static str, &'static str) {
    let mut state: u64 = 0xCBF2_9CE4_8422_2325;
    for b in seed_text.bytes() {
        state ^= u64::from(b);
        state = state.wrapping_mul(0x0100_0000_01B3);
    }
    let idx = (unit_hash(state) * RANDOM_KINDS.len() as f64) as usize;
    let kind = RANDOM_KINDS
        .get(idx.min(RANDOM_KINDS.len() - 1))
        .copied()
        .unwrap_or("fade");
    let option = joydeck_model::anim::TRANSITIONS
        .iter()
        .find(|entry| entry.0 == kind)
        .and_then(|entry| entry.4.first().copied())
        .unwrap_or("");
    (kind, option)
}

pub(crate) fn layers(kind: &str, option: &str, t: f64, aspect: f64, depth: u32) -> Vec<Layer> {
    if let Some(layers) = crate::slide_motion::layers(kind, option, t) {
        return layers;
    }
    match kind {
        "wipe" | "split" => crate::transition_mask::layers(kind == "split", option, t),
        "reveal" => crate::transition_subtle::reveal(option, t),
        "flash" => crate::transition_subtle::flash(t),
        "randomBar" => crate::transition_pattern::bars(option == "vert", t),
        "shape" => crate::transition_pattern::shape(option, t, aspect),
        "dissolve" => crate::transition_pattern::dissolve(t),
        "clock" => crate::transition_pattern::clock(option, t),
        "checker" => crate::transition_panels::checker(option == "vert", t, aspect),
        "blinds" => crate::transition_panels::blinds(option == "vert", t),
        "ripple" => crate::transition_panels::ripple(option, t, aspect),
        "comb" => crate::transition_panels::comb(option == "vert", t),
        "fallOver" => crate::transition_fabric::fall_over(option == "r", t),
        "drape" => crate::transition_fabric::drape(option == "r", t),
        "curtains" => crate::transition_fabric::curtains(t),
        "wind" => crate::transition_fabric::wind(option == "r", t),
        "prestige" => crate::transition_fabric::prestige(t),
        "fracture" => crate::transition_paper::fracture(t, aspect),
        "crush" => crate::transition_paper::crush(t),
        "peelOff" => crate::transition_paper::peel(option == "r", t, aspect),
        "pageCurlDouble" => crate::transition_paper::turn(option, t),
        "airplane" => crate::transition_fold::airplane(option == "r", t),
        "origami" => crate::transition_fold::origami(option == "r", t),
        "cube" | "box" | "rotate" | "orbit" => {
            crate::transition_prism::frame(kind, option, t, aspect)
        }
        "honeycomb" => crate::transition_particles::honeycomb(t, aspect),
        "glitter" => crate::transition_particles::glitter(option, t, aspect),
        "vortex" => crate::transition_particles::vortex(option, t, aspect),
        "shred" => crate::transition_particles::shred(option, t, aspect),
        "switch" => crate::transition_stage::switch(option == "r", t, aspect),
        "flip" => crate::transition_stage::flip(option == "r", t, aspect),
        "gallery" => crate::transition_stage::gallery(option == "r", t, aspect),
        "doors" => crate::transition_stage::doors(option == "horz", t, aspect),
        "zoom" => crate::transition_stage::zoom(option, t, aspect),
        "pan" => crate::transition_dynamic::pan(option, t),
        "ferris" => crate::transition_dynamic::ferris(option == "r", t, aspect),
        "conveyor" => crate::transition_dynamic::conveyor(option == "r", t, aspect),
        "window" => crate::transition_dynamic::window(option == "horz", t, aspect),
        "flythrough" => crate::transition_dynamic::flythrough(option, t),
        "random" if depth == 0 => {
            let (chosen, opt) = choose_random(option);
            layers(chosen, opt, t, aspect, depth + 1)
        }
        _ => vec![
            Layer::full(Source::Old, 1.),
            Layer::full(Source::New, smoothstep(t)),
        ],
    }
}
