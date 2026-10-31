//! Public PresentationML subset. Unsupported trees remain in the source package, with a diagnostic.
use crate::{
    document::{Result, Slide},
    motion::{Effect, Timing},
};
use roxmltree::Node;
use std::collections::BTreeMap;
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
fn node<'a, 'b>(n: Node<'a, 'b>, tag: &str) -> Option<Node<'a, 'b>> {
    n.descendants().find(|n| n.has_tag_name((P, tag)))
}
fn cond(id: usize, dur: u64) -> String {
    format!(r#"<p:cTn id="{id}" dur="{dur}" fill="hold"/>"#)
}
fn target(spid: usize, paragraph: Option<usize>) -> String {
    format!(
        r#"<p:tgtEl><p:spTgt spid="{spid}">{}</p:spTgt></p:tgtEl>"#,
        paragraph.map_or(String::new(), |p| format!(
            r#"<p:txEl><p:pRg st="{p}" end="{p}"/></p:txEl>"#
        ))
    )
}
pub fn write(s: &Slide, t: &Timing, width: f32, height: f32) -> Result<String> {
    let transition = if t.transition == "none" && t.advance_ms.is_none() {
        String::new()
    } else {
        let effect=match t.transition.as_str(){"none"=>String::new(),"fade"=>"<p:fade/>".into(),"push"=>"<p:push dir=\"l\"/>".into(),"wipe"=>"<p:wipe dir=\"r\"/>".into(),"morph"=>r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:p159="http://schemas.microsoft.com/office/powerpoint/2015/09/main" Requires="p159"><p159:morph option="byObject"/></mc:Choice><mc:Fallback><p:fade/></mc:Fallback></mc:AlternateContent>"#.into(),_=>return Err("无效转场".into())};
        format!(
            r#"<p:transition xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" spd="med" p14:dur="{}" advClick="1"{}>{effect}</p:transition>"#,
            t.duration_ms,
            t.advance_ms
                .map_or(String::new(), |n| format!(" advTm=\"{n}\""))
        )
    };
    if t.effects.is_empty() {
        return Ok(transition);
    }
    let mut groups: Vec<(usize, String)> = vec![];
    let times = crate::motion::schedule(t);
    let mut build = String::new();
    let mut built = std::collections::BTreeSet::new();
    for (i, (e, time)) in t.effects.iter().zip(times).enumerate() {
        let Some(spid) = s
            .objects
            .iter()
            .position(|o| o.id == e.target)
            .map(|i| i + 2)
        else {
            return Err("动画对象不存在".into());
        };
        let n = 10 + i * 8;
        let target = target(spid, e.paragraph);
        let behaviour = |id, dur| format!("<p:cBhvr>{}{}{}</p:cBhvr>", cond(id, dur), target, "");
        let visible = format!(
            r#"<p:set><p:cBhvr>{}{target}<p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="visible"/></p:to></p:set>"#,
            cond(n + 1, 1)
        );
        let effect = match e.kind.as_str() {
            "appear" => visible.clone(),
            "fade" | "fade-out" => format!(
                r#"{visible}<p:animEffect transition="{}" filter="fade">{}</p:animEffect>"#,
                if e.kind == "fade-out" { "out" } else { "in" },
                behaviour(n + 2, e.duration_ms)
            ),
            "fly" | "move" => format!(
                r#"{visible}<p:animMotion origin="layout" pathEditMode="relative" path="M {} {} L {} {} E">{}</p:animMotion>"#,
                if e.kind == "fly" { e.dx / width } else { 0. },
                if e.kind == "fly" { e.dy / height } else { 0. },
                if e.kind == "fly" { 0. } else { e.dx / width },
                if e.kind == "fly" { 0. } else { e.dy / height },
                behaviour(n + 2, e.duration_ms)
            ),
            "zoom" => format!(
                r#"{visible}<p:animScale>{}<p:from x="10000" y="10000"/><p:to x="100000" y="100000"/></p:animScale>"#,
                behaviour(n + 2, e.duration_ms)
            ),
            "spin" => format!(
                r#"<p:animRot by="{}">{}</p:animRot>"#,
                (e.amount * 60000.) as i64,
                behaviour(n + 2, e.duration_ms)
            ),
            _ => return Err("未支持的公开动画".into()),
        };
        let kind = match e.trigger.as_str() {
            "click" => "clickEffect",
            "with" => "withEffect",
            _ => "afterEffect",
        };
        let preset = if e.kind == "fade" || e.kind == "fade-out" {
            10
        } else if e.kind == "appear" {
            1
        } else {
            2
        };
        let body = format!(
            r#"<p:par><p:cTn id="{n}" presetID="{preset}" presetClass="{}" nodeType="{kind}" dur="{}" fill="{}" repeatCount="{}" autoRev="{}"><p:stCondLst><p:cond delay="{}"/></p:stCondLst><p:childTnLst>{effect}</p:childTnLst></p:cTn></p:par>"#,
            if e.kind == "fade-out" {
                "exit"
            } else if e.kind == "spin" || e.kind == "move" {
                "emph"
            } else {
                "entr"
            },
            e.duration_ms,
            if e.rewind { "remove" } else { "hold" },
            e.repeats * 1000,
            u8::from(e.auto_reverse),
            time.start
        );
        if let Some((_, group)) = groups.last_mut().filter(|(step, _)| *step == time.step) {
            *group += &body;
        } else {
            groups.push((time.step, body));
        }
        if built.insert(spid) {
            build += &format!(r#"<p:bldP spid="{spid}" grpId="0"/>"#);
        }
    }
    let groups=groups.into_iter().enumerate().map(|(i,(step,body))|format!(r#"<p:par><p:cTn id="{}" fill="hold"><p:stCondLst><p:cond delay="{}"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="{}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>{body}</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#,3000+i*2,if step==0{"0"}else{"indefinite"},3001+i*2)).collect::<String>();
    Ok(transition
        + &format!(
            r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>{groups}</p:childTnLst></p:cTn><p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst><p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst><p:bldLst>{build}</p:bldLst></p:timing>"#
        ))
}
pub fn read(
    root: Node<'_, '_>,
    ids: &BTreeMap<String, String>,
    width: f32,
    height: f32,
) -> Result<(Timing, bool)> {
    let mut t = Timing::default();
    let mut unsupported = false;
    if let Some(tr) = root.children().find(|n| n.has_tag_name((P, "transition"))) {
        t.advance_ms = tr
            .attribute("advTm")
            .map(|v| v.parse().map_err(|_| "转场计时无效"))
            .transpose()?;
        t.duration_ms = tr
            .attribute((
                "http://schemas.microsoft.com/office/powerpoint/2010/main",
                "dur",
            ))
            .and_then(|v| v.parse().ok())
            .unwrap_or(600);
        t.transition = if tr.descendants().any(|n| n.tag_name().name() == "morph") {
            "morph"
        } else if node(tr, "fade").is_some() {
            "fade"
        } else if node(tr, "push").is_some() {
            "push"
        } else if node(tr, "wipe").is_some() {
            "wipe"
        } else {
            if tr.children().any(|n| n.is_element()) {
                unsupported = true;
            }
            "none"
        }
        .into();
    }
    for n in root.descendants().filter(|n| {
        n.has_tag_name((P, "cTn"))
            && n.attribute("nodeType")
                .is_some_and(|s| ["clickEffect", "withEffect", "afterEffect"].contains(&s))
    }) {
        if t.effects.len() >= 256 {
            return Err("动画数超过256".into());
        }
        let sp = node(n, "spTgt").and_then(|n| n.attribute("spid"));
        let Some(target) = sp.and_then(|id| ids.get(id)) else {
            unsupported = true;
            continue;
        };
        let fade = node(n, "animEffect").filter(|n| n.attribute("filter") == Some("fade"));
        let kind = if let Some(f) = fade {
            if f.attribute("transition") == Some("out") {
                "fade-out"
            } else {
                "fade"
            }
        } else if node(n, "animRot").is_some() {
            "spin"
        } else if node(n, "animScale").is_some() {
            "zoom"
        } else if node(n, "animMotion").is_some() {
            if n.attribute("presetClass") == Some("entr") {
                "fly"
            } else {
                "move"
            }
        } else if node(n, "set").is_some() {
            "appear"
        } else {
            unsupported = true;
            continue;
        };
        let duration = fade
            .and_then(|f| node(f, "cTn"))
            .or_else(|| node(n, "animRot").and_then(|f| node(f, "cTn")))
            .or_else(|| node(n, "animScale").and_then(|f| node(f, "cTn")))
            .or_else(|| node(n, "animMotion").and_then(|f| node(f, "cTn")))
            .and_then(|n| n.attribute("dur"))
            .and_then(|v| v.parse().ok())
            .or_else(|| n.attribute("dur").and_then(|v| v.parse().ok()))
            .unwrap_or(1);
        let trigger = match n.attribute("nodeType") {
            Some("withEffect") => "with",
            Some("afterEffect") => "after",
            _ => "click",
        };
        let start = node(n, "cond")
            .and_then(|n| n.attribute("delay"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let repeats = n
            .attribute("repeatCount")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1000)
            / 1000;
        let mut e = Effect {
            id: format!("pptx-effect-{}", t.effects.len()),
            target: target.clone(),
            kind: kind.into(),
            trigger: trigger.into(),
            duration_ms: duration,
            delay_ms: start,
            repeats,
            auto_reverse: n.attribute("autoRev") == Some("1"),
            rewind: n.attribute("fill") == Some("remove"),
            paragraph: node(n, "pRg")
                .and_then(|n| n.attribute("st"))
                .and_then(|s| s.parse().ok()),
            ..Default::default()
        };
        if let Some(rot) = node(n, "animRot") {
            e.amount = rot
                .attribute("by")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(21600000.)
                / 60000.;
        }
        if let Some(motion) = node(n, "animMotion") {
            let path = motion
                .attribute("path")
                .unwrap_or("")
                .split_whitespace()
                .collect::<Vec<_>>();
            if path.len() != 7 || path[0] != "M" || path[3] != "L" || path[6] != "E" {
                unsupported = true;
                continue;
            }
            let parsed = [path[1], path[2], path[4], path[5]]
                .iter()
                .map(|v| v.parse::<f32>())
                .collect::<std::result::Result<Vec<_>, _>>();
            let Ok(v) = parsed else {
                unsupported = true;
                continue;
            };
            if motion.attribute("origin") != Some("layout")
                || (kind == "fly" && (v[2] != 0. || v[3] != 0.))
                || (kind == "move" && (v[0] != 0. || v[1] != 0.))
            {
                unsupported = true;
                continue;
            }
            let at = if kind == "fly" { 0 } else { 2 };
            e.dx = v[at] * width;
            e.dy = v[at + 1] * height;
        }
        // Public delays are absolute within a click group; convert to the evaluator's relative start rules.
        if trigger != "click" {
            let times = crate::motion::schedule(&t);
            if let Some(previous) = times.last() {
                e.delay_ms = start.saturating_sub(if trigger == "with" {
                    previous.start
                } else {
                    previous.end
                });
            }
        }
        t.effects.push(e);
    }
    if node(root, "timing").is_some() && t.effects.is_empty() {
        unsupported = true;
    }
    Ok((t, unsupported))
}
