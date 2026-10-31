//! Real-window M5 adapter checks. This is separate from physical input acceptance.
use super::*;
pub(super) fn run(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    let original = cx.read_from_clipboard();
    cx.spawn(async move |cx| {
        let output=std::env::var_os("JOYDECK_M5_OUTPUT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let _=std::fs::create_dir_all(&output);
        let mut phase=0;
        let start=Instant::now();
        let mut settled=Instant::now();
        loop {
            cx.background_executor().timer(Duration::from_millis(200)).await;
            let result=handle.update(cx,|_,w,cx|view.update(cx,|s,cx| -> std::result::Result<bool,String> {
                if !w.is_window_active() || s.canvas_origin.0==0. { return Ok(false); }
                match phase {
                    0 => {
                        s.command(Command::PageSize(960.,512.),cx);
                        let pixels=image::RgbaImage::from_fn(120,80,|x,y| image::Rgba([if x<60 {240} else {20},if x>=60 {200} else {50},y as u8*3,if y<40 {255} else {128}]));
                        let mut bytes=std::io::Cursor::new(vec![]);
                        pixels.write_to(&mut bytes,image::ImageFormat::Png).map_err(|e|e.to_string())?;
                        let image=Image::from_bytes(ImageFormat::Png,bytes.into_inner());
                        cx.write_to_clipboard(ClipboardItem::new_image(&image));
                        s.paste_media(w,cx); phase=1;
                    }
                    1 if s.io.is_none() && s.session.deck.slides[0].objects.len()==1 => {
                        eprintln!("M5_SMOKE system_image_paste=true");
                        let id=s.session.deck.slides[0].objects[0].id.clone(); s.selection=BTreeSet::from([id]);s.selected=Some(0);
                        s.copy_objects(cx);s.paste_media(w,cx);
                        if s.session.deck.slides[0].objects.len()!=2 || s.session.deck.resources.len()!=1 { return Err("internal clipboard/dedup failed".into()); }
                        let image=s.session.deck.slides[0].objects[0].image.as_ref().unwrap().resource.clone();
                        s.crop_fields[0].update(cx,|f,cx|f.set_value("0.5",w,cx));
                        s.crop_fields[2].update(cx,|f,cx|f.set_value("0.5",w,cx));
                        s.crop_media(false,cx);
                        let cropped=s.session.deck.clone();
                        s.session.undo();s.session.redo();
                        if s.session.deck!=cropped || !s.session.deck.resources.contains_key(&image) { return Err("crop history failed".into()); }
                        let mut objects=s.session.deck.slides[0].objects.clone();
                        objects[0].width=240.;objects[0].height=160.;objects[1].x=400.;objects[1].width=240.;objects[1].height=160.;objects[1].rotation=25.;objects[1].opacity=0.7;
                        s.command(Command::ReplaceMany(0,objects),cx);
                        let path=output.join("roundtrip.joydeck");edit::save(&path,&s.session.deck)?;
                        if edit::open_editable(&path)?!=*s.session.deck { return Err("roundtrip failed".into()); }
                        s.selection.clear();s.selected=None;s.accelerated=true;s.changed(cx);
                        eprintln!("M5_SMOKE internal_clipboard_dedup=true crop_undo_roundtrip=true");
                        phase=2;settled=Instant::now();
                    }
                    2 | 3 if settled.elapsed()>Duration::from_secs(2) && if phase==2 {s.scene.is_some()} else {s.image.is_some()} => {
                        let backend=if phase==2 {"GPUI"} else {"CPU"};
                        let scale=s.canvas_scale(w);
                        let width=s.session.deck.width*scale;let height=s.session.deck.height*scale;
                        let raster=preview::render(&s.session.deck,0,(width*w.scale_factor()).round() as u32,&AtomicBool::new(false))?;
                        let mut rgba=raster.bgra;for p in rgba.as_chunks_mut::<4>().0 {p.swap(0,2);}
                        image::RgbaImage::from_raw(raster.width,raster.height,rgba).unwrap().save(output.join(format!("M5-{backend}-reference.png"))).map_err(|e|e.to_string())?;
                        eprintln!("M5_CAPTURE {backend} {}",serde_json::json!({"x":s.canvas_origin.0,"y":s.canvas_origin.1,"width":width,"height":height,"viewportHeight":f32::from(w.viewport_size().height)}));
                        phase+=2;
                    }
                    4 | 5 if output.join(format!("capture-{}.ack",phase-4)).exists() => {
                        if phase==5 {return Ok(true);}
                        s.accelerated=false;s.changed(cx);phase=3;settled=Instant::now();
                    }
                    _ => {}
                }
                Ok(false)
            }));
            match result {
                Ok(Ok(true)) => {eprintln!("M5_SMOKE complete=true");break;},
                Ok(Err(e)) => {eprintln!("M5_SMOKE failed={e}");break;},
                Err(e) => {eprintln!("M5_SMOKE failed={e}");break;},
                _ if start.elapsed()>Duration::from_secs(35) => {eprintln!("M5_SMOKE failed=timeout phase={phase}");break;},
                _ => {}
            }
        }
        cx.update(|cx| { if let Some(item)=original {cx.write_to_clipboard(item);} else {cx.write_to_clipboard(ClipboardItem::new_string(String::new()));} cx.quit(); });
    }).detach();
}
