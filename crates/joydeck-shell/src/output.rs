//! Snapshot exports. Prepare completely, check cancellation, then publish a new output.
use crate::{Cancellation, Document, storage};
pub use joydeck_pdf::PageLayout;
use joydeck_pdf::PdfOptions;
use serde::{Deserialize, Serialize};
use std::{io::Cursor, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub format: String,
    pub pages: Option<Vec<usize>>,
    pub scale: f64,
    pub dpi: f64,
    pub layout: PageLayout,
    pub transparent: bool,
    pub seconds: f64,
    pub fps: u32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            format: "pdf".into(),
            pages: None,
            scale: 1.,
            dpi: 144.,
            layout: PageLayout::Slides,
            transparent: false,
            seconds: 1.,
            fps: 10,
        }
    }
}
#[derive(Serialize)]
pub struct Report {
    pub format: String,
    pub pages: Vec<usize>,
    pub files: usize,
    pub bytes: usize,
    pub warnings: Vec<String>,
}
pub fn pages(doc: &Document, opts: &Options) -> Result<Vec<usize>, String> {
    let pages = opts
        .pages
        .clone()
        .unwrap_or_else(|| (0..doc.presentation.slides.len()).collect());
    if pages.is_empty()
        || pages.len() > 500
        || pages.iter().any(|p| *p >= doc.presentation.slides.len())
    {
        return Err("invalid output page range (1–500 pages)".into());
    }
    if !opts.scale.is_finite()
        || !(0.05..=8.).contains(&opts.scale)
        || !opts.dpi.is_finite()
        || !(36.0..=300.).contains(&opts.dpi)
        || !opts.seconds.is_finite()
        || !(0.1..=30.).contains(&opts.seconds)
        || !(1..=30).contains(&opts.fps)
    {
        return Err("invalid scale, DPI or animation duration/rate".into());
    }
    Ok(pages)
}
pub fn export(
    doc: &Document,
    path: &Path,
    opts: &Options,
    cancel: &Cancellation,
) -> Result<Report, String> {
    let selected = pages(doc, opts)?;
    if opts.transparent && opts.format != "png" {
        return Err("transparent background applies only to PNG".into());
    }
    cancel.check()?;
    if path.exists() {
        return Err("output must be a new path".into());
    }
    let mut report = Report {
        format: opts.format.clone(),
        pages: selected.iter().map(|p| p + 1).collect(),
        files: 0,
        bytes: 0,
        warnings: doc.info.warnings.clone(),
    };
    if matches!(opts.format.as_str(), "png" | "jpeg") {
        // Track only files created by this job for rollback. The directory must be new.
        std::fs::create_dir(path).map_err(|e| e.to_string())?;
        let mut created = vec![];
        let result = (|| {
            for &page in &selected {
                let image = doc.render_output(page, opts.scale, opts.transparent, cancel)?;
                let bytes = if opts.format == "png" {
                    image.to_png_at(opts.scale * 72.)
                } else {
                    image.to_jpeg_at(90, Some(opts.scale * 72.))
                };
                if bytes.is_empty() {
                    return Err("image encoder failed".into());
                }
                report.bytes += bytes.len();
                if report.bytes > 64 << 20 {
                    return Err("image output exceeds 64 MiB".into());
                }
                cancel.check()?;
                let file = path.join(format!(
                    "slide-{:04}.{}",
                    page + 1,
                    if opts.format == "png" { "png" } else { "jpg" }
                ));
                storage::save(&file, &bytes, None)?;
                created.push(file);
                report.files += 1;
            }
            cancel.check()?;
            Ok(())
        })();
        if let Err(e) = result {
            for f in created {
                let _ = std::fs::remove_file(f);
            }
            let _ = std::fs::remove_dir(path);
            return Err(e);
        }
        return Ok(report);
    }
    let bytes = match opts.format.as_str() {
        "pdf" => {
            let _guard = crate::document::RENDER_LOCK
                .lock()
                .map_err(|_| "render gate poisoned")?;
            joydeck_pdf::export_with(
                &doc.presentation,
                &PdfOptions {
                    slides: Some(selected.clone()),
                    include_hidden: true,
                    dpi: opts.dpi,
                    layout: opts.layout,
                    ..Default::default()
                },
                &|| cancel.check().is_err(),
            )
            .map_err(|e| e.to_string())?
        }
        "outline" | "rtf" | "vtt" => text(doc, &selected, &opts.format, opts.seconds)?,
        "gif" => gif(doc, &selected, opts, cancel)?,
        "mp4" => video(doc, &selected, opts, cancel)?,
        _ => return Err("unsupported export format (pdf/png/jpeg/gif/mp4/outline/rtf/vtt)".into()),
    };
    if bytes.len() > 64 << 20 {
        return Err("output exceeds 64 MiB".into());
    }
    cancel.check()?;
    storage::save(path, &bytes, None)?;
    report.bytes = bytes.len();
    report.files = 1;
    if opts.format == "pdf" {
        report.warnings.push("Slide artwork is rasterized; searchable positioned glyph subsets, links and bookmarks use the shared layout. No PDF/A or tagged-PDF claim.".into());
    }
    if opts.format == "mp4" {
        report.warnings.push("Static slide MP4 only; object animations, embedded media and speaker notes are not included. VTT exports slide titles separately.".into());
    }
    Ok(report)
}

