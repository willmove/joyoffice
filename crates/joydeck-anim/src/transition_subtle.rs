//! Subtle exposure and content-reveal transitions (`p14:reveal` and `p14:flash`).
use crate::transition::{FULL, Layer, Source};
use crate::transition_surface::{smoothstep, span, transform_rect};

/// Reveal slides outgoing content away while cross-fading backgrounds (smoothly)
/// or dipping cleanly through black (`blackLeft` / `blackRight`).
pub(crate) fn reveal(option: &str, t: f64) -> Vec<Layer> {
    let dir = if option.ends_with("Right") { 1. } else { -1. };
    if option.starts_with("black") {
        let mut layers = vec![Layer::full(Source::Black, 1.)];
        if t < 0.42 {
            let u = span(t, 0., 0.40);
            let dx = dir * 0.13 * u * u;
            layers.push(Layer::rect(
                Source::Old,
                FULL,
                transform_rect(FULL, 1., [0.5, 0.5], [dx, 0.]),
                1. - smoothstep(u),
            ));
        }
        if t > 0.54 {
            let u = span(t, 0.54, 1.);
            let rem = 1. - u;
            let dx = -dir * 0.13 * rem * rem;
            layers.push(Layer::rect(
                Source::New,
                FULL,
                transform_rect(FULL, 1., [0.5, 0.5], [dx, 0.]),
                smoothstep(u),
            ));
        }
        return layers;
    }

    let mut layers = vec![
        Layer::full(Source::OldBack, 1.),
        Layer::full(Source::NewBack, smoothstep(t)),
    ];
    if t < 0.50 {
        let u = span(t, 0., 0.47);
        let dx = dir * 0.13 * u * u;
        layers.push(Layer::rect(
            Source::OldContent,
            FULL,
            transform_rect(FULL, 1., [0.5, 0.5], [dx, 0.]),
            1. - smoothstep(u),
        ));
    }
    if t > 0.44 {
        let u = span(t, 0.44, 0.96);
        let rem = 1. - u;
        let dx = -dir * 0.13 * rem * rem;
        layers.push(Layer::rect(
            Source::NewContent,
            FULL,
            transform_rect(FULL, 1., [0.5, 0.5], [dx, 0.]),
            smoothstep(u),
        ));
    }
    layers
}

/// Photographic flash bulb: fast over-exposure to white, hidden slide cut, then smooth decay.
pub(crate) fn flash(t: f64) -> Vec<Layer> {
    let white = if t < 0.11 {
        let u = span(t, 0., 0.11);
        1. - (1. - u) * (1. - u)
    } else if t < 0.45 {
        1.
    } else {
        1. - smoothstep(span(t, 0.45, 0.90))
    };
    let slide = if t < 0.22 { Source::Old } else { Source::New };
    vec![Layer::full(slide, 1.), Layer::full(Source::White, white)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_separates_smooth_and_black_variants_and_mirrors_direction() {
        let left = reveal("smoothlyLeft", 0.25);
        let right = reveal("smoothlyRight", 0.25);
        assert!(left.iter().all(|l| l.source.is_part()));
        assert_eq!(left.len(), right.len());
        let old_l = left.iter().find(|l| l.source == Source::OldContent);
        let old_r = right.iter().find(|l| l.source == Source::OldContent);
        assert!(
            old_l
                .is_some_and(|a| old_r.is_some_and(|b| (a.quad[0][0] + b.quad[0][0]).abs() < 1e-9))
        );

        let black_mid = reveal("blackLeft", 0.48);
        assert_eq!(black_mid.len(), 1);
        assert_eq!(black_mid[0].source, Source::Black);
    }

    #[test]
    fn flash_peaks_at_full_white_across_slide_cut() {
        let before_cut = flash(0.18);
        let after_cut = flash(0.28);
        assert_eq!(before_cut[0].source, Source::Old);
        assert_eq!(after_cut[0].source, Source::New);
        assert!((before_cut[1].alpha - 1.).abs() < 1e-9);
        assert!((after_cut[1].alpha - 1.).abs() < 1e-9);
        assert!(flash(0.95)[1].alpha == 0.);
    }
}
