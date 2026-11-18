//! Whole-slide cuts, dissolves and translations. A track is expressed in units
//! of slide width and height, independent of texture dimensions and DPI.
use crate::{Layer, Source, easing::smooth, transition::FULL};

#[derive(Clone, Copy)]
enum Travel {
    Push,
    Cover,
    Uncover,
}

const COVER: [f64; 9] = [0., 0.005, 0.053, 0.186, 0.512, 0.848, 0.96, 0.995, 1.];
const UNCOVER: [f64; 9] = [0., 0.020, 0.073, 0.22, 0.510, 0.84, 0.965, 0.997, 1.];
const COVER_DIAGONAL: [f64; 9] = [0., 0.195, 0.472, 0.75, 0.895, 0.96, 0.987, 1., 1.];
const UNCOVER_DIAGONAL: [f64; 9] = [0., 0.087, 0.604, 0.952, 1., 1., 1., 1., 1.];

/// Monotone cubic interpolation at eighth-duration samples from the motion
/// reference. A plateau stops the track at that key rather than overshooting it.
fn sampled(keys: &[f64; 9], time: f64) -> f64 {
    let scaled = time * 8.;
    let i = (scaled.floor() as usize).min(7);
    let t = scaled - i as f64;
    let speed = |index: usize| {
        if index == 0 || index == 8 {
            return 0.;
        }
        let before = keys[index] - keys[index - 1];
        let after = keys[index + 1] - keys[index];
        if before * after > 0. {
            2. * before * after / (before + after)
        } else {
            0.
        }
    };
    let rest = 1. - t;
    keys[i] * (rest * rest * (1. + 2. * t))
        + keys[i + 1] * (t * t * (3. - 2. * t))
        + (speed(i) * rest - speed(i + 1) * t) * t * rest
}

fn direction(option: &str) -> [f64; 2] {
    let axis = |negative, positive| {
        if option.contains(positive) {
            1.
        } else if option.contains(negative) {
            -1.
        } else {
            0.
        }
    };
    let vector = [axis('l', 'r'), axis('u', 'd')];
    if vector == [0., 0.] {
        [-1., 0.]
    } else {
        vector
    }
}

fn translate(source: Source, vector: [f64; 2], distance: f64) -> Layer {
    let [x, y] = vector.map(|v| v * distance);
    Layer::rect(source, FULL, [x, y, 1. + x, 1. + y], 1.)
}

impl Travel {
    fn frame(self, option: &str, t: f64) -> Vec<Layer> {
        let vector = direction(option);
        let distance = match self {
            Self::Push => crate::ease(t, 0.5, 0.5, 0.),
            Self::Cover => sampled(
                if option.len() == 2 {
                    &COVER_DIAGONAL
                } else {
                    &COVER
                },
                t,
            ),
            Self::Uncover => sampled(
                if option.len() == 2 {
                    &UNCOVER_DIAGONAL
                } else {
                    &UNCOVER
                },
                t,
            ),
        };
        let incoming = translate(Source::New, vector, distance - 1.);
        let outgoing = translate(Source::Old, vector, distance);
        match self {
            Self::Push => vec![outgoing, incoming],
            Self::Cover => vec![Layer::full(Source::Old, 1.), incoming],
            Self::Uncover => vec![Layer::full(Source::New, 1.), outgoing],
        }
    }
}

pub(super) fn layers(kind: &str, option: &str, t: f64) -> Option<Vec<Layer>> {
    Some(match kind {
        "none" => vec![Layer::full(Source::New, 1.)],
        "cut" => vec![Layer::full(
            if option == "throughBlack" && t < 0.5 {
                Source::Black
            } else {
                Source::New
            },
            1.,
        )],
        "fade" if option == "throughBlack" => {
            let mut layers = vec![Layer::full(Source::Black, 1.)];
            if t != 0.5 {
                let (source, remaining) = if t < 0.5 {
                    (Source::Old, 1. - 2. * t)
                } else {
                    (Source::New, 2. * t - 1.)
                };
                layers.push(Layer::full(source, smooth(remaining)));
            }
            layers
        }
        "fade" | "morph" => vec![
            Layer::full(Source::Old, 1.),
            Layer::full(Source::New, smooth(t)),
        ],
        "push" => Travel::Push.frame(option, t),
        "cover" => Travel::Cover.frame(option, t),
        "uncover" => Travel::Uncover.frame(option, t),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonal_uncover_progress_matches_exported_slide_displacement() {
        // Incoming area in the PowerPoint video gives 1 - sqrt(1 - area)
        // for a departing diagonal rectangle. The fourth direction is a holdout.
        for (t, expected) in [(0.125, 0.088), (0.25, 0.608), (0.375, 0.952)] {
            let layers = Travel::Uncover.frame("rd", t);
            for coordinate in layers[1].quad[0] {
                assert!((coordinate - expected).abs() < 0.025);
            }
        }
        for keys in [&COVER, &UNCOVER, &COVER_DIAGONAL, &UNCOVER_DIAGONAL] {
            let mut previous = 0.;
            for i in 1..=512 {
                let current = sampled(keys, f64::from(i) / 512.);
                assert!((previous..=1.).contains(&current));
                previous = current;
            }
            assert_eq!(previous, 1.);
        }
    }
}
