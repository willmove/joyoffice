//! Explicit native validation routes. Compiled only with the validation feature.
use super::*;

pub(super) fn smoke(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    view.update(cx, |s, cx| {
        s.command(Command::Batch(vec![Command::AddPage, Command::AddPage]), cx);
        s.command(Command::AddObject(0, edit::object("rect")), cx);
        let mut ellipse = edit::object("ellipse");
        ellipse.x = 450.;
        ellipse.y = 300.;
        ellipse.width = 120.;
        s.command(Command::AddObject(0, ellipse), cx);
        let mut arrow = edit::object("arrow");
        arrow.x = 700.;
        arrow.y = 350.;
        arrow.width = 140.;
        arrow.height = 80.;
        s.command(Command::AddObject(0, arrow), cx);
        s.command(Command::AddObject(1, edit::object("text")), cx);
    });
    cx.spawn(async move |cx| {
        let mut phase = 0;
        let mut closing: Option<AnyWindowHandle> = None;
        let mut close_path: Option<PathBuf> = None;
        let mut discarded: Option<Recovery> = None;
        let began = Instant::now();
        let mut since = Instant::now();
        let output = std::env::temp_dir().join(format!(
            "joydeck-m2-ui-{}-{}.joydeck",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            if began.elapsed() > Duration::from_secs(25) {
                eprintln!("M2_SMOKE error=timeout phase {phase}");
                break;
            }
            let result = handle.update(cx, |_, w, cx| {
                view.update(cx, |s, cx| {
                    match phase {
                        0 if (s.image.is_some() || s.scene.is_some()) && w.is_window_active() => {
                            let scale = s.canvas_scale(w);
                            let origin = s.canvas_origin;
                            let point_at = |p: (f32, f32)| {
                                point(px(origin.0 + p.0 * scale), px(origin.1 + p.1 * scale))
                            };
                            let down = |p: (f32, f32)| MouseDownEvent {
                                button: MouseButton::Left,
                                position: point_at(p),
                                modifiers: Modifiers::default(),
                                click_count: 1,
                                first_mouse: false,
                            };
                            let motion = |p: (f32, f32)| MouseMoveEvent {
                                position: point_at(p),
                                pressed_button: Some(MouseButton::Left),
                                modifiers: Modifiers::default(),
                            };
                            let before = s.session.deck.clone();
                            let rev = s.session.revision;
                            s.down(&down((200., 130.)), scale, w, cx);
                            s.move_pointer(&motion((201., 130.)), cx);
                            s.text_anchor = None;
                            s.end_gesture(cx);
                            assert!(Arc::ptr_eq(&before, &s.session.deck));
                            assert_eq!(s.session.revision, rev);
                            s.page_drag = Some(vec![0, 1]);
                            s.cancel_gesture(&CancelGesture, w, cx);
                            assert!(s.page_drag.is_none());
                            eprintln!("M2_SMOKE click_jitter_and_page_drag_cancel=true");
                            s.down(&down((200., 130.)), scale, w, cx);
                            s.move_pointer(&motion((230., 150.)), cx);
                            assert!(Arc::ptr_eq(&before, &s.session.deck));
                            s.text_anchor = None;
                            s.end_gesture(cx);
                            assert_eq!(s.session.revision, rev + 1);
                            assert_eq!(s.session.deck.slides[0].objects[0].x, 110.);
                            s.session.undo();
                            assert!(Arc::ptr_eq(&before, &s.session.deck));
                            s.session.redo();
                            let r = geometry::bounds(&s.session.deck.slides[0].objects[0]);
                            let corner = (r.x + r.w, r.y + r.h);
                            let next = (corner.0 + 60., corner.1 + 40.);
                            let saved = s.session.deck.clone();
                            s.down(&down(corner), scale, w, cx);
                            assert!(s.gesture.as_ref().unwrap().mode == Some(Mode::Scale));
                            s.move_pointer(&motion(next), cx);
                            s.cancel_gesture(&CancelGesture, w, cx);
                            assert!(Arc::ptr_eq(&saved, &s.session.deck));
                            s.down(&down(corner), scale, w, cx);
                            s.move_pointer(&motion(next), cx);
                            s.text_anchor = None;
                            s.end_gesture(cx);
                            assert!((s.session.deck.slides[0].objects[0].width - 300.).abs() < 0.1);
                            let r = geometry::bounds(&s.session.deck.slides[0].objects[0]);
                            let start = (r.x + r.w / 2., r.y - 24. / scale);
                            let end = (r.x + r.w / 2. + 100., r.y + r.h / 2.);
                            s.down(&down(start), scale, w, cx);
                            assert!(s.gesture.as_ref().unwrap().mode == Some(Mode::Rotate));
                            s.move_pointer(&motion(end), cx);
                            s.text_anchor = None;
                            s.end_gesture(cx);
                            assert!(
                                (s.session.deck.slides[0].objects[0].rotation - 90.).abs() < 0.1
                            );
                            eprintln!("M2_SMOKE drag_scale_rotate_cancel_single_history=true");
                            s.select_multi(1, true, w, cx);
                            assert_eq!(s.selection.len(), 2);
                            let ids = s.selection.clone();
                            let base = s.selected_objects();
                            s.gesture = Some(Gesture {
                                image_before: s.image.clone(),
                                handle: (1, 1),
                                shift: false,
                                center: false,
                                base,
                                start: (0., 0.),
                                end: (20., 20.),
                                mode: Some(Mode::Move),
                                scale,
                            });
                            s.text_anchor = None;
                            s.end_gesture(cx);
                            assert_eq!(s.selection, ids);
                            s.layer(true, cx);
                            assert_eq!(s.session.deck.slides[0].objects[0].kind, "arrow");
                            s.session.undo();
                            s.selection.clear();
                            s.down(&down((5., 5.)), scale, w, cx);
                            s.move_pointer(&motion((950., 530.)), cx);
                            s.text_anchor = None;
                            s.end_gesture(cx);
                            assert_eq!(s.selection.len(), 3);
                            s.page_selection = BTreeSet::from([0, 1]);
                            let first = s.session.deck.slides[0].id.clone();
                            s.page_command("down", cx);
                            assert_eq!(s.session.deck.slides[1].id, first);
                            s.session.undo();
                            s.page = 0;
                            s.page_selection = BTreeSet::from([0]);
                            s.command(Command::HidePages(vec![2]), cx);
                            assert!(s.session.deck.slides[2].hidden);
                            s.session.undo();
                            s.page = 0;
                            s.selection.clear();
                            s.select(0, w, cx);
                            s.fields[4].update(cx, |f, cx| f.set_value("#da8350", w, cx));
                            s.apply(cx);
                            eprintln!("M2_SMOKE marquee_multiselect_reorder=true");
                            s.close(&Close, w, cx);
                            assert!(w.has_active_dialog(cx));
                            w.close_dialog(cx);
                            eprintln!("M2_SMOKE properties_close_cancel=true");
                            s.path.update(cx, |p, cx| {
                                p.set_value(output.to_string_lossy().to_string(), w, cx)
                            });
                            s.save(&Save, w, cx);
                            phase = 1;
                        }
                        1 if !s.saving && s.disk.is_some() && !s.session.dirty() => {
                            eprintln!("M2_SMOKE ui_save_clean=true");
                            s.command(Command::AddObject(0, edit::object("roundRect")), cx);
                            s.save(&Save, w, cx);
                            s.command(Command::AddObject(0, edit::object("line")), cx);
                            phase = 2;
                        }
                        2 if !s.saving => {
                            assert!(s.session.dirty());
                            eprintln!("M2_SMOKE overwrite_old_snapshot_stays_dirty=true");
                            s.save(&Save, w, cx);
                            phase = 3;
                        }
                        3 if !s.saving && !s.session.dirty() => {
                            s.command(Command::AddObject(0, edit::object("rect")), cx);
                            s.close(&Close, w, cx);
                            assert!(w.has_active_dialog(cx));
                            w.close_dialog(cx);
                            since = Instant::now();
                            phase = 4;
                        }
                        4 if since.elapsed() > Duration::from_secs(2) => {
                            phase = 5;
                        }
                        6 if !s.saving && s.session.dirty() => {
                            assert_eq!(s.session.deck.slides.len(), 3);
                            eprintln!("M2_SMOKE save_reopen_recovery=true");
                            phase = 7;
                        }
                        _ => {}
                    }
                    phase
                })
            });
            match result {
                Ok(5) => {
                    let readonly = cx.update(|cx| crate::viewer::open_window(cx).unwrap().0);
                    cx.update(quit);
                    let _ = handle.update(cx, |_, w, cx| {
                        assert!(w.has_active_dialog(cx));
                        w.close_dialog(cx);
                    });
                    let _ = readonly.update(cx, |_, w, _| w.remove_window());
                    eprintln!("M2_SMOKE global_quit_dirty_guard=true");
                    let service = view.read_with(cx, |s, _| s.recovery.clone());
                    let own_session = service.session.clone();
                    let records = cx
                        .background_executor()
                        .spawn(async move { service.list() })
                        .await
                        .unwrap();
                    let entry = records
                        .into_iter()
                        .find(|e| {
                            e.record.session == own_session && e.problem.is_none() && !e.active
                        })
                        .expect("own recovery snapshot");
                    let _ = handle.update(cx, |_, w, cx| {
                        view.update(cx, |s, cx| {
                            s.session.undo();
                            assert!(!s.session.dirty());
                            s.restore(entry.record, w, cx);
                        })
                    });
                    phase = 6;
                }
                Ok(7) => {
                    let (other, weak) = cx.update(|cx| {
                        open(cx).unwrap();
                        cx.global::<Registry>().0.last().unwrap().clone()
                    });
                    let path = output.with_file_name(format!(
                        "close-{}-{}.joydeck",
                        std::process::id(),
                        began.elapsed().as_nanos()
                    ));
                    let target = path.clone();
                    let _ = other.update(cx, |_, w, cx| {
                        let _ = weak.update(cx, |s, cx| {
                            s.command(Command::AddObject(0, edit::object("rect")), cx);
                            s.close(&Close, w, cx);
                            assert!(w.has_active_dialog(cx));
                            w.close_dialog(cx);
                            s.path.update(cx, |p, cx| {
                                p.set_value(target.to_string_lossy().to_string(), w, cx)
                            });
                            s.close_after_save = Some(w.window_handle());
                            s.save(&Save, w, cx);
                        });
                    });
                    closing = Some(other);
                    close_path = Some(path);
                    phase = 8;
                }
                Ok(8) => {
                    if closing
                        .as_ref()
                        .is_some_and(|h| h.update(cx, |_, _, _| ()).is_err())
                    {
                        let path = close_path.take().unwrap();
                        let result = cx
                            .background_executor()
                            .spawn(async move {
                                let deck = edit::open_editable(&path)?;
                                std::fs::remove_file(path).map_err(|e| e.to_string())?;
                                Ok::<_, String>(deck.slides[0].objects.len())
                            })
                            .await;
                        assert_eq!(result.unwrap(), 1);
                        eprintln!("M2_SMOKE save_then_close=true");
                        let (other, weak) = cx.update(|cx| {
                            open(cx).unwrap();
                            cx.global::<Registry>().0.last().unwrap().clone()
                        });
                        let _ = other.update(cx, |_, _w, cx| {
                            let _ = weak.update(cx, |s, cx| {
                                s.command(Command::AddObject(0, edit::object("rect")), cx);
                                discarded = Some(s.recovery.clone());
                            });
                        });
                        closing = Some(other);
                        since = Instant::now();
                        phase = 9;
                    }
                }
                Ok(9) if since.elapsed() > Duration::from_secs(2) => {
                    let service = discarded.as_ref().unwrap().clone();
                    let session = service.session.clone();
                    let rows = cx
                        .background_executor()
                        .spawn(async move { service.list() })
                        .await
                        .unwrap();
                    assert!(rows.iter().any(|e| e.record.session == session));
                    let (h, weak) =
                        cx.update(|cx| cx.global::<Registry>().0.last().unwrap().clone());
                    let _ = h.update(cx, |_, w, cx| {
                        let _ = weak.update(cx, |s, cx| s.discard_and_close(w.window_handle(), cx));
                    });
                    phase = 10;
                }
                Ok(10) => {
                    if closing
                        .as_ref()
                        .is_some_and(|h| h.update(cx, |_, _, _| ()).is_err())
                    {
                        let service = discarded.take().unwrap();
                        let session = service.session.clone();
                        let rows = cx
                            .background_executor()
                            .spawn(async move { service.list() })
                            .await
                            .unwrap();
                        assert!(!rows.iter().any(|e| e.record.session == session));
                        eprintln!("M2_SMOKE discard_then_close=true");
                        eprintln!("M2_SMOKE complete=true");
                        break;
                    }
                }
                Ok(_) => {}
                _ => {
                    eprintln!("M2_SMOKE error=window closed");
                    break;
                }
            }
        }
    })
    .detach();
}

pub(super) fn text_smoke(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let started = Instant::now();
        loop {
            if handle.update(cx, |_, w, _| w.is_window_active()).unwrap_or(false) { break; }
            if started.elapsed() > Duration::from_secs(10) {
                eprintln!("M3_SMOKE blocked=no_active_window");
                cx.update(|cx| cx.quit());
                return;
            }
            cx.background_executor().timer(Duration::from_millis(100)).await;
        }
        handle.update(cx, |_, w, cx| view.update(cx, |s, cx| {
            s.command(Command::AddObject(0, edit::object("text")), cx);
            s.begin_text(0,w,cx);
            s.replace_and_mark_text_in_range(None,"zhong",Some(5..5),w,cx);
            assert_eq!(s.session.deck.slides[0].objects[0].text,"输入文字");
            s.replace_and_mark_text_in_range(None,"中文",Some(2..2),w,cx);
            s.replace_text_in_range(None,"中文😀 e\u{301}\n第二行",w,cx);
            let committed=s.session.deck.clone();
            s.replace_and_mark_text_in_range(None,"cancel",None,w,cx);
            s.text_edit.as_mut().unwrap().cancel();
            assert_eq!(s.text_edit.as_ref().unwrap().object.text,committed.slides[0].objects[0].text);
            s.finish_text(cx);
            s.history(false,w,cx);
            assert_eq!(s.session.deck.slides[0].objects[0].text,"输入文字");
            s.history(true,w,cx);
            assert_eq!(s.session.deck.slides[0].objects[0].text,committed.slides[0].objects[0].text);
            s.begin_text(0,w,cx);
            let t=s.text_edit.as_mut().unwrap();
            t.typing.bold=true;
            t.typing.color="#c02060".into();
            joydeck::text::format(&mut t.object,0..6,t.typing.clone()).unwrap();
            s.publish_text(cx);
            let path=std::env::temp_dir().join(format!("joydeck-m3-smoke-{}.joydeck",std::process::id()));
            edit::save_new(&path,&s.session.deck).unwrap();
            assert_eq!(edit::open_editable(&path).unwrap(),*s.session.deck);
            std::fs::remove_file(path).unwrap();
            eprintln!("M3_SMOKE composition_draft=true commit_once_undo=true cancel=true rich_roundtrip=true");
        })).unwrap();
        cx.background_executor().timer(Duration::from_secs(2)).await;
        handle.update(cx, |_,_,cx| view.update(cx, |s,_| {
            if s.text_lines.is_empty() || s.text_bounds.is_none() {
                eprintln!("M3_SMOKE blocked=no_canvas_frame");
                return;
            }
            eprintln!("M3_SMOKE painted_lines={} complete=true",s.text_lines.len());
        })).unwrap();
        let shapes = handle.update(cx, |_,_,cx| view.update(cx, |s,cx| {
            let shapes=s.text_shape_count;
            if let Some(t)=&mut s.text_edit { t.move_to(0,false); }
            cx.notify();
            shapes
        })).unwrap();
        cx.background_executor().timer(Duration::from_millis(200)).await;
        handle.update(cx, |_,_,cx| view.update(cx, |s,_| {
            assert!(shapes > 0);
            assert_eq!(s.text_shape_count,shapes,"caret-only repaint must reuse shaping");
            eprintln!("M3_SMOKE overlay_shape_cache=true");
        })).unwrap();
        if !std::env::args().any(|s| s == "--hold") { cx.update(|cx| cx.quit()); }
    }).detach();
}

