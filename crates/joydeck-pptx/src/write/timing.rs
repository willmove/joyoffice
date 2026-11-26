//! Transitions and the animation timing tree.

use joydeck_model::anim::{
    AnimClass, AnimStart, Animation, BuildRole, TextBuild, build_filled, paragraph_build_roles,
};
use joydeck_model::{Shape, ShapeId, ShapeKind, find_shape};

use super::shapes::IdMap;
use crate::xml::{A, W};

/// One effect to write (an animation, or one paragraph of a by-paragraph build).
struct Eff<'a> {
    a: &'a Animation,
    spid: u32,
    start: AnimStart,
    paragraph: Option<u32>,
    grp: u32,
}

struct Ids(u32);

impl Ids {
    fn next(&mut self) -> u32 {
        self.0 = self.0.saturating_add(1);
        self.0
    }
}

fn tgt(w: &mut W, spid: u32, para: Option<u32>) {
    w.open0("p:tgtEl");
    match para {
        Some(p) => {
            w.open("p:spTgt", A::new().a("spid", spid));
            w.open0("p:txEl");
            w.empty("p:pRg", A::new().a("st", p).a("end", p));
            w.close("p:txEl");
            w.close("p:spTgt");
        }
        None => w.empty("p:spTgt", A::new().a("spid", spid)),
    }
    w.close("p:tgtEl");
}

fn cbhvr(
    w: &mut W,
    ids: &mut Ids,
    e: &Eff,
    dur: u32,
    delay: Option<u32>,
    attrs: &[&str],
    additive: Option<&str>,
) {
    w.open("p:cBhvr", A::new().o("additive", additive));
    let a = A::new()
        .a("id", ids.next())
        .a("dur", dur.max(1))
        .a("fill", "hold");
    match delay {
        Some(d) => {
            w.open("p:cTn", a);
            w.open0("p:stCondLst");
            w.empty("p:cond", A::new().a("delay", d));
            w.close("p:stCondLst");
            w.close("p:cTn");
        }
        None => w.empty("p:cTn", a),
    }
    tgt(w, e.spid, e.paragraph);
    if !attrs.is_empty() {
        w.open0("p:attrNameLst");
        for n in attrs {
            w.elt("p:attrName", n);
        }
        w.close("p:attrNameLst");
    }
    w.close("p:cBhvr");
}

fn set_vis(w: &mut W, ids: &mut Ids, e: &Eff, delay: u32, visible: bool) {
    w.open0("p:set");
    cbhvr(w, ids, e, 1, Some(delay), &["style.visibility"], None);
    w.open0("p:to");
    w.val("p:strVal", if visible { "visible" } else { "hidden" });
    w.close("p:to");
    w.close("p:set");
}

fn anim_effect(w: &mut W, ids: &mut Ids, e: &Eff, dur: u32, filter: &str, out: bool) {
    w.open(
        "p:animEffect",
        A::new()
            .a("transition", if out { "out" } else { "in" })
            .a("filter", filter),
    );
    cbhvr(w, ids, e, dur, None, &[], None);
    w.close("p:animEffect");
}

fn anim_prop(w: &mut W, ids: &mut Ids, e: &Eff, dur: u32, attr: &str, from: &str, to: &str) {
    w.open(
        "p:anim",
        A::new().a("calcmode", "lin").a("valueType", "num"),
    );
    cbhvr(w, ids, e, dur, None, &[attr], Some("base"));
    w.open0("p:tavLst");
    for (tm, v) in [(0, from), (100_000, to)] {
        w.open("p:tav", A::new().a("tm", tm));
        w.open0("p:val");
        w.val("p:strVal", v);
        w.close("p:val");
        w.close("p:tav");
    }
    w.close("p:tavLst");
    w.close("p:anim");
}

