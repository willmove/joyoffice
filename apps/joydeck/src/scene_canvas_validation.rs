use super::*;

pub(crate) fn smoke(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    view.update(cx, |s, cx| {
        s.command(Command::AddObject(0, edit::object("rect")), cx);
        let mut text = edit::object("text");
        text.x = 400.;
        text.text = "共享轮廓 Cached glyphs".into();
        s.command(Command::AddObject(0, text), cx);
        let mut translucent = edit::object("ellipse");
        translucent.x = 680.;
        translucent.width = 100.;
        translucent.opacity = 0.5;
        s.command(Command::AddObject(0, translucent), cx);
    });
    cx.spawn(async move|cx|{
        let began=Instant::now();let mut phase=0;
        let mut before=PaintStats::default();let mut builds=0;
        let mut moved:Option<Arc<Deck>>=None;
        loop {
            cx.background_executor().timer(Duration::from_millis(150)).await;
            let done=handle.update(cx,|_,w,cx|view.update(cx,|s,cx|{
                match phase {
                    0 if s.scene.as_ref().is_some_and(|scene|scene.nodes.len()==3)&&s.gpu_cache.stats.paths>0&&s.gpu_cache.stats.fallbacks>0=>{
                        assert!(s.accelerated);assert!(s.scene.as_ref().unwrap().coverage().2>0);
                        before=s.gpu_cache.stats.clone();builds=s.scene_cache.lock().unwrap().stats.builds;
                        let scale=s.canvas_scale(w);
                        let origin=s.canvas_origin;
                        let p=|x,y|point(px(origin.0+x*scale),px(origin.1+y*scale));
                        s.down(&MouseDownEvent{position:p(520.,130.),button:MouseButton::Left,modifiers:Modifiers::default(),click_count:1,first_mouse:false},scale,w,cx);
                        for i in 1..=48 {
                            s.move_pointer(&MouseMoveEvent{position:p(520.+30.*i as f32/48.,130.+20.*i as f32/48.),pressed_button:Some(MouseButton::Left),modifiers:Modifiers::default()},cx);
                        }
                        phase=1;
                    },
                    1=>{
                        assert_eq!(s.gpu_cache.stats.full_rasters,before.full_rasters);
                        assert_eq!(s.gpu_cache.stats.uploads,before.uploads);
                        assert_eq!(s.gpu_cache.stats.tessellations,before.tessellations);
                        assert_eq!(s.scene_cache.lock().unwrap().stats.builds,builds);
                        s.end_gesture(cx);phase=2;
                        eprintln!("M4_SMOKE move_no_page_raster=true move_no_upload=true cached_glyphs=true");
                    },
                    2 if s.scene.as_ref().is_some_and(|scene|scene.revision==s.session.revision)=>{
                        assert!((s.session.deck.slides[0].objects[1].x-430.).abs()<0.001);
                        assert!((s.session.deck.slides[0].objects[1].y-100.).abs()<0.001);
                        assert_eq!(s.scene_cache.lock().unwrap().stats.builds,builds);
                        s.history(false,w,cx);assert!((s.session.deck.slides[0].objects[1].x-400.).abs()<0.001);
                        s.history(true,w,cx);assert!((s.session.deck.slides[0].objects[1].x-430.).abs()<0.001);
                        moved=Some(s.session.deck.clone());s.toggle_renderer(cx);phase=3;
                    },
                    3 if s.image.is_some()&&!s.render_busy=>{
                        assert!(!s.accelerated);assert!(s.gpu_cache.stats.full_rasters>0);
                        assert!(Arc::ptr_eq(moved.as_ref().unwrap(),&s.session.deck));
                        s.gpu_cache.clear();assert_eq!(s.gpu_cache.stats.gpu_cache_bytes,0);
                        s.toggle_renderer(cx);before=s.gpu_cache.stats.clone();phase=4;
                    },
                    4 if s.gpu_cache.stats.uploads>before.uploads&&s.scene.as_ref().is_some_and(|scene|scene.revision==s.session.revision)=>{
                        assert!(s.accelerated);assert!(s.gpu_cache.stats.atlas_removals>before.atlas_removals);assert!(Arc::ptr_eq(moved.as_ref().unwrap(),&s.session.deck));
                        assert_eq!(s.gpu_cache.stats.full_rasters,before.full_rasters);
                        eprintln!("M4_SMOKE cpu_switch_undo_redo=true cache_rebuild_preserves_document=true");
                        let mut changed=s.session.deck.slides[0].objects[1].clone();changed.fill="#336699".into();
                        s.command(Command::ReplaceObject(0,1,changed),cx);
                        s.request_scene(cx);
                        s.session=Session::new();s.page=0;s.selection.clear();s.changed(cx);
                        phase=5;
                    },
                    5 if !s.scene_busy&&s.scene.as_ref().is_some_and(|scene|scene.document==s.session.id)=>{
                        assert!(s.accelerated);assert!(s.scene.as_ref().unwrap().nodes.is_empty());
                        eprintln!("M4_SMOKE stale_document_job_discarded=true complete=true");
                        return true;
                    },_=>(),
                }
                false
            })).unwrap_or(true);
            if done {break;}
            if began.elapsed()>Duration::from_secs(25) {eprintln!("M4_SMOKE failed=timeout phase={phase}");break;}
        }
        if !std::env::args().any(|s|s=="--hold"){cx.update(|cx|cx.quit());}
    }).detach();
}
