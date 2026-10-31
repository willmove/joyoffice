//! Repeatable qualification data; all corpus documents are project-authored, not external Office corpus.
use joydeck::{
    authoring_demo,
    document::Result,
    edit::{self, Command, Session},
    export, interop, media, pptx_write, preview, review, storage, table,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
fn hash(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}
fn write(path: &Path, value: &serde_json::Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn quantile(samples: &mut [f64], q: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[((samples.len() - 1) as f64 * q).ceil() as usize]
}
fn corpus(out: &Path) -> Result<()> {
    let cancel = AtomicBool::new(false);
    std::fs::create_dir(out).map_err(|e| e.to_string())?;
    let mut manifest = vec![];
    for index in 0..128 {
        let family = index % 8;
        let variant = index / 8;
        let mut d = authoring_demo::deck();
        d.title = format!("Original qualification {index:03}");
        d.field_date = "2026-10-31".into();
        match family {
            0 => {
                d.slides[0].objects[0].text = format!("业务 {variant} · café 中文😀");
                d.slides[0].objects[0].font_size = 18. + (variant % 4) as f32 * 6.;
            }
            1 => {
                for (i, kind) in ["rect", "ellipse", "roundRect", "line", "arrow"]
                    .iter()
                    .enumerate()
                {
                    let mut o = edit::object(kind);
                    o.id = format!("geometry-{index}-{i}");
                    o.x = 40. + i as f32 * 150.;
                    o.y = 80. + variant as f32 * 4.;
                    o.rotation = (variant * 15) as f32;
                    o.opacity = 0.25 + variant as f32 / 24.;
                    o.fill = "#48a89c".into();
                    d.slides[0].objects.push(o);
                }
            }
            2 => {
                let mut o = table::object(2 + variant % 4, 3)?;
                o.id = format!("table-{index}");
                o.y = 120.;
                for (i, c) in o.table.as_mut().unwrap().cells.iter_mut().enumerate() {
                    c.text = format!("项目{variant}-{i}");
                }
                d.slides[0].objects.push(o);
            }
            3 => {
                d.slides[0].objects[1].chart.as_mut().unwrap().series[0].values = vec![
                    Some(-20. - variant as f64),
                    Some(0.),
                    Some(31. + variant as f64),
                    Some(60.),
                ];
            }
            4 => {
                let image = image::RgbaImage::from_fn(32, 32, |x, y| {
                    image::Rgba([(x * 8) as u8, (y * 8) as u8, (variant * 15) as u8, 255])
                });
                let mut png = std::io::Cursor::new(vec![]);
                image::DynamicImage::ImageRgba8(image)
                    .write_to(&mut png, image::ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                let resource = media::Resource::import(png.get_ref())?;
                let id = resource.id();
                d.resources.insert(id.clone(), Arc::new(resource));
                let mut o = edit::object("image");
                o.id = format!("image-{index}");
                o.image = Some(media::ImageRef {
                    resource: id,
                    crop: [0.1, 0.1, 0.8, 0.8],
                });
                d.slides[0].objects.push(o);
            }
            5 => {
                d.slides[1].hidden = true;
                d.slides[0].notes = format!("Original speaker notes {variant}");
                d.presentation.order = vec!["page-0".into(), "page-2".into()];
            }
            6 => {
                d.review.threads.push(review::Thread {
                    id: format!("thread-{index}"),
                    slide: "page-0".into(),
                    object: Some("title-0".into()),
                    resolved: variant.is_multiple_of(2),
                    messages: vec![review::Message {
                        author: "JoyDeck original fixture".into(),
                        text: format!("Review {variant}"),
                    }],
                });
                d.review.accessibility.insert(
                    "content-0".into(),
                    review::Access {
                        alt: "Original business chart".into(),
                        decorative: false,
                    },
                );
            }
            _ => {
                let t = d.presentation.slides.get_mut("page-0").unwrap();
                t.transition = if variant.is_multiple_of(2) {
                    "push"
                } else {
                    "wipe"
                }
                .into();
                t.duration_ms = 300 + variant as u64 * 20;
                t.effects[0].kind = if variant.is_multiple_of(2) {
                    "fade"
                } else {
                    "fly"
                }
                .into();
            }
        }
        d.validate()?;
        let name = format!("case-{index:03}");
        let native = out.join(format!("{name}.joydeck"));
        let public = out.join(format!("{name}.pptx"));
        edit::save_new(&native, &d)?;
        let reopened = edit::open_editable(&native)?;
        if reopened != d {
            return Err(format!("native mismatch {name}"));
        }
        let mut parts = pptx_write::generate(&d)?;
        // Invalidate the private snapshot digest to require a public DrawingML import.
        parts
            .get_mut("ppt/slides/slide1.xml")
            .unwrap()
            .extend_from_slice(b"<!-- force public qualification -->");
        export::save_parts(&public, &parts, &cancel)?;
        let imported = interop::import(&public)?;
        if imported.slides.len() != 9 {
            return Err(format!("public pages {name}"));
        }
        let raster = preview::render(&d, 0, 320, &cancel)?;
        let expected_values = d.slides[0]
            .objects
            .iter()
            .find_map(|o| o.chart.as_ref())
            .unwrap()
            .series[0]
            .values
            .clone();
        manifest.push(json!({"id":name,"family":(["text","geometry","table","chart-data","image-crop","organization","review-native","motion"][family]),"variant":variant,"origin":"project-authored","license":"MIT OR Apache-2.0","nativeSha256":hash(&native),"publicPptxSha256":hash(&public),"pages":9,"firstTitle":d.slides[0].objects[0].text,"firstChartValues":expected_values,"nativeRoundtrip":true,"publicImportForced":true,"render320Sha256":format!("{:x}",Sha256::digest(&raster.bgra)),"declaredPublicLosses":pptx_write::losses(&d)}));
    }
    write(
        &out.join("manifest.json"),
        &json!({"documents":manifest,"count":128,"externalDocuments":0,"fonts":"runtime system fonts; no fonts redistributed","scope":"128 variations from 8 project-authored families; not 100 independently sourced Office files"}),
    );
    Ok(())
}
fn soak(out: &Path, seconds: u64) -> Result<()> {
    let mut s = Session::new();
    let mut seed = 0x4a6f796465636b13u64;
    let start = Instant::now();
    let stop = Duration::from_secs(seconds);
    let mut operations = 0u64;
    let mut checkpoints = 0u64;
    let mut times = vec![];
    let path = out.join("soak-checkpoint.joydeck");
    let mut fingerprint = None;
    while start.elapsed() < stop {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let page = (seed as usize) % s.deck.slides.len();
        let count = s.deck.slides[page].objects.len();
        let action = match seed % 12 {
            0 if s.deck.slides.len() < 8 => Command::AddPage,
            1 if s.deck.slides.len() > 1 => Command::DeletePage(page),
            2 | 3 if count > 0 => Command::DeleteObject(page, seed as usize % count),
            4..=8 if count > 0 => {
                let index = seed as usize % count;
                let mut o = s.deck.slides[page].objects[index].clone();
                o.x = (seed % 700) as f32;
                o.y = ((seed >> 12) % 400) as f32;
                o.rotation = (seed % 360) as f32;
                Command::ReplaceObject(page, index, o)
            }
            _ if count < 48 => {
                let mut o = edit::object(if seed.is_multiple_of(2) {
                    "rect"
                } else {
                    "text"
                });
                o.x = (seed % 700) as f32;
                o.text = format!("seeded 中文😀 {}", seed % 100);
                Command::AddObject(page, o)
            }
            _ => Command::DeleteObject(page, 0),
        };
        let before = s.deck.clone();
        let now = Instant::now();
        s.execute(s.id, action)?;
        times.push(now.elapsed().as_secs_f64() * 1000.);
        if operations.is_multiple_of(7) && s.deck != before {
            let after = s.deck.clone();
            s.undo();
            if s.deck != before {
                return Err("soak undo mismatch".into());
            }
            s.redo();
            if s.deck != after {
                return Err("soak redo mismatch".into());
            }
        }
        s.deck.validate()?;
        operations += 1;
        if operations.is_multiple_of(500) {
            let now = Instant::now();
            fingerprint = Some(storage::save(&path, &s.deck, fingerprint)?);
            let (loaded, _) = storage::open(&path)?;
            if loaded != *s.deck {
                return Err("soak checkpoint mismatch".into());
            }
            checkpoints += 1;
            if times.len() > 100000 {
                times.drain(..50000);
            }
            eprintln!(
                "SOAK operations={operations} checkpoints={checkpoints} elapsed={:.1}",
                start.elapsed().as_secs_f64()
            );
            let _ = now;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut undo = 0;
    while s.can_undo() {
        s.undo();
        undo += 1;
        if undo > 100 {
            return Err("history budget exceeded".into());
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    let p95 = quantile(&mut times, 0.95);
    let p99 = quantile(&mut times, 0.99);
    write(
        &out.join("soak.json"),
        &json!({"seed":"0x4a6f796465636b13","elapsedSeconds":elapsed,"operations":operations,"saveOpenCheckpoints":checkpoints,"undoDepthAtEnd":undo,"commandMsP95":p95,"commandMsP99":p99,"samplesRetained":times.len(),"passed":true,"scope":"seeded transactional engine and atomic checkpoint/save/reopen on this machine; not power-loss hardware certification"}),
    );
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let out = PathBuf::from(args.get(1).ok_or("output directory required")?);
    std::fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::create_dir(&out).map_err(|e| e.to_string())?;
    corpus(&out.join("corpus"))?;
    let seconds = args
        .get(2)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(600)
        .clamp(1, 3600);
    soak(&out, seconds)?;
    write(
        &out.join("qualification.json"),
        &json!({"corpusCount":128,"externalCorpusCount":0,"soakSecondsRequested":seconds,"passed":true}),
    );
    Ok(())
}
