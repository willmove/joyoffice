use super::*;
use joydeck::{
    design,
    design_commands::{Action, PageTransfer},
};
pub(super) fn run(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    let clipboard = cx.read_from_clipboard();
    cx.spawn(async move|cx|{
        let output=std::env::var_os("JOYDECK_M6_OUTPUT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);let _=std::fs::create_dir_all(&output);
        let mut phase=0;let mut peer:Option<(AnyWindowHandle,Entity<Editor>)>=None;let mut since=Instant::now();let start=Instant::now();
        let run:std::result::Result<(),String>=async {
            loop {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                if start.elapsed()>Duration::from_secs(45){return Err(format!("timeout phase={phase}"));}
                if phase==0 {
                    let ready=handle.update(cx,|_,w,_|w.is_window_active()).map_err(|e|e.to_string())?;if !ready{continue;}
                    let session=handle.update(cx,|_,w,cx|view.update(cx,|s,cx|->std::result::Result<Session,String>{
                        w.set_window_title("JoyDeck — M6 Acceptance Source");
                        s.command(Command::PageSize(960.,512.),cx);
                        s.command(Command::Design(Action::SetDesign(design::starter())),cx);
                        s.command(Command::Design(Action::ApplyLayout(0,"layout-1".into(),false)),cx);
                        let mut objects=s.effective_objects();objects[0].text="JoyDeck 文稿设计".into();objects[1].text="主题继承与直接格式\nIndependent document views".into();objects[1].fill="#c05050".into();s.command(Command::ReplaceMany(0,objects),cx);
                        s.command(Command::Design(Action::Notes(0,"本机双窗口与版式继承验证".into())),cx);
                        s.command(Command::Design(Action::Section(vec![0],"intro".into(),"开场".into())),cx);
                        s.selection=BTreeSet::from([s.session.deck.slides[0].objects[0].id.clone()]);s.selected=Some(0);
                        Ok(s.session.clone())
                    })).map_err(|e|e.to_string())??;
                    let new=cx.update(|cx|->std::result::Result<_,String>{open_session(cx,Some(session)).map_err(|e|e.to_string())?;let (h,weak)=cx.global::<Registry>().0.last().unwrap().clone();Ok((h,weak.upgrade().ok_or("peer closed")?))})?;
                    new.0.update(cx,|_,w,cx|new.1.update(cx,|s,cx|{w.set_window_title("JoyDeck — M6 Acceptance Shared");s.selection=BTreeSet::from([s.session.deck.slides[0].objects[1].id.clone()]);s.selected=Some(1);cx.notify();})).map_err(|e|e.to_string())?;
                    peer=Some(new);phase=1;since=Instant::now();continue;
                }
                if phase==1 && since.elapsed()>Duration::from_millis(500){
                    let (peer_handle,peer_view)=peer.as_ref().unwrap();
                    peer_handle.update(cx,|_,_,cx|peer_view.update(cx,|s,cx|{
                        let mut d=s.session.deck.design.clone();d.themes.get_mut("theme-1").unwrap().colors.insert("accent".into(),"#1265c5".into());s.command(Command::Design(Action::SetDesign(d)),cx);
                    })).map_err(|e|e.to_string())?;
                    phase=2;since=Instant::now();continue;
                }
                if phase==2 && since.elapsed()>Duration::from_millis(500){
                    handle.update(cx,|_,w,cx|view.update(cx,|s,cx|->std::result::Result<(),String>{s.sync_document(cx);let resolved=design::resolve_slide(&s.session.deck,0)?;
                        if resolved.objects[0].fill!="#1265c5" || resolved.objects[1].fill!="#c05050" || !s.selection.contains(&resolved.objects[0].id){return Err("theme/direct format/selection sync failed".into());}
                        s.history(false,w,cx);if design::resolve_slide(&s.session.deck,0)?.objects[0].fill!="#087f8c"{return Err("shared undo failed".into());}s.history(true,w,cx);s.copy_pages(cx);Ok(())
                    })).map_err(|e|e.to_string())??;
                    let transfer=cx.update(|cx|cx.read_from_clipboard().and_then(|i|i.metadata().cloned()).ok_or("page clipboard missing".to_string()).and_then(|s|serde_json::from_str::<PageTransfer>(&s).map_err(|e|e.to_string())))?;
                    let (h,p)=peer.as_ref().unwrap();h.update(cx,|_,_,cx|p.update(cx,|s,cx|->std::result::Result<(),String>{s.sync_document(cx);if s.selected!=Some(1){return Err("peer selection lost".into());}s.session=Session::new();s.renew_recovery();s.command(Command::Design(Action::PastePages(1,transfer,None)),cx);if s.session.deck.slides.len()!=2{return Err("cross document paste failed".into());}Ok(())})).map_err(|e|e.to_string())??;
                    eprintln!("M6_SMOKE shared_history_selection=true theme_direct_override=true cross_document_pages=true");
                    handle.update(cx,|_,w,cx|view.update(cx,|s,cx|->std::result::Result<(),String>{w.activate_window();s.selection.clear();s.selected=None;s.text_edit=None;
                        let path=output.join("roundtrip.joydeck");edit::save(&path,&s.session.deck)?;if edit::open_editable(&path)?!=*s.session.deck{return Err("design roundtrip failed".into());}s.changed(cx);Ok(())})).map_err(|e|e.to_string())??;
                    phase=3;since=Instant::now();continue;
                }
                if (phase==3 || phase==5) && since.elapsed()>Duration::from_secs(2){
                    let ready=handle.update(cx,|_,w,cx|view.update(cx,|s,cx|->std::result::Result<bool,String>{
                        let backend=if phase==3{"GPUI"}else{"CPU"};if (phase==3 && s.scene.is_none())||(phase==5 && s.image.is_none()){return Ok(false);}
                        let scale=s.canvas_scale(w);let width=s.session.deck.width*scale;let height=s.session.deck.height*scale;
                        let raster=preview::render(&s.session.deck,0,(width*w.scale_factor()).round() as u32,&AtomicBool::new(false))?;let mut rgba=raster.bgra;for p in rgba.as_chunks_mut::<4>().0{p.swap(0,2);}
                        image::RgbaImage::from_raw(raster.width,raster.height,rgba).unwrap().save(output.join(format!("M6-{backend}-reference.png"))).map_err(|e|e.to_string())?;
                        eprintln!("M6_CAPTURE {backend} {}",serde_json::json!({"x":s.canvas_origin.0,"y":s.canvas_origin.1,"width":width,"height":height,"viewportHeight":f32::from(w.viewport_size().height)}));cx.notify();Ok(true)
                    })).map_err(|e|e.to_string())??;
                    if ready{phase+=1;}continue;
                }
                if phase==4 && output.join("capture-0.ack").exists(){handle.update(cx,|_,_,cx|view.update(cx,|s,cx|{s.accelerated=false;s.changed(cx);})).map_err(|e|e.to_string())?;phase=5;since=Instant::now();}
                if phase==6 && output.join("capture-1.ack").exists(){handle.update(cx,|_,w,cx|view.update(cx,|s,cx|{s.workspace_panel=true;s.ribbon="设计".into();s.sync_doc_fields(w,cx);cx.notify();})).map_err(|e|e.to_string())?;phase=7;since=Instant::now();}
                if phase==7 && since.elapsed()>Duration::from_secs(1){eprintln!("M6_PANEL_READY");phase=8;}
                if phase==8 && output.join("panel.ack").exists(){break;}
            }Ok(())
        }.await;
        match run{Ok(())=>eprintln!("M6_SMOKE design_roundtrip=true\nM6_SMOKE complete=true"),Err(e)=>eprintln!("M6_SMOKE failed={e}")}
        cx.update(|cx|{cx.write_to_clipboard(clipboard.unwrap_or_else(||ClipboardItem::new_string(String::new())));cx.quit();});
    }).detach();
}
