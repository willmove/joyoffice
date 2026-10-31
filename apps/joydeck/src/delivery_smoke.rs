use super::*;
use joydeck::{
    design_commands::Action,
    export::{self, ImageFormat, Options},
    interop, pptx_write,
};
pub(super) fn run(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    cx.spawn(async move|cx|{
        let output=std::env::var_os("JOYDECK_M7_OUTPUT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);let _=std::fs::create_dir_all(&output);
        let run:std::result::Result<(),String>=async {
            let deck=handle.update(cx,|_,w,cx|view.update(cx,|s,cx|->std::result::Result<Arc<Deck>,String>{
                w.set_window_title("JoyDeck — M7 Acceptance Editor");s.command(Command::PageSize(960.,512.),cx);s.command(Command::Design(Action::SetDesign(joydeck::design::starter())),cx);s.command(Command::Design(Action::ApplyLayout(0,"layout-1".into(),false)),cx);
                let mut objects=s.effective_objects();objects[0].text="M7 文件交付 / Delivery".into();objects[1].text="PNG · JPEG · PDF · PPTX\n中文导出与静态放映".into();s.command(Command::ReplaceMany(0,objects),cx);
                let pixels=image::RgbaImage::from_fn(32,24,|x,y|image::Rgba([(x*7) as u8,120,(y*10) as u8,if y>12{128}else{255}]));let mut bytes=std::io::Cursor::new(vec![]);pixels.write_to(&mut bytes,image::ImageFormat::Png).map_err(|e|e.to_string())?;let resource=joydeck::media::Resource::import(bytes.get_ref())?;s.command(Command::ImportImage(0,None,resource),cx);let i=s.session.deck.slides[0].objects.len()-1;let mut o=s.session.deck.slides[0].objects[i].clone();o.x=650.;o.y=300.;o.width=200.;o.height=150.;s.command(Command::ReplaceObject(0,i,o),cx);
                s.command(Command::Design(Action::Notes(0,"演讲备注只出现在备注导出，不显示在观众放映窗口。".into())),cx);s.command(Command::DuplicatePage(0),cx);s.command(Command::DuplicatePage(0),cx);s.command(Command::HidePages(vec![1]),cx);s.page=0;s.selection.clear();s.selected=None;s.delivery_panel=true;s.workspace_panel=false;s.changed(cx);Ok(s.session.deck.clone())
            })).map_err(|e|e.to_string())??;
            let dest=output.clone();let source=deck.clone();cx.background_executor().spawn(async move{let cancel=AtomicBool::new(false);let options=Options{pages:vec![0,2],dpi:144,transparent:false,notes:false};
                export::pdf(&dest.join("slides.pdf"),&source,&options,&cancel)?;export::images(&dest.join("png"),&source,&options,ImageFormat::Png,&cancel)?;export::images(&dest.join("jpeg"),&source,&options,ImageFormat::Jpeg,&cancel)?;let mut notes=options.clone();notes.notes=true;export::pdf(&dest.join("notes.pdf"),&source,&notes,&cancel)?;
                let parts=pptx_write::generate(&source)?;export::save_parts(&dest.join("slides.pptx"),&parts,&cancel)?;if interop::import(&dest.join("slides.pptx"))?!=*source{return Err("PPTX native snapshot roundtrip failed".into());}
                edit::save(&dest.join("slides.joydeck"),&source)?;
                let corpus=dest.join("corpus");std::fs::create_dir(&corpus).map_err(|e|e.to_string())?;let mut entries=vec![];
                for i in 0..40{let mut d=(*source).clone();d.slides.truncate(1);d.slides[0].notes.clear();d.slides[0].objects[0].text=format!("原创语料 {i:02} / JoyDeck");d.slides[0].objects[1].rotation=(i%7) as f32*5.;d.slides[0].objects[2].opacity=0.3+(i%7) as f32*0.1;let name=format!("synthetic-{i:02}.pptx");let parts=pptx_write::generate(&d)?;export::save_parts(&corpus.join(&name),&parts,&cancel)?;if interop::import(&corpus.join(&name))?!=d{return Err("corpus roundtrip failed".into());}entries.push(serde_json::json!({"file":name,"origin":"independently generated","structural":true,"nativeSnapshotSemantic":true,"externalPowerPoint":false}));}
                std::fs::write(dest.join("corpus.json"),serde_json::to_vec_pretty(&entries).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;Ok::<_,String>(())
            }).await?;
            eprintln!("M7_SMOKE exports_roundtrip=true corpus40=true");
            let (show,show_view)=cx.update(|cx|super::super::slideshow::open(deck.clone(),0,cx).map_err(|e|e.to_string()))?;
            handle.update(cx,|_,_,cx|view.update(cx,|s,cx|s.command(Command::AddPage,cx))).map_err(|e|e.to_string())?;
            let started=Instant::now();loop{cx.background_executor().timer(Duration::from_millis(150)).await;let ready=show.update(cx,|_,w,cx|{show_view.read(cx).ready()&&w.is_fullscreen()}).map_err(|e|e.to_string())?;if ready{break;}
                if started.elapsed()>Duration::from_secs(15){return Err("slideshow fullscreen/first frame timeout".into());}}
            show.update(cx,|_,_,cx|->std::result::Result<(),String>{let s=show_view.read(cx);if !Arc::ptr_eq(&s.snapshot.deck,&deck)||s.snapshot.deck.slides.len()!=3{return Err("show snapshot changed with editor".into());}Ok(())}).map_err(|e|e.to_string())??;
            let viewport=show.update(cx,|_,w,_|serde_json::json!({"width":f32::from(w.viewport_size().width),"height":f32::from(w.viewport_size().height)})).map_err(|e|e.to_string())?;
            eprintln!("M7_SHOW_CAPTURE {viewport}");let started=Instant::now();while !output.join("show.ack").exists(){if started.elapsed()>Duration::from_secs(15){return Err("show screenshot acknowledgement timeout".into());}cx.background_executor().timer(Duration::from_millis(100)).await;}
            show.update(cx,|_,w,cx|show_view.update(cx,|s,cx|->std::result::Result<(),String>{s.navigation("right",w,cx);if s.snapshot.page()!=2{return Err("hidden slide not skipped".into());}s.navigation("left",w,cx);if s.snapshot.page()!=0{return Err("previous slide failed".into());}s.navigation("end",w,cx);if s.snapshot.page()!=2{return Err("end failed".into());}s.navigation("escape",w,cx);Ok(())})).map_err(|e|e.to_string())??;
            cx.background_executor().timer(Duration::from_millis(500)).await;if show.update(cx,|_,_,_|()).is_ok(){return Err("Escape did not close show".into());}
            handle.update(cx,|_,w,cx|view.update(cx,|s,cx|{w.activate_window();s.delivery_panel=true;s.ribbon="文件".into();cx.notify();})).map_err(|e|e.to_string())?;cx.background_executor().timer(Duration::from_secs(1)).await;eprintln!("M7_EDITOR_CAPTURE");let started=Instant::now();while !output.join("editor.ack").exists(){if started.elapsed()>Duration::from_secs(15){return Err("editor capture timeout".into());}cx.background_executor().timer(Duration::from_millis(100)).await;}
            Ok(())
        }.await;
        match run{Ok(())=>eprintln!("M7_SMOKE fullscreen_snapshot_hidden_navigation_escape=true complete=true"),Err(e)=>eprintln!("M7_SMOKE failed={e}")};cx.update(|cx|cx.quit());
    }).detach();
}
