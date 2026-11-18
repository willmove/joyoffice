//! Hash every supported transition and option on patterned, partially transparent slide layers.
use joydeck_model::anim::TRANSITIONS;
use joydeck_render::{Image, SlideParts, composite_transition, transition_frame};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn image(seed: u32, translucent: bool) -> Image {
    let (width, height) = (64u32, 36u32);
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let a = if translucent {
                (11 * x + 23 * y + seed) % 256
            } else {
                255
            };
            for channel in [x * 13 + seed, y * 27 + seed, (x + y) * 7 + seed] {
                pixels.push(((channel % 256) * a / 255) as u8);
            }
            pixels.push(a as u8);
        }
    }
    Image {
        width,
        height,
        pixels,
    }
}

fn main() -> Result<(), serde_json::Error> {
    let (old, new) = (image(31, false), image(209, false));
    let parts = SlideParts {
        old_back: Some(old.clone()),
        old_content: Some(image(78, true)),
        new_back: new.clone(),
        new_content: image(183, true),
    };
    let mut frames = BTreeMap::new();
    for &(kind, _, _, _, options) in TRANSITIONS {
        for option in std::iter::once("").chain(options.iter().copied().filter(|o| !o.is_empty())) {
            for progress in [0, 22, 50, 78, 100] {
                let layers = transition_frame(kind, option, f64::from(progress) / 100., 16. / 9.);
                let result = composite_transition(Some(&old), &new, Some(&parts), &layers);
                frames.insert(
                    format!("{kind}/{option}/{progress}"),
                    format!("{:x}", Sha256::digest(result.pixels)),
                );
            }
        }
    }
    println!("{}", serde_json::to_string(&frames)?);
    Ok(())
}
