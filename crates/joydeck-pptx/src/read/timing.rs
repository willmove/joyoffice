//! Slide transitions (`p:transition`) and animations (`p:timing`).

use joydeck_model::anim::{
    AnimClass, AnimStart, Animation, BuildRole, TextBuild, build_filled, paragraph_build_roles,
};
use joydeck_model::{Shape, ShapeKind, TextBody, find_shape, find_shape_mut};

use super::shapes::IdCtx;
use super::{Imp, Part, dml};
use crate::transition_part::milliseconds as ms_attr;
use crate::xml::El;
use crate::{animation_xml, maps};
use std::collections::HashSet;

/// Volume, mute, Loop until Stopped, Hide While Not Playing and Play Full Screen live in the timing tree
/// (`p:video`/`p:audio` > `p:cMediaNode`), not on the picture: copy them onto the target's `MediaClip`.
pub fn media_settings(timing: &El, ctx: &IdCtx, shapes: &mut [Shape]) {
    for (node, cm) in animation_xml::media(timing) {
        let Some(id) = cm
            .path(&["tgtEl", "spTgt"])
            .and_then(|t| t.u32("spid"))
            .and_then(|f| ctx.lookup(f))
        else {
            continue;
        };
        let Some(ShapeKind::Media(clip)) = find_shape_mut(shapes, id).map(|s| &mut s.kind) else {
            continue;
        };
        if let Some(v) = cm
            .attr("vol")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
        {
            clip.volume = (v / 100_000.0).clamp(0.0, 1.0);
        }
        if cm.bool("mute") == Some(true) {
            clip.volume = 0.0;
        }
        if cm.bool("showWhenStopped") == Some(false) {
            clip.hide_while_not_playing = true;
        }
        if cm.child("cTn").and_then(|c| c.attr("repeatCount")) == Some("indefinite") {
            clip.loop_play = true;
        }
        if node.bool("fullScrn") == Some(true) {
            clip.full_screen = true;
        }
    }
}

