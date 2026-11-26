//! A slide's animation sequence: click steps, effect start times and per-shape state over time.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use joydeck_model::anim::{BuildRole, TextBuild, paragraph_build_roles};
use joydeck_model::style::Fill;
use joydeck_model::{
    AnimClass, AnimStart, ColorScheme, Rgba, SchemeSlot, Shape, ShapeId, Slide, Xfrm,
};

use crate::AnimState;
use crate::effects::EffectSpec;

/// Delay between paragraphs of a by-paragraph build that isn't on click (seconds).
pub const PARAGRAPH_STAGGER: f64 = 0.5;
const MAX_EFFECTS: usize = 100_000;
const MAX_ITERATIONS: f64 = 1.0e6;

/// Which sequence an effect belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sequence {
    /// The slide's main click sequence.
    Main,
    /// An interactive sequence started by clicking this shape.
    Trigger(ShapeId),
}

/// Timing of one (sub-)effect, for the Animation Pane timeline and the UI.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectTiming {
    /// Index into `slide.animations`.
    pub anim: usize,
    /// Animated shape and paragraph (`None` = the whole shape).
    pub shape: ShapeId,
    pub paragraph: Option<usize>,
    pub sequence: Sequence,
    /// Click step within its sequence.
    pub step: usize,
    /// Start within the step (seconds, delay included).
    pub start: f64,
    /// One iteration (seconds).
    pub duration: f64,
    /// Repeat count (fractional allowed); 1 = once.
    pub iterations: f64,
    /// Repeats until the next click.
    pub until_next_click: bool,
    pub auto_reverse: bool,
    pub rewind: bool,
    /// Time from start to end (all iterations, reverse included; one iteration if until-click).
    pub span: f64,
    pub class: AnimClass,
}

/// Playhead of a triggered sequence: `step` clicks on the trigger fully played, the current one
/// playing for `t` seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriggerPlay {
    pub trigger: ShapeId,
    pub step: usize,
    pub t: f64,
}

/// When a step of a triggered sequence was started, counted in started steps on the slide. Effects
/// of different triggers apply in this order, so that a shape hidden by one trigger and shown again
/// by another ends up as the later of the two left it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepStamp {
    pub trigger: ShapeId,
    pub step: usize,
    pub at: u64,
}

#[derive(Clone, Debug, Default)]
struct StepInfo {
    auto: bool,
    duration: f64,
}

/// Separate the last group start (With Previous) from the latest effect end (After Previous).
#[derive(Default)]
struct SequenceClock {
    steps: Vec<StepInfo>,
    group_start: f64,
    group_end: f64,
}

impl SequenceClock {
    fn place(
        &mut self,
        mode: AnimStart,
        delay: f64,
        staggered_start: Option<f64>,
        span: f64,
        media: bool,
    ) -> (usize, f64) {
        if mode == AnimStart::OnClick || self.steps.is_empty() {
            self.steps.push(StepInfo {
                auto: mode != AnimStart::OnClick,
                duration: 0.0,
            });
            self.group_start = 0.0;
            self.group_end = 0.0;
        }
        if mode == AnimStart::AfterPrevious {
            self.group_start = self.group_end;
        }
        let start = staggered_start.unwrap_or(self.group_start + delay);
        let end = start + span;
        self.group_end = self.group_end.max(end);
        let index = self.steps.len() - 1;
        let step = &mut self.steps[index];
        // Media continues in the background after its cue but delays After Previous.
        step.duration = step.duration.max(if media { start } else { end });
        (index, start)
    }
}

#[derive(Clone, Debug)]
enum After {
    None,
    Hide,
    HideOnNextClick,
    Dim(Rgba),
}

#[derive(Clone, Debug)]
struct Effect {
    timing: EffectTiming,
    /// Shared by every paragraph effect of one animation, so a large motion path is not copied
    /// per paragraph.
    spec: Arc<EffectSpec>,
    after: After,
}

