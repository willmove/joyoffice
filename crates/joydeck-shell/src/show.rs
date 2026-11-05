//! Immutable-document presentation runtime; the UI and headless control sample the same state.
use crate::Cancellation;
use joydeck_anim::{ShowAction, ShowState, Timeline};
use joydeck_model::{Presentation, ShapeId};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct Player {
    pub model: Arc<Presentation>,
    pub state: ShowState,
    pub paused: bool,
    pub blank: Option<bool>,
    pub elapsed: f64,
    pub slide_elapsed: f64,
    pub total: f64,
    pub transition_elapsed: f64,
    pub previous: Option<usize>,
    pub serial: u64,
}
impl Player {
    pub fn new(
        model: Arc<Presentation>,
        page: Option<usize>,
        custom: Option<&str>,
    ) -> Result<Self, String> {
        if page.is_some_and(|p| p >= model.slides.len()) {
            return Err("show start page outside document".into());
        }
        let state = if let Some(name) = custom {
            ShowState::play_custom(&model, name).ok_or("custom show missing/empty")?
        } else if let Some(p) = page {
            ShowState::start_at(&model, p)
        } else {
            ShowState::new(&model)
        };
        Ok(Self {
            model,
            state,
            paused: false,
            blank: None,
            elapsed: 0.,
            slide_elapsed: 0.,
            total: 0.,
            transition_elapsed: 0.,
            previous: None,
            serial: 0,
        })
    }
    fn transition_duration(&self) -> f64 {
        if self.state.backward {
            return 0.;
        }
        self.model
            .slides
            .get(self.state.slide)
            .and_then(|s| s.transition.as_ref())
            .filter(|t| t.kind != "none")
            .map_or(0., |t| f64::from(t.duration_ms.min(60_000)) / 1000.)
    }
    fn changed(&mut self, old: usize, action: ShowAction) {
        if matches!(action, ShowAction::Slide(_)) {
            self.previous = Some(old);
            self.elapsed = 0.;
            self.slide_elapsed = 0.;
            self.transition_elapsed = 0.;
        }
        if matches!(
            action,
            ShowAction::PlayStep(_) | ShowAction::FinishStep(_) | ShowAction::StepBack(_)
        ) {
            self.elapsed = 0.;
        }
        self.serial += 1;
    }
    pub fn navigate(&mut self, action: &str, page: Option<usize>) -> Result<bool, String> {
        let old = self.state.slide;
        let result = match action {
            "next" => self.state.next(&self.model),
            "previous" => self.state.prev(&self.model),
            "goto" => self.state.goto(
                &self.model,
                page.filter(|p| *p < self.model.slides.len())
                    .ok_or("invalid show target")?,
            ),
            "pause" => {
                self.paused = !self.paused;
                self.serial += 1;
                return Ok(false);
            }
            "black" => {
                self.blank = if self.blank == Some(false) {
                    None
                } else {
                    Some(false)
                };
                self.serial += 1;
                return Ok(false);
            }
            "white" => {
                self.blank = if self.blank == Some(true) {
                    None
                } else {
                    Some(true)
                };
                self.serial += 1;
                return Ok(false);
            }
            _ => return Err("unknown show action".into()),
        };
        self.changed(old, result);
        Ok(result == ShowAction::Exit)
    }
    pub fn advance(&mut self, delta: f64) -> Result<(), String> {
        if !delta.is_finite() || !(0.0..=3600.).contains(&delta) {
            return Err("show time delta outside 0..3600 seconds".into());
        }
        if self.paused || self.state.ended {
            return Ok(());
        }
        self.total += delta;
        self.slide_elapsed += delta;
        let duration = self.transition_duration();
        let blocked = (duration - self.transition_elapsed).max(0.).min(delta);
        self.transition_elapsed += delta;
        let timeline = ShowState::timeline(&self.model, self.state.slide);
        self.state.tick(&timeline, self.total);
        if self.state.playing {
            self.elapsed += delta - blocked;
        }
        for _ in 0..128 {
            if !self.state.playing || self.elapsed < timeline.step_duration(self.state.step) {
                break;
            }
            self.elapsed = (self.elapsed - timeline.step_duration(self.state.step)).max(0.);
            self.state.step_done(&self.model);
            if self.state.step < timeline.steps() && timeline.step_is_auto(self.state.step) {
                self.state.next(&self.model);
            } else {
                self.elapsed = 0.;
                break;
            }
        }
        if self.state.auto_advance_due(&self.model, self.slide_elapsed) {
            let old = self.state.slide;
            let action = self.state.next(&self.model);
            self.changed(old, action);
        }
        Ok(())
    }
    pub fn status(&self) -> Value {
        json!({"page":self.state.slide+1,"step":self.state.step,"playing":self.state.playing,"ended":self.state.ended,"paused":self.paused,"blank":self.blank,"elapsed_seconds":self.elapsed,"slide_seconds":self.slide_elapsed,"total_seconds":self.total,"order":self.state.visit(&self.model).iter().map(|p|p+1).collect::<Vec<_>>(),"previous":self.previous.map(|p|p+1),"serial":self.serial})
    }
    pub fn animated(&self) -> bool {
        self.state.playing
            || self.state.trigger_playing()
            || self.transition_elapsed < self.transition_duration()
    }
    pub fn frame_key(&self) -> (u64, usize, usize, u64) {
        (
            self.serial,
            self.state.slide,
            self.state.step,
            if self.animated() {
                (self.total * 1000.) as u64
            } else {
                0
            },
        )
    }
    pub fn click(&mut self, x: f64, y: f64) -> Result<Option<String>, String> {
        if !x.is_finite() || !y.is_finite() {
            return Err("invalid pointer".into());
        }
        let placed = joydeck_render::place_slide(&self.model, self.state.slide);
        let tl = ShowState::timeline(&self.model, self.state.slide);
        let text_hits: std::collections::BTreeMap<_, _> = placed
            .links
            .into_iter()
            .filter(|link| link.text && link.rect.contains((x, y)))
            .map(|link| (link.shape, link.link.action))
            .collect();
        let shapes = placed
            .shapes
            .into_iter()
            .map(|s| joydeck_anim::HitShape {
                id: s.id,
                parent: s.parent,
                is_group: s.is_group,
                link: text_hits
                    .get(&s.id)
                    .cloned()
                    .or_else(|| s.link.map(|l| l.action)),
                under_pointer: s.rect.contains((x, y)) || text_hits.contains_key(&s.id),
            })
            .collect::<Vec<_>>();
        let old = self.state.slide;
        let visible = |id| self.state.anim_state(&tl, id, self.elapsed).visible;
        let visibility = shapes
            .iter()
            .map(|s| (s.id, visible(s.id)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let action = self.state.click(
            &self.model,
            &tl,
            &shapes,
            &|id| visibility.get(&id).copied().unwrap_or(true),
            self.total,
            self.previous,
        );
        match action {
            joydeck_anim::Click::Show(a) => self.changed(old, a),
            joydeck_anim::Click::Open(url) => return Ok(Some(url)),
            _ => self.serial += 1,
        }
        Ok(None)
    }
    pub fn frame(
        &self,
        scale: f64,
        cancel: &Cancellation,
    ) -> Result<joydeck_render::Image, String> {
        let w = (self.model.slide_size.width * scale).ceil();
        let h = (self.model.slide_size.height * scale).ceil();
        if !scale.is_finite()
            || !(0.05..=4.).contains(&scale)
            || w > 8192.
            || h > 8192.
            || w * h > 16_000_000.
        {
            return Err("show frame exceeds render budget".into());
        }
        cancel.check()?;
        if self.state.ended || self.blank.is_some() {
            return Ok(joydeck_render::Image {
                width: w as u32,
                height: h as u32,
                pixels: if self.blank == Some(true) {
                    vec![255; (w * h * 4.) as usize]
                } else {
                    [0, 0, 0, 255].repeat((w * h) as usize)
                },
            });
        }
        let _guard = crate::document::RENDER_LOCK
            .lock()
            .map_err(|_| "show renderer lock poisoned")?;
        let panics = joydeck_render::panics_caught();
        let tl = ShowState::timeline(&self.model, self.state.slide);
        let f = |id| self.shape_state(&tl, id);
        let gif_times = self.model.slides[self.state.slide]
            .shapes
            .iter()
            .map(|s| (s.id, self.slide_elapsed))
            .collect::<Vec<_>>();
        let opts = joydeck_render::RenderOpts {
            scale,
            state: Some(&f),
            gif_times: &gif_times,
            threads: 0,
            ..Default::default()
        };
        let duration = self.transition_duration();
        let image = if duration > 0. && self.transition_elapsed < duration {
            joydeck_render::render_transition_between(
                &self.model,
                self.state.slide,
                self.previous,
                self.transition_elapsed / duration,
                &opts,
            )
        } else {
            joydeck_render::render_slide(&self.model, self.state.slide, &opts)
        };
        cancel.check()?;
        if image.pixels.is_empty() || panics != joydeck_render::panics_caught() {
            return Err("show raster failed".into());
        }
        Ok(image)
    }
    fn shape_state(&self, tl: &Timeline, id: ShapeId) -> joydeck_render::ShapeState {
        let s = self.state.anim_state(tl, id, self.elapsed);
        joydeck_render::ShapeState {
            visible: s.visible,
            opacity: s.opacity,
            offset: (s.offset_x, s.offset_y).into(),
            scale: (s.scale_x, s.scale_y),
            rotate: s.rotate,
            clip: s.clip,
            tint: s.tint,
            paras: tl
                .paragraph_targets(id)
                .into_iter()
                .map(|p| {
                    let s = tl.state_ordered(
                        id,
                        Some(p),
                        self.state.step,
                        self.elapsed,
                        &self.state.trigger_plays(),
                        &self.state.trigger_stamps(),
                    );
                    (
                        p,
                        joydeck_render::ParaState {
                            visible: s.visible,
                            opacity: s.opacity,
                            offset: (s.offset_x, s.offset_y).into(),
                        },
                    )
                })
                .collect(),
        }
    }
}
