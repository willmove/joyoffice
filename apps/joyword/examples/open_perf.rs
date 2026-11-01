//! Reproducible open-path timing on caller-provided files; no document text is printed.
//! Usage: open_perf FONT FACE DOCX... (set JOYWORD_PERF_RUNS for repetitions).
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutInvalidation, LayoutLimits, LayoutReuse,
    LoadedFont, ParagraphLayoutCache, SingleFontProfile, ViewOptions, layout_incremental_reviewed,
};
use joyword_model::RevisionView;
use joyword_shell::{
    docx::DocxSession,
    input::{InputLimits, TextInput},
};
use std::{
    hash::{DefaultHasher, Hasher},
    sync::Arc,
    time::Instant,
};
fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: open_perf FONT FACE DOCX...".into());
    }
    let started = Instant::now();
    let font = LoadedFont::from_bytes(
        Arc::from(std::fs::read(&args[0])?),
        args[1].to_string_lossy().parse()?,
    )
    .map_err(|e| format!("font: {e:?}"))?;
    eprintln!("font_ms={:.3}", ms(started));
    let profile = SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    };
    let runs: usize = std::env::var("JOYWORD_PERF_RUNS")
        .unwrap_or_else(|_| "5".into())
        .parse()?;
    println!(
        "sample,run,read_ms,import_ms,input_ms,compat_ms,layout_ms,raster_ms,reuse_ms,pages,omitted,scene_digest"
    );
    for (sample, path) in args.iter().skip(2).enumerate() {
        for run in 0..runs {
            let t = Instant::now();
            let bytes = std::fs::read(path)?;
            let read = ms(t);
            let t = Instant::now();
            let session = DocxSession::open_default(&bytes)?;
            let import = ms(t);
            let t = Instant::now();
            let input = TextInput::new(session, InputLimits::default())?;
            let index = ms(t);
            let t = Instant::now();
            let display =
                joyword_layout::compatibility::with_compatible_styles(input.view().snapshot());
            let compat = ms(t);
            let mut cache = ParagraphLayoutCache::default();
            let cancel = LayoutCancellation::default();
            let invalidation = LayoutInvalidation::default();
            let t = Instant::now();
            let layout = layout_incremental_reviewed(
                &display,
                FaceSet::uniform(&font),
                profile,
                LayoutLimits::default(),
                &cancel,
                LayoutReuse {
                    cache: &mut cache,
                    previous: None,
                    invalidation: &invalidation,
                },
                RevisionView::FinalWithMarkup,
                ViewOptions::default(),
            )?;
            let layout_ms = ms(t);
            let t = Instant::now();
            if let Some(scene) = layout.pages().first() {
                let raster = joyword_render::rasterize(
                    scene,
                    std::slice::from_ref(&font),
                    joyword_render::Projection::new(96.0, 0.8)?,
                    joyword_render::RasterLimits::default(),
                    &joyword_render::RasterCancellation::default(),
                )?;
                std::hint::black_box(raster);
            }
            let raster_ms = ms(t);
            let t = Instant::now();
            let current =
                joyword_layout::compatibility::with_compatible_styles(input.view().snapshot());
            layout.ensure_current(&current, &font, profile)?;
            let reuse_ms = ms(t);
            // Equality evidence only, outside timed stages. Never print private text or
            // image data. This is a same-toolchain regression digest, not a file identity.
            let mut digest = DefaultHasher::new();
            for page in layout.pages() {
                digest.write(
                    format!("{:?}", (page.width, page.height, &page.runs, &page.rects)).as_bytes(),
                );
                for image in &page.images {
                    digest.write(
                        format!(
                            "{:?}",
                            (
                                &image.media_type,
                                image.x,
                                image.y,
                                image.width,
                                image.height
                            )
                        )
                        .as_bytes(),
                    );
                    digest.write(&image.bytes);
                }
            }
            digest.write(
                format!(
                    "{:?}",
                    (layout.lines(), layout.notes(), layout.diagnostics())
                )
                .as_bytes(),
            );
            println!(
                "{},{},{read:.3},{import:.3},{index:.3},{compat:.3},{layout_ms:.3},{raster_ms:.3},{reuse_ms:.3},{},{},{:016x}",
                sample + 1,
                run + 1,
                layout.pages().len(),
                layout.omitted_blocks(),
                digest.finish()
            );
        }
    }
    Ok(())
}
