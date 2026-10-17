//! Viewport-neighborhood page raster cache (LAYOUT-015).
//!
//! Each page is rasterized under the per-page pixel budget. The cache holds
//! only the pages around the viewport, so a long document is not failed as
//! one shared pixel total. Caret, selection and highlight overlays are not
//! part of the cached page.

use std::collections::{BTreeMap, VecDeque};

use joyword_layout::{LoadedFont, PageScene};

use crate::{Projection, RasterCancellation, RasterError, RasterLimits, RasterPage, rasterize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    identity: u64,
    projection: u64,
    page: usize,
}

/// One neighborhood raster: the document identity, its page scenes, and the viewport.
pub struct NeighborhoodRequest<'a> {
    pub identity: u64,
    pub scenes: &'a [PageScene],
    pub fonts: &'a [LoadedFont],
    pub projection: Projection,
    pub focus: usize,
    pub radius: usize,
    pub cancellation: &'a RasterCancellation,
}

/// Cached page images for one scrolling viewport.
#[derive(Debug)]
pub struct NeighborhoodCache {
    limit_pages: usize,
    pages: BTreeMap<Key, RasterPage>,
    order: VecDeque<Key>,
    draws: u64,
}

impl NeighborhoodCache {
    pub fn new(limit_pages: usize) -> Self {
        Self {
            limit_pages: limit_pages.max(1),
            pages: BTreeMap::new(),
            order: VecDeque::new(),
            draws: 0,
        }
    }

    pub fn draws(&self) -> u64 {
        self.draws
    }

    pub fn cached(
        &self,
        identity: u64,
        projection: Projection,
        page: usize,
    ) -> Option<&RasterPage> {
        self.pages.get(&Key {
            identity,
            projection: projection_key(projection),
            page,
        })
    }

    /// Raster `focus ± radius`, reusing cached pages. Pages outside the
    /// neighborhood are dropped once the cache exceeds its page limit.
    pub fn raster_neighborhood(
        &mut self,
        request: NeighborhoodRequest<'_>,
    ) -> Result<Vec<usize>, RasterError> {
        let NeighborhoodRequest {
            identity,
            scenes,
            fonts,
            projection,
            focus,
            radius,
            cancellation,
        } = request;
        if scenes.is_empty() {
            return Ok(Vec::new());
        }
        let focus = focus.min(scenes.len() - 1);
        let start = focus.saturating_sub(radius);
        let end = (focus + radius).min(scenes.len() - 1);
        let proj = projection_key(projection);
        let mut painted = Vec::new();
        for (page, scene) in scenes.iter().enumerate() {
            if page < start || page > end {
                continue;
            }
            let key = Key {
                identity,
                projection: proj,
                page,
            };
            if !self.pages.contains_key(&key) {
                let image = rasterize(
                    scene,
                    fonts,
                    projection,
                    RasterLimits::default(),
                    cancellation,
                )?;
                self.draws += 1;
                self.pages.insert(key, image);
                self.order.push_back(key);
            }
            painted.push(page);
        }
        let keep = |key: &Key| {
            key.identity == identity && key.projection == proj && (start..=end).contains(&key.page)
        };
        while self.pages.len() > self.limit_pages {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if keep(&old) {
                self.order.push_back(old);
                if self.order.iter().all(keep) {
                    break;
                }
                continue;
            }
            self.pages.remove(&old);
        }
        Ok(painted)
    }
}

fn projection_key(projection: Projection) -> u64 {
    projection.dpi().to_bits() ^ projection.zoom().to_bits().rotate_left(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use joyword_foundation::Emu;
    use joyword_layout::PageScene;

    fn scene() -> PageScene {
        PageScene {
            width: Emu::new(8 * Emu::PER_INCH),
            height: Emu::new(8 * Emu::PER_INCH),
            runs: Vec::new(),
        }
    }

    #[test]
    fn a_hundred_pages_at_two_x_raster_by_neighborhood() {
        let scenes = vec![scene(); 100];
        let projection = Projection::new(96.0, 2.0).unwrap();
        let mut cache = NeighborhoodCache::new(5);
        let cancel = RasterCancellation::default();
        let first = cache
            .raster_neighborhood(NeighborhoodRequest {
                identity: 1,
                scenes: &scenes,
                fonts: &[],
                projection,
                focus: 0,
                radius: 1,
                cancellation: &cancel,
            })
            .unwrap();
        assert_eq!(first, vec![0, 1]);
        let draws = cache.draws();
        assert!(draws <= 2);
        let far = cache
            .raster_neighborhood(NeighborhoodRequest {
                identity: 1,
                scenes: &scenes,
                fonts: &[],
                projection,
                focus: 99,
                radius: 1,
                cancellation: &cancel,
            })
            .unwrap();
        assert!(far.contains(&99));
        assert!(cache.cached(1, projection, 99).is_some());
        let page = cache.cached(1, projection, 99).unwrap().clone();
        let bytes = page.clone().into_bgra();
        assert!(!bytes.is_empty());
        // Scrolling back reuses the cached far page instead of redrawing it.
        let before = cache.draws();
        cache
            .raster_neighborhood(NeighborhoodRequest {
                identity: 1,
                scenes: &scenes,
                fonts: &[],
                projection,
                focus: 99,
                radius: 0,
                cancellation: &cancel,
            })
            .unwrap();
        assert_eq!(cache.draws(), before);
        let again = cache.cached(1, projection, 99).unwrap().clone().into_bgra();
        assert_eq!(again, bytes);
        let overlay = crate::paint_caret_overlay(
            &bytes,
            page.width(),
            page.height(),
            projection,
            Emu::new(Emu::PER_INCH),
            Emu::new(Emu::PER_INCH),
            Emu::new(Emu::PER_INCH / 2),
        );
        assert_eq!(
            cache.cached(1, projection, 99).unwrap().clone().into_bgra(),
            bytes
        );
        assert_ne!(overlay, bytes);
    }
}
