use super::*;
impl Viewer {
    pub(super) fn request_scene(&mut self, width: u32, cx: &mut Context<Self>) {
        let key = (self.render_generation, self.page, width);
        if self
            .scene
            .as_ref()
            .is_some_and(|(k, s)| *k == key && s.fonts_current())
            || (self.scene_busy && self.scene_pending == Some(key))
        {
            return;
        }
        if self.scene_busy {
            self.scene_cancel.store(true, Ordering::Relaxed);
            return;
        }
        let Some(e) = &self.editor else { return };
        let model = e.session.model_snapshot();
        let cache = self.scene_cache.clone();
        let scale = f64::from(width) / model.slide_size.width;
        let cancel = Arc::new(AtomicBool::new(false));
        self.scene_cancel = cancel.clone();
        self.scene_pending = Some(key);
        self.scene_busy = true;
        self.scene_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    cache
                        .lock()
                        .map_err(|_| "scene cache lock poisoned".to_string())?
                        .build(&model, key.1, scale, &|| cancel.load(Ordering::Relaxed))
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                s.scene_busy = false;
                if key.0 != s.render_generation
                    || key.1 != s.page
                    || s.scene_cancel.load(Ordering::Relaxed)
                {
                    s.scene_pending = None;
                    cx.notify();
                    return;
                }
                match result {
                    Ok(scene) => {
                        eprintln!(
                            "R4 scene {}",
                            serde_json::to_string(&scene.stats).unwrap_or_default()
                        );
                        s.scene = Some((key, Arc::new(scene)));
                    }
                    Err(error) => {
                        s.message = format!("场景失败，已切换 CPU 对照：{error}");
                        s.cpu_view = true;
                    }
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn paint_scene(&mut self, bounds: Bounds<Pixels>, scale: f32, w: &mut Window) {
        if self.benchmark.is_some() {
            return;
        }
        if self.cpu_view || self.inline.is_some() {
            return;
        }
        let Some((key, scene)) = &self.scene else {
            return;
        };
        if key.0 != self.render_generation || key.1 != self.page {
            return;
        }
        let delta = self
            .drag
            .as_ref()
            .map(editing::Drag::move_preview)
            .unwrap_or_default();
        let moved = self
            .editor
            .as_ref()
            .map(|e| e.session.selection().shapes.as_slice())
            .unwrap_or_default();
        if let Err(error) = self.gpu_cache.paint(scene, bounds, scale, moved, delta, w) {
            self.message = format!("GPUI 绘制错误：{error}");
        }
    }
    pub(super) fn scene_report(&self, w: &mut Window, cx: &mut Context<Self>) {
        let scene = self.scene.as_ref().map(|(_, s)| &s.stats);
        let reasons: Vec<_> = self
            .scene
            .iter()
            .flat_map(|(_, s)| &s.nodes)
            .filter_map(|n| {
                n.geometry
                    .fallback
                    .as_ref()
                    .map(|r| format!("{:?}: {r}", n.id))
            })
            .collect();
        let value = serde_json::json!({"view": if self.cpu_view {"cpu"}else{"gpui-scene"}, "scene":scene,"gpui":self.gpu_cache.stats,"fallbacks":reasons,"timing":"CPU build / draw submission, not presentation latency"});
        let report = serde_json::to_string_pretty(&value).unwrap_or_default();
        eprintln!("R4 diagnostics {value}");
        let view = cx.entity().downgrade();
        let report_height = (f32::from(w.viewport_size().height) - 260.).max(140.);
        w.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title("GPUI 场景 / CPU 对照诊断")
                .child(
                    div()
                        .id("scene-report-scroll")
                        .max_h(px(report_height))
                        .overflow_y_scroll()
                        .text_xs()
                        .child(report.clone()),
                )
                .child(
                    Button::new("run-render-benchmark")
                        .label("运行桌面对照（各 60 帧）")
                        .on_click({
                            let view = view.clone();
                            move |_, w, cx| {
                                let _ = view.update(cx, |s, cx| {
                                    w.close_dialog(cx);
                                    s.start_benchmark(cx);
                                });
                            }
                        }),
                )
        });
    }
}