fn filter_for(effect: &str, option: &str) -> &'static str {
    match effect {
        "wipe" | "wipeOut" => match option {
            "t" => "wipe(down)",
            "l" => "wipe(right)",
            "r" => "wipe(left)",
            _ => "wipe(up)",
        },
        "split" | "splitOut" => match option {
            "horzOut" => "barn(outHorizontal)",
            "vertIn" => "barn(inVertical)",
            "vertOut" => "barn(outVertical)",
            _ => "barn(inHorizontal)",
        },
        "randomBars" | "randomBarsOut" => {
            if option == "vert" {
                "randombar(vertical)"
            } else {
                "randombar(horizontal)"
            }
        }
        "blinds" | "blindsOut" => {
            if option == "vert" {
                "blinds(vertical)"
            } else {
                "blinds(horizontal)"
            }
        }
        "box" | "boxOut" => {
            if option == "out" {
                "box(out)"
            } else {
                "box(in)"
            }
        }
        "circle" => {
            if option == "out" {
                "circle(out)"
            } else {
                "circle(in)"
            }
        }
        "diamond" | "diamondOut" => {
            if option == "out" {
                "diamond(out)"
            } else {
                "diamond(in)"
            }
        }
        "shape" | "shapeOut" => match option {
            "circleOut" => "circle(out)",
            "boxIn" => "box(in)",
            "boxOut" => "box(out)",
            "diamondIn" => "diamond(in)",
            "diamondOut" => "diamond(out)",
            "plusIn" => "plus(in)",
            "plusOut" => "plus(out)",
            _ => "circle(in)",
        },
        "checkerboard" | "checkerboardOut" => {
            if option == "down" {
                "checkerboard(down)"
            } else {
                "checkerboard(across)"
            }
        }
        "dissolve" | "dissolveOut" => "dissolve",
        "wheel" | "wheelOut" => match option {
            "2" => "wheel(2)",
            "3" => "wheel(3)",
            "4" => "wheel(4)",
            "8" => "wheel(8)",
            _ => "wheel(1)",
        },
        "strips" | "stripsOut" => match option {
            "ru" => "strips(upRight)",
            "ld" => "strips(downLeft)",
            "rd" => "strips(downRight)",
            _ => "strips(upLeft)",
        },
        "peek" | "peekOut" => match option {
            "t" => "slide(fromTop)",
            "l" => "slide(fromLeft)",
            "r" => "slide(fromRight)",
            _ => "slide(fromBottom)",
        },
        _ => "fade",
    }
}

fn fly_from(option: &str) -> (&'static str, &'static str) {
    let x = if option.contains('l') {
        "0-#ppt_w/2"
    } else if option.contains('r') {
        "1+#ppt_w/2"
    } else {
        "#ppt_x"
    };
    let y = if option.contains('t') {
        "0-#ppt_h/2"
    } else if option.contains('b') || option.is_empty() {
        "1+#ppt_h/2"
    } else {
        "#ppt_y"
    };
    (x, y)
}

fn behaviors(w: &mut W, ids: &mut Ids, effect: &Eff) {
    let animation = effect.a;
    let duration = animation.duration_ms.clamp(1, 3_600_000);
    match animation.class {
        AnimClass::Entrance | AnimClass::Exit => visibility(w, ids, effect, duration),
        AnimClass::Emphasis => emphasis(w, ids, effect, duration),
        AnimClass::Path => motion(w, ids, effect, duration),
        AnimClass::Media => media_command(w, ids, effect),
    }
}

fn visibility(w: &mut W, ids: &mut Ids, effect: &Eff, duration: u32) {
    let name = effect.a.effect.as_str();
    let option = effect.a.option.as_str();
    let outward = effect.a.class == AnimClass::Exit;
    let immediate = matches!(name, "appear" | "disappear");
    if !outward {
        set_vis(w, ids, effect, 0, true);
    }
    let mut animate = |attr: &str, displaced: &str, resting: &str| {
        let (from, to) = if outward {
            (resting, displaced)
        } else {
            (displaced, resting)
        };
        anim_prop(w, ids, effect, duration, attr, from, to);
    };
    let fade = match name {
        "appear" | "disappear" => None,
        "fly" | "flyOut" => {
            let (horizontal, vertical) = fly_from(option);
            animate("ppt_x", horizontal, "#ppt_x");
            animate("ppt_y", vertical, "#ppt_y");
            None
        }
        "zoom" | "grow" | "expand" | "spinner" | "zoomOut" | "shrinkTurn" | "spinnerOut"
        | "contract" => {
            animate("ppt_w", "0", "#ppt_w");
            animate("ppt_h", "0", "#ppt_h");
            Some("fade")
        }
        "float" | "rise" | "floatOut" | "sinkDown" => {
            let below = if outward {
                name == "sinkDown" || option == "d"
            } else {
                option != "d"
            };
            animate(
                "ppt_y",
                if below { "#ppt_y+.1" } else { "#ppt_y-.1" },
                "#ppt_y",
            );
            Some("fade")
        }
        _ => Some(filter_for(name, option)),
    };
    if let Some(filter) = fade {
        anim_effect(w, ids, effect, duration, filter, outward);
    }
    if outward {
        set_vis(
            w,
            ids,
            effect,
            if immediate { 0 } else { duration - 1 },
            false,
        );
    }
}

