//! Device-pixel targets and a bounded static raster cache for presentation windows.
//! Animation sampling, transitions and media controls remain owned by `show::Player`.
use crate::{Cancellation, show::Player};
use joydeck_model::Presentation;
use joydeck_render::Image;
use std::sync::Arc;

const MAX_SIDE: f64 = 8192.;
const MAX_PIXELS: f64 = 16_000_000.;
const MAX_SCALE: f64 = 8.;

/// A render scale and its allocation bound. Equality includes scale, not just rounded dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderTarget {
    scale_bits: u64,
    pub width: u32,
    pub height: u32,
}
impl RenderTarget {
    pub fn new(width: f64, height: f64, scale: f64) -> Result<Self, String> {
        let w = (width * scale).ceil();
        let h = (height * scale).ceil();
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.
            || height <= 0.
            || !scale.is_finite()
            || !(0.05..=MAX_SCALE).contains(&scale)
            || w < 1.
            || h < 1.
            || w > MAX_SIDE
            || h > MAX_SIDE
            || w * h > MAX_PIXELS
        {
            return Err("show frame exceeds render budget".into());
        }
        Ok(Self {
            scale_bits: scale.to_bits(),
            width: w as u32,
            height: h as u32,
        })
    }

    /// `viewport` is the available slide area in logical pixels, excluding UI chrome.
    pub fn fit(slide: (f64, f64), viewport: (f64, f64), dpi: f64) -> Result<Self, String> {
        let (w, h) = slide;
        if [w, h, viewport.0, viewport.1, dpi]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
        {
            return Err("invalid show viewport or scale factor".into());
        }
        let requested = ((viewport.0 / w).min(viewport.1 / h) * dpi).clamp(0.05, MAX_SCALE);
        if let Ok(target) = Self::new(w, h, requested) {
            return Ok(target);
        }
        // Include ceil-rounded dimensions in the allocation budget, including extreme aspects.
        let mut low = 0.05;
        let mut high = requested;
        let mut target = Self::new(w, h, low)?;
        for _ in 0..40 {
            let mid = (low + high) * 0.5;
            if let Ok(candidate) = Self::new(w, h, mid) {
                low = mid;
                target = candidate;
            } else {
                high = mid;
            }
        }
        Ok(target)
    }

    pub fn scale(self) -> f64 {
        f64::from_bits(self.scale_bits)
    }
}

struct CachedRaster {
    model: Arc<Presentation>,
    state: (u64, usize, usize, u64),
    target: RenderTarget,
    fonts: u64,
    image: Image,
}

/// At most one raster (64 MB under the existing frame budget). Only retained for media
/// compositing on a static page; continuously changing frames never incur a cache copy.
#[derive(Default)]
pub struct RasterCache {
    cached: Option<CachedRaster>,
    pub renders: u64,
    pub hits: u64,
}
impl RasterCache {
    pub fn render(
        &mut self,
        player: &Player,
        target: RenderTarget,
        retain_for_media: bool,
        cancel: &Cancellation,
    ) -> Result<Image, String> {
        cancel.check()?;
        let fonts = joydeck_fonts::FontDb::global().environment_revision();
        let state = player.frame_key();
        // GIF decoding is time dependent even when the shape animation timeline is idle.
        // Conservatively avoid retaining pages from a document containing GIF resources.
        let retain = retain_for_media
            && !player.animated()
            && !player.model.media.iter().any(|m| {
                m.content_type.eq_ignore_ascii_case("image/gif")
                    || m.name.to_ascii_lowercase().ends_with(".gif")
            });
        if retain
            && let Some(cached) = &self.cached
            && Arc::ptr_eq(&cached.model, &player.model)
            && cached.state == state
            && cached.target == target
            && cached.fonts == fonts
        {
            self.hits += 1;
            return Ok(cached.image.clone());
        }
        self.cached = None;
        let image = player.frame(target.scale(), cancel)?;
        self.renders += 1;
        if retain && fonts == joydeck_fonts::FontDb::global().environment_revision() {
            self.cached = Some(CachedRaster {
                model: player.model.clone(),
                state,
                target,
                fonts,
                image: image.clone(),
            });
        }
        Ok(image)
    }
}
