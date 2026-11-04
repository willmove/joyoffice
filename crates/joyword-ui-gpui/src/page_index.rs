//! Scroll geometry in document order. The prefix sums are independent of zoom.
use joyword_shell::reader::PageSize;
use std::ops::Range;

const TOP: f64 = 24.0;
const GAP: f64 = 42.0;

#[derive(Default)]
pub(super) struct PageIndex {
    heights: Vec<f64>,
    width: f32,
}

impl PageIndex {
    pub fn is_empty(&self) -> bool {
        self.heights.len() < 2
    }

    pub fn new(pages: &[PageSize]) -> Self {
        let mut heights = Vec::with_capacity(pages.len() + 1);
        heights.push(0.0);
        let mut total = 0.0;
        let mut width: f32 = 0.0;
        for page in pages {
            total += f64::from(page.height);
            heights.push(total);
            width = width.max(page.width);
        }
        Self { heights, width }
    }

    fn offset(height: f64, page: usize, scale: f32) -> f64 {
        TOP + height * f64::from(scale) + page as f64 * GAP
    }

    pub fn top(&self, page: usize, scale: f32) -> f32 {
        Self::offset(
            self.heights.get(page).copied().unwrap_or_default(),
            page,
            scale,
        ) as f32
    }

    pub fn width(&self, scale: f32) -> f32 {
        self.width * scale
    }

    pub fn zoomed_scroll(&self, old_scale: f32, new_scale: f32, scroll: f32, viewport: f32) -> f32 {
        if scroll <= 0.0 || self.is_empty() {
            return 0.0;
        }
        let page = self.visible(old_scale, scroll, viewport).start;
        let within = scroll - self.top(page, old_scale);
        (self.top(page, new_scale) + within * new_scale / old_scale).max(0.0)
    }

    pub fn visible(&self, scale: f32, scroll: f32, viewport: f32) -> Range<usize> {
        let count = self.heights.len().saturating_sub(1);
        if count == 0 {
            return 0..0;
        }
        let lower = f64::from(scroll) + 0.5;
        let upper = f64::from(scroll) + f64::from(viewport);
        let first = self
            .search(count, scale, |top| top <= lower, 1)
            .min(count - 1);
        let end = self
            .search(count, scale, |top| top < upper, 0)
            .max(first + 1);
        first..end
    }

    fn search(
        &self,
        count: usize,
        scale: f32,
        before: impl Fn(f64) -> bool,
        offset: usize,
    ) -> usize {
        let (mut lo, mut hi) = (0, count);
        while lo < hi {
            let middle = lo + (hi - lo) / 2;
            let page = middle + offset;
            if before(Self::offset(self.heights[page], page, scale)) {
                lo = middle + 1;
            } else {
                hi = middle;
            }
        }
        lo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_range_at_boundaries_and_after_shrinking_document() {
        let index = PageIndex::new(&[
            PageSize {
                width: 600.,
                height: 800.,
            },
            PageSize {
                width: 800.,
                height: 600.,
            },
        ]);
        assert_eq!(index.visible(1., 0., 600.), 0..1);
        assert_eq!(index.visible(1., 866., 600.), 1..2);
        assert_eq!(index.visible(2., 1666., 900.), 1..2);
        assert_eq!(index.visible(1., 4000., 900.), 1..2);
        assert_eq!(index.visible(1., 700., 500.), 0..2);
        assert_eq!(index.width(2.), 1600.);
        assert_eq!(index.zoomed_scroll(1., 2., 886., 600.), 1706.);
        assert_eq!(index.zoomed_scroll(1., 2., 0., 600.), 0.);
    }
}
