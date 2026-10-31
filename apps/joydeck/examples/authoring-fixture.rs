use joydeck::{authoring_demo, edit, export, motion, pptx_write, preview, show};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
fn main() {
    let out = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("output folder");
    std::fs::create_dir_all(&out).unwrap();
    let d = authoring_demo::deck();
    edit::save(&out.join("authoring.joydeck"), &d).unwrap();
    let parts = pptx_write::generate(&d).unwrap();
    export::save_parts(&out.join("authoring.pptx"), &parts, &AtomicBool::new(false)).unwrap();
    let mut public = parts;
    public
        .get_mut("ppt/slides/slide1.xml")
        .unwrap()
        .extend_from_slice(b"<!-- independent public import -->");
    export::save_parts(
        &out.join("authoring-public.pptx"),
        &public,
        &AtomicBool::new(false),
    )
    .unwrap();
    let r = preview::render(&d, 0, 1600, &AtomicBool::new(false)).unwrap();
    let mut rgba = r.bgra;
    for p in rgba.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    image::RgbaImage::from_raw(r.width, r.height, rgba)
        .unwrap()
        .save(out.join("chart-reference.png"))
        .unwrap();
    let mut bench = (*edit::Session::new().deck).clone();
    bench.slides.clear();
    for i in 0..30 {
        let mut page = d.slides[0].clone();
        page.id = format!("bench-{i}");
        page.objects.clear();
        let mut effects = vec![];
        for j in 0..20 {
            let mut o = edit::object("rect");
            o.id = format!("bench-{i}-{j}");
            o.x = 40. + (j % 5) as f32 * 170.;
            o.y = 40. + (j / 5) as f32 * 105.;
            o.width = 140.;
            o.height = 80.;
            o.fill = "#4472c4".into();
            if j < 5 {
                effects.push(motion::Effect {
                    id: format!("e-{j}"),
                    target: o.id.clone(),
                    ..Default::default()
                });
            }
            page.objects.push(o);
        }
        bench.presentation.slides.insert(
            page.id.clone(),
            motion::Timing {
                effects,
                ..Default::default()
            },
        );
        bench.slides.push(page);
    }
    bench.validate().unwrap();
    let mut s = show::ShowSnapshot::new(Arc::new(bench), 0).unwrap();
    let mut c = motion::Controller::default();
    let mut samples = vec![];
    for p in 0..30 {
        c.jump(&mut s, p);
        for step in 1..=5 {
            c.step = step;
            for ms in [0, 100, 300, 600] {
                let start = Instant::now();
                c.seek(ms);
                let frame = c.frame(&s).unwrap();
                preview::render(&frame, s.page(), 1024, &AtomicBool::new(false)).unwrap();
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            }
        }
    }
    samples.sort_by(f64::total_cmp);
    let report = serde_json::json!({"pages":30,"objectsPerPage":20,"effectsPerPage":5,"frames":600,"width":1024,"cpuFrameMsP50":samples[300],"cpuFrameMsP95":samples[570],"cpuFrameMsMax":samples[599],"scope":"offline CPU raster; native latency and physical dual-display acceptance are separate"});
    std::fs::write(
        out.join("BENCH-SHOW.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}