fn emphasis(w: &mut W, ids: &mut Ids, effect: &Eff, duration: u32) {
    let animation = effect.a;
    let name = animation.effect.as_str();
    match name {
        "spin" | "teeter" => {
            let amount = animation
                .amount
                .unwrap_or(if name == "teeter" { 4.0 } else { 360.0 });
            let signed = if animation.option == "counterClockwise" {
                -amount.abs()
            } else {
                amount
            };
            w.open(
                "p:animRot",
                A::new().a("by", units(signed, -36_000.0, 36_000.0, 60_000.0)),
            );
            cbhvr(w, ids, effect, duration, None, &["r"], None);
            w.close("p:animRot");
        }
        "transparency" => {
            let opacity = (1.0 - animation.amount.unwrap_or(0.5)).clamp(0.0, 1.0);
            w.open0("p:set");
            cbhvr(w, ids, effect, duration, None, &["style.opacity"], None);
            w.open0("p:to");
            w.val("p:strVal", format!("{opacity:.2}"));
            w.close("p:to");
            w.close("p:set");
        }
        "colorPulse" => {
            let color = animation.color.clone().unwrap_or_else(|| {
                joydeck_model::ColorRef::rgb(joydeck_color::Rgba::rgb(192, 0, 0))
            });
            rgb_behavior(w, ids, effect, duration, "style.color", &color, Some(true));
            rgb_behavior(w, ids, effect, duration, "fillcolor", &color, Some(false));
        }
        "darken" | "desaturate" => {
            let (saturation, luminance) = if name == "darken" {
                (0, -25_098)
            } else {
                (-70_588, 0)
            };
            for property in ["style.color", "fillcolor", "stroke.color"] {
                w.open("p:animClr", A::new().a("clrSpc", "hsl").a("dir", "cw"));
                cbhvr(w, ids, effect, duration, None, &[property], None);
                w.open0("p:by");
                w.empty(
                    "p:hsl",
                    A::new().a("h", 0).a("s", saturation).a("l", luminance),
                );
                w.close("p:by");
                w.close("p:animClr");
            }
        }
        "fillColor" | "objectColor" | "fontColor" | "lineColor" | "complementaryColor"
        | "lighten" => {
            let property = match name {
                "fontColor" => "style.color",
                "lineColor" => "stroke.color",
                _ => "fillcolor",
            };
            let color = animation.color.clone().unwrap_or_else(|| {
                joydeck_model::ColorRef::scheme(joydeck_color::SchemeSlot::Accent2)
            });
            rgb_behavior(w, ids, effect, duration, property, &color, None);
        }
        _ => {
            let scale = animation
                .amount
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(if name == "growShrink" { 1.5 } else { 1.1 });
            let axes = match animation.option.as_str() {
                "horz" => (scale, 1.0),
                "vert" => (1.0, scale),
                _ => (scale, scale),
            };
            w.open0("p:animScale");
            cbhvr(w, ids, effect, duration, None, &[], None);
            w.empty(
                "p:by",
                A::new()
                    .a("x", units(axes.0, 0.01, 100.0, 100_000.0))
                    .a("y", units(axes.1, 0.01, 100.0, 100_000.0)),
            );
            w.close("p:animScale");
        }
    }
}

fn units(value: f64, min: f64, max: f64, scale: f64) -> i64 {
    (value.clamp(min, max) * scale).round() as i64
}

fn rgb_behavior(
    w: &mut W,
    ids: &mut Ids,
    effect: &Eff,
    duration: u32,
    property: &str,
    color: &joydeck_model::ColorRef,
    pulse: Option<bool>,
) {
    w.open("p:animClr", A::new().a("clrSpc", "rgb").a("dir", "cw"));
    if let Some(child_style) = pulse {
        w.open(
            "p:cBhvr",
            A::new().o("override", child_style.then_some("childStyle")),
        );
        w.empty(
            "p:cTn",
            A::new()
                .a("id", ids.next())
                .a("dur", duration)
                .a("autoRev", 1)
                .a("fill", "remove"),
        );
        tgt(w, effect.spid, effect.paragraph);
        w.open0("p:attrNameLst");
        w.elt("p:attrName", property);
        w.close("p:attrNameLst");
        w.close("p:cBhvr");
    } else {
        cbhvr(w, ids, effect, duration, None, &[property], None);
    }
    w.open0("p:to");
    super::dml::color(w, color);
    w.close("p:to");
    w.close("p:animClr");
}

