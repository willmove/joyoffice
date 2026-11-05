//! Live desktop experiment. Reports actual preparation and canvas submission on this GPUI
//! window; neither on_next_frame nor paint return is called a displayed/presented frame.
use super::*;
use joydeck_render::scene::Scene;
pub(super) struct Benchmark {
    model: Arc<joydeck_model::Presentation>,
    scene: Arc<Scene>,
    page: usize,
    id: ShapeId,
    index: usize,
    cpu: bool,
    painted: bool,
    image: Option<Arc<RenderImage>>,
    prepared: u64,
    ready: Instant,
    samples: Vec<serde_json::Value>,
    tessellations: u64,
    textures: u64,
    initial_cpu_tiles: u64,
}
fn percentiles(values: &[u64]) -> serde_json::Value {
    if values.is_empty() {
        return serde_json::Value::Null;
    }
    let mut v = values.to_vec();
    v.sort_unstable();
    let at = |p: f64| {
        v[((v.len() as f64 * p).ceil() as usize)
            .saturating_sub(1)
            .min(v.len() - 1)]
    };
    serde_json::json!({"p50_us":at(0.5),"p95_us":at(0.95),"p99_us":at(0.99),"count":v.len()})
}
impl Viewer {
    pub(super) fn start_benchmark(&mut self, cx: &mut Context<Self>) {
        if self.inline.is_some() || self.benchmark.is_some() {
            return;
        }
        let Some((key, scene)) = &self.scene else {
            self.message = "请先等待 GPUI 场景完成".into();
            cx.notify();
            return;
        };
        if key.0 != self.render_generation || key.1 != self.page || !scene.fonts_current() {
            return;
        }
        let Some(id) = scene
            .nodes
            .iter()
            .find(|n| n.id.is_some() && n.geometry.fallback.is_none())
            .and_then(|n| n.id)
        else {
            self.message = "此页没有可供对照的原生对象".into();
            cx.notify();
            return;
        };
        let Some(e) = &self.editor else { return };
        self.benchmark = Some(Box::new(Benchmark {
            model: e.session.model_snapshot(),
            scene: scene.clone(),
            page: self.page,
            id,
            index: 0,
            cpu: false,
            painted: false,
            image: None,
            prepared: 0,
            ready: Instant::now(),
            samples: vec![],
            tessellations: self.gpu_cache.stats.tessellations,
            textures: self.gpu_cache.stats.texture_preparations,
            initial_cpu_tiles: scene.stats.cpu_tiles,
        }));
        self.message = "桌面对照：GPUI 与 CPU 各 60 帧，临时移动一个文字对象；文稿保持原样".into();
        cx.notify();
    }
    pub(super) fn paint_benchmark(
        &mut self,
        bounds: Bounds<Pixels>,
        scale: f32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(b) = &mut self.benchmark else { return };
        if b.page != self.page {
            self.benchmark = None;
            self.message = "切换页面已取消性能对照".into();
            return;
        }
        let begin = Instant::now();
        let delta = if b.index % 2 == 0 { 1. } else { -1. };
        let result = if b.cpu {
            if let Some(image) = &b.image {
                w.paint_image(
                    bounds,
                    bounds,
                    Corners::all(px(0.)),
                    image.clone(),
                    0,
                    false,
                )
                .map_err(|e| e.to_string())
            } else {
                return;
            }
        } else {
            self.gpu_cache.paint(
                &b.scene,
                bounds,
                scale,
                &[b.id],
                point(px(delta * scale), px(0.)),
                w,
            )
        };
        if let Err(error) = result {
            self.message = error;
            self.benchmark = None;
            return;
        }
        if b.painted {
            return;
        }
        b.painted = true;
        let draw = begin.elapsed().as_micros() as u64;
        b.samples.push(serde_json::json!({"mode":if b.cpu {"cpu"}else{"gpui"},"index":b.index,"prepare_us":b.prepared,"canvas_submission_us":draw,"ready_to_canvas_us":b.ready.elapsed().as_micros() as u64}));
        if b.index == 59 && b.cpu {
            let b = self.benchmark.take();
            if let Some(b) = b {
                let summaries:Vec<_>=["gpui","cpu"].into_iter().map(|mode|{
                    let rows:Vec<_>=b.samples.iter().filter(|r|r["mode"]==mode).collect();
                    let values=|key:&str|rows.iter().filter_map(|r|r[key].as_u64()).collect::<Vec<_>>();
                    serde_json::json!({"mode":mode,"preparation":percentiles(&values("prepare_us")),"canvas_submission":percentiles(&values("canvas_submission_us")),"ready_to_canvas":percentiles(&values("ready_to_canvas_us"))})
                }).collect();
                eprintln!(
                    "R4 desktop_benchmark {}",
                    serde_json::json!({"samples":b.samples,"summary":summaries,"width_px":f32::from(bounds.size.width)*w.scale_factor(),"height_px":f32::from(bounds.size.height)*w.scale_factor(),"dpi":w.scale_factor(),"zoom":scale,"page":b.page+1,"shape":b.id,"native_tessellations_added":self.gpu_cache.stats.tessellations-b.tessellations,"native_texture_preparations_added":self.gpu_cache.stats.texture_preparations-b.textures,"cpu_tiles_at_start":b.initial_cpu_tiles,"measurement":"actual desktop preparation / canvas submission; timer-driven 50 Hz upper bound; no presentation or input-to-photon measurement","cpu_frames":60,"gpui_frames":60,"gpu_specs":w.gpu_specs().map(|g|serde_json::json!({"device_name":g.device_name,"driver_name":g.driver_name,"driver_info":g.driver_info,"is_software_emulated":g.is_software_emulated}))})
                );
                if let Some(image) = b.image {
                    self.retired_images.push(image);
                }
            }
            self.message = "桌面对照完成：各 60 帧；详细百分位与样本已写入运行日志".into();
            cx.defer_in(w, |_, _, cx| cx.notify());
            return;
        }
        let next_cpu = b.cpu || b.index == 59;
        let next_index = if b.index == 59 { 0 } else { b.index + 1 };
        let (model, page, id) = (b.model.clone(), b.page, b.id);
        let dpi = w.scale_factor();
        self.benchmark_task = Some(cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(20))
                .await;
            let result = if next_cpu {
                cx.background_executor()
                    .spawn(async move {
                        let begin = Instant::now();
                        let mut p = model.as_ref().clone();
                        let ctx = joydeck_model::resolve::Ctx::for_slide(&p, &p.slides[page])
                            .ok_or("benchmark master missing")?;
                        let shape = p.slides[page].shape(id).ok_or("benchmark object missing")?;
                        let mut x = joydeck_model::resolve::xfrm(&ctx, shape);
                        x.x += if next_index % 2 == 0 { 1. } else { -1. };
                        if let Some(s) = Arc::make_mut(&mut p.slides[page]).shape_mut(id) {
                            s.xfrm = Some(x);
                        }
                        let mut image = joydeck_render::render_slide(
                            &p,
                            page,
                            &joydeck_render::RenderOpts {
                                scale: f64::from(scale * dpi),
                                threads: 0,
                                ..Default::default()
                            },
                        );
                        for p in image.pixels.as_chunks_mut::<4>().0 {
                            p.swap(0, 2);
                        }
                        let image = image_of(Raster {
                            width: image.width,
                            height: image.height,
                            bgra: image.pixels,
                        })?;
                        Ok::<_, String>((Some(image), begin.elapsed().as_micros() as u64))
                    })
                    .await
            } else {
                Ok((None, 0))
            };
            let _ = view.update(cx, |s, cx| {
                let Some(b) = &mut s.benchmark else { return };
                match result {
                    Ok((image, prepared)) => {
                        if let Some(old) = b.image.take() {
                            s.retired_images.push(old);
                        }
                        b.image = image;
                        b.prepared = prepared;
                        b.cpu = next_cpu;
                        b.index = next_index;
                        b.painted = false;
                        b.ready = Instant::now();
                    }
                    Err(e) => {
                        s.message = e;
                        s.benchmark = None;
                    }
                }
                cx.notify();
            });
        }));
    }
}
