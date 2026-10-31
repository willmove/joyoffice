//! Deterministic animation sampling. All time is milliseconds; no window or wall clock in the model.
use crate::{
    document::{Deck, Object, Result},
    show::ShowSnapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Presentation {
    pub slides: BTreeMap<String, Timing>,
    pub order: Vec<String>,
    pub loop_show: bool,
    pub links: BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Timing {
    pub transition: String,
    pub duration_ms: u64,
    pub advance_ms: Option<u64>,
    pub effects: Vec<Effect>,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            transition: "none".into(),
            duration_ms: 600,
            advance_ms: None,
            effects: vec![],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Effect {
    pub id: String,
    pub target: String,
    pub kind: String,
    pub trigger: String,
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub repeats: u32,
    pub auto_reverse: bool,
    pub rewind: bool,
    pub paragraph: Option<usize>,
    pub dx: f32,
    pub dy: f32,
    pub amount: f32,
}
impl Default for Effect {
    fn default() -> Self {
        Self {
            id: "effect-1".into(),
            target: String::new(),
            kind: "fade".into(),
            trigger: "click".into(),
            duration_ms: 600,
            delay_ms: 0,
            repeats: 1,
            auto_reverse: false,
            rewind: false,
            paragraph: None,
            dx: -180.,
            dy: 0.,
            amount: 360.,
        }
    }
}
impl Effect {
    pub fn span(&self) -> u64 {
        self.duration_ms * self.repeats as u64 * if self.auto_reverse { 2 } else { 1 }
    }
}
impl Presentation {
    pub fn validate(&self, d: &Deck) -> Result<()> {
        if self.order.len() > 1000 || self.slides.len() > 500 {
            return Err("放映页序/时间线超过预算".into());
        }
        if self.links.len() > 1000 {
            return Err("放映链接超限".into());
        }
        for (object, page) in &self.links {
            if !d
                .slides
                .iter()
                .flat_map(|s| &s.objects)
                .any(|o| &o.id == object)
                || !d.slides.iter().any(|s| &s.id == page)
            {
                return Err("放映链接目标无效".into());
            }
        }
        for id in &self.order {
            if !d.slides.iter().any(|s| &s.id == id && !s.hidden) {
                return Err("自定义放映引用了无效/隐藏页".into());
            }
        }
        for s in &d.slides {
            let mut morph = BTreeSet::new();
            for o in &s.objects {
                if let Some(key) = &o.morph_key
                    && (!morph.insert(key) || key.is_empty() || key.len() > 128)
                {
                    return Err("同页Morph匹配key重复或无效".into());
                }
            }
        }
        for (id, t) in &self.slides {
            let s = d
                .slides
                .iter()
                .find(|s| &s.id == id)
                .ok_or("时间线页面不存在")?;
            if !["none", "fade", "push", "wipe", "morph"].contains(&t.transition.as_str())
                || t.duration_ms > 10000
                || t.advance_ms.is_some_and(|n| n == 0 || n > 3600000)
                || t.effects.len() > 256
            {
                return Err("转场/自动翻页/效果数量超限".into());
            }
            let mut ids = BTreeSet::new();
            for e in &t.effects {
                let o = s
                    .objects
                    .iter()
                    .find(|o| o.id == e.target)
                    .ok_or("动画目标不存在")?;
                if e.id.is_empty()
                    || !ids.insert(&e.id)
                    || !["appear", "fade", "fade-out", "fly", "move", "zoom", "spin"]
                        .contains(&e.kind.as_str())
                    || !["click", "with", "after"].contains(&e.trigger.as_str())
                    || e.duration_ms == 0
                    || e.duration_ms > 60000
                    || e.delay_ms > 60000
                    || e.repeats == 0
                    || e.repeats > 64
                    || e.span() > 600000
                    || ![e.dx, e.dy, e.amount]
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 10000.)
                {
                    return Err("动画参数或预算无效".into());
                }
                if e.paragraph.is_some_and(|i| {
                    o.kind != "text" || o.rotation != 0. || i >= o.text.split('\n').count()
                }) {
                    return Err("按段动画需要有效的未旋转文本段落".into());
                }
            }
        }
        Ok(())
    }
    pub fn timing(&self, page: &str) -> Timing {
        self.slides.get(page).cloned().unwrap_or_default()
    }
}
pub fn reconcile(d: &mut Deck) {
    let pages: &BTreeSet<_> = &d.slides.iter().map(|s| s.id.clone()).collect();
    d.presentation
        .order
        .retain(|id| d.slides.iter().any(|s| &s.id == id && !s.hidden));
    d.presentation.slides.retain(|id, _| pages.contains(id));
    d.presentation.links.retain(|o, p| {
        pages.contains(p) && d.slides.iter().flat_map(|s| &s.objects).any(|n| &n.id == o)
    });
    for s in &d.slides {
        if let Some(t) = d.presentation.slides.get_mut(&s.id) {
            t.effects.retain(|e| {
                s.objects.iter().any(|o| {
                    o.id == e.target
                        && e.paragraph.is_none_or(|i| {
                            o.kind == "text" && o.rotation == 0. && i < o.text.split('\n').count()
                        })
                })
            });
        }
    }
}
#[derive(Clone, Debug)]
pub struct Scheduled {
    pub step: usize,
    pub start: u64,
    pub end: u64,
}
pub fn schedule(t: &Timing) -> Vec<Scheduled> {
    let mut step = 0;
    let mut start = 0;
    let mut end = 0;
    let mut out = vec![];
    for e in &t.effects {
        match e.trigger.as_str() {
            "click" => {
                step += 1;
                start = e.delay_ms;
            }
            "with" => {
                start += e.delay_ms;
            }
            _ => {
                start = end + e.delay_ms;
            }
        }
        end = start + e.span();
        out.push(Scheduled { step, start, end });
    }
    out
}
fn progress(e: &Effect, s: &Scheduled, step: usize, elapsed: u64) -> f32 {
    if s.step > step || (s.step == step && elapsed < s.start) {
        return -1.;
    }
    let local = if s.step < step {
        e.span()
    } else {
        elapsed.saturating_sub(s.start)
    };
    if local >= e.span() {
        return if e.rewind || e.auto_reverse { 0. } else { 1. };
    }
    if e.kind == "appear" {
        return 1.;
    }
    let cycle = e.duration_ms * if e.auto_reverse { 2 } else { 1 };
    let tick = local % cycle;
    let p = tick as f32 / e.duration_ms as f32;
    if p > 1. { 2. - p } else { p }
}
fn transform(o: &mut Object, e: &Effect, p: f32) {
    let p = p.max(0.);
    match e.kind.as_str() {
        "appear" => o.opacity *= p,
        "fade" => o.opacity *= p,
        "fade-out" => o.opacity *= 1. - p,
        "fly" => {
            o.x += e.dx * (1. - p);
            o.y += e.dy * (1. - p);
            o.opacity *= p;
        }
        "move" => {
            o.x += e.dx * p;
            o.y += e.dy * p;
        }
        "zoom" => {
            let scale = 0.1 + 0.9 * p;
            let w = o.width * scale;
            let h = o.height * scale;
            o.x += (o.width - w) / 2.;
            o.y += (o.height - h) / 2.;
            o.width = w;
            o.height = h;
            crate::table::resize(o);
        }
        "spin" => o.rotation += e.amount * p,
        _ => {}
    }
}
/// Paragraphs become transient view objects with byte-correct rich run slices.
pub fn sample(d: &Deck, page: usize, step: usize, elapsed: u64) -> Result<Deck> {
    let mut out = d.clone();
    let slide = &d.slides[page];
    let t = d.presentation.timing(&slide.id);
    let times = schedule(&t);
    let mut objects = vec![];
    for o in &slide.objects {
        let split = t
            .effects
            .iter()
            .any(|e| e.target == o.id && e.paragraph.is_some());
        let mut parts = vec![];
        if split {
            let mut at = 0;
            for (i, line) in o.text.split('\n').enumerate() {
                let mut p = o.clone();
                p.id = format!("{}:paragraph:{i}", o.id);
                p.text = line.into();
                p.y += o.font_size * 1.3 * i as f32;
                p.height = o.font_size * 1.3;
                p.runs = crate::text::runs(o)
                    .iter()
                    .filter_map(|r| {
                        let a = r.start.max(at);
                        let b = r.end.min(at + line.len());
                        (a < b).then(|| crate::text::Run {
                            start: a - at,
                            end: b - at,
                            style: r.style.clone(),
                        })
                    })
                    .collect();
                parts.push((Some(i), p));
                at += line.len() + 1;
            }
        } else {
            parts.push((None, o.clone()));
        }
        for (para, mut o2) in parts {
            let mut applied = false;
            for (e, time) in
                t.effects.iter().zip(&times).filter(|(e, _)| {
                    e.target == o.id && (e.paragraph.is_none() || e.paragraph == para)
                })
            {
                let p = progress(e, time, step, elapsed);
                if p < 0. {
                    if !applied && ["appear", "fade", "fly", "zoom"].contains(&e.kind.as_str()) {
                        o2.opacity = 0.;
                    }
                } else {
                    transform(&mut o2, e, p);
                    applied = true;
                }
            }
            objects.push(o2);
        }
    }
    out.slides[page].objects = objects;
    out.presentation = Presentation::default();
    Ok(out)
}
pub fn morph(d: &Deck, from: usize, to: usize, p: f32) -> Result<Deck> {
    let mut frame = d.clone();
    let p = p.clamp(0., 1.);
    let mut used = BTreeSet::new();
    let mut objects = vec![];
    for current in &d.slides[to].objects {
        let previous = current.morph_key.as_ref().and_then(|key| {
            d.slides[from].objects.iter().find(|o| {
                o.morph_key.as_ref() == Some(key)
                    && o.kind == current.kind
                    && o.text == current.text
                    && o.fill == current.fill
                    && o.stroke == current.stroke
                    && o.runs == current.runs
                    && o.chart == current.chart
                    && o.diagram == current.diagram
                    && o.table == current.table
                    && o.image.as_ref().map(|i| &i.resource)
                        == current.image.as_ref().map(|i| &i.resource)
            })
        });
        let mut o = current.clone();
        if let Some(a) = previous {
            used.insert(a.id.clone());
            let lerp = |x: f32, y: f32| x + (y - x) * p;
            o.x = lerp(a.x, o.x);
            o.y = lerp(a.y, o.y);
            o.width = lerp(a.width, o.width);
            o.height = lerp(a.height, o.height);
            o.rotation = lerp(a.rotation, o.rotation);
            o.opacity = lerp(a.opacity, o.opacity);
            if let (Some(ai), Some(bi)) = (&a.image, &mut o.image)
                && ai.resource == bi.resource
            {
                for i in 0..4 {
                    bi.crop[i] = lerp(ai.crop[i], bi.crop[i]);
                }
            }
            crate::table::resize(&mut o);
        } else {
            o.opacity *= p;
        }
        objects.push(o);
    }
    for o in &d.slides[from].objects {
        if !used.contains(&o.id) {
            let mut o = o.clone();
            o.id = format!("morph-out:{}", o.id);
            o.opacity *= 1. - p;
            objects.insert(0, o);
        }
    }
    frame.slides[to].objects = objects;
    frame.presentation = Presentation::default();
    Ok(frame)
}
#[derive(Clone, Debug, PartialEq)]
pub enum Blank {
    Black,
    White,
}
#[derive(Clone, Debug)]
pub struct Ink {
    pub page: usize,
    pub points: Vec<(f32, f32)>,
}
#[derive(Default)]
pub struct Controller {
    pub step: usize,
    pub elapsed: u64,
    pub page_elapsed: u64,
    pub total: u64,
    pub paused: bool,
    pub blank: Option<Blank>,
    pub previous_page: Option<usize>,
    pub transition_elapsed: u64,
    pub ink: Vec<Ink>,
    pub laser: Option<(f32, f32)>,
    pub finished: bool,
    pub rehearsing: bool,
    pub rehearsed: BTreeMap<String, u64>,
}
impl Controller {
    fn reset(&mut self, from: usize) {
        self.step = 0;
        self.elapsed = 0;
        self.page_elapsed = 0;
        self.transition_elapsed = 0;
        self.previous_page = Some(from);
        self.blank = None;
        self.finished = false;
    }
    pub fn next(&mut self, s: &mut ShowSnapshot) {
        let t = s.deck.presentation.timing(&s.deck.slides[s.page()].id);
        let times = schedule(&t);
        let end = times
            .iter()
            .filter(|s| s.step == self.step)
            .map(|s| s.end)
            .max()
            .unwrap_or(0);
        if self.elapsed < end {
            self.elapsed = end;
            return;
        }
        let max = times.iter().map(|s| s.step).max().unwrap_or(0);
        if self.step < max {
            self.step += 1;
            self.elapsed = 0;
        } else {
            self.next_page(s);
        }
    }
    pub fn next_page(&mut self, s: &mut ShowSnapshot) {
        let from = s.page();
        if self.rehearsing && self.page_elapsed > 0 {
            self.rehearsed.insert(
                s.deck.slides[from].id.clone(),
                self.page_elapsed.min(3600000),
            );
        }
        if s.cursor + 1 == s.pages.len() {
            if s.deck.presentation.loop_show {
                s.first();
            } else {
                self.finished = true;
                return;
            }
        } else {
            s.next_page();
        }
        self.reset(from);
    }
    pub fn previous(&mut self, s: &mut ShowSnapshot) {
        self.finished = false;
        if self.step > 0 {
            self.step -= 1;
            self.elapsed = schedule(&s.deck.presentation.timing(&s.deck.slides[s.page()].id))
                .iter()
                .filter(|s| s.step == self.step)
                .map(|s| s.end)
                .max()
                .unwrap_or(0);
            self.blank = None;
            return;
        }
        let from = s.page();
        s.previous();
        self.reset(from);
        let t = s.deck.presentation.timing(&s.deck.slides[s.page()].id);
        let times = schedule(&t);
        self.step = times.iter().map(|t| t.step).max().unwrap_or(0);
        self.elapsed = times
            .iter()
            .filter(|t| t.step == self.step)
            .map(|t| t.end)
            .max()
            .unwrap_or(0);
    }
    pub fn jump(&mut self, s: &mut ShowSnapshot, cursor: usize) {
        let from = s.page();
        s.cursor = cursor.min(s.pages.len() - 1);
        self.reset(from);
    }
    pub fn advance(&mut self, s: &mut ShowSnapshot, delta: u64) {
        if self.paused {
            return;
        }
        let delta = delta.min(3600000);
        self.elapsed = self.elapsed.saturating_add(delta);
        self.page_elapsed = self.page_elapsed.saturating_add(delta);
        self.total = self.total.saturating_add(delta);
        self.transition_elapsed = self.transition_elapsed.saturating_add(delta);
        let t = s.deck.presentation.timing(&s.deck.slides[s.page()].id);
        if t.advance_ms.is_some_and(|ms| self.page_elapsed >= ms) {
            self.next_page(s);
        }
    }
    pub fn follow_link(&mut self, s: &mut ShowSnapshot, object: &str) -> Result<()> {
        let page = s
            .deck
            .presentation
            .links
            .get(object)
            .ok_or("对象没有放映链接")?;
        let cursor = s
            .pages
            .iter()
            .position(|i| &s.deck.slides[*i].id == page)
            .ok_or("链接目标不在本次可见放映顺序中")?;
        self.jump(s, cursor);
        Ok(())
    }
    pub fn restart_current(&mut self) {
        self.step = 0;
        self.elapsed = 0;
        self.page_elapsed = 0;
        self.previous_page = None;
        self.transition_elapsed = 0;
        self.finished = false;
        self.blank = None;
    }
    pub fn seek(&mut self, ms: u64) {
        self.elapsed = ms.min(3600000);
    }
    pub fn frame(&self, s: &ShowSnapshot) -> Result<Deck> {
        let t = s.deck.presentation.timing(&s.deck.slides[s.page()].id);
        if t.transition == "morph"
            && self.transition_elapsed < t.duration_ms
            && let Some(from) = self.previous_page
        {
            return morph(
                &s.deck,
                from,
                s.page(),
                self.transition_elapsed as f32 / t.duration_ms.max(1) as f32,
            );
        }
        sample(&s.deck, s.page(), self.step, self.elapsed)
    }
}
/// Bounded document-copy annotations. Each segment is a rotated positive-size line.
pub fn append_ink(d: &mut Deck, ink: &[Ink]) -> Result<()> {
    if ink.len() > 256 || ink.iter().map(|i| i.points.len()).sum::<usize>() > 2048 {
        return Err("墨迹超过预算".into());
    }
    for (n, i) in ink.iter().enumerate() {
        let page = d.slides.get_mut(i.page).ok_or("墨迹页无效")?;
        for (j, pair) in i.points.windows(2).enumerate() {
            let (a, b) = (pair[0], pair[1]);
            if ![a.0, a.1, b.0, b.1]
                .iter()
                .all(|v| v.is_finite() && v.abs() < 100000.)
            {
                return Err("墨迹坐标无效".into());
            }
            let dx = b.0 - a.0;
            let dy = b.1 - a.1;
            let len = dx.hypot(dy);
            if len < 0.1 {
                continue;
            }
            let mut o = crate::edit::object("line");
            o.id = format!("show-ink-{n}-{j}");
            while page.objects.iter().any(|p| p.id == o.id) {
                o.id.push('x');
            }
            o.width = len;
            o.height = 1.;
            o.x = (a.0 + b.0) / 2. - len / 2.;
            o.y = (a.1 + b.1) / 2. - 0.5;
            o.rotation = dy.atan2(dx).to_degrees() - (1. / len).atan().to_degrees();
            o.fill = "#e63333".into();
            o.stroke = o.fill.clone();
            o.stroke_width = 3.;
            page.objects.push(o);
        }
    }
    Ok(())
}
/// Pixel fallback for basic slide transitions; both inputs have identical dimensions.
pub fn composite(
    a: &crate::preview::Raster,
    b: &mut crate::preview::Raster,
    kind: &str,
    p: f32,
) -> Result<()> {
    if a.width != b.width || a.height != b.height || a.bgra.len() != b.bgra.len() || !p.is_finite()
    {
        return Err("转场缓冲尺寸或进度无效".into());
    }
    let p = p.clamp(0., 1.);
    let w = b.width as usize;
    let offset = (w as f32 * p).round() as usize;
    if kind == "fade" {
        for (x, y) in a.bgra.iter().zip(&mut b.bgra) {
            *y = (*x as f32 * (1. - p) + *y as f32 * p).round() as u8;
        }
        return Ok(());
    }
    let current = b.bgra.clone();
    for y in 0..b.height as usize {
        for x in 0..w {
            let dst = (y * w + x) * 4;
            let (source, sx) = if kind == "push" {
                if x < w - offset {
                    (&a.bgra, x + offset)
                } else {
                    (&current, x - (w - offset))
                }
            } else if x < offset {
                (&current, x)
            } else {
                (&a.bgra, x)
            };
            b.bgra[dst..dst + 4].copy_from_slice(&source[(y * w + sx) * 4..(y * w + sx) * 4 + 4]);
        }
    }
    Ok(())
}
