//! The audience owns one device and one immutable slide pair. Font changes, navigation,
//! backward playback and output-size changes cannot reuse a previous pair.
use super::RenderTarget;
use crate::{Cancellation, show::Player};
use joydeck_model::{Presentation, Shape, ShapeKind, TextBody, resolve, text::RunKind};
use joydeck_render::{
    Image, SlideParts, render_slide, transition_frame,
    transition_gpu::{Compositor, SlidePair},
    uses_slide_parts,
};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
pub(super) struct Presenter {
    device: Option<Compositor>,
    attempted: bool,
    pair: Option<CachedPair>,
}

struct CachedPair {
    document: Arc<Presentation>,
    slide: usize,
    previous: Option<usize>,
    target: RenderTarget,
    fonts: u64,
    sampled_second: Option<u64>,
    textures: SlidePair,
}

// Dates in fields can tick while the two slides otherwise stay still. Resolve
// parts against the same model, including inherited layout/master shapes.
fn has_fields(model: &Presentation, slide: usize) -> bool {
    fn text_fields(text: &TextBody) -> bool {
        text.paragraphs
            .iter()
            .flat_map(|p| &p.runs)
            .any(|r| matches!(r.kind, RunKind::Field { .. }))
    }
    fn shape_fields(shapes: &[Shape], backdrop: bool) -> bool {
        shapes.iter().filter(|shape| !backdrop || shape.ph.is_none()).any(|shape| {
            let mut found = false;
            joydeck_model::walk(std::slice::from_ref(shape), &mut |s, _| {
                found |= s.text.as_ref().is_some_and(text_fields)
                    || matches!(&s.kind, ShapeKind::Table(t) if t.rows.iter().flat_map(|r| &r.cells).any(|c| text_fields(&c.text)));
            });
            found
        })
    }
    let page = &model.slides[slide];
    if shape_fields(&page.shapes, false) {
        return true;
    }
    let Some(ctx) = resolve::Ctx::for_slide(model, page) else {
        return false;
    };
    let (layout, master) = resolve::show_master_shapes(page, ctx.layout);
    (master && shape_fields(&ctx.master.shapes, true))
        || (layout && ctx.layout.is_some_and(|l| shape_fields(&l.shapes, true)))
}

impl Presenter {
    pub(super) fn frame(
        &mut self,
        player: &Player,
        target: RenderTarget,
        fonts: u64,
        cancel: &Cancellation,
    ) -> Result<Option<Image>, String> {
        let Some(progress) = player
            .transition_progress()
            .filter(|_| !player.state.ended && player.blank.is_none())
        else {
            self.pair = None;
            return Ok(None);
        };
        let slide = &player.model.slides[player.state.slide];
        let Some(effect) = slide.transition.as_ref().filter(|e| e.kind != "morph") else {
            self.pair = None;
            return Ok(None);
        };
        if !self.attempted {
            self.attempted = true;
            if std::env::var_os("JOYDECK_GPU_TRANSITIONS").is_some_and(|s| s == "0") {
                eprintln!("JoyDeck show_transition backend=cpu reason=configuration");
            } else {
                match Compositor::open(false) {
                    Ok(device) => {
                        eprintln!(
                            "JoyDeck show_transition backend={:?} adapter={:?}",
                            device.adapter.backend, device.adapter.name
                        );
                        self.device = Some(device);
                    }
                    Err(e) => eprintln!("JoyDeck show_transition backend=cpu reason={e}"),
                }
            }
        }
        let Some(device) = &self.device else {
            return Ok(None);
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if !self.pair.as_ref().is_some_and(|p| {
            Arc::ptr_eq(&p.document, &player.model)
                && p.slide == player.state.slide
                && p.previous == player.previous
                && p.target == target
                && p.fonts == fonts
                && p.sampled_second.is_none_or(|second| second == now)
        }) {
            self.pair = None;
            let parts = uses_slide_parts(&effect.kind, &effect.option);
            if !device.accepts(target.width, target.height, parts)
                || !slide.animations.is_empty()
                || player.model.media.iter().any(|m| {
                    m.content_type.eq_ignore_ascii_case("image/gif") || m.data.starts_with(b"GIF8")
                })
            {
                return Ok(None);
            }
            let textures = player.with_render_options(target.scale(), |opts| {
                let new = render_slide(&player.model, player.state.slide, opts);
                let old = player
                    .previous
                    .map(|i| render_slide(&player.model, i, opts));
                let parts = parts.then(|| {
                    SlideParts::render(&player.model, player.previous, player.state.slide, opts)
                });
                device.upload(old.as_ref(), &new, parts.as_ref())
            })?;
            cancel.check()?;
            let textures = match textures {
                Ok(pair) => pair,
                Err(e) => {
                    eprintln!("JoyDeck show_transition backend=cpu upload={e}");
                    self.device = None;
                    return Ok(None);
                }
            };
            self.pair = Some(CachedPair {
                document: player.model.clone(),
                slide: player.state.slide,
                previous: player.previous,
                target,
                fonts,
                sampled_second: (has_fields(&player.model, player.state.slide)
                    || player
                        .previous
                        .is_some_and(|i| has_fields(&player.model, i)))
                .then_some(now),
                textures,
            });
            eprintln!(
                "JoyDeck show_transition uploaded slide={} pixels={}x{} parts={parts}",
                player.state.slide + 1,
                target.width,
                target.height
            );
        }
        let Some(pair) = &mut self.pair else {
            return Ok(None);
        };
        let size = player.model.slide_size;
        let layers = transition_frame(
            &effect.kind,
            &effect.option,
            progress,
            size.width / size.height.max(1.0),
        );
        let image = match device.render(&mut pair.textures, &layers) {
            Ok(image) => Some(image),
            Err(e) => {
                self.pair = None;
                self.device = None;
                eprintln!("JoyDeck show_transition backend=cpu render={e}");
                None
            }
        };
        cancel.check()?;
        Ok(image)
    }
}
