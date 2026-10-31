//! Native-window benchmark of the same GPUI adapter as the editor. Timings end at
//! GPUI's post-render frame callback; no claim of display scan-out measurement.
use super::*;
use joydeck::scene::{self, bench};
use scene_canvas::GpuCache;
use std::sync::Mutex;

#[derive(serde::Serialize)]
struct Measurement {
    scenario: String,
    backend: String,
    samples: usize,
    feedback_p50_ms: f64,
    feedback_p95_ms: f64,
    feedback_p99_ms: f64,
    frame_p95_ms: f64,
    stats: scene_canvas::PaintStats,
}
fn percentile(values: &[f64], p: f64) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    values[((values.len().saturating_sub(1)) as f64 * p).ceil() as usize]
}
struct BenchView {
    scenes: Vec<scene::Scene>,
    phase: usize,
    frame: usize,
    cache: GpuCache,
    cpu: Option<Arc<RenderImage>>,
    task: Option<Task<()>>,
    retired: Vec<Arc<RenderImage>>,
    cpu_ready: bool,
    pending: bool,
    submitted: bool,
    input: Instant,
    last: Option<Instant>,
    feedback: Vec<f64>,
    intervals: Vec<f64>,
    measurements: Vec<Measurement>,
    bounds: Option<Bounds<Pixels>>,
    output: PathBuf,
    build_stats: scene::Stats,
    device_scale: f32,
}
impl BenchView {
    fn scene(&self) -> &scene::Scene {
        &self.scenes[self.phase / 2]
    }
    fn cpu_mode(&self) -> bool {
        self.phase.is_multiple_of(2)
    }
    fn start(&mut self, cx: &mut Context<Self>) {
        self.pending = true;
        self.cpu_ready = false;
        self.submitted = false;
        self.input = Instant::now();
        let base = if self.phase < 2 { (8., 8.) } else { (0., 0.) };
        let position = bench::position(self.frame.saturating_sub(3).min(bench::FRAMES - 1));
        self.scenes[self.phase / 2].nodes[0].position = (base.0 + position.0, base.1 + position.1);
        if self.cpu_mode() {
            let scene = self.scene().clone();
            let raster_width = (1000. * self.device_scale).round() as u32;
            self.task = Some(cx.spawn(async move |view, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let began = Instant::now();
                        let result = scene.raster(raster_width, &AtomicBool::new(false));
                        (result, began.elapsed().as_micros() as u64)
                    })
                    .await;
                let _ = view.update(cx, |s, cx| {
                    let (result, micros) = result;
                    s.cache.stats.cpu_raster_micros += micros;
                    let r = result.expect("benchmark CPU reference");
                    s.cache.stats.full_rasters += 1;
                    s.cache.stats.uploads += 1;
                    s.cache.stats.upload_bytes += r.bgra.len() as u64;
                    s.cpu_ready = true;
                    if let Some(old) = s.cpu.take() {
                        s.retired.push(old);
                    }
                    s.cpu = Some(Arc::new(RenderImage::new(vec![image::Frame::new(
                        image::RgbaImage::from_raw(r.width, r.height, r.bgra).unwrap(),
                    )])));
                    cx.notify();
                });
            }));
        } else {
            cx.notify();
        }
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, w: &mut Window, cx: &mut Context<Self>) {
        for image in self.retired.drain(..) {
            let _ = w.drop_image(image);
        }
        self.bounds = Some(bounds);
        if self.cpu_mode() {
            let Some(image) = self.cpu.clone() else {
                return;
            };
            w.paint_image(bounds, bounds, Corners::all(px(0.)), image, 0, false)
                .expect("benchmark CPU upload");
        } else {
            let scene = self.scene().clone();
            let scale = 1000. / scene.width;
            self.cache
                .paint(&scene, &[], None, bounds, scale, w)
                .expect("benchmark GPU draw");
        }
        if !self.pending || self.submitted || (self.cpu_mode() && !self.cpu_ready) {
            return;
        }
        self.submitted = true;
        let view = cx.entity().downgrade();
        w.on_next_frame(move |w, cx| {
            let _ = view.update(cx, |s, cx| s.complete_frame(w, cx));
        });
    }
    fn complete_frame(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        if self.frame >= 3 {
            self.feedback
                .push(now.duration_since(self.input).as_secs_f64() * 1000.);
            if let Some(last) = self.last {
                self.intervals
                    .push(now.duration_since(last).as_secs_f64() * 1000.);
            }
        }
        self.last = Some(now);
        self.frame += 1;
        self.pending = false;
        if self.frame < bench::FRAMES + 3 {
            self.start(cx);
            return;
        }
        let name = if self.phase < 2 {
            "BENCH-EDIT"
        } else {
            "BENCH-EFFECT"
        };
        let backend = if self.cpu_mode() { "CPU" } else { "GPUI" };
        self.measurements.push(Measurement {
            scenario: name.into(),
            backend: backend.into(),
            samples: self.feedback.len(),
            feedback_p50_ms: percentile(&self.feedback, 0.5),
            feedback_p95_ms: percentile(&self.feedback, 0.95),
            feedback_p99_ms: percentile(&self.feedback, 0.99),
            frame_p95_ms: percentile(&self.intervals, 0.95),
            stats: self.cache.stats.clone(),
        });
        let bounds = self.bounds.unwrap();
        eprintln!(
            "M4_CAPTURE {} {} {}",
            name,
            backend,
            serde_json::json!({"x":f32::from(bounds.left()),"y":f32::from(bounds.top()),"width":f32::from(bounds.size.width),"height":f32::from(bounds.size.height),"viewportHeight":f32::from(w.viewport_size().height)})
        );
        let mut scene = self.scene().clone();
        let raster_width = (1000. * self.device_scale).round() as u32;
        let capture_ack = self.output.join(format!("capture-{}.ack", self.phase));
        // Independent CPU image is emitted at exactly the final transform.
        let path = self.output.join(format!("{name}-{backend}-reference.png"));
        self.task=Some(cx.spawn(async move|view,cx|{
            cx.background_executor().spawn(async move {
                let r=scene.raster(raster_width,&AtomicBool::new(false)).unwrap();let mut rgba=r.bgra;
                for p in rgba.as_chunks_mut::<4>().0{p.swap(0,2);}
                resvg::tiny_skia::Pixmap::from_vec(rgba,resvg::tiny_skia::IntSize::from_wh(r.width,r.height).unwrap()).unwrap().save_png(path).unwrap();
                // Retain the scene until CPU raster output is complete.
                scene.nodes.clear();
            }).await;
            if std::env::var_os("JOYDECK_M4_CAPTURE_ACK").is_some() {
                let began=Instant::now();
                while !capture_ack.exists() && began.elapsed()<Duration::from_secs(15) {
                    cx.background_executor().timer(Duration::from_millis(50)).await;
                }
                if !capture_ack.exists() {eprintln!("M4_BENCH capture_timeout=true");}
            } else {cx.background_executor().timer(Duration::from_millis(750)).await;}
            let _=view.update(cx,|s,cx|{
                s.phase+=1;
                if s.phase<4 {
                    s.frame=0;s.feedback.clear();s.intervals.clear();s.last=None;s.cache.clear();s.cache.stats=scene_canvas::PaintStats::default();if let Some(old)=s.cpu.take(){s.retired.push(old);}s.start(cx);
                } else {
                    let edit_cpu=s.measurements[0].feedback_p95_ms;let edit_gpu=s.measurements[1].feedback_p95_ms;
                    let relative=1.-edit_gpu/edit_cpu;
                    let gpu=&s.measurements[1].stats;
                    let missing_glyphs=s.scenes[0].nodes.iter().map(|n|n.geometry.missing_glyphs).sum::<usize>();
                    let gate=missing_glyphs==0&&(edit_gpu<=20. || relative>=0.4)&&gpu.full_rasters==0&&gpu.paths>0&&gpu.images>0&&gpu.glyph_masks>0;
                    let report=serde_json::json!({"deviceScale":s.device_scale,"rasterWidth":(1000.*s.device_scale).round() as u32,"seed":bench::SEED,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"framesPerBackend":bench::FRAMES,"warmupFrames":3,"metric":"synthetic scene transform -> GPUI post-render frame callback; not hardware scan-out or physical input", "geometryCache":s.build_stats,"fixture":{"objects":s.scenes[0].nodes.len(),"geometryObjects":120,"textObjects":60,"imageObjects":20,"distinctImageResources":20,"imageDimensions":[512,512],"shapedGlyphs":s.scenes[0].coverage().2,"missingGlyphs":missing_glyphs},"measurements":s.measurements,"editRelativeP95Reduction":relative,"measuredRenderingGate":gate,"overallM4Accepted":false});
                    std::fs::write(s.output.join("native-benchmark.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
                    eprintln!("M4_BENCH complete=true measured_gate={gate} edit_p95_reduction={relative:.4}");
                    s.phase=3;
                    if !std::env::args().any(|a|a=="--hold") {cx.quit();}
                }
            });
        }));
    }
}
impl Render for BenchView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text = format!(
            "M4 · {} · {} · frame {}/{}",
            if self.phase < 2 {
                "BENCH-EDIT"
            } else {
                "BENCH-EFFECT"
            },
            if self.cpu_mode() {
                "CPU reference"
            } else {
                "GPUI geometry/images"
            },
            self.frame,
            bench::FRAMES
        );
        let height = if self.phase < 2 { 750. } else { 620. };
        let weak = cx.entity().downgrade();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(0x15202d))
            .text_color(rgb(0xffffff))
            .child(text)
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, w, cx| {
                        let _ = weak.update(cx, |s, cx| s.paint(bounds, w, cx));
                    },
                )
                .w(px(1000.))
                .h(px(height)),
            )
    }
}
pub(super) fn open(cx: &mut App) {
    let output = std::env::var_os("JOYDECK_M4_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("logs/m4-native-benchmark"));
    std::fs::create_dir_all(&output).expect("benchmark output directory");
    let cache = Arc::new(Mutex::new(scene::SceneCache::default()));
    cx.spawn(async move |cx| {
        let (edit, effect, stats) = cx
            .background_executor()
            .spawn(async move {
                let mut c = cache.lock().unwrap();
                let edit = bench::edit(&mut c).unwrap();
                let effect = bench::effect().unwrap();
                (edit, effect, c.stats.clone())
            })
            .await;
        cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1120.), px(880.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                cx,
                move |w, cx| {
                    w.set_window_title("JoyDeck — M4 Native Benchmark");
                    let view = cx.new(|_| BenchView {
                        scenes: vec![edit, effect],
                        phase: 0,
                        frame: 0,
                        cache: GpuCache::default(),
                        cpu: None,
                        task: None,
                        retired: vec![],
                        cpu_ready: false,
                        pending: false,
                        submitted: false,
                        input: Instant::now(),
                        last: None,
                        feedback: vec![],
                        intervals: vec![],
                        measurements: vec![],
                        bounds: None,
                        output,
                        build_stats: stats,
                        device_scale: w.scale_factor(),
                    });
                    view.update(cx, |s, cx| s.start(cx));
                    view
                },
            )
            .unwrap();
            cx.activate(true);
        });
    })
    .detach();
}