/// A slide's animations laid out in time.
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    effects: Vec<Effect>,
    // Each target's effect indices, in Animation Pane order. Sampling a frame touches only the
    // effects that can change that shape or paragraph.
    targets: HashMap<(ShapeId, Option<usize>), Vec<usize>>,
    main: Vec<StepInfo>,
    triggers: Vec<(ShapeId, Vec<StepInfo>)>,
    /// Base fill colour per shape (for colour emphasis).
    colors: HashMap<ShapeId, Rgba>,
    /// By-paragraph builds animate first-level paragraphs; deeper paragraphs follow their parent:
    /// (shape, member paragraph, owner paragraph).
    groups: HashMap<(ShapeId, usize), usize>,
}

fn secs(ms: u32) -> f64 {
    ms as f64 / 1000.0
}

fn finite_or(v: f64, d: f64) -> f64 {
    if v.is_finite() { v } else { d }
}

fn sanitize_xfrm(x: Xfrm, sw: f64, sh: f64) -> Xfrm {
    Xfrm {
        x: finite_or(x.x, 0.0).clamp(-1.0e7, 1.0e7),
        y: finite_or(x.y, 0.0).clamp(-1.0e7, 1.0e7),
        w: finite_or(x.w, sw / 2.0).clamp(0.0, 1.0e7),
        h: finite_or(x.h, sh / 2.0).clamp(0.0, 1.0e7),
        ..x
    }
}

fn default_scheme() -> Option<ColorScheme> {
    joydeck_model::theme::builtin_color_schemes()
        .into_iter()
        .next()
}

impl Timeline {
    /// Build the timeline of `slide` (slide size in points). Placeholders without their own box
    /// get a centred default box; colours resolve against the default theme.
    pub fn new(slide: &Slide, slide_w: f64, slide_h: f64) -> Timeline {
        Timeline::with(slide, slide_w, slide_h, None, &|_| None)
    }

