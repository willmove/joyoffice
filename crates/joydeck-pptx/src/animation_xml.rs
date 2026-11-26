//! PresentationML timing-node paths, targets and behavior properties.

use crate::transition_part::milliseconds;
use crate::xml::El;

pub(crate) const MAX_EFFECTS: usize = 10_000;
const MAX_DEPTH: usize = 64;

pub(crate) struct EffectNode<'a> {
    pub(crate) clock: &'a El,
    pub(crate) trigger: Option<u32>,
}

pub(crate) fn effects(tree: &El) -> Vec<EffectNode<'_>> {
    let Some(root) = tree.child("tnLst") else {
        return Vec::new();
    };
    let mut effects = Vec::new();
    descendants(root, |node, trigger| {
        if node.is("cTn") && node.has_attr("presetClass") {
            effects.push(EffectNode {
                clock: node,
                trigger,
            });
            if effects.len() == MAX_EFFECTS {
                return Visit::Stop;
            }
            return Visit::Skip;
        }
        let trigger = if node.is("seq")
            && let Some(clock) = node
                .child("cTn")
                .filter(|clock| clock.attr("nodeType") == Some("interactiveSeq"))
        {
            trigger_target(clock.child("stCondLst"))
                .or_else(|| trigger_target(node.child("nextCondLst")))
        } else {
            trigger
        };
        Visit::Enter(trigger)
    });
    effects
}

fn trigger_target(conditions: Option<&El>) -> Option<u32> {
    conditions?
        .children_named("cond")
        .find_map(|condition| condition.path(&["tgtEl", "spTgt"])?.u32("spid"))
}

enum Visit {
    Enter(Option<u32>),
    Skip,
    Stop,
}

fn descendants<'a>(root: &'a El, mut visit: impl FnMut(&'a El, Option<u32>) -> Visit) {
    let mut stack = vec![(root.elements(), None)];
    while let Some((nodes, trigger)) = stack.last_mut() {
        let Some(node) = nodes.next() else {
            stack.pop();
            continue;
        };
        match visit(node, *trigger) {
            Visit::Enter(trigger) if stack.len() <= MAX_DEPTH => {
                stack.push((node.elements(), trigger))
            }
            Visit::Stop => break,
            _ => {}
        }
    }
}

#[derive(Default)]
pub(crate) struct Behaviors<'a> {
    pub(crate) target: Option<&'a El>,
    pub(crate) duration: u32,
    pub(crate) auto_reverse: bool,
    pub(crate) motion: Option<&'a El>,
    pub(crate) rotation: Option<&'a El>,
    pub(crate) scale: Option<&'a El>,
    pub(crate) color: Option<&'a El>,
}

pub(crate) fn inspect(clock: &El) -> Behaviors<'_> {
    let mut behavior = Behaviors::default();
    let mut encountered = false;
    let mut all_reverse = true;
    let mut fallback = None;
    descendants(clock, |node, _| {
        match node.local() {
            "cBhvr" => {
                let target = node.path(&["tgtEl", "spTgt"]);
                behavior.target = behavior.target.or(target);
                encountered = true;
                all_reverse &=
                    node.child("cTn").and_then(|time| time.bool("autoRev")) == Some(true);
            }
            "spTgt" => fallback = fallback.or(Some(node)),
            "cTn" => {
                if let Some(duration) = milliseconds(node, "dur").filter(|duration| *duration > 1) {
                    behavior.duration = behavior.duration.max(duration);
                }
            }
            "animMotion" => behavior.motion = behavior.motion.or(Some(node)),
            "animRot" => behavior.rotation = behavior.rotation.or(Some(node)),
            "animScale" => behavior.scale = behavior.scale.or(Some(node)),
            "animClr" => behavior.color = behavior.color.or(Some(node)),
            _ => {}
        }
        Visit::Enter(None)
    });
    behavior.target = behavior.target.or(fallback);
    behavior.auto_reverse = encountered && all_reverse;
    behavior
}

pub(crate) fn media(timing: &El) -> impl Iterator<Item = (&El, &El)> {
    let mut videos = Vec::new();
    let mut audio = Vec::new();
    descendants(timing, |element, _| {
        if let Some(properties) = element.child("cMediaNode") {
            match element.local() {
                "video" if videos.len() < MAX_EFFECTS => videos.push((element, properties)),
                "audio" if audio.len() < MAX_EFFECTS => audio.push((element, properties)),
                _ => {}
            }
        }
        Visit::Enter(None)
    });
    videos.into_iter().chain(audio).take(MAX_EFFECTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_target_comes_from_its_behavior() -> Result<(), crate::PptxError> {
        let timing = crate::xml::parse(br#"
            <p:timing><p:tnLst><p:seq><p:cTn nodeType="interactiveSeq">
                <p:stCondLst><p:cond evt="onBegin" delay="0"/><p:cond evt="onClick" delay="0">
                    <p:tgtEl><p:spTgt spid="5"/></p:tgtEl>
                </p:cond></p:stCondLst>
                <p:childTnLst><p:par><p:cTn presetClass="emph">
                    <p:stCondLst><p:cond delay="0"><p:tgtEl><p:spTgt spid="6"/></p:tgtEl></p:cond></p:stCondLst>
                    <p:childTnLst><p:animRot by="10800000"><p:cBhvr>
                        <p:cTn dur="750" autoRev="1"/><p:tgtEl><p:spTgt spid="7"/></p:tgtEl>
                    </p:cBhvr></p:animRot></p:childTnLst>
                </p:cTn></p:par></p:childTnLst>
            </p:cTn></p:seq></p:tnLst></p:timing>
        "#)?;
        let effects = effects(&timing.root);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].trigger, Some(5));
        let behavior = inspect(effects[0].clock);
        assert_eq!(
            behavior.target.and_then(|target| target.u32("spid")),
            Some(7)
        );
        assert_eq!(behavior.duration, 750);
        assert!(behavior.auto_reverse);
        Ok(())
    }
}