/// Flatten the timing tree into the Animation Pane list.
pub fn animations(
    imp: &mut Imp,
    part: &Part,
    ctx: &IdCtx,
    timing: &El,
    shapes: &[Shape],
) -> Vec<Animation> {
    let _ = (imp, part);
    let found = animation_xml::effects(timing);
    // Paragraph builds (`bldP build="p"`) by (spid, grpId).
    let by_para: HashSet<(u32, &str)> = timing
        .child("bldLst")
        .map(|b| {
            b.children_named("bldP")
                .filter(|p| p.attr("build") == Some("p"))
                .filter_map(|p| Some((p.u32("spid")?, p.attr("grpId").unwrap_or("0"))))
                .collect()
        })
        .unwrap_or_default();
    let mut out: Vec<(Animation, u32, &str, bool)> = vec![];
    for f in found {
        let c = f.clock;
        let behavior = animation_xml::inspect(c);
        let class = AnimClass::from_xml(c.attr("presetClass").unwrap_or("entr"));
        let preset = c.u32("presetID").unwrap_or(0);
        let sub = c.u32("presetSubtype").unwrap_or(0);
        let Some(tgt) = behavior.target else { continue };
        let Some(file_id) = tgt.u32("spid") else {
            continue;
        };
        let Some(shape) = ctx.lookup(file_id) else {
            continue;
        };
        let paragraph = tgt.path(&["txEl", "pRg"]).and_then(|p| p.u32("st"));
        let effect =
            maps::effect_for_preset(class, preset).unwrap_or_else(|| maps::fallback_effect(class));
        // PowerPoint writes Float Down as its own preset (47), not as Float with a direction.
        let (effect, option) = match (class, preset) {
            (AnimClass::Entrance, 47) => ("float", "d".to_string()),
            (AnimClass::Exit, 47) => ("floatOut", "d".to_string()),
            _ => (effect, maps::option_from_subtype(effect, sub)),
        };
        let start = match c.attr("nodeType") {
            Some("clickEffect") => AnimStart::OnClick,
            Some("afterEffect") => AnimStart::AfterPrevious,
            _ => AnimStart::WithPrevious,
        };
        let mut a = Animation {
            shape,
            class,
            effect: effect.to_string(),
            option,
            start,
            duration_ms: behavior.duration,
            delay_ms: c
                .path(&["stCondLst", "cond"])
                .and_then(|d| ms_attr(d, "delay"))
                .unwrap_or(0),
            paragraph,
            trigger: f.trigger.and_then(|t| ctx.lookup(t)),
            preset_id: Some(preset),
            preset_subtype: Some(sub),
            // PowerPoint's "Auto-reverse" is `autoRev` on the behaviour nodes (`p:cBhvr/p:cTn`); Color Pulse
            // always writes it there and plays back and forth on its own, so it isn't read as the option.
            auto_reverse: c.bool("autoRev").unwrap_or(false)
                || (effect != "colorPulse" && behavior.auto_reverse),
            // PowerPoint's "Rewind when done playing" is `fill="remove"` on the effect node.
            rewind: c.attr("fill") == Some("remove"),
            smooth_start: c.f64("accel").map(|v| v / 100_000.0).unwrap_or(0.0),
            smooth_end: c.f64("decel").map(|v| v / 100_000.0).unwrap_or(0.0),
            bounce_end: c
                .f64("bounceEnd")
                .map(|v| (v / 100_000.0).clamp(0.0, 1.0))
                .unwrap_or(0.0),
            ..Default::default()
        };
        a.repeat = match c.attr("repeatCount") {
            Some("indefinite") => u32::MAX,
            Some(v) => v
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map(|v| (v / 1000.0).round().clamp(1.0, 10_000.0) as u32)
                .unwrap_or(1),
            None => 1,
        };
        if let Some(m) = behavior.motion {
            a.path = m.attr("path").map(String::from);
        }
        if let Some(r) = behavior.rotation {
            a.amount = r.f64("by").map(|v| v / 60_000.0);
            if a.effect == "spin" && a.amount.is_some_and(|v| v < 0.0) {
                a.option = "counterClockwise".into();
            }
        } else if let Some(s) = behavior.scale.and_then(|s| s.child("by")) {
            a.amount = s.f64("x").map(|v| v / 100_000.0);
        }
        if let Some(clr) = behavior.color.and_then(|x| x.child("to")) {
            a.color = dml::color(clr);
        }
        let grp = c.attr("grpId").unwrap_or("0");
        let build = paragraph.is_some() && by_para.contains(&(file_id, grp));
        out.push((a, file_id, grp, build));
    }
    // A paragraph build's effects, one per paragraph, become one by-paragraph animation only when
    // the timeline plays that back exactly: PowerPoint lets each paragraph keep its own start and
    // effect, and other shapes' effects can sit between them (#844).
    let same_build = |x: &(Animation, u32, &str, bool), y: &(Animation, u32, &str, bool)| {
        x.3 && y.3 && x.1 == y.1 && x.2 == y.2 && x.0.trigger == y.0.trigger
    };
    let mut anims = Vec::with_capacity(out.len());
    for run in out.chunk_by(same_build) {
        let effects: Vec<&Animation> = run.iter().map(|(a, ..)| a).collect();
        let build = run.first().is_some_and(|r| r.3);
        let text = effects
            .first()
            .and_then(|a| find_shape(shapes, a.shape))
            .and_then(|s| s.text.as_ref());
        match (effects.first(), text) {
            (Some(first), Some(text)) if build && plays_as_build(&effects, text) => {
                anims.push(Animation {
                    text_build: TextBuild::ByParagraph,
                    paragraph: None,
                    ..(*first).clone()
                });
            }
            _ => anims.extend(effects.into_iter().cloned()),
        }
    }
    anims
}

/// Are `effects` (paragraph effects of one shape, in file order) what a by-paragraph build of the
/// first one plays: one effect per paragraph the build animates, in paragraph order, later top
/// paragraphs on click (after previous when the build doesn't start on click), deeper ones with
/// their parent, all with the same settings?
fn plays_as_build(effects: &[&Animation], text: &TextBody) -> bool {
    let roles = paragraph_build_roles(text);
    let expected = roles.iter().enumerate().filter(|(i, r)| match r {
        BuildRole::Top => true,
        BuildRole::Member(_) => text.paragraphs.get(*i).is_some_and(build_filled),
        BuildRole::Unowned => false,
    });
    let Some(first) = effects.first() else {
        return false;
    };
    let settings = |a: &Animation| Animation {
        start: AnimStart::OnClick,
        paragraph: None,
        ..a.clone()
    };
    let base = settings(first);
    let mut matched = 0;
    for (k, (i, role)) in expected.enumerate() {
        let Some(a) = effects.get(k) else {
            return false;
        };
        let start_ok = match role {
            BuildRole::Top if k == 0 => true,
            BuildRole::Top if first.start == AnimStart::OnClick => a.start == AnimStart::OnClick,
            BuildRole::Top => a.start == AnimStart::AfterPrevious,
            _ => a.start == AnimStart::WithPrevious,
        };
        if a.paragraph != u32::try_from(i).ok() || !start_ok || settings(a) != base {
            return false;
        }
        matched = k + 1;
    }
    matched == effects.len()
}
