//! Draw real native frames on the executable's UI thread and its default stack.
use super::*;

pub(super) fn run(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let result: std::result::Result<(), String> = async {
            let mut cases = 0;
            for selection in ["empty", "shape", "table"] {
                handle
                    .update(cx, |_, w, cx| {
                        view.update(cx, |s, cx| -> std::result::Result<(), String> {
                            s.inspector_open = true;
                            s.show_notes = true;
                            if selection == "shape" {
                                s.command(
                                    Command::Design(joydeck::design_commands::Action::SetDesign(
                                        joydeck::design::starter(),
                                    )),
                                    cx,
                                );
                                s.command(
                                    Command::AddObject(0, joydeck::preset::object("roundRect")?),
                                    cx,
                                );
                                s.select(0, w, cx);
                            } else if selection == "table" {
                                table_demo::seed(s, w, cx);
                            }
                            Ok(())
                        })
                    })
                    .map_err(|e| e.to_string())??;
                for tab in [
                    "开始",
                    "插入",
                    "表格",
                    "设计",
                    "视图",
                    "文件",
                    "图表",
                    "图示",
                    "动画",
                    "切换",
                    "幻灯片放映",
                    "审阅",
                    "媒体",
                    "输出",
                    "比较",
                    "维护",
                ] {
                    handle
                        .update(cx, |_, w, cx| {
                            view.update(cx, |s, cx| s.select_ribbon(tab, w, cx));
                            // Release the entity borrow before GPUI renders it. A
                            // real draw catches cumulative stack use through the
                            // workspace, active tab, forms and child components.
                            w.refresh();
                            w.draw(cx).clear(cx);
                        })
                        .map_err(|e| e.to_string())?;
                    cases += 1;
                    eprintln!("STARTUP_SMOKE selection={selection} tab={tab} drawn=true");
                    cx.background_executor()
                        .timer(Duration::from_millis(20))
                        .await;
                }
            }
            eprintln!("STARTUP_SMOKE complete=true cases={cases}");
            Ok(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("STARTUP_SMOKE failed={error}");
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}