fn motion(w: &mut W, ids: &mut Ids, effect: &Eff, duration: u32) {
    let path = effect
        .a
        .path
        .as_deref()
        .filter(|path| {
            !path.trim().is_empty()
                && path
                    .bytes()
                    .all(|b| b.is_ascii() && !matches!(b, b'"' | b'<' | b'&'))
        })
        .unwrap_or(match effect.a.option.as_str() {
            "up" => "M 0 0 L 0 -0.25 E",
            "right" => "M 0 0 L 0.25 0 E",
            "left" => "M 0 0 L -0.25 0 E",
            _ => "M 0 0 L 0 0.25 E",
        });
    w.open(
        "p:animMotion",
        A::new()
            .a("origin", "layout")
            .a("path", path)
            .a("pathEditMode", "relative")
            .a("ptsTypes", ""),
    );
    cbhvr(w, ids, effect, duration, None, &["ppt_x", "ppt_y"], None);
    w.close("p:animMotion");
}

fn media_command(w: &mut W, ids: &mut Ids, effect: &Eff) {
    let command = match effect.a.effect.as_str() {
        "pause" => "togglePause",
        "stop" => "stop",
        _ => "playFrom(0.0)",
    };
    w.open("p:cmd", A::new().a("type", "call").a("cmd", command));
    cbhvr(w, ids, effect, 1, None, &[], None);
    w.close("p:cmd");
}

fn effect_par(w: &mut W, ids: &mut Ids, e: &Eff) {
    let a = e.a;
    let node = match e.start {
        AnimStart::OnClick => "clickEffect",
        AnimStart::WithPrevious => "withEffect",
        AnimStart::AfterPrevious => "afterEffect",
    };
    let preset = crate::maps::preset_for(a);
    let sub = if preset == 47 {
        0
    } else {
        crate::maps::subtype_from_option(&a.effect, &a.option)
            .or(a.preset_subtype.filter(|_| a.preset_id == Some(preset)))
            .unwrap_or(0)
    };
    let repeat = match a.repeat {
        u32::MAX => Some("indefinite".to_string()),
        0 | 1 => None,
        n => Some((n.min(10_000) * 1000).to_string()),
    };
    let accel =
        (a.smooth_start > 0.0).then(|| (a.smooth_start.clamp(0.0, 1.0) * 100_000.0).round() as i64);
    let decel =
        (a.smooth_end > 0.0).then(|| (a.smooth_end.clamp(0.0, 1.0) * 100_000.0).round() as i64);
    // `bounceEnd` is a positive percentage (100000 = 100%), same scale as accel/decel. The model stores 0–1.
    let bounce = (a.bounce_end.is_finite() && a.bounce_end > 0.0)
        .then(|| (a.bounce_end.clamp(0.0, 1.0) * 100_000.0).round() as i64)
        .filter(|n| *n > 0);
    w.open0("p:par");
    w.open(
        "p:cTn",
        A::new()
            .a("id", ids.next())
            .a("presetID", preset)
            .a("presetClass", a.class.xml())
            .a("presetSubtype", sub)
            .o("repeatCount", repeat)
            .o("accel", accel)
            .o("decel", decel)
            .o("bounceEnd", bounce)
            .t("autoRev", a.auto_reverse)
            .a("fill", if a.rewind { "remove" } else { "hold" })
            .a("grpId", e.grp)
            .a("nodeType", node),
    );
    w.open0("p:stCondLst");
    w.empty("p:cond", A::new().a("delay", a.delay_ms.min(3_600_000)));
    w.close("p:stCondLst");
    w.open0("p:childTnLst");
    behaviors(w, ids, e);
    w.close("p:childTnLst");
    w.close("p:cTn");
    w.close("p:par");
}

fn end_time(e: &Eff) -> u32 {
    let rep = match e.a.repeat {
        0 | 1 => 1,
        u32::MAX => 1,
        n => n.min(100),
    };
    e.a.delay_ms
        .saturating_add(e.a.duration_ms.saturating_mul(rep))
        .min(3_600_000)
}

/// click groups → after-groups → effects
type Groups<'a> = Vec<(bool, Vec<Vec<Eff<'a>>>)>;