pub(super) fn ime_smoke(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(500)).await;
        handle.update(cx, |_, w, cx| view.update(cx, |s, cx| {
            let table=std::env::args().any(|s|s=="--table-ime-smoke");
            let mut o = if table{joydeck::table::object(4,5).unwrap()}else{edit::object("text")}; o.text.clear(); if !table{o.width=480.;}
            s.command(Command::AddObject(0,o),cx);
            s.select(0,w,cx);
            if std::env::args().any(|s|s=="--table-ime-smoke"){s.begin_cell(0,0,0,w,cx);}else{s.begin_text(0,w,cx);}
            eprintln!("M3_IME ready=true");
        })).unwrap();
        let began=Instant::now();
        let mut phase=0;
        let mut committed_revision=0;
        let mut marks=0;
        loop {
            cx.background_executor().timer(Duration::from_millis(50)).await;
            let complete=handle.update(cx, |_, w,cx| view.update(cx, |s,cx| {
                match phase {
                    0 if s.ime_text()=="中文" => {
                        assert!(s.ime_mark_calls > 0, "OS input method must produce preedit calls");
                        assert_eq!(s.ime_commit_calls,1);
                        committed_revision=s.session.revision;
                        marks=s.ime_mark_calls;
                        eprintln!("M3_IME committed=true marked_calls={} commit_calls=1",marks);
                        phase=1;
                    },
                    1 if s.ime_mark_calls > marks && s.text_edit.as_ref().is_some_and(|t| t.marked.is_some()) => {
                        assert_eq!(s.ime_text(),"中文");
                        eprintln!("M3_IME cancel_ready=true");
                        phase=2;
                    },
                    2 if s.text_edit.as_ref().is_some_and(|t| t.marked.is_none() && t.object.text=="中文") => {
                        assert_eq!(s.session.revision,committed_revision);
                        s.finish_text(cx);
                        s.history(false,w,cx);
                        assert!(s.ime_text().is_empty());
                        s.history(true,w,cx);
                        assert_eq!(s.ime_text(),"中文");
                        if let Some(path)=std::env::var_os("JOYDECK_IME_EVIDENCE") {
                            let path=PathBuf::from(path);
                            edit::save_new(&path,&s.session.deck).unwrap();
                            assert_eq!(edit::open_editable(&path).unwrap(),*s.session.deck);
                        }
                        if std::env::args().any(|s|s=="--table-ime-smoke"){s.begin_cell(0,0,0,w,cx);}else{s.begin_text(0,w,cx);}
                        eprintln!("M3_IME cancel=true single_undo=true roundtrip=true complete=true");
                        phase=3;
                    }, _=>(),
                }
                phase==3
            })).unwrap();
            if complete { break; }
            if began.elapsed()>Duration::from_secs(60) { eprintln!("M3_IME failed=timeout phase={phase}"); break; }
        }
        if !std::env::args().any(|s|s=="--hold") {cx.update(|cx|cx.quit());}
    }).detach();
}

impl Editor {
    fn ime_text(&self) -> String {
        let o = &self.session.deck.slides[0].objects[0];
        o.table
            .as_ref()
            .map_or_else(|| o.text.clone(), |t| t.cells[0].text.clone())
    }
}