fn video(
    doc: &Document,
    pages: &[usize],
    opts: &Options,
    cancel: &Cancellation,
) -> Result<Vec<u8>, String> {
    use std::{
        io::Read,
        process::Stdio,
        time::{Duration, Instant},
    };
    if pages.len() > 120 || pages.len() as f64 * opts.seconds > 60. {
        return Err("static MP4 exceeds 120 pages / 60 seconds".into());
    }
    let helper = crate::helpers::find("video")?;
    let scratch = crate::helpers::Scratch::new()?;
    let directory = scratch.0.join("pages");
    let report = export(
        doc,
        &directory,
        &Options {
            format: "png".into(),
            pages: Some(pages.to_vec()),
            scale: opts.scale,
            ..Default::default()
        },
        cancel,
    )?;
    let names = report
        .pages
        .iter()
        .map(|p| format!("slide-{p:04}.png"))
        .collect::<Vec<_>>();
    let manifest = directory.join("video.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(
            &serde_json::json!({"pages":names,"seconds":opts.seconds,"fps":opts.fps}),
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    cancel.check()?;
    let mut child = crate::helpers::command(helper)
        .arg(crate::helpers::native_path(&manifest)?)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        if cancel.check().is_err() || start.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("video export cancelled/timed out".into());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err("native MP4 encoder failed; no output published".into());
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    let mut bytes = vec![];
    std::fs::File::open(directory.join("video.mp4"))
        .map_err(|e| e.to_string())?
        .take((64 << 20) + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > 64 << 20 {
        return Err("MP4 output empty or exceeds 64 MiB".into());
    }
    cancel.check()?;
    Ok(bytes)
}
fn text(doc: &Document, pages: &[usize], kind: &str, seconds: f64) -> Result<Vec<u8>, String> {
    let mut text = if kind == "vtt" {
        "WEBVTT\n\n".to_string()
    } else {
        String::new()
    };
    for (i, &p) in pages.iter().enumerate() {
        if kind == "vtt" {
            let stamp = |s: f64| {
                let m = (s * 1000.).round() as u64;
                format!(
                    "{:02}:{:02}:{:02}.{:03}",
                    m / 3600000,
                    m / 60000 % 60,
                    m / 1000 % 60,
                    m % 1000
                )
            };
            let title = doc.info.slides[p]
                .title
                .replace(['\r', '\n'], " ")
                .replace("-->", "→");
            text.push_str(&format!(
                "{}\n{} --> {}\n{}\n\n",
                i + 1,
                stamp(i as f64 * seconds),
                stamp((i + 1) as f64 * seconds),
                title
            ));
        } else {
            text.push_str(&format!("Page {} · {}\n", p + 1, doc.info.slides[p].title));
            for placed in joydeck_render::place_slide(&doc.presentation, p).texts {
                text.push_str(&placed.body.text());
                text.push('\n');
            }
            text.push('\n');
        }
        if text.len() > 8 << 20 {
            return Err("text output exceeds 8 MiB".into());
        }
    }
    if kind != "rtf" {
        return Ok(text.into_bytes());
    }
    let mut r = String::from("{\\rtf1\\ansi\\uc1 ");
    for c in text.chars() {
        match c {
            '\n' => r.push_str("\\par\n"),
            '\\' | '{' | '}' => {
                r.push('\\');
                r.push(c)
            }
            c if c.is_ascii() => r.push(c),
            c => {
                for u in c.encode_utf16(&mut [0; 2]) {
                    r.push_str(&format!("\\u{}?", *u as i16));
                }
            }
        }
    }
    r.push('}');
    Ok(r.into_bytes())
}
fn gif(
    doc: &Document,
    pages: &[usize],
    opts: &Options,
    cancel: &Cancellation,
) -> Result<Vec<u8>, String> {
    let frames = (opts.seconds * f64::from(opts.fps)).ceil() as u32;
    let w = (doc.info.width_pt * opts.scale).ceil();
    let h = (doc.info.height_pt * opts.scale).ceil();
    if w > 4096.
        || h > 4096.
        || w * h > 4_000_000.
        || w * h * f64::from(frames) * pages.len() as f64 > 64_000_000.
        || frames as usize * pages.len() > 300
    {
        return Err("GIF exceeds 4 MP/frame, 64 MP aggregate or 300 frames".into());
    }
    let mut out = Cursor::new(Vec::new());
    let _guard = crate::document::RENDER_LOCK
        .lock()
        .map_err(|_| "render gate poisoned")?;
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut out);
        encoder
            .set_repeat(image::codecs::gif::Repeat::Infinite)
            .map_err(|e| e.to_string())?;
        for &p in pages {
            let timeline = joydeck_anim::ShowState::timeline(&doc.presentation, p);
            for frame in 0..frames {
                cancel.check()?;
                let time = f64::from(frame) / f64::from(opts.fps);
                let state = |id| crate::output::shape_state(&timeline, id, time);
                let panics = joydeck_render::panics_caught();
                let gif_times = doc.presentation.slides[p]
                    .shapes
                    .iter()
                    .map(|s| (s.id, time))
                    .collect::<Vec<_>>();
                let image = joydeck_render::render_slide(
                    &doc.presentation,
                    p,
                    &joydeck_render::RenderOpts {
                        scale: opts.scale,
                        state: Some(&state),
                        gif_times: &gif_times,
                        threads: 0,
                        ..Default::default()
                    },
                );
                if joydeck_render::panics_caught() != panics || image.pixels.is_empty() {
                    return Err("GIF renderer failed".into());
                }
                let pixels =
                    image::RgbaImage::from_raw(image.width, image.height, image.to_straight())
                        .ok_or("GIF raster failure")?;
                encoder
                    .encode_frame(image::Frame::from_parts(
                        pixels,
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(1000, opts.fps),
                    ))
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(out.into_inner())
}
pub fn shape_state(
    timeline: &joydeck_anim::Timeline,
    id: joydeck_model::ShapeId,
    time: f64,
) -> joydeck_render::ShapeState {
    let s = timeline.state_at(id, None, time);
    joydeck_render::ShapeState {
        visible: s.visible,
        opacity: s.opacity,
        offset: (s.offset_x, s.offset_y).into(),
        scale: (s.scale_x, s.scale_y),
        rotate: s.rotate,
        clip: s.clip,
        tint: s.tint,
        paras: timeline
            .paragraph_targets(id)
            .into_iter()
            .map(|p| {
                let s = timeline.state_at(id, Some(p), time);
                (
                    p,
                    joydeck_render::ParaState {
                        visible: s.visible,
                        opacity: s.opacity,
                        offset: (s.offset_x, s.offset_y).into(),
                    },
                )
            })
            .collect(),
    }
}

pub fn print(doc: &Document, opts: &Options, cancel: &Cancellation) -> Result<String, String> {
    use std::time::{Duration, Instant};
    if opts.layout != PageLayout::Slides {
        return Err(
            "Windows print currently supports slide pages; export notes/handouts as PDF".into(),
        );
    }
    let helper = crate::helpers::find("print")?;
    let scratch = crate::helpers::Scratch::new()?;
    let directory = scratch.0.join("pages");
    let report = export(
        doc,
        &directory,
        &Options {
            format: "png".into(),
            scale: opts.dpi / 72.,
            pages: opts.pages.clone(),
            ..Default::default()
        },
        cancel,
    )?;
    let names = report
        .pages
        .iter()
        .map(|p| format!("slide-{p:04}.png"))
        .collect::<Vec<_>>();
    let manifest = directory.join("print.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({"pages":names})).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    cancel.check()?;
    let mut child = crate::helpers::command(helper)
        .arg(crate::helpers::native_path(&manifest)?)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        if cancel.check().is_err() || start.elapsed() > Duration::from_secs(300) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("print cancelled or timed out; a job already submitted to the spooler may still print".into());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return match status.code() {
                Some(0) => Ok("submitted".into()),
                Some(2) => Ok("cancelled".into()),
                _ => Err("Windows printing failed; no success claimed".into()),
            };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