    /// Build with a colour scheme for theme colours and a lookup for shape boxes (placeholders
    /// inherit their box from the layout; the lookup wins over `shape.xfrm`).
    pub fn with(
        slide: &Slide,
        slide_w: f64,
        slide_h: f64,
        scheme: Option<&ColorScheme>,
        xfrm_of: &dyn Fn(ShapeId) -> Option<Xfrm>,
    ) -> Timeline {
        let sw = finite_or(slide_w, 960.0).clamp(1.0, 1.0e7);
        let sh = finite_or(slide_h, 540.0).clamp(1.0, 1.0e7);
        let owned;
        let scheme = match scheme {
            Some(s) => Some(s),
            None => {
                owned = default_scheme();
                owned.as_ref()
            }
        };
        let resolve = |c: &joydeck_model::ColorRef| -> Rgba {
            match scheme {
                Some(s) => c.resolve(s, None),
                None => match &c.base {
                    joydeck_model::ColorBase::Rgb { rgb } => *rgb,
                    _ => Rgba::rgb(0x15, 0x60, 0x82),
                },
            }
        };
        let accent = scheme
            .map(|s| s.get(SchemeSlot::Accent1))
            .unwrap_or(Rgba::rgb(0x15, 0x60, 0x82));

        let mut tl = Timeline::default();
        if slide.animations.is_empty() {
            return tl;
        }
        // Every shape by id, built once: resolving each animation's target by tree search would
        // cost shapes x animations.
        let shapes = shape_index(&slide.shapes);
        // Paragraph targets of each shape, worked out for its first by-paragraph animation only.
        let mut para_cache: HashMap<ShapeId, Arc<Vec<Option<usize>>>> = HashMap::new();
        // Group the animations by sequence in one pass, sequences in order of first appearance
        // (main first), so many distinct triggers cost linear work, not a scan per trigger.
        let mut seqs: Vec<(Sequence, Vec<usize>)> = vec![(Sequence::Main, Vec::new())];
        let mut seq_index: HashMap<Sequence, usize> = HashMap::from([(Sequence::Main, 0)]);
        for (ai, a) in slide.animations.iter().enumerate() {
            let s = a.trigger.map(Sequence::Trigger).unwrap_or(Sequence::Main);
            let i = *seq_index.entry(s).or_insert_with(|| {
                seqs.push((s, Vec::new()));
                seqs.len() - 1
            });
            if let Some((_, members)) = seqs.get_mut(i) {
                members.push(ai);
            }
        }
        for (seq, members) in seqs {
            if tl.effects.len() >= MAX_EFFECTS {
                break;
            }
            let mut clock = SequenceClock::default();
            for ai in members {
                let Some(a) = slide.animations.get(ai) else {
                    continue;
                };
                if tl.effects.len() >= MAX_EFFECTS {
                    break;
                }
                let shape = shapes.get(&a.shape).copied();
                // Paragraph targets of this animation.
                let paras: Arc<Vec<Option<usize>>> = match (a.paragraph, a.text_build) {
                    (Some(p), _) => Arc::new(vec![Some(p as usize)]),
                    (None, TextBuild::ByParagraph) => {
                        let cached = para_cache.entry(a.shape).or_insert_with(|| {
                            let mut v: Vec<Option<usize>> = vec![];
                            if let Some(t) = shape.and_then(|s| s.text.as_ref()) {
                                for (i, role) in paragraph_build_roles(t).into_iter().enumerate() {
                                    match role {
                                        BuildRole::Top => v.push(Some(i)),
                                        BuildRole::Member(o) => {
                                            tl.groups.insert((a.shape, i), o);
                                        }
                                        BuildRole::Unowned => {}
                                    }
                                }
                            }
                            if v.is_empty() {
                                v.push(None)
                            }
                            Arc::new(v)
                        });
                        Arc::clone(cached)
                    }
                    _ => Arc::new(vec![None]),
                };
                let xf = xfrm_of(a.shape)
                    .or_else(|| shape.and_then(|s| s.xfrm))
                    .unwrap_or(Xfrm::new(sw / 4.0, sh / 4.0, sw / 2.0, sh / 2.0));
                let xf = sanitize_xfrm(xf, sw, sh);
                let spec = Arc::new(EffectSpec::compile(a, xf, sw, sh, &resolve));
                let dur = secs(a.duration_ms);
                let delay = secs(a.delay_ms);
                let until_next_click = a.repeat == u32::MAX;
                let iterations = if until_next_click || a.repeat <= 1 {
                    1.0
                } else {
                    (a.repeat as f64).min(MAX_ITERATIONS)
                };
                let span = dur * if a.auto_reverse { 2.0 } else { 1.0 } * iterations;
                let after = match a.after.as_deref() {
                    Some("hide") => After::Hide,
                    Some("hideOnNextClick") => After::HideOnNextClick,
                    Some(c) => Rgba::from_hex(c).map(After::Dim).unwrap_or(After::None),
                    None => After::None,
                };
                let mut first_start = 0.0;
                for (k, para) in paras.iter().enumerate() {
                    if tl.effects.len() == MAX_EFFECTS {
                        break;
                    }
                    let staggered = (k > 0 && a.start != AnimStart::OnClick)
                        .then_some(first_start + k as f64 * PARAGRAPH_STAGGER);
                    let mode = match (clock.steps.is_empty(), seq, staggered) {
                        (true, Sequence::Trigger(_), _) => AnimStart::OnClick,
                        (_, _, Some(_)) => AnimStart::WithPrevious,
                        _ => a.start,
                    };
                    let (step, start) =
                        clock.place(mode, delay, staggered, span, a.class == AnimClass::Media);
                    if k == 0 {
                        first_start = start;
                    }
                    tl.targets
                        .entry((a.shape, *para))
                        .or_default()
                        .push(tl.effects.len());
                    tl.effects.push(Effect {
                        timing: EffectTiming {
                            anim: ai,
                            shape: a.shape,
                            paragraph: *para,
                            sequence: seq,
                            step,
                            start,
                            duration: dur,
                            iterations,
                            until_next_click,
                            auto_reverse: a.auto_reverse,
                            rewind: a.rewind,
                            span,
                            class: a.class,
                        },
                        spec: Arc::clone(&spec),
                        after: after.clone(),
                    });
                }
            }
            match seq {
                Sequence::Main => tl.main = clock.steps,
                Sequence::Trigger(s) => tl.triggers.push((s, clock.steps)),
            }
        }
        for indices in tl.targets.values_mut() {
            indices.sort_by_key(|&i| tl.effects[i].timing.anim);
        }
        // Base colours of animated shapes.
        for e in &tl.effects {
            let id = e.timing.shape;
            tl.colors.entry(id).or_insert_with(|| {
                match shapes.get(&id).copied().and_then(|s| s.fill.as_ref()) {
                    Some(Fill::Solid { color }) => resolve(color),
                    _ => accent,
                }
            });
        }
        tl
    }

