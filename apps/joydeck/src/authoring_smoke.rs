//! Native authoring acceptance. The external verifier captures the actual canvas before acknowledging it.
use super::*;
pub(super) fn run(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    let clipboard = cx.read_from_clipboard();
    cx.spawn(async move|cx|{
        let output=std::env::var_os("JOYDECK_M9_OUTPUT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);let _=std::fs::create_dir_all(&output);
        let result:std::result::Result<Arc<Deck>,String>=async{
            let start=Instant::now();let mut phase=0;let mut since=Instant::now();let mut snapshot=None;
            loop{cx.background_executor().timer(Duration::from_millis(100)).await;if start.elapsed()>Duration::from_secs(55){return Err(format!("timeout phase {phase}"));}
                if phase==0 {if !handle.update(cx,|_,w,_|w.is_window_active()).map_err(|e|e.to_string())?{continue;}
                    let deck=handle.update(cx,|_,w,cx|view.update(cx,|s,cx|->std::result::Result<Arc<Deck>,String>{
                        w.set_window_title("JoyDeck — M9 Authoring Acceptance");s.session=Session::from_deck(joydeck::authoring_demo::deck(),false);s.renew_recovery();s.page=0;s.select(1,w,cx);s.sync_author(w,cx);
                        let base=s.session.deck.clone();s.chart_grid[4][1].update(cx,|f,cx|f.set_value("131",w,cx));s.chart_op("grid-data",w,cx);
                        if s.session.deck.slides[0].objects[1].chart.as_ref().unwrap().series[0].values[3]!=Some(131.){return Err("chart grid failed".into());}s.history(false,w,cx);if s.session.deck!=base{return Err("chart grid undo failed".into());}s.history(true,w,cx);
                        let valid=s.session.deck.clone();s.author_data.update(cx,|f,cx|f.set_value("类别\t数值\nA\tNaN",w,cx));s.chart_op("data",w,cx);if s.session.deck!=valid{return Err("invalid chart paste changed document".into());}
                        s.copy_objects(cx);s.paste_at_focus(w,cx);if s.session.deck.slides[0].objects.len()!=4{return Err("chart clipboard failed".into());}s.history(false,w,cx);if s.session.deck!=valid{return Err("chart clipboard undo failed".into());}
                        s.motion_fields[0].update(cx,|f,cx|f.set_value("900",w,cx));s.effect_pick=0;s.timing_op("timing",w,cx);if s.session.deck.presentation.slides["page-0"].effects[0].duration_ms!=900{return Err("animation timing pane failed".into());}s.history(false,w,cx);if s.session.deck!=valid{return Err("animation undo failed".into());}
                        edit::save(&output.join("authoring.joydeck"),&s.session.deck)?;if edit::open_editable(&output.join("authoring.joydeck"))?!=*s.session.deck{return Err("authoring native roundtrip failed".into());}
                        let mut parts=joydeck::pptx_write::generate(&s.session.deck)?;joydeck::export::save_parts(&output.join("authoring.pptx"),&parts,&AtomicBool::new(false))?;parts.get_mut("ppt/slides/slide1.xml").unwrap().extend_from_slice(b"<!-- independent public import -->");joydeck::export::save_parts(&output.join("authoring-public.pptx"),&parts,&AtomicBool::new(false))?;
                        joydeck::export::pdf(&output.join("authoring.pdf"),&s.session.deck,&joydeck::export::Options{pages:vec![0,5],dpi:144,notes:false,transparent:false},&AtomicBool::new(false))?;
                        s.selection.clear();s.selected=None;s.ribbon="开始".into();s.message="图表网格、无效粘贴、剪贴板与动画撤销已通过".into();s.changed(cx);Ok(s.session.deck.clone())
                    })).map_err(|e|e.to_string())??;snapshot=Some(deck);phase=1;since=Instant::now();continue;
                }
                if (phase==1||phase==3)&&since.elapsed()>Duration::from_secs(2){let ready=handle.update(cx,|_,w,cx|view.update(cx,|s,_|->std::result::Result<bool,String>{let backend=if phase==1{"GPUI"}else{"CPU"};if (phase==1&&s.scene.is_none())||(phase==3&&s.image.is_none()){return Ok(false);}let scale=s.canvas_scale(w);let width=s.session.deck.width*scale;let height=s.session.deck.height*scale;let r=preview::render(&s.session.deck,0,(width*w.scale_factor()).round()as u32,&AtomicBool::new(false))?;let mut rgba=r.bgra;for p in rgba.as_chunks_mut::<4>().0{p.swap(0,2);}image::RgbaImage::from_raw(r.width,r.height,rgba).unwrap().save(output.join(format!("M9-{backend}-reference.png"))).map_err(|e|e.to_string())?;eprintln!("M9_CAPTURE {backend} {}",serde_json::json!({"x":s.canvas_origin.0,"y":s.canvas_origin.1,"width":width,"height":height,"viewportHeight":f32::from(w.viewport_size().height)}));Ok(true)})).map_err(|e|e.to_string())??;if ready{phase+=1;}continue;}
                if phase==2&&output.join("capture-0.ack").exists(){handle.update(cx,|_,_,cx|view.update(cx,|s,cx|{s.accelerated=false;s.changed(cx);})).map_err(|e|e.to_string())?;phase=3;since=Instant::now();}
                if phase==4&&output.join("capture-1.ack").exists(){break;}
            }snapshot.ok_or("snapshot missing".into())
        }.await;
        cx.update(|cx|cx.write_to_clipboard(clipboard.unwrap_or_else(||ClipboardItem::new_string(String::new()))));
        match result{Ok(d)=>{eprintln!("M9_SMOKE grid_clipboard_undo_native_public_pdf=true complete=true");let focus=handle.update(cx,|_,w,cx|view.update(cx,|s,cx|{s.prepare_show(w,cx);s.focus.clone()})).unwrap();cx.update(|cx|crate::slideshow::smoke(d,output,handle,focus,cx));},Err(e)=>{eprintln!("M9_SMOKE failed={e}");cx.update(|cx|cx.quit());}}
    }).detach();
}
