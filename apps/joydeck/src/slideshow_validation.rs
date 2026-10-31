//! Explicit native show validation, independent of the normal show controller.
use super::*;

/// Real monotonic timer and rendering acceptance, independently of virtual model tests.
pub fn smoke(
    deck: Arc<Deck>,
    output: std::path::PathBuf,
    parent: AnyWindowHandle,
    parent_focus: FocusHandle,
    cx: &mut App,
) {
    let baseline = deck.clone();
    let shared = match runtime(deck, 0) {
        Ok(shared) => shared,
        Err(error) => {
            eprintln!("M10_SMOKE failed={error}");
            return;
        }
    };
    let (h, v) = match open_runtime(shared.clone(), true, None, cx) {
        Ok(window) => window,
        Err(error) => {
            eprintln!("M10_SMOKE failed={error}");
            return;
        }
    };
    cx.spawn(async move|cx|{
        let result:std::result::Result<(),String>=async{
            cx.background_executor().timer(Duration::from_millis(350)).await;
            h.update(cx,|_,w,cx|v.update(cx,|s,cx|s.navigation("right",w,cx))).map_err(|e|e.to_string())?;
            cx.background_executor().timer(Duration::from_millis(900)).await;
            let before=shared.lock().map_err(|_| "放映状态异常".to_string())?.control.total;if before<500{return Err("native clock did not advance".into());}
            h.update(cx,|_,w,cx|v.update(cx,|s,cx|{s.navigation("p",w,cx);s.navigation("b",w,cx);})).map_err(|e|e.to_string())?;
            let frozen=shared.lock().map_err(|_| "放映状态异常".to_string())?.control.total;cx.background_executor().timer(Duration::from_millis(250)).await;if shared.lock().map_err(|_| "放映状态异常".to_string())?.control.total!=frozen{return Err("paused clock advanced".into());}
            h.update(cx,|_,w,cx|v.update(cx,|s,cx|{s.navigation("b",w,cx);s.navigation("w",w,cx);})).map_err(|e|e.to_string())?;if shared.lock().map_err(|_| "放映状态异常".to_string())?.control.blank!=Some(Blank::White){return Err("white screen failed".into());}
            h.update(cx,|_,w,cx|v.update(cx,|s,cx|{s.navigation("w",w,cx);s.navigation("p",w,cx);s.navigation("right",w,cx);})).map_err(|e|e.to_string())?;
            cx.background_executor().timer(Duration::from_millis(700)).await;
            if shared.lock().map_err(|_| "放映状态异常".to_string())?.snapshot.page()!=1{return Err("effect/page navigation failed".into());}
            let report=h.update(cx,|_,_,cx|{let s=v.read(cx);let mut times=s.render_times.clone();times.sort();serde_json::json!({"renderedFrames":s.frames,"renderP95Ms":times.get(times.len()*95/100).copied().unwrap_or(0)as f64/1000.,"totalRenderMs":s.render_ms,"monotonicPause":true,"blackWhite":true,"snapshotUnchanged":*s.snapshot.deck==*baseline,"displays":cx.displays().iter().map(|d|u64::from(d.id())).collect::<Vec<_>>(),"scope":"native preview timer/render + one physical display; not physical dual-screen/hotplug or end-to-end 60fps certification"})}).map_err(|e|e.to_string())?;
            let before=shared.lock().map_err(|_| "放映状态异常".to_string())?.control.total;
            let (second,second_view)=cx.update(|cx|open_runtime(shared.clone(),true,None,cx)).map_err(|e|e.to_string())?;
            cx.background_executor().timer(Duration::from_millis(250)).await;
            let delta=shared.lock().map_err(|_| "放映状态异常".to_string())?.control.total.saturating_sub(before);
            if !(100..=400).contains(&delta){return Err(format!("clock handoff delta={delta}"));}
            let _=h.update(cx,|_,w,_|w.remove_window());
            second.update(cx,|_,w,cx|second_view.update(cx,|s,cx|s.navigation("escape",w,cx))).map_err(|e|e.to_string())?;
            parent.update(cx,|_,w,_|w.activate_window()).map_err(|e|e.to_string())?;
            cx.background_executor().timer(Duration::from_millis(250)).await;
            if !parent.update(cx,|_,w,_|parent_focus.is_focused(w)).map_err(|e|e.to_string())?{return Err("editor focus was not restored after show".into());}
            let mut report=report;report["editorFocusRestored"]=serde_json::json!(true);report["clockHandoff"]=serde_json::json!(true);report["handoffDeltaMs"]=serde_json::json!(delta);
            std::fs::write(output.join("native-show.json"),serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e|e.to_string())?;
            if shared.lock().map_err(|_| "放映状态异常".to_string())?.snapshot.deck!=baseline{return Err("show mutated document".into());}
            Ok(())
        }.await;
        match result{Ok(())=>eprintln!("M10_SMOKE clock_pause_blank_navigation_snapshot=true complete=true"),Err(e)=>eprintln!("M10_SMOKE failed={e}")};cx.update(|cx|cx.quit());
    }).detach();
}