    /// Number of steps in the main sequence (clicks, plus an automatic first step when the first
    /// effect starts with/after previous).
    pub fn steps(&self) -> usize {
        self.main.len()
    }

    /// Does main step `i` start by itself (no click), as the slide appears?
    pub fn step_is_auto(&self, i: usize) -> bool {
        self.main.get(i).is_some_and(|s| s.auto)
    }

    /// Duration of main step `i` in seconds (0 when out of range).
    pub fn step_duration(&self, i: usize) -> f64 {
        self.main.get(i).map(|s| s.duration).unwrap_or(0.0)
    }

    /// Media effects (Play, Pause, Stop) of main step `step` that start after `from` and by `to`
    /// seconds into it, in order: what a show running the step from `from` to `to` sets off.
    pub fn media_cues(&self, step: usize, from: f64, to: f64) -> Vec<EffectTiming> {
        let mut v: Vec<EffectTiming> = self
            .effects
            .iter()
            .map(|e| &e.timing)
            .filter(|e| {
                e.class == AnimClass::Media
                    && e.sequence == Sequence::Main
                    && e.step == step
                    && e.start > from
                    && e.start <= to
            })
            .cloned()
            .collect();
        v.sort_by(|a, b| a.start.total_cmp(&b.start));
        v
    }

    /// Total duration of the main sequence played without pauses.
    pub fn total_duration(&self) -> f64 {
        self.main.iter().map(|s| s.duration).sum()
    }

    /// Map a time on the "play everything" clock to `(step, t)` for [`Timeline::state`].
    pub fn locate(&self, time: f64) -> (usize, f64) {
        let mut t = finite_or(time, 0.0).max(0.0);
        for (i, s) in self.main.iter().enumerate() {
            if t < s.duration || (t == 0.0 && s.duration == 0.0 && i == 0) {
                return (i, t);
            }
            t -= s.duration;
        }
        (self.main.len(), 0.0)
    }

    /// Where effect `anim` (index into `slide.animations`) starts on the play-everything clock:
    /// Animation Pane ▸ Play From. `None` for a triggered effect or one that isn't on the slide.
    pub fn start_of(&self, anim: usize) -> Option<f64> {
        let e = self
            .effects
            .iter()
            .map(|e| &e.timing)
            .filter(|e| e.anim == anim && e.sequence == Sequence::Main)
            .min_by(|a, b| a.step.cmp(&b.step).then(a.start.total_cmp(&b.start)))?;
        let before: f64 = self.main.iter().take(e.step).map(|s| s.duration).sum();
        Some(before + e.start)
    }

    /// Shapes that start an interactive sequence when clicked.
    pub fn trigger_shapes(&self) -> Vec<ShapeId> {
        self.triggers.iter().map(|(s, _)| *s).collect()
    }

