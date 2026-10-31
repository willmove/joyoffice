use super::*;
pub(super) fn run(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    cx.spawn(async move|cx|{
        let out=PathBuf::from(std::env::var_os("JOYDECK_M14_OUTPUT").unwrap());
        let result:std::result::Result<(),String>=async {
            let base=joydeck::authoring_demo::deck();let mut reference=base.clone();
            reference.slides[0].objects[0].text="已核对的业务标题".into();reference.slides[0].objects[0].runs.clear();
            let files=out.clone();
            cx.background_executor().spawn(async move {
                edit::save_new(&files.join("current.joydeck"),&base)?;
                edit::save_new(&files.join("reference.joydeck"),&reference)?;
                Ok::<_,String>(())
            }).await?;
            handle.update(cx,|_,w,cx|view.update(cx,|s,cx|{
                s.session=Session::from_deck(joydeck::authoring_demo::deck(),true);s.renew_recovery();
                s.zoom=0.9;s.show_notes=true;s.ribbon="维护".into();
                s.remember_path(out.join("current.joydeck"),cx);s.save_preferences(w,cx);
                s.changed(cx);
            })).map_err(|e|e.to_string())?;
            for phase in 0..5 {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                if phase==1 {
                    let ready=handle.update(cx,|_,_,cx|view.read(cx).comparison.is_some()).map_err(|e|e.to_string())?;
                    if !ready {return Err("comparison did not complete".into());}
                }
                if phase==4 {
                    handle.update(cx,|_,_,cx|view.update(cx,|s,cx|s.media_action("pause",cx))).map_err(|e|e.to_string())?;
                    cx.background_executor().timer(Duration::from_millis(300)).await;
                    if !handle.update(cx,|_,_,cx|view.read(cx).av_image.is_some()).map_err(|e|e.to_string())? {return Err("exported video not displayed".into());}
                }
                if phase==2 {
                    handle.update(cx,|_,w,cx|->std::result::Result<(),String>{
                        let s=view.read(cx);let scale=s.canvas_scale(w);let width=s.session.deck.width*scale;let height=s.session.deck.height*scale;
                        let raster=preview::render(&s.session.deck,0,(width*w.scale_factor()).round() as u32,&AtomicBool::new(false))?;
                        let mut rgba=raster.bgra;for p in rgba.as_chunks_mut::<4>().0 {p.swap(0,2);}
                        image::RgbaImage::from_raw(raster.width,raster.height,rgba).unwrap().save(out.join("M14-compare-reference.png")).map_err(|e|e.to_string())?;
                        eprintln!("M14_CANVAS {}",serde_json::json!({"x":s.canvas_origin.0,"y":s.canvas_origin.1,"width":width,"height":height,"viewportHeight":f32::from(w.viewport_size().height)}));Ok(())
                    }).map_err(|e|e.to_string())??;
                }
                eprintln!("M14_CAPTURE {phase}");
                let start=Instant::now();
                while !out.join(format!("capture-{phase}.ack")).exists() {
                    if start.elapsed()>Duration::from_secs(20){return Err("GUI capture timeout".into());}
                    cx.background_executor().timer(Duration::from_millis(100)).await;
                }
                match phase {
                    0=>{
                        handle.update(cx,|_,w,cx|view.update(cx,|s,cx|{
                            assert_eq!(s.preferences.zoom,0.9);assert!(s.preferences.notes);assert_eq!(s.preferences.recent.len(),1);
                            s.ribbon="比较".into();s.comparison_path.update(cx,|f,cx|f.set_value(out.join("reference.joydeck").to_string_lossy().to_string(),w,cx));s.compare_file(cx);
                        })).map_err(|e|e.to_string())?;
                    },
                    1=>{
                        handle.update(cx,|_,_,cx|view.update(cx,|s,cx|{
                            let (report,_)=s.comparison.as_ref().unwrap();
                            s.comparison_pick=report.differences.iter().position(|r|r.kind=="object.changed").unwrap();
                            let before=s.session.deck.clone();
                            s.accept_comparison(cx);assert_eq!(s.session.deck.slides[0].objects[0].text,"已核对的业务标题");
                            s.session.undo();assert_eq!(s.session.deck,before);s.session.redo();
                            s.ribbon="开始".into();s.changed(cx);
                        })).map_err(|e|e.to_string())?;
                    },
                    2=>{
                        let before=handle.update(cx,|_,w,cx|view.update(cx,|s,cx|{
                            let before=(s.session.deck.clone(),s.session.revision,s.session.dirty());
                            s.extended_fields[8].update(cx,|f,cx|f.set_value(out.join("slides.mp4").to_string_lossy().to_string(),w,cx));
                            s.extended_export("mp4",cx);s.ribbon="输出".into();before
                        })).map_err(|e|e.to_string())?;
                        let start=Instant::now();
                        loop {
                            cx.background_executor().timer(Duration::from_millis(100)).await;
                            if !handle.update(cx,|_,_,cx|view.read(cx).delivery_busy).map_err(|e|e.to_string())?{break;}
                            if start.elapsed()>Duration::from_secs(60){return Err("MP4 export timeout".into());}
                        }
                        handle.update(cx,|_,_,cx|->std::result::Result<(),String>{
                            let s=view.read(cx);if s.message!="操作已完成；文稿未修改"{return Err(s.message.clone());}
                            assert_eq!(before,(s.session.deck.clone(),s.session.revision,s.session.dirty()));
                            Ok(())
                        }).map_err(|e|e.to_string())??;
                    },
                    3=>{
                        handle.update(cx,|_,w,cx|view.update(cx,|s,cx|{
                            s.extended_fields[3].update(cx,|f,cx|f.set_value(out.join("slides.mp4").to_string_lossy().to_string(),w,cx));
                            s.ribbon="媒体".into();s.media_action("import",cx);
                        })).map_err(|e|e.to_string())?;
                        let start=Instant::now();
                        while !handle.update(cx,|_,_,cx|!view.read(cx).session.deck.playback.clips.is_empty()).map_err(|e|e.to_string())? {
                            if start.elapsed()>Duration::from_secs(12){return Err("media import timeout".into());}
                            cx.background_executor().timer(Duration::from_millis(100)).await;
                        }
                        handle.update(cx,|_,w,cx|view.update(cx,|s,cx|{
                            s.sync_clip_fields(w,cx);assert_eq!(s.extended_fields[5].read(cx).value(),"18");
                            s.media_action("preview",cx);s.changed(cx);
                        })).map_err(|e|e.to_string())?;
                    },
                    _=>{}
                }
            }
            // Repeated actual window lifetimes. The primary window and its document survive.
            for _ in 0..24 {
                cx.update(|cx|{
                    open_session(cx,None).unwrap();let (h,weak)=cx.global::<Registry>().0.last().unwrap().clone();
                    let fresh=weak.upgrade().unwrap();assert_eq!(fresh.read(cx).zoom,0.9);assert!(fresh.read(cx).show_notes);
                    h.update(cx,|_,w,cx|{let expected=fresh.read(cx).preferences.height;assert_eq!(f32::from(w.window_bounds().get_bounds().size.height).round() as u32,expected);}).unwrap();
                    h.update(cx,|_,w,_|w.remove_window()).unwrap();
                });
                cx.background_executor().timer(Duration::from_millis(30)).await;
            }
            for i in 0..120 {
                handle.update(cx,|_,_,cx|view.update(cx,|s,cx|{
                    let before=s.session.deck.clone();let mut o=s.session.deck.slides[0].objects[0].clone();
                    o.opacity=if i%2==0 {0.7}else{0.8};s.command(Command::ReplaceObject(0,0,o),cx);
                    s.session.undo();assert_eq!(s.session.deck,before);s.changed(cx);
                })).map_err(|e|e.to_string())?;
                cx.background_executor().timer(Duration::from_millis(30)).await;
            }
            handle.update(cx,|_,_,cx|view.update(cx,|s,cx|s.media_action("stop",cx))).map_err(|e|e.to_string())?;
            let report=cx.update(|cx|{
                let live=cx.global::<Registry>().0.iter().filter(|(_,v)|v.upgrade().is_some()).count();
                assert_eq!(live,1);
                serde_json::json!({"nativeWindowsOpenedClosed":24,"editUndoCycles":120,"liveWindows":live,"preferencesRestored":true,"windowHeightRestoredWithoutShrink":true,"comparisonAcceptedUndoRedo":true,"mp4ExportDocumentUnchanged":true,"exportedVideoDisplayed":true,"scope":"native handlers; screenshot status recorded separately; no physical dual-display/reader certification"})
            });
            std::fs::write(out.join("native-state.json"),serde_json::to_vec_pretty(&report).unwrap()).map_err(|e|e.to_string())?;
            Ok(())
        }.await;
        match result {Ok(())=>eprintln!("M14_SMOKE complete=true"),Err(e)=>eprintln!("M14_SMOKE failed={e}")};
        cx.update(|cx|cx.quit());
    }).detach();
}