fn group(effects: Vec<Eff<'_>>) -> Groups<'_> {
    let mut activations: Groups<'_> = Vec::new();
    for effect in effects {
        let automatic = effect.start != AnimStart::OnClick;
        if automatic && let Some((_, batches)) = activations.last_mut() {
            if effect.start == AnimStart::WithPrevious
                && let Some(batch) = batches.last_mut()
            {
                batch.push(effect);
            } else {
                batches.push(vec![effect]);
            }
        } else {
            activations.push((automatic, vec![vec![effect]]));
        }
    }
    activations
}

fn click_pars(w: &mut W, ids: &mut Ids, clicks: &Groups, interactive: bool) {
    for (ci, (auto, afters)) in clicks.iter().enumerate() {
        w.open0("p:par");
        w.open("p:cTn", A::new().a("id", ids.next()).a("fill", "hold"));
        w.open0("p:stCondLst");
        if interactive && ci == 0 {
            w.empty("p:cond", A::new().a("delay", 0));
        } else {
            w.empty("p:cond", A::new().a("delay", "indefinite"));
            if *auto {
                w.open("p:cond", A::new().a("evt", "onBegin").a("delay", 0));
                w.val("p:tn", 2);
                w.close("p:cond");
            }
        }
        w.close("p:stCondLst");
        w.open0("p:childTnLst");
        let mut t = 0u32;
        for g in afters {
            w.open0("p:par");
            w.open("p:cTn", A::new().a("id", ids.next()).a("fill", "hold"));
            w.open0("p:stCondLst");
            w.empty("p:cond", A::new().a("delay", t));
            w.close("p:stCondLst");
            w.open0("p:childTnLst");
            for effect in g {
                effect_par(w, ids, effect);
            }
            w.close("p:childTnLst");
            w.close("p:cTn");
            w.close("p:par");
            t = t.saturating_add(g.iter().map(end_time).max().unwrap_or(0));
        }
        w.close("p:childTnLst");
        w.close("p:cTn");
        w.close("p:par");
    }
}

/// Write `p:timing` for a slide's animations.
pub fn timing(w: &mut W, anims: &[Animation], shapes: &[Shape], idmap: &IdMap) {
    let mut grp_count: std::collections::HashMap<u32, u32> = Default::default();
    let mut main: Vec<Eff> = vec![];
    let mut triggered: Vec<(u32, Vec<Eff>)> = vec![];
    let mut bld: Vec<(u32, u32, bool)> = vec![];
    // The last paragraph effect written: (shape, class, trigger, grpId).
    let mut prev_para: Option<(ShapeId, AnimClass, Option<ShapeId>, u32)> = None;
    for a in anims.iter().take(crate::animation_xml::MAX_EFFECTS) {
        let Some(spid) = idmap.get(a.shape) else {
            log::warn!("pptx: animation on missing shape {} skipped", a.shape);
            continue;
        };
        let Some(shape) = find_shape(shapes, a.shape) else {
            continue;
        };
        let text_sp = matches!(shape.kind, ShapeKind::Shape) && shape.text.is_some();
        let is_text_sp = matches!(shape.kind, ShapeKind::Shape)
            && shape.text.as_ref().is_some_and(|t| !t.is_empty());
        let text = shape
            .text
            .as_ref()
            .filter(|_| a.text_build == TextBuild::ByParagraph && is_text_sp);
        let roles = text.map(paragraph_build_roles).unwrap_or_default();
        let by_para = roles.contains(&BuildRole::Top);
        // PowerPoint keeps a shape's consecutive paragraph effects in one build (`grpId`) marked
        // `build="p"`; the reader turns them back into a by-paragraph build only if that plays them
        // the same way.
        let para_effect = !by_para && text_sp && a.paragraph.is_some();
        let joins = para_effect
            && prev_para.is_some_and(|(s, c, t, _)| s == a.shape && c == a.class && t == a.trigger);
        let grp = match prev_para {
            Some((.., g)) if joins => g,
            _ => {
                let c = grp_count.entry(spid).or_insert(0);
                let g = *c;
                *c += 1;
                g
            }
        };
        prev_para = para_effect.then_some((a.shape, a.class, a.trigger, grp));
        let mut effs = vec![];
        if by_para {
            // One effect per paragraph the build animates on its own, deeper ones with their
            // parent; indices in the file count empty paragraphs too.
            let mut tops = 0;
            for (i, role) in roles.iter().enumerate() {
                let start = match role {
                    BuildRole::Top => {
                        tops += 1;
                        if tops == 1 {
                            a.start
                        } else if a.start == AnimStart::OnClick {
                            AnimStart::OnClick
                        } else {
                            AnimStart::AfterPrevious
                        }
                    }
                    BuildRole::Member(_)
                        if text
                            .and_then(|t| t.paragraphs.get(i))
                            .is_some_and(build_filled) =>
                    {
                        AnimStart::WithPrevious
                    }
                    _ => continue,
                };
                let Ok(pi) = u32::try_from(i) else { break };
                effs.push(Eff {
                    a,
                    spid,
                    start,
                    paragraph: Some(pi),
                    grp,
                });
            }
        } else {
            effs.push(Eff {
                a,
                spid,
                start: a.start,
                paragraph: a.paragraph,
                grp,
            });
        }
        if text_sp && !joins {
            bld.push((spid, grp, by_para || para_effect));
        }
        match a.trigger.and_then(|t| idmap.get(t)) {
            Some(t) => match triggered.iter_mut().find(|(s, _)| *s == t) {
                Some((_, v)) => v.extend(effs),
                None => triggered.push((t, effs)),
            },
            None => main.extend(effs),
        }
    }
    if main.is_empty() && triggered.is_empty() {
        return;
    }
    let mut ids = Ids(0);
    w.open0("p:timing");
    w.open0("p:tnLst");
    w.open0("p:par");
    w.open(
        "p:cTn",
        A::new()
            .a("id", ids.next())
            .a("dur", "indefinite")
            .a("restart", "never")
            .a("nodeType", "tmRoot"),
    );
    w.open0("p:childTnLst");
    if !main.is_empty() {
        write_sequence(w, &mut ids, main, None);
    }
    for (target, effects) in triggered {
        write_sequence(w, &mut ids, effects, Some(target));
    }
    w.close("p:childTnLst");
    w.close("p:cTn");
    w.close("p:par");
    w.close("p:tnLst");
    if !bld.is_empty() {
        w.open0("p:bldLst");
        for (spid, grp, build_p) in bld {
            w.empty(
                "p:bldP",
                A::new()
                    .a("spid", spid)
                    .a("grpId", grp)
                    .o("build", build_p.then_some("p"))
                    .t("animBg", !build_p),
            );
        }
        w.close("p:bldLst");
    }
    w.close("p:timing");
}

fn write_sequence(w: &mut W, ids: &mut Ids, mut effects: Vec<Eff<'_>>, target: Option<u32>) {
    if target.is_some()
        && let Some(first) = effects.first_mut()
    {
        first.start = AnimStart::OnClick;
    }
    let clicks = group(effects);
    w.open("p:seq", A::new().a("concurrent", 1).a("nextAc", "seek"));
    let attributes = A::new().a("id", ids.next());
    if let Some(shape) = target {
        w.open(
            "p:cTn",
            attributes
                .a("restart", "whenNotActive")
                .a("fill", "hold")
                .a("evtFilter", "cancelBubble")
                .a("nodeType", "interactiveSeq"),
        );
        sequence_condition(w, "p:stCondLst", "onClick", Some(shape));
        w.open("p:endSync", A::new().a("evt", "end").a("delay", 0));
        w.val("p:rtn", "all");
        w.close("p:endSync");
    } else {
        w.open(
            "p:cTn",
            attributes.a("dur", "indefinite").a("nodeType", "mainSeq"),
        );
    }
    w.open0("p:childTnLst");
    click_pars(w, ids, &clicks, target.is_some());
    w.close("p:childTnLst");
    w.close("p:cTn");
    if let Some(shape) = target {
        sequence_condition(w, "p:nextCondLst", "onClick", Some(shape));
    } else {
        sequence_condition(w, "p:prevCondLst", "onPrev", None);
        sequence_condition(w, "p:nextCondLst", "onNext", None);
    }
    w.close("p:seq");
}

fn sequence_condition(w: &mut W, list: &str, event: &str, shape: Option<u32>) {
    w.open0(list);
    w.open("p:cond", A::new().a("evt", event).a("delay", 0));
    w.open0("p:tgtEl");
    match shape {
        Some(id) => w.empty("p:spTgt", A::new().a("spid", id)),
        None => w.empty0("p:sldTgt"),
    }
    w.close("p:tgtEl");
    w.close("p:cond");
    w.close(list);
}
