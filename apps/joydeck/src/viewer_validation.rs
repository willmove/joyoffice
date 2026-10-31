//! Explicit native viewer acceptance harness; enabled only by validation.
use super::*;

/// Drives production actions on a real native window; does not simulate physical keys.
pub fn smoke(handle: AnyWindowHandle, view: &Entity<Viewer>, cx: &mut App) {
    let fixtures = std::env::var_os("JOYDECK_FIXTURES")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("fixtures")))
                .filter(|p| p.is_dir())
        })
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/joydeck")
        });
    load_path(view, fixtures.join("native-20.joydeck"), cx);
    let view = view.downgrade();
    cx.spawn(async move |cx| {
        let start = Instant::now();
        let mut phase = 0;
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(60))
                .await;
            if start.elapsed().as_secs() > 30 {
                eprintln!("M1_SMOKE failed=true timeout_phase={phase}");
                break;
            }
            let result = handle.update(cx, |_, w, cx| {
                view.update(cx, |s, cx| {
                    let ready = s.image.as_ref().is_some_and(|(k, _)| {
                        k.0 == s.render_generation && k.1 == s.page && s.pending == Some(*k)
                    });
                    match phase {
                        0 if !s.loading && ready => {
                            assert_eq!(s.deck.as_ref().unwrap().slides.len(), 20);
                            eprintln!("M1_SMOKE native_open=true");
                            w.dispatch_action(Box::new(Next), cx);
                            phase = 1;
                        }
                        1 if s.page == 1 && ready => {
                            eprintln!("M1_SMOKE native_next=true");
                            w.dispatch_action(Box::new(Last), cx);
                            phase = 2;
                        }
                        2 if s.page == 19 && ready => {
                            eprintln!("M1_SMOKE last_page=true");
                            w.dispatch_action(Box::new(First), cx);
                            phase = 3;
                        }
                        3 if s.page == 0 && ready => {
                            s.select(2, cx);
                            s.request_image(640, cx);
                            s.select(0, cx);
                            phase = 4;
                        }
                        4 if ready => {
                            s.select(2, cx);
                            phase = 5;
                        }
                        5 if s.page == 2 && ready => {
                            eprintln!("M1_SMOKE rapid_navigation_no_stale=true");
                            for _ in 0..20 {
                                s.zoom_in(&ZoomIn, w, cx);
                            }
                            phase = 6;
                        }
                        6 if ready
                            && s.zoom == Some(4.)
                            && s.image.as_ref().unwrap().0.2 == 2048 =>
                        {
                            eprintln!("M1_SMOKE zoom_max=true raster_bounded=true");
                            for _ in 0..20 {
                                s.zoom_out(&ZoomOut, w, cx);
                            }
                            phase = 7;
                        }
                        7 if ready
                            && s.zoom == Some(0.25)
                            && s.image.as_ref().unwrap().0.2 < 2048 =>
                        {
                            eprintln!("M1_SMOKE zoom_min=true");
                            s.fit(&Fit, w, cx);
                            w.resize(size(px(940.), px(680.)));
                            phase = 8;
                        }
                        8 if ready && s.zoom.is_none() && s.scale(w) < 1. => {
                            eprintln!("M1_SMOKE fit_resize=true");
                            let old = s.deck.clone().unwrap();
                            s.load(fixtures.join("public-format-20.pptx"), cx);
                            s.cancel(&Cancel, w, cx);
                            assert!(Arc::ptr_eq(&old, s.deck.as_ref().unwrap()));
                            assert!(!s.loading);
                            eprintln!("M1_SMOKE cancel_preserves_document=true");
                            s.load(fixtures.join("native-20.joydeck"), cx);
                            s.load(fixtures.join("public-format-20.pptx"), cx);
                            phase = 9;
                        }
                        9 if !s.loading
                            && ready
                            && s.source
                                .as_ref()
                                .is_some_and(|p| p.extension().is_some_and(|e| e == "pptx")) =>
                        {
                            let d = s.deck.as_ref().unwrap();
                            assert_eq!(d.slides.len(), 20);
                            assert!(d.slides[0].title.contains("20"));
                            eprintln!("M1_SMOKE pptx_open_order=true");
                            w.dispatch_action(Box::new(Next), cx);
                            phase = 10;
                        }
                        10 if s.page == 1 && ready => {
                            assert!(
                                s.deck
                                    .as_ref()
                                    .unwrap()
                                    .warnings
                                    .iter()
                                    .any(|w| w.page == Some(1))
                            );
                            s.report(&Report, w, cx);
                            assert!(w.has_active_dialog(cx));
                            w.close_dialog(cx);
                            eprintln!("M1_SMOKE pptx_next_report=true");
                            s.load(fixtures.join("broken.pptx"), cx);
                            phase = 11;
                        }
                        11 if !s.loading && s.message.starts_with("打开失败") => {
                            assert_eq!(s.page, 1);
                            assert_eq!(s.deck.as_ref().unwrap().slides.len(), 20);
                            assert!(
                                s.source
                                    .as_ref()
                                    .unwrap()
                                    .ends_with("public-format-20.pptx")
                            );
                            eprintln!("M1_SMOKE failure_preserves_document=true");
                            assert!(s.thumbnails.len() <= 32);
                            eprintln!(
                                "M1_SMOKE passed=true native_pages=20 pptx_pages=20 elapsed_ms={}",
                                start.elapsed().as_millis()
                            );
                            phase = 12;
                        }
                        _ => {}
                    }
                    phase == 12
                })
            });
            match result {
                Ok(Ok(true)) => break,
                Ok(Ok(false)) => {}
                _ => {
                    eprintln!("M1_SMOKE failed=true window_closed");
                    break;
                }
            }
        }
    })
    .detach();
}
