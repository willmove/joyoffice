//! Draw real native frames on the executable's UI thread and its default stack.
use super::*;

pub(super) fn run(handle: AnyWindowHandle, view: Entity<Editor>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let result: std::result::Result<(), String> = async {
            let require_a11y = std::env::args().any(|arg| arg == "--require-a11y");
            if require_a11y {
                eprintln!("STARTUP_SMOKE waiting_for_a11y=true");
                let began = Instant::now();
                while !handle
                    .update(cx, |_, w, _| w.is_a11y_active())
                    .map_err(|e| e.to_string())?
                {
                    if began.elapsed() > Duration::from_secs(30) {
                        return Err("no accessibility client connected within 30 seconds".into());
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                }
            }
            let mut cases = 0;
            for selection in ["empty", "shape", "table"] {
                handle
                    .update(cx, |_, w, cx| {
                        view.update(cx, |s, cx| -> std::result::Result<(), String> {
                            s.inspector_open = true;
                            s.show_notes = true;
                            s.workspace_panel = true;
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
                    // The accessibility regression targets the composed design
                    // form; the regular run still checks all 16 toolbars.
                    if require_a11y && tab != "设计" {
                        continue;
                    }
                    handle
                        .update(cx, |_, w, cx| -> std::result::Result<(), String> {
                            view.update(cx, |s, cx| s.select_ribbon(tab, w, cx));
                            // Release the entity borrow before GPUI renders it. A
                            // real draw catches cumulative stack use through the
                            // workspace, active tab, forms and child components.
                            w.refresh();
                            w.draw(cx).clear(cx);
                            if require_a11y {
                                check_a11y(w, true, true)?;
                            }
                            Ok(())
                        })
                        .map_err(|e| e.to_string())??;
                    cases += 1;
                    eprintln!("STARTUP_SMOKE selection={selection} tab={tab} drawn=true");
                    cx.background_executor()
                        .timer(Duration::from_millis(20))
                        .await;
                }
            }
            if require_a11y {
                check_drafts(handle, &view, cx).await?;
                for _ in 0..3 {
                    for (advanced, notes) in
                        [(false, true), (false, false), (true, false), (true, true)]
                    {
                        handle
                            .update(cx, |_, w, cx| -> std::result::Result<(), String> {
                                view.update(cx, |s, cx| {
                                    s.workspace_panel = advanced;
                                    s.show_notes = notes;
                                    cx.notify();
                                });
                                w.refresh();
                                w.draw(cx).clear(cx);
                                check_a11y(w, advanced, notes)
                            })
                            .map_err(|e| e.to_string())??;
                    }
                }
                eprintln!("A11Y_SMOKE design_notes_complete=true");
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

fn check_a11y(w: &Window, advanced: bool, notes: bool) -> std::result::Result<(), String> {
    if !w.is_a11y_active() {
        return Err("accessibility must remain active during the regression".into());
    }
    let json = w
        .debug_a11y_tree_json()
        .ok_or("no a11y tree was generated")?;
    let tree: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let nodes = tree["nodes"].as_object().ok_or("a11y nodes are missing")?;
    let mut ids = BTreeSet::new();
    for node in nodes.values() {
        if !ids.insert(node["accesskit_id"].clone().to_string()) {
            return Err("a11y node identity was reused".into());
        }
    }
    for (label, visible) in [
        ("演讲者备注", notes),
        ("高级设计 · 当前页备注", advanced),
        ("强调色 · 精确值", true),
        ("主题 accent #RRGGBB", advanced),
        ("背景色 · 精确值", true),
        ("主题背景 #RRGGBB", advanced),
    ] {
        let count = nodes
            .values()
            .filter(|node| node["aria"]["label"] == label)
            .count();
        if count != usize::from(visible) {
            return Err(format!(
                "a11y control {label}: expected visible={visible}, found {count}"
            ));
        }
    }
    Ok(())
}

async fn check_drafts(
    handle: AnyWindowHandle,
    view: &Entity<Editor>,
    cx: &mut AsyncApp,
) -> std::result::Result<(), String> {
    // Mutate one control, then yield so the same observers used by actual input
    // events deliver the draft to the other independently focused control.
    let draft_revision = view.read_with(cx, |s, _| s.session.revision);
    for from_advanced in [false, true] {
        let accent = if from_advanced { "#654321" } else { "#123456" };
        let background = if from_advanced { "#eeddee" } else { "#c0ffee" };
        let text = if from_advanced {
            "高级设计备注 draft"
        } else {
            "演讲者备注 draft"
        };
        handle
            .update(cx, |_, w, cx| {
                view.update(cx, |s, cx| {
                    let input = if from_advanced {
                        &s.workspace_notes_input
                    } else {
                        &s.notes_input
                    };
                    input.update(cx, |f, cx| f.set_value(text, w, cx));
                    let color = if from_advanced {
                        &s.workspace_colors[0]
                    } else {
                        &s.doc_fields[7]
                    };
                    color.update(cx, |f, cx| f.set_value(accent, w, cx));
                    let color = if from_advanced {
                        &s.workspace_colors[1]
                    } else {
                        &s.doc_fields[11]
                    };
                    color.update(cx, |f, cx| f.set_value(background, w, cx));
                })
            })
            .map_err(|e| e.to_string())?;
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
        handle
            .update(cx, |_, _, cx| {
                view.update(cx, |s, cx| -> std::result::Result<(), String> {
                    if s.session.revision != draft_revision {
                        return Err("editing drafts must not change the document".into());
                    }
                    if s.notes_input.read(cx).value() != text
                        || s.workspace_notes_input.read(cx).value() != text
                    {
                        return Err("notes draft did not synchronize in both directions".into());
                    }
                    for (index, input, value) in [
                        (7, &s.workspace_colors[0], accent),
                        (11, &s.workspace_colors[1], background),
                    ] {
                        if input.read(cx).value() != value
                            || s.doc_fields[index].read(cx).value() != value
                        {
                            return Err("theme draft did not synchronize in both directions".into());
                        }
                    }
                    Ok(())
                })
            })
            .map_err(|e| e.to_string())??;
    }
    for (step, expected) in [
        ("commit", "高级设计备注 draft"),
        ("undo", ""),
        ("redo", "高级设计备注 draft"),
        ("next-page", ""),
        ("previous-page", "高级设计备注 draft"),
        ("stale", "其他窗口的更新"),
    ] {
        handle
            .update(cx, |_, w, cx| -> std::result::Result<(), String> {
                view.update(cx, |s, cx| -> std::result::Result<(), String> {
                    match step {
                        "commit" => {
                            s.notes(cx);
                            if s.session.revision != draft_revision + 1 {
                                return Err("notes did not commit as one document command".into());
                            }
                        }
                        "undo" => s.history(false, w, cx),
                        "redo" => s.history(true, w, cx),
                        "next-page" => {
                            s.command(Command::AddPage, cx);
                            s.page_click(1, false, false, cx);
                        }
                        "previous-page" => s.page_click(0, false, false, cx),
                        "stale" => {
                            s.command(
                                Command::Design(joydeck::design_commands::Action::Notes(
                                    s.page,
                                    expected.into(),
                                )),
                                cx,
                            );
                            s.notes(cx);
                            if !s.message.contains("请刷新后重新编辑") {
                                return Err("stale notes were not rejected".into());
                            }
                        }
                        _ => {}
                    }
                    if s.session.deck.slides[s.page].notes != expected {
                        return Err(format!("notes document value is wrong after {step}"));
                    }
                    if step == "stale" {
                        // After rejecting an external change, the user explicitly
                        // refreshes. Undo/redo and page navigation must refresh
                        // through their production paths without test assistance.
                        s.sync_doc_fields(w, cx);
                    }
                    Ok(())
                })?;
                w.refresh();
                w.draw(cx).clear(cx);
                check_a11y(w, true, true)
            })
            .map_err(|e| e.to_string())??;
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
        // The notify effects from navigation must be delivered before drawing
        // its view. A background native window need not receive display ticks.
        handle
            .update(cx, |_, w, cx| -> std::result::Result<(), String> {
                w.refresh();
                w.draw(cx).clear(cx);
                check_a11y(w, true, true)
            })
            .map_err(|e| e.to_string())??;
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
        view.update(cx, |s, cx| -> std::result::Result<(), String> {
            if s.notes_input.read(cx).value() != expected
                || s.workspace_notes_input.read(cx).value() != expected
            {
                return Err(format!(
                    "notes controls diverged after {step}: main={:?}, advanced={:?}, page={}, doc_page={:?}, slide={:?}",
                    s.notes_input.read(cx).value(), s.workspace_notes_input.read(cx).value(), s.page, s.doc_page,
                    s.session.deck.slides[s.page].id
                ));
            }
            Ok(())
        })?;
    }
    eprintln!(
        "A11Y_SMOKE bidirectional_drafts=true notes_commit_undo_redo=true page_refresh=true stale_rejected=true"
    );
    Ok(())
}
