//! Hash every supported transition and option on patterned, partially transparent slide layers.
use joydeck_model::anim::TRANSITIONS;
use joydeck_render::{Image, SlideParts, composite_transition, transition_frame};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn image((width, height): (u32, u32), seed: u32, translucent: bool) -> Image {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let gpu_check = args.next().as_deref() == Some("--gpu");
    let size = match (args.next(), args.next()) {
        (Some(w), Some(h)) => (w.parse::<u32>()?, h.parse::<u32>()?),
        (None, None) => (64, 36),
        _ => return Err("expected width and height".into()),
    };
    if !(1..=1920).contains(&size.0) || !(1..=1080).contains(&size.1) {
        return Err("test dimensions outside budget".into());
    }
    let (old, new) = (image(size, 31, false), image(size, 209, false));
    let parts = SlideParts {
        old_back: Some(old.clone()),
        old_content: Some(image(size, 78, true)),
        new_back: new.clone(),
        new_content: image(size, 183, true),
    };
    #[cfg(not(feature = "transition-gpu"))]
    if gpu_check {
        return Err("build with --features transition-gpu".into());
    }
    #[cfg(feature = "transition-gpu")]
    let mut gpu = if gpu_check {
        let device = joydeck_render::transition_gpu::Compositor::open(true)?;
        let pair = device.upload(Some(&old), &new, Some(&parts))?;
        Some((device, pair))
    } else {
        None
    };
    #[cfg(feature = "transition-gpu")]
    let missing_sources = if let Some((device, _)) = &gpu {
        use joydeck_anim::{Layer, Source};
        let some_parts = SlideParts {
            old_back: None,
            old_content: None,
            ..parts.clone()
        };
        let mut checks = 0;
        let mut maximum = 0;
        for (old, parts) in [(None, None), (Some(&old), None), (None, Some(&some_parts))] {
            let mut pair = device.upload(old, &new, parts)?;
            for source in [
                Source::Old,
                Source::New,
                Source::Black,
                Source::White,
                Source::OldBack,
                Source::NewBack,
                Source::OldContent,
                Source::NewContent,
            ] {
                for alpha in [0.29, 1.] {
                    let layers = [Layer::full(source, alpha)];
                    let expected = composite_transition(old, &new, parts, &layers);
                    let actual = device.render(&mut pair, &layers)?;
                    maximum = maximum.max(
                        expected
                            .pixels
                            .iter()
                            .zip(actual.pixels)
                            .map(|(a, b)| a.abs_diff(b))
                            .max()
                            .unwrap_or(0),
                    );
                    checks += 1;
                }
            }
        }
        if maximum > 2 || device.accepts(8192, 8192, true) || device.accepts(0, 1, false) {
            return Err(
                "missing slide, premultiplied source or device allocation regression".into(),
            );
        }
        Some(serde_json::json!({"frames":checks,"maximumChannelDifference":maximum}))
    } else {
        None
    };
    let mut frames = BTreeMap::new();
    #[cfg(feature = "transition-gpu")]
    let mut differences = BTreeMap::new();
    for &(kind, _, _, _, options) in TRANSITIONS {
        for option in std::iter::once("").chain(options.iter().copied().filter(|o| !o.is_empty())) {
            for progress in [0, 22, 50, 78, 100] {
                let layers = transition_frame(kind, option, f64::from(progress) / 100., 16. / 9.);
                let result = composite_transition(Some(&old), &new, Some(&parts), &layers);
                let key = format!("{kind}/{option}/{progress}");
                #[cfg(feature = "transition-gpu")]
                if let Some((device, pair)) = &mut gpu {
                    let actual = device.render(pair, &layers)?;
                    let mut counts = [0u64; 5];
                    for (a, b) in actual
                        .pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .zip(result.pixels.as_chunks::<4>().0)
                    {
                        let error = a
                            .iter()
                            .zip(b)
                            .map(|(x, &y)| x.abs_diff(y))
                            .max()
                            .unwrap_or(0);
                        for (i, threshold) in [0, 1, 4, 16].into_iter().enumerate() {
                            counts[i] += u64::from(error > threshold);
                        }
                        counts[4] = counts[4].max(u64::from(error));
                    }
                    if counts[4] > 2 || ((progress == 0 || progress == 100) && counts[4] != 0) {
                        return Err(format!("GPU disagreement at {key}, counts {counts:?}").into());
                    }
                    differences.insert(key.clone(), counts);
                }
                frames.insert(key, format!("{:x}", Sha256::digest(result.pixels)));
            }
        }
    }
    #[cfg(feature = "transition-gpu")]
    if let Some((device, _)) = gpu {
        let reference = serde_json::to_vec(&frames)?;
        let all = serde_json::json!({"adapter":device.adapter.name,"backend":format!("{:?}",device.adapter.backend),
            "type":format!("{:?}",device.adapter.device_type),"size":size,"missingSources":missing_sources,"cpuManifestSha256":format!("{:x}",Sha256::digest(reference)),
            "columns":["pixelsOver0","pixelsOver1","pixelsOver4","pixelsOver16","maximumChannelDifference"],"frames":differences});
        println!("{}", serde_json::to_string(&all)?);
        return Ok(());
    }
    println!("{}", serde_json::to_string(&frames)?);
    Ok(())
}
