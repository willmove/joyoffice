//! Single-flight requests coalesce clock/resize events and reject obsolete device-pixel frames.
use joydeck_shell::show_render::RenderTarget;
type ClockStamp = ((u64, usize, usize, u64), Option<u64>);

#[derive(Default)]
pub(super) struct Clock {
    previous: Option<ClockStamp>,
}
impl Clock {
    pub fn changed(&mut self, frame: (u64, usize, usize, u64), media_tick: Option<u64>) -> bool {
        let next = Some((frame, media_tick));
        let changed = self.previous != next;
        self.previous = next;
        changed
    }
}

/// Some Windows DPI changes retain the physical window size without sending a useful
/// resize. Force a one-device-pixel resize, then restore the last fullscreen extent.
#[cfg(any(target_os = "windows", test))]
#[derive(Default)]
pub(super) struct Viewport {
    dpi: Option<f32>,
    physical: Option<(f32, f32)>,
    pending: Option<(f32, f32)>,
}
#[cfg(any(target_os = "windows", test))]
impl Viewport {
    pub fn update(
        &mut self,
        logical: (f32, f32),
        dpi: f32,
        fullscreen: bool,
    ) -> Option<(f32, f32)> {
        let changed = self.dpi.is_some_and(|old| old != dpi);
        self.dpi = Some(dpi);
        if fullscreen
            && changed
            && let Some(physical) = self.physical
        {
            self.pending = Some(physical);
            return Some(((physical.0 + 1.) / dpi, physical.1 / dpi));
        }
        let physical = (logical.0 * dpi, logical.1 * dpi);
        if fullscreen
            && let Some(target) = self.pending
            && ((physical.0 - target.0).abs() > 0.25 || (physical.1 - target.1).abs() > 0.25)
        {
            return Some((target.0 / dpi, target.1 / dpi));
        }
        self.pending = None;
        self.physical = Some(physical);
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Key {
    pub state: (u64, usize, usize, u64),
    pub target: RenderTarget,
    pub fonts: u64,
}
impl Key {
    fn same_content(self, other: Self) -> bool {
        // Time advances while a worker renders. Rejecting every older timestamp would starve
        // animations; navigation, resolution and fonts must still match exactly.
        self.state.0 == other.state.0
            && self.state.1 == other.state.1
            && self.state.2 == other.state.2
            && self.target == other.target
            && self.fonts == other.fonts
    }
}

#[derive(Default)]
pub(super) struct Requests {
    desired: Option<Key>,
    active: Option<Key>,
    completed: Option<Key>,
}
impl Requests {
    pub fn request(&mut self, key: Key) -> bool {
        self.desired = Some(key);
        if self.active.is_some() || self.completed == Some(key) {
            return false;
        }
        self.active = Some(key);
        true
    }
    pub fn complete(&mut self, key: Key, content_current: bool) -> bool {
        if self.active != Some(key) {
            return false;
        }
        self.active = None;
        if !content_current
            || !self
                .desired
                .is_some_and(|desired| key.same_content(desired))
        {
            return false;
        }
        // Also remember failures to avoid a tight retry loop on a static invalid frame.
        self.completed = Some(key);
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Test fixture construction")]
mod tests {
    use super::*;
    #[test]
    fn clock_observes_presenter_changes_while_static_or_paused() {
        let mut clock = Clock::default();
        assert!(clock.changed((0, 0, 0, 0), None));
        assert!(!clock.changed((0, 0, 0, 0), None));
        assert!(clock.changed((1, 0, 0, 0), Some(10))); // External media start.
        assert!(clock.changed((1, 0, 0, 0), Some(11)));
        assert!(clock.changed((2, 0, 0, 0), None)); // External pause.
        assert!(!clock.changed((2, 0, 0, 0), None));
        assert!(clock.changed((3, 0, 0, 0), None)); // Seek while paused.
        assert!(clock.changed((4, 0, 0, 0), Some(11))); // Resume.
    }
    #[test]
    fn fullscreen_dpi_refit_keeps_physical_extent_until_resize_completes() {
        let mut viewport = Viewport::default();
        assert_eq!(viewport.update((2048., 1152.), 1.25, true), None);
        assert_eq!(
            viewport.update((2048., 1152.), 2., true),
            Some((1280.5, 720.))
        );
        // A render or resize callback can still see the old logical size before OS completion.
        assert_eq!(
            viewport.update((2048., 1152.), 2., true),
            Some((1280., 720.))
        );
        assert_eq!(
            viewport.update((1280.5, 720.), 2., true),
            Some((1280., 720.))
        );
        assert_eq!(viewport.update((1280., 720.), 2., true), None);
        assert_eq!(
            viewport.update((1280., 720.), 1., true),
            Some((2561., 1440.))
        );
        assert_eq!(
            viewport.update((2561., 1440.), 1., true),
            Some((2560., 1440.))
        );
        assert_eq!(viewport.update((2560., 1440.), 1., true), None);
        assert_eq!(viewport.update((1100., 720.), 1.5, false), None);
    }
    fn key(dpi: f64, time: u64) -> Key {
        Key {
            state: (1, 0, 0, time),
            fonts: 1,
            target: RenderTarget::fit((960., 540.), (960., 540.), dpi).unwrap(),
        }
    }
    #[test]
    fn resize_coalesces_and_discards_old_result_without_parallel_work() {
        let mut requests = Requests::default();
        let old = key(1., 0);
        let high_dpi = key(2., 0);
        assert!(requests.request(old));
        assert!(!requests.request(key(1.5, 0)));
        assert!(!requests.request(high_dpi));
        assert!(!requests.complete(old, true));
        assert!(requests.request(high_dpi));
        assert!(requests.complete(high_dpi, true));
        assert!(!requests.request(high_dpi));
    }
    #[test]
    fn advancing_clock_does_not_starve_animation_but_navigation_and_fonts_invalidate() {
        let mut requests = Requests::default();
        let first = key(1., 10);
        let next = key(1., 20);
        assert!(requests.request(first));
        assert!(!requests.request(next));
        assert!(requests.complete(first, true));
        assert!(requests.request(next));
        let mut changed = next;
        changed.state.1 = 1;
        assert!(!requests.request(changed));
        assert!(!requests.complete(next, true));
        assert!(requests.request(changed));
        let mut fonts = changed;
        fonts.fonts += 1;
        assert!(!requests.request(fonts));
        assert!(!requests.complete(changed, true));
        assert!(requests.request(fonts));
        assert!(!requests.complete(fonts, false));
        assert!(requests.request(fonts));
    }
}
