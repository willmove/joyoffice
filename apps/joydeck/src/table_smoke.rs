//! Real GPUI acceptance route. Screenshot acknowledgements come from the external verifier.
use super::*;
use joydeck::table::{self, Action, Range};
pub(super) fn seed(s: &mut Editor, w: &mut Window, cx: &mut Context<Editor>) {
    s.session = Session::new();
    s.renew_recovery();
    s.page = 0;
    let mut title = edit::object("text");
    title.x = 64.;
    title.y = 42.;
    title.width = 820.;
    title.height = 48.;
    title.text = "季度业务回顾".into();
    title.fill = "#25334b".into();
    title.font_size = 32.;
    let mut sub = title.clone();
    sub.y = 94.;
    sub.height = 32.;
    sub.font_size = 14.;
    sub.fill = "#8490a5".into();
    sub.text = "FY 2026  /  运营表现与下一阶段重点".into();
    s.command(
        Command::Batch(vec![
            Command::AddObject(0, title),
            Command::AddObject(0, sub),
        ]),
        cx,
    );
    let mut o = table::object(4, 5).unwrap();
    o.x = 64.;
    o.y = 166.;
    o.width = 832.;
    o.height = 240.;
    table::resize(&mut o);
    s.command(Command::AddObject(0, o), cx);
    let id = s.session.deck.slides[0].objects[2].id.clone();
    s.command(Command::Table(0,id.clone(),Action::Paste(0,0,"业务板块\t目标 / 万\t实际 / 万\t完成率\t同比增长\n企业服务\t120\t136\t113%\t+24%\n软件订阅\t240\t268\t112%\t+32%\n专业支持\t80\t92\t115%\t+18%".into())),cx);
    let mut note = edit::object("text");
    note.x = 64.;
    note.y = 441.;
    note.width = 832.;
    note.height = 40.;
    note.text = "核心业务持续增长，订阅收入占比进一步提升。".into();
    note.font_size = 16.;
    note.fill = "#69788f".into();
    s.command(Command::AddObject(0, note), cx);
    s.command(
        Command::Design(joydeck::design_commands::Action::Properties(
            "季度业务回顾".into(),
            joydeck::design::Organization {
                author: "JoyDeck".into(),
                subject: "原创表格验收样例".into(),
                ..Default::default()
            },
            "2026-10-30".into(),
        )),
        cx,
    );
    s.select(2, w, cx);
    s.table_pick = Some(table_ui::Pick {
        id,
        anchor: (0, 0),
        focus: (0, 0),
    });
    s.ribbon = "表格".into();
    s.changed(cx);
}
pub(super) fn run(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    let clipboard = cx.read_from_clipboard();
    cx.spawn(async move|cx|{
        let output=std::env::var_os("JOYDECK_M8_OUTPUT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);let _=std::fs::create_dir_all(&output);
        let mut phase=0;let mut since=Instant::now();let start=Instant::now();
        let run:std::result::Result<(),String>=async{
            loop {cx.background_executor().timer(Duration::from_millis(150)).await;if start.elapsed()>Duration::from_secs(45){return Err(format!("timeout phase={phase}"));}
                if phase==0 {
                    let ready=handle.update(cx,|_,w,_|w.is_window_active()).map_err(|e|e.to_string())?;if !ready{continue;}
                    handle.update(cx,|_,w,cx|view.update(cx,|s,cx|->std::result::Result<(),String>{if !s.focus.is_focused(w){return Err("initial editor keyboard focus missing".into());}w.set_window_title("JoyDeck — M8 Table Acceptance");seed(s,w,cx);
                        let id=s.table_pick.as_ref().unwrap().id.clone();let base=s.session.deck.clone();
                        s.command(Command::Table(0,id.clone(),Action::Merge(Range{top:2,left:3,bottom:3,right:4})),cx);s.command(Command::Table(0,id.clone(),Action::Split(2,3)),cx);s.history(false,w,cx);s.history(false,w,cx);if s.session.deck!=base{return Err("merge/split undo failed".into());}
                        s.command(Command::Table(0,id.clone(),Action::InsertRow(2)),cx);s.command(Command::Table(0,id.clone(),Action::DeleteRow(2)),cx);s.command(Command::Table(0,id.clone(),Action::InsertColumn(3)),cx);s.command(Command::Table(0,id.clone(),Action::DeleteColumn(3)),cx);if s.session.deck!=base{return Err("insert/delete preservation failed".into());}
                        s.begin_cell(2,1,0,w,cx);s.replace_and_mark_text_in_range(None,"取消",None,w,cx);if s.session.deck!=base{return Err("preedit published".into());}s.text_edit.as_mut().unwrap().cancel();s.finish_text(cx);
                        let event=KeyDownEvent{keystroke:Keystroke{key:"tab".into(),..Default::default()},is_held:false,prefer_character_input:false};if !s.table_key(&event,w,cx)||s.table_pick.as_ref().unwrap().focus!=(1,1){return Err("Tab navigation failed".into());}s.finish_text(cx);
                        let mut reverse=event;reverse.keystroke.modifiers.shift=true;s.table_key(&reverse,w,cx);if s.table_pick.as_ref().unwrap().focus!=(1,0){return Err("reverse Tab failed".into());}s.finish_text(cx);
                        // Exercise a real row-divider gesture: draft is separate, mouse-up is one command.
                        let o=s.session.deck.slides[0].objects[2].clone();s.table_resize=Some(table_ui::Resize{base:o,row:true,index:1,start:0.,delta:8.,scale:1.});s.end_table_resize(cx);s.history(false,w,cx);if s.session.deck!=base{return Err("row resize undo failed".into());}
                        s.begin_cell(2,1,0,w,cx);s.cancel_gesture(&CancelGesture,w,cx);if s.text_edit.is_some()||s.table_pick.is_none(){return Err("first Escape must leave cell text".into());}s.cancel_gesture(&CancelGesture,w,cx);if s.table_pick.is_some(){return Err("second Escape must release table navigation".into());}s.table_pick=Some(table_ui::Pick{id:id.clone(),anchor:(0,0),focus:(0,0)});
                        s.select(2,w,cx);s.copy_objects(cx);s.paste_at_focus(w,cx);if s.session.deck.slides[0].objects.len()!=base.slides[0].objects.len()+1{return Err("table object clipboard routed into cell text".into());}s.history(false,w,cx);if s.session.deck!=base{return Err("table clipboard undo failed".into());}
                        // Save normal, public-table-only and merged samples for independent consumers.
                        edit::save(&output.join("table.joydeck"),&s.session.deck)?;if edit::open_editable(&output.join("table.joydeck"))?!=*s.session.deck{return Err("native roundtrip failed".into());}
                        joydeck::export::save_parts(&output.join("table.pptx"),&joydeck::pptx_write::generate(&s.session.deck)?,&AtomicBool::new(false))?;
                        s.command(Command::Table(0,id.clone(),Action::Merge(Range{top:2,left:3,bottom:3,right:4})),cx);let merged=s.session.deck.clone();
                        joydeck::export::save_parts(&output.join("merged.pptx"),&joydeck::pptx_write::generate(&merged)?,&AtomicBool::new(false))?;s.history(false,w,cx);
                        joydeck::export::pdf(&output.join("table.pdf"),&s.session.deck,&joydeck::export::Options{pages:vec![0],dpi:144,transparent:false,notes:false},&AtomicBool::new(false))?;
                        s.selection.clear();s.selected=None;s.table_pick=None;s.ribbon="开始".into();s.changed(cx);
                        eprintln!("M8_SMOKE native_roundtrip=true structure_undo=true table_navigation=true composition_draft=true divider_undo=true");Ok(())
                    })).map_err(|e|e.to_string())??;phase=1;since=Instant::now();continue;
                }
                if (phase==1||phase==3)&&since.elapsed()>Duration::from_secs(2){
                    let ready=handle.update(cx,|_,w,cx|view.update(cx,|s,_|->std::result::Result<bool,String>{let backend=if phase==1{"GPUI"}else{"CPU"};if (phase==1&&s.scene.is_none())||(phase==3&&s.image.is_none()){return Ok(false);}
                        let scale=s.canvas_scale(w);let width=s.session.deck.width*scale;let height=s.session.deck.height*scale;let raster=preview::render(&s.session.deck,0,(width*w.scale_factor()).round() as u32,&AtomicBool::new(false))?;let mut rgba=raster.bgra;for p in rgba.as_chunks_mut::<4>().0{p.swap(0,2);}image::RgbaImage::from_raw(raster.width,raster.height,rgba).unwrap().save(output.join(format!("M8-{backend}-reference.png"))).map_err(|e|e.to_string())?;
                        eprintln!("M8_CAPTURE {backend} {}",serde_json::json!({"x":s.canvas_origin.0,"y":s.canvas_origin.1,"width":width,"height":height,"viewportHeight":f32::from(w.viewport_size().height)}));Ok(true)
                    })).map_err(|e|e.to_string())??;if ready{phase+=1;}continue;
                }
                if phase==2&&output.join("capture-0.ack").exists(){handle.update(cx,|_,_,cx|view.update(cx,|s,cx|{s.accelerated=false;s.changed(cx);})).map_err(|e|e.to_string())?;phase=3;since=Instant::now();}
                if phase==4&&output.join("capture-1.ack").exists(){break;}
            }Ok(())
        }.await;
        match run{Ok(())=>eprintln!("M8_SMOKE complete=true"),Err(e)=>eprintln!("M8_SMOKE failed={e}")}
        cx.update(|cx|{cx.write_to_clipboard(clipboard.unwrap_or_else(||ClipboardItem::new_string(String::new())));cx.quit();});
    }).detach();
}
