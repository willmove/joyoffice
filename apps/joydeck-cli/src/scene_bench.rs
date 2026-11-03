use joydeck_shell::{Cancellation, Document};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc, time::Instant};
fn percentiles(values: &[u64]) -> serde_json::Value {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let p = |q: f64| {
        sorted[((sorted.len() as f64 * q).ceil() as usize)
            .saturating_sub(1)
            .min(sorted.len() - 1)]
    };
    serde_json::json!({"samples":values,"unit":"microseconds","p50":p(0.5),"p95":p(0.95),"p99":p(0.99)})
}
pub(super) fn run(input: &Path, page: usize, count: usize, output: &Path) -> Result<(), String> {
    if !(1..=1000).contains(&count) {
        return Err("iterations must be 1..1000".into());
    }
    let doc = Document::open(input, &Cancellation::default())?;
    let page = page
        .checked_sub(1)
        .filter(|i| *i < doc.presentation.slides.len())
        .ok_or("page outside document")?;
    let p = &doc.presentation;
    let scale = (960. / p.slide_size.width)
        .min(540. / p.slide_size.height)
        .clamp(0.05, 1.);
    let mut cache = joydeck_render::scene::Cache::default();
    let start = Instant::now();
    let scene = cache.build(p, page, scale, &|| false)?;
    let cold = start.elapsed().as_micros() as u64;
    let reference = doc.render(page, scale, &Cancellation::default())?;
    let actual = joydeck_render::scene::raster(&scene, scale)?;
    let mut bad = 0usize;
    let mut absolute = 0u64;
    let mut peak = 0u8;
    if reference.pixels.len() != actual.pixels.len() {
        return Err("scene dimensions mismatch".into());
    }
    for (a, b) in reference.pixels.iter().zip(&actual.pixels) {
        let d = a.abs_diff(*b);
        bad += usize::from(d > 12);
        absolute += u64::from(d);
        peak = peak.max(d);
    }
    let fraction = bad as f64 / reference.pixels.len() as f64;
    let mut cpu = vec![];
    let mut cached = vec![];
    let mut model = p.clone();
    let id = scene
        .nodes
        .iter()
        .find(|n| n.id.is_some() && n.geometry.fallback.is_none())
        .and_then(|n| n.id);
    let base = joydeck_model::resolve::Ctx::for_slide(p, &p.slides[page]).and_then(|ctx| {
        id.and_then(|id| p.slides[page].shape(id))
            .map(|s| joydeck_model::resolve::xfrm(&ctx, s))
    });
    for i in 0..count {
        if let (Some(id), Some(mut x)) = (id, base) {
            x.x += if i % 2 == 0 { 1. } else { -1. };
            let slide = Arc::make_mut(&mut model.slides[page]);
            if let Some(shape) = slide.shapes.iter_mut().find(|s| s.id == id) {
                shape.xfrm = Some(x);
            }
        }
        let start = Instant::now();
        let _ = joydeck_render::render_slide(
            &model,
            page,
            &joydeck_render::RenderOpts {
                scale,
                threads: 0,
                ..Default::default()
            },
        );
        cpu.push(start.elapsed().as_micros() as u64);
        let start = Instant::now();
        let _ = cache.build(&model, page, scale, &|| false)?;
        cached.push(start.elapsed().as_micros() as u64);
    }
    let fonts: Vec<_> = scene
        .nodes
        .iter()
        .flat_map(|n| &n.geometry.glyphs)
        .map(|g| {
            (
                g.face.index(),
                format!("{:x}", Sha256::digest(g.face.data())),
            )
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let report = serde_json::json!({"input":input,"source_sha256":format!("{:x}",Sha256::digest(doc.original.as_ref())),"page":page+1,"scale":scale,"build":"release flag must be recorded by runner","consumer":"headless vello_cpu scene reference; GPUI comparison requires desktop screenshots","workload":"alternate +1/-1 pt translation of first native object; no text reshaping expected","translated_shape":id,"cold_scene_build_us":cold,"cpu_full_page":percentiles(&cpu),"cached_scene_build":percentiles(&cached),"scene_stats":cache.stats,"font_bytes_sha256_and_face":fonts,"parity":{"channels_over_12":bad,"fraction":fraction,"mean_absolute_error":absolute as f64/reference.pixels.len() as f64,"max_error":peak,"fixed_fraction_limit":0.01,"passed":fraction<=0.01},"presentation_latency":"unmeasured: CPU preparation is not GPU draw or presented-frame latency"});
    std::fs::create_dir(output).map_err(|e| format!("output directory must be new: {e}"))?;
    joydeck_shell::platform::publish(&output.join("cpu.png"), &reference.to_png(), false)?;
    joydeck_shell::platform::publish(&output.join("scene-reference.png"), &actual.to_png(), false)?;
    joydeck_shell::platform::publish(
        &output.join("benchmark.json"),
        &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        false,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    if fraction > 0.01 {
        return Err("scene reference parity exceeds fixed 1% channel threshold".into());
    }
    Ok(())
}