    /// Steps (clicks on the trigger) of a triggered sequence.
    pub fn trigger_steps(&self, trigger: ShapeId) -> usize {
        self.triggers
            .iter()
            .find(|(s, _)| *s == trigger)
            .map(|(_, v)| v.len())
            .unwrap_or(0)
    }

    pub fn trigger_step_duration(&self, trigger: ShapeId, i: usize) -> f64 {
        self.triggers
            .iter()
            .find(|(s, _)| *s == trigger)
            .and_then(|(_, v)| v.get(i))
            .map(|s| s.duration)
            .unwrap_or(0.0)
    }

    /// All effect timings in sequence order.
    pub fn effects(&self) -> Vec<EffectTiming> {
        self.effects.iter().map(|e| e.timing.clone()).collect()
    }

    /// Is anything animated on this slide?
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Paragraph indices of `shape` animated separately (by-paragraph builds).
    pub fn paragraph_targets(&self, shape: ShapeId) -> Vec<usize> {
        let mut v: Vec<usize> = self
            .effects
            .iter()
            .filter(|e| e.timing.shape == shape)
            .filter_map(|e| e.timing.paragraph)
            .collect();
        v.extend(
            self.groups
                .iter()
                .filter(|&(&(id, _), owner)| id == shape && v.contains(owner))
                .map(|(&(_, member), _)| member)
                .collect::<Vec<_>>(),
        );
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Shapes with any effect.
    pub fn animated_shapes(&self) -> Vec<ShapeId> {
        let mut seen = HashSet::new();
        self.effects
            .iter()
            .map(|e| e.timing.shape)
            .filter(|&id| seen.insert(id))
            .collect()
    }

    /// State of `shape` (or of one of its paragraphs, `para`) when `step` main steps have been
    /// fully played and step `step` has been playing for `t` seconds (`t <= 0`: not started;
    /// `step >= steps()`: everything done). Paragraph states are relative to the shape's own state
    /// (compose them). Triggered sequences are considered not played.
    pub fn state(&self, shape: ShapeId, para: Option<usize>, step: usize, t: f64) -> AnimState {
        self.state_with(shape, para, step, t, &[])
    }

    /// [`Timeline::state`] with the playheads of triggered sequences.
    pub fn state_with(
        &self,
        shape: ShapeId,
        para: Option<usize>,
        step: usize,
        t: f64,
        triggers: &[TriggerPlay],
    ) -> AnimState {
        self.state_ordered(shape, para, step, t, triggers, &[])
    }

    /// [`Timeline::state_with`] applying the effects of triggered sequences in the order their steps
    /// were started (`stamps`), after those of the main sequence. Without stamps they apply in
    /// Animation Pane order.
    pub fn state_ordered(
        &self,
        shape: ShapeId,
        para: Option<usize>,
        step: usize,
        t: f64,
        triggers: &[TriggerPlay],
        stamps: &[StepStamp],
    ) -> AnimState {
        let para = para.map(|p| self.groups.get(&(shape, p)).copied().unwrap_or(p));
        let Some(indices) = self.targets.get(&(shape, para)) else {
            return AnimState::default();
        };
        let base_color = self
            .colors
            .get(&shape)
            .copied()
            .unwrap_or(Rgba::rgb(0x15, 0x60, 0x82));
        let initial = AnimState {
            visible: indices
                .first()
                .is_none_or(|&i| self.effects[i].timing.class != AnimClass::Entrance),
            ..Default::default()
        };
        let apply = |state, &i: &usize| {
            let effect = &self.effects[i];
            let cursor = match effect.timing.sequence {
                Sequence::Main => Cursor { step, elapsed: t },
                Sequence::Trigger(id) => triggers.iter().find(|play| play.trigger == id).map_or(
                    Cursor::default(),
                    |play| Cursor {
                        step: play.step,
                        elapsed: play.t,
                    },
                ),
            };
            effect.apply(state, base_color, cursor)
        };
        let value = if stamps.is_empty() {
            indices.iter().fold(initial, apply)
        } else {
            let mut ordered = indices.clone();
            ordered.sort_by_cached_key(|&i| {
                let timing = &self.effects[i].timing;
                match timing.sequence {
                    Sequence::Main => 0,
                    Sequence::Trigger(id) => stamps
                        .iter()
                        .find(|s| s.trigger == id && s.step == timing.step)
                        .map_or(u64::MAX, |s| s.at.saturating_add(1)),
                }
            });
            ordered.iter().fold(initial, apply)
        };
        value.sanitized()
    }

    /// State on the "play everything" clock (Animation Pane ▸ Play All, preview).
    pub fn state_at(&self, shape: ShapeId, para: Option<usize>, time: f64) -> AnimState {
        let (s, t) = self.locate(time);
        self.state(shape, para, s, t)
    }
}

#[derive(Clone, Copy, Default)]
struct Cursor {
    step: usize,
    elapsed: f64,
}

enum Phase {
    Pending,
    Running(f64),
    Held { beyond_step: bool },
}

impl Cursor {
    fn phase(self, timing: &EffectTiming) -> Phase {
        match self.step.cmp(&timing.step) {
            std::cmp::Ordering::Less => Phase::Pending,
            std::cmp::Ordering::Greater => Phase::Held { beyond_step: true },
            std::cmp::Ordering::Equal => {
                let t = finite_or(self.elapsed, 0.0);
                let age = t - timing.start;
                let d = timing.duration;
                if t <= 0.0 || age < 0.0 {
                    Phase::Pending
                } else if !d.is_finite()
                    || d <= 0.0
                    || (!timing.until_next_click && age >= timing.span)
                {
                    Phase::Held { beyond_step: false }
                } else {
                    let length = if timing.auto_reverse { d * 2.0 } else { d };
                    let iteration = finite_or(age % length, 0.0);
                    let progress = if iteration > d {
                        2.0 - iteration / d
                    } else {
                        iteration / d
                    };
                    Phase::Running(progress.clamp(0.0, 1.0))
                }
            }
        }
    }
}

impl Effect {
    fn apply(&self, mut state: AnimState, base: Rgba, cursor: Cursor) -> AnimState {
        let color = state.tint.unwrap_or(base);
        match cursor.phase(&self.timing) {
            Phase::Pending => state,
            Phase::Running(progress) => {
                if matches!(self.timing.class, AnimClass::Entrance | AnimClass::Exit) {
                    state.visible = true;
                }
                state.compose(&self.spec.eval(progress, color))
            }
            Phase::Held { beyond_step } => {
                let restore = self.timing.rewind || self.timing.auto_reverse;
                match self.timing.class {
                    AnimClass::Entrance => state.visible = !restore,
                    AnimClass::Exit => state.visible = restore,
                    AnimClass::Emphasis | AnimClass::Path if !restore => {
                        state = state.compose(&self.spec.end_state(color));
                    }
                    _ => {}
                }
                match self.after {
                    After::Hide => state.visible = false,
                    After::HideOnNextClick if beyond_step => state.visible = false,
                    After::Dim(fill) if beyond_step => state.tint = Some(fill),
                    _ => {}
                }
                state
            }
        }
    }
}

/// Shapes by id in the order `Slide::shape` searches (a shape before its children, groups at most
/// 64 deep); the first shape with an id wins a duplicate.
fn shape_index(shapes: &[Shape]) -> HashMap<ShapeId, &Shape> {
    fn walk<'a>(shapes: &'a [Shape], depth: usize, out: &mut HashMap<ShapeId, &'a Shape>) {
        if depth > 64 {
            return;
        }
        for s in shapes {
            out.entry(s.id).or_insert(s);
            walk(s.children(), depth + 1, out);
        }
    }
    let mut out = HashMap::new();
    walk(shapes, 0, &mut out);
    out
}
