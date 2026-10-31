//! Exercises the same desktop handlers that buttons invoke, then asks an external process to capture.
use super::*;
use crate::slideshow;
pub(super) fn run(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let out = PathBuf::from(std::env::var_os("JOYDECK_M12_OUTPUT").unwrap());
        let _ = std::fs::create_dir_all(&out);
        let result: std::result::Result<(), String> = async {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            handle
                .update(cx, |_, w, cx| {
                    view.update(cx, |s, cx| -> std::result::Result<(), String> {
                        w.set_window_title("JoyDeck — M11 M12 Acceptance");
                        s.session = Session::from_deck(joydeck::authoring_demo::deck(), false);
                        s.renew_recovery();
                        s.page = 0;
                        s.select(0, w, cx);
                        s.ribbon = "审阅".into();
                        s.extended_fields[1]
                            .update(cx, |f, cx| f.set_value("请核对季度数据", w, cx));
                        s.extended_review("add", cx);
                        s.extended_fields[1].update(cx, |f, cx| f.set_value("数据已经复核", w, cx));
                        s.extended_review("reply", cx);
                        s.extended_review("resolve", cx);
                        assert_eq!(s.session.deck.review.threads.len(), 1);
                        assert_eq!(s.session.deck.review.threads[0].messages.len(), 2);
                        assert!(s.session.deck.review.threads[0].resolved);
                        let baseline = s.session.deck.clone();
                        s.session.undo();
                        assert!(!s.session.deck.review.threads[0].resolved);
                        s.session.redo();
                        assert_eq!(baseline, s.session.deck);
                        s.extended_review("check", cx);
                        s.changed(cx);
                        Ok(())
                    })
                })
                .map_err(|e| e.to_string())??;
            for phase in 0..3 {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                if phase == 1 {
                    handle.update(cx, |_, _, cx| view.update(cx, |s, cx| s.media_action("pause", cx)))
                        .map_err(|e| e.to_string())?;
                    cx.background_executor().timer(Duration::from_millis(300)).await;
                    handle.update(cx, |_, _, cx| view.update(cx, |s, _| -> std::result::Result<(), String> {
                        let status = s.av_player.as_ref().ok_or("no player")?.status.lock().unwrap().clone();
                        let frame = status.frame.ok_or("no paused video frame")?;
                        if s.av_image.is_none() || status.sequence < 3 { return Err("video was not presented".into()); }
                        let mut rgba = frame.bgra.clone();
                        for p in rgba.as_chunks_mut::<4>().0 { p.swap(0, 2); }
                        image::RgbaImage::from_raw(frame.width, frame.height, rgba).unwrap()
                            .save(out.join("video-reference.png")).map_err(|e| e.to_string())?;
                        Ok(())
                    })).map_err(|e| e.to_string())??;
                }
                eprintln!("M12_CAPTURE {phase}");
                let start = Instant::now();
                while !out.join(format!("capture-{phase}.ack")).exists() {
                    if start.elapsed() > Duration::from_secs(20) {
                        return Err("capture timeout".into());
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                }
                if phase == 0 {
                    let asset = cx
                        .background_executor()
                        .spawn(async {
                            joydeck::playback::import(
                                &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                    .join("../../tests/fixtures/media/original-h264.mp4"),
                            )
                        })
                        .await?;
                    handle
                        .update(cx, |_, w, cx| {
                            view.update(cx, |s, cx| {
                                let id = asset.id();
                                let mut p = s.session.deck.playback.clone();
                                p.assets.insert(id.clone(), Arc::new(asset));
                                p.clips.push(joydeck::playback::Clip {
                                    id: "video-1".into(),
                                    slide: s.session.deck.slides[0].id.clone(),
                                    asset: id,
                                    start: 0.,
                                    end: 4.8,
                                    volume: 0.,
                                    looping: true,
                                    autoplay: true,
                                    cross_slide: false,
                                });
                                s.command(Command::Playback(p), cx);
                                s.clip_pick = 0;
                                s.ribbon = "媒体".into();
                                s.extended_fields[5].update(cx, |f, cx| f.set_value("4.8", w, cx));
                                s.media_action("preview", cx);
                                s.changed(cx);
                            })
                        })
                        .map_err(|e| e.to_string())?;
                } else if phase == 1 {
                    handle
                        .update(cx, |_, _, cx| {
                            view.update(cx, |s, cx| -> std::result::Result<(), String> {
                                let status = s
                                    .av_player
                                    .as_ref()
                                    .ok_or("no player")?
                                    .status
                                    .lock()
                                    .unwrap()
                                    .clone();
                                if status.frame.is_none() || status.sequence < 3 {
                                    return Err(format!("missing video frame {:?}", status.error));
                                }
                                s.media_action("pause", cx);
                                s.media_action("stop", cx);
                                s.ribbon = "输出".into();
                                s.changed(cx);
                                Ok(())
                            })
                        })
                        .map_err(|e| e.to_string())??;
                }
            }
            let d = handle
                .update(cx, |_, _, cx| view.read(cx).session.deck.clone())
                .map_err(|e| e.to_string())?;
            if std::env::var_os("JOYDECK_M12_GUI").is_some() {
                let before = handle.update(cx, |_, _, cx| {
                    view.update(cx, |s, cx| {
                        let before = (s.session.deck.clone(), s.session.revision, s.session.dirty());
                        s.extended_export("print", cx);
                        before
                    })
                }).map_err(|e| e.to_string())?;
                let start = Instant::now();
                loop {
                    cx.background_executor().timer(Duration::from_millis(100)).await;
                    let busy = handle.update(cx, |_, _, cx| view.read(cx).delivery_busy).map_err(|e|e.to_string())?;
                    if !busy { break; }
                    if start.elapsed() > Duration::from_secs(45) { return Err("print cancel timeout".into()); }
                }
                handle.update(cx, |_, _, cx| -> std::result::Result<(), String> {
                    let s = view.read(cx);
                    if before != (s.session.deck.clone(), s.session.revision, s.session.dirty()) {
                        return Err("print cancellation changed document".into());
                    }
                    if s.message != "操作已完成；文稿未修改" { return Err(s.message.clone()); }
                    std::fs::write(out.join("print-unchanged.json"), serde_json::to_vec_pretty(&serde_json::json!({
                        "revisionUnchanged":true,"dirtyUnchanged":true,"documentUnchanged":true
                    })).unwrap()).map_err(|e|e.to_string())
                }).map_err(|e|e.to_string())??;
                let (show, _) = cx.update(|cx| slideshow::open(d.clone(), 0, cx)).map_err(|e|e.to_string())?;
                cx.background_executor().timer(Duration::from_secs(2)).await;
                eprintln!("M12_SHOW_CAPTURE");
                let start = Instant::now();
                while !out.join("capture-show.ack").exists() {
                    if start.elapsed()>Duration::from_secs(20) { return Err("show capture timeout".into()); }
                    cx.background_executor().timer(Duration::from_millis(100)).await;
                }
                show.update(cx, |_, w, _| w.remove_window()).map_err(|e|e.to_string())?;
            }
            let dir = out.clone();
            cx.background_executor()
                .spawn(async move {
                    let c = AtomicBool::new(false);
                    let pages = vec![0, 1];
                    joydeck::edit::save_new(&dir.join("review-media.joydeck"), &d)?;
                    let re = joydeck::edit::open_editable(&dir.join("review-media.joydeck"))?;
                    assert_eq!(re.review, d.review);
                    assert_eq!(re.playback, d.playback);
                    joydeck::output::vector_pdf(&dir.join("vector.pdf"), &d, &pages, &c)?;
                    for k in ["outline", "rtf", "gif"] {
                        joydeck::output::other(
                            &dir.join(format!("outline.{k}")),
                            &d,
                            &pages,
                            k,
                            &c,
                        )?;
                    }
                    for k in ["potx", "ppsx"] {
                        joydeck::output::presentation(
                            &dir.join(format!("authoring.{k}")),
                            &d,
                            k,
                            &c,
                        )?;
                    }
                    let raster = preview::render(&d, 0, 960, &c)?;
                    let mut pixels = raster.bgra;
                    for p in pixels.as_chunks_mut::<4>().0 {
                        p.swap(0, 2);
                    }
                    image::RgbaImage::from_raw(raster.width, raster.height, pixels)
                        .unwrap()
                        .save(dir.join("pdf-reference.png"))
                        .map_err(|e| e.to_string())?;
                    Ok::<_, String>(())
                })
                .await?;
            Ok(())
        }
        .await;
        match result {
            Ok(()) => {
                eprintln!("M12_SMOKE review_undo_media_frames_native_outputs=true complete=true")
            }
            Err(e) => eprintln!("M12_SMOKE failed={e}"),
        };
        cx.update(|cx| cx.quit());
    })
    .detach();
}
