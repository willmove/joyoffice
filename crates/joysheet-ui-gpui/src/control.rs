//! Production view automation: requests dispatch real GPUI events on the GUI thread.
use crate::desktop::{After, Desktop};
use gpui_kit::{prelude::*, *};
use joysheet_app::control::{ControlServer, UiCall, UiQueue};
use serde_json::{Value, json};

pub struct ControlRuntime {
    pub _server: Option<ControlServer>,
    pub queue: UiQueue,
    pub diagnostics: bool,
}
impl Global for ControlRuntime {}
pub struct Capture {
    pub call: UiCall,
    pub frame: u64,
    pub deadline: std::time::Instant,
}
pub struct Barrier {
    pub id: u64,
    pub ticket: Option<u64>,
    pub result: Option<Result<Value, String>>,
    pub call: UiCall,
    pub errors: u64,
    pub doc: u64,
    pub kind: String,
    pub deadline: std::time::Instant,
}
fn version_ready(params: &Value, view: Option<&joysheet_app::WindowSnapshot>, sequence: u64) -> bool {
    params.get("minSequence").and_then(Value::as_u64).is_none_or(|n| sequence >= n)
        && params.get("minRevision").and_then(Value::as_u64).is_none_or(|n| view.is_some_and(|v| v.revision >= n))
}
fn flag(params: &Value, name: &str) -> bool {
    params.get(name).and_then(Value::as_bool).unwrap_or(false)
}
fn number(params: &Value, name: &str) -> Result<f32, String> {
    params.get(name).and_then(Value::as_f64).filter(|n| n.is_finite()).map(|n| n as f32).ok_or_else(|| format!("missing finite {name}"))
}
fn modifiers(params: &Value) -> Modifiers {
    Modifiers {
        shift: flag(params, "shift"),
        alt: flag(params, "alt"),
        control: flag(params, "ctrl") || (!cfg!(target_os = "macos") && flag(params, "cmd")),
        platform: cfg!(target_os = "macos") && flag(params, "cmd"),
        ..Default::default()
    }
}
impl Desktop {
    pub fn start_control_command(&mut self, id: u64, command: &str, params: Value, cx: &mut Context<Self>) {
        let target = self.target();
        if let Some(b) = self.barriers.iter_mut().find(|b| b.id == id) {
            match self.engine.submit(command, params, target) {
                Ok(ticket) => {
                    b.ticket = Some(ticket.id);
                    self.pending.push(crate::desktop::Pending { ticket, after: After::None, commit: false, clipboard: false });
                }
                Err(e) => b.result = Some(Err(e)),
            }
        }
        cx.notify();
    }
    pub fn finish_barriers(&mut self) {
        let now = std::time::Instant::now();
        let mut i = 0;
        while i < self.barriers.len() {
            if let Some(result) = self.barriers.get_mut(i).and_then(|b| b.result.take()) {
                self.barriers.remove(i).call.respond(result);
                continue;
            }
            let b = &self.barriers[i];
            let missing = self.window_id.is_some_and(|id| self.snapshot.window(id).is_none());
            let error = if missing {
                Some("target window closed".into())
            } else if b.kind == "ui.awaitIdle" && self.error_count > b.errors {
                Some(self.message.clone())
            } else if now >= b.deadline {
                Some("UI completion timed out; an admitted engine operation may still be running".into())
            } else {
                None
            };
            let ready = b.kind == "ui.awaitIdle"
                && self.pending.is_empty()
                && !self.committing
                && self.draft.is_none()
                && self.engine.latest().sequence == self.snapshot.sequence
                && self.painted_sequence == self.snapshot.sequence
                && version_ready(&b.call.params, self.view(), self.snapshot.sequence);
            if let Some(e) = error {
                self.barriers.remove(i).call.respond(Err(e));
            } else if ready {
                let b = self.barriers.remove(i);
                let view = if matches!(b.kind.as_str(), "app.open" | "app.new") { self.snapshot.active() } else { self.view() };
                let result=view.map(|v|json!({"window":v.window,"doc":v.doc,"path":v.path,"dirty":v.dirty,"revision":v.revision,"sequence":self.snapshot.sequence})).ok_or_else(||"No target document".to_string());
                if !matches!(b.kind.as_str(), "app.open" | "app.new") && view.is_none_or(|v| v.doc != b.doc) {
                    b.call.respond(Err("target document changed".into()));
                } else {
                    b.call.respond(result);
                }
            } else {
                i += 1;
            }
        }
    }
    pub fn poll_control(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_barriers();
        let now = std::time::Instant::now();
        let mut expired = 0;
        while expired < self.captures.len() {
            if self.captures.get(expired).is_some_and(|c| c.deadline <= now) {
                self.captures.remove(expired).call.respond(Err("screenshot timed out before a current frame became available".into()));
            } else {
                expired += 1;
            }
        }
        if !self.committing && self.engine.latest().sequence == self.snapshot.sequence && self.painted_sequence == self.snapshot.sequence {
            let mut index = 0;
            while index < self.captures.len() {
                if self.captures.get(index).is_some_and(|capture| {
                    self.frame_number > capture.frame
                        && (self.pending.is_empty() || flag(&capture.call.params, "allowBusy"))
                        && version_ready(&capture.call.params, self.view(), self.snapshot.sequence)
                }) {
                    let call = self.captures.remove(index).call;
                    let stamp = self
                        .view()
                        .map(|v| json!({"window":v.window,"doc":v.doc,"revision":v.revision,"sequence":self.painted_sequence}))
                        .unwrap_or(Value::Null);
                    window.defer(cx, move |window, _| {
                        let result = screenshot(window, &call.params).map(|mut result| {
                            if let (Some(m), Some(stamp)) = (result.as_object_mut(), stamp.as_object()) {
                                m.extend(stamp.clone());
                            }
                            result
                        });
                        call.respond(result);
                    });
                } else {
                    index += 1;
                }
            }
        }
        let queue = cx.global::<ControlRuntime>().queue.clone();
        if let Some(call) = queue.take(self.window_id) {
            if !call.claim() {
                return;
            }
            self.control_call(call, window, cx);
        }
    }
    fn control_call(&mut self, call: UiCall, window: &mut Window, cx: &mut Context<Self>) {
        for key in ["minRevision", "minSequence"] {
            if call.params.get(key).is_some_and(|v| v.as_u64().is_none()) {
                call.respond(Err(format!("{key} must be an unsigned integer")));
                return;
            }
        }
        let params = &call.params;
        match call.method.as_str() {
            "ui.accessibility" => {
                let cells = self
                    .view()
                    .zip(self.geometry.as_ref())
                    .map(|(v, g)| crate::accessibility::cells(v, &self.layout, g, &self.selection()))
                    .unwrap_or_default();
                let result = json!({"enabled":window.is_a11y_enabled(),"platformActive":window.is_a11y_active(),"cells":cells.iter().map(|c|c.inspect()).collect::<Vec<_>>(),"tree":window.debug_a11y_tree_json()});
                call.respond(Ok(result));
            }
            #[cfg(feature = "automation")]
            "ui.dpi" => {
                if flag(params, "reset") {
                    window.bounds_changed(cx);
                } else if let Some(scale) = params.get("scale").and_then(Value::as_f64).filter(|s| (0.75..=3.).contains(s)) {
                    window.set_scale_factor(scale as f32);
                } else {
                    call.respond(Err("Diagnostic scale must be between 0.75 and 3".into()));
                    return;
                }
                window.invalidate_character_coordinates();
                cx.notify();
                let result = json!({"scaleFactor":window.scale_factor(),"synthetic":!flag(params,"reset")});
                call.respond(Ok(result));
            }
            "ui.openFiles" => {
                let paths = params
                    .get("paths")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "paths must be a list of local paths".to_string())
                    .and_then(|v| {
                        v.iter()
                            .map(|p| p.as_str().ok_or_else(|| "Expected a local file path".to_string()).and_then(joysheet_platform::local_open_path))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .and_then(joysheet_platform::workbook_paths);
                match paths {
                    Ok(paths) => {
                        self.commit(After::OpenPaths(paths), window, cx);
                        call.respond(Ok(json!({"submitted":true})));
                    }
                    Err(e) => call.respond(Err(e)),
                }
            }
            "ui.ribbon" => {
                let tab = params.get("tab").and_then(Value::as_str).unwrap_or("Home").to_string();
                self.daily_group = match tab.as_str() {
                    "Objects" => 22,
                    "Recovery" => 21,
                    "Settings" => 20,
                    "Home" => 1,
                    "Insert" => 10,
                    "Data" => 4,
                    "Review" => 15,
                    "View" => 3,
                    "Automate" => 16,
                    "Page Layout" => 13,
                    "Formulas" => 19,
                    _ => {
                        call.respond(Err("unsupported tab in GPUI shell".into()));
                        return;
                    }
                };
                if self.daily_group == 16 {
                    self.issue("__joysheet.scriptDetails", json!({}), After::None, cx);
                }
                cx.notify();
                call.respond(Ok(json!({"panel":tab})));
            }
            "ui.message.dismiss" => {
                self.error = false;
                self.message = "就绪 / Ready".into();
                cx.notify();
                call.respond(Ok(Value::Null));
            }
            "app.save" | "app.open" | "app.new" | "ui.awaitIdle" => {
                if call.method != "ui.awaitIdle" && self.composing(window, cx) {
                    call.respond(Err("Finish or cancel IME composition before document operations".into()));
                    return;
                }
                if self.barriers.len() >= 8 {
                    call.respond(Err("At most eight UI completion waits".into()));
                    return;
                }
                if self.confirm_close || self.trust_request.is_some() {
                    call.respond(Err("A desktop confirmation is active".into()));
                    return;
                }
                if self.committing && call.method != "ui.awaitIdle" {
                    call.respond(Err("A draft commit is in progress; wait for ui.awaitIdle before retrying".into()));
                    return;
                }
                let kind = call.method.clone();
                let doc = self.view().map(|v| v.doc).unwrap_or(0);
                let errors = self.error_count;
                let mut p = call.params.clone();
                if let Some(m) = p.as_object_mut() {
                    m.remove("window");
                    m.remove("doc");
                    m.remove("minSequence");
                    m.remove("minRevision");
                }
                if kind == "app.save" && p.get("path").is_none() && self.view().is_none_or(|v| v.path.is_none()) {
                    call.respond(Err("Specify a save path for a new workbook".into()));
                    return;
                }
                if kind == "app.open" && p.get("path").is_none() && p.get("base64").is_none() {
                    call.respond(Err("Specify path or base64 to open a workbook".into()));
                    return;
                }
                self.next_barrier = self.next_barrier.saturating_add(1);
                let id = self.next_barrier;
                let action = if kind != "ui.awaitIdle" {
                    let command = match kind.as_str() {
                        "app.save" => "file.save",
                        "app.open" => "file.open",
                        _ => "file.new",
                    };
                    Some((command.to_string(), p))
                } else {
                    None
                };
                self.barriers.push(Barrier {
                    id,
                    ticket: None,
                    result: None,
                    call,
                    errors,
                    doc,
                    deadline: std::time::Instant::now() + std::time::Duration::from_secs(if kind.starts_with("app.") { 58 } else { 7 }),
                    kind,
                });
                if let Some((command, params)) = action {
                    self.commit(After::Control(id, command, params), window, cx);
                    if self.error_count > errors
                        && let Some(b) = self.barriers.iter_mut().find(|b| b.id == id)
                    {
                        b.result = Some(Err(self.message.clone()));
                    }
                }
                cx.notify();
            }
            "ui.recovery.list" => {
                call.respond(serde_json::to_value(&cx.global::<crate::recovery::RecoveryRuntime>().entries).map_err(|e| e.to_string()));
            }
            "ui.recovery.restore" => {
                if let Some(id) = params.get("id").and_then(Value::as_str) {
                    self.restore_recovery(id.into(), params.get("draft").and_then(Value::as_u64).unwrap_or(0) as usize, window, cx);
                    call.respond(Ok(json!({"submitted":true})));
                } else {
                    call.respond(Err("Missing recovery id".into()));
                }
            }
            "ui.inspect" => {
                let mut inspect = self.snapshot.inspect();
                inspect["window"] = json!(self.window_id);
                inspect["editor"] =
                    self.draft.as_ref().map(|d| json!({"cell":d.cell.a1(),"text":d.text,"inline":d.inline,"doc":d.doc})).unwrap_or(Value::Null);
                inspect["selection"] = json!(self.selection().a1());
                inspect["scroll"] = json!({"x":self.scroll.0,"y":self.scroll.1});
                inspect["visibleColumns"] =
                    json!(self.layout.cols.iter().map(|l| json!({"index":l.index,"start":l.start,"size":l.size})).collect::<Vec<_>>());
                inspect["visibleRows"] =
                    json!(self.layout.rows.iter().map(|l| json!({"index":l.index,"start":l.start,"size":l.size})).collect::<Vec<_>>());
                inspect["sheetNames"] = json!(self.view().map(|v| v.workbook.sheets.iter().map(|s| s.name.clone()).collect::<Vec<_>>()));
                inspect["grid"] = json!({"x":f32::from(self.grid_bounds.origin.x),"y":f32::from(self.grid_bounds.origin.y),"width":self.layout.width,"height":self.layout.height});
                inspect["perf"] = json!({"paintMs":self.paint_ms,"paintedCells":self.painted_cells,"frame":self.frame_number});
                inspect["message"] = json!(self.message);
                inspect["autosave"] = json!(self.preferences.autosave);
                inspect["closeSaving"] = json!(self.close_saving);
                inspect["closeButtons"] = if self.confirm_close {
                    json!(self.close_button_bounds.iter().map(|(name,b)|(name.to_string(),json!({"x":f32::from(b.origin.x),"y":f32::from(b.origin.y),"width":f32::from(b.size.width),"height":f32::from(b.size.height)}))).collect::<serde_json::Map<_,_>>())
                } else {
                    Value::Null
                };
                inspect["worksheet"]=self.view().and_then(|v|v.sheet()).map(|s|json!({"name":s.name,"tables":s.tables.len(),"conditionalFormats":s.cond_formats.len(),"validations":s.validations.len(),"hiddenRows":s.rows.keys().filter(|r|s.is_row_hidden(**r)).take(1000).map(|r|r+1).collect::<Vec<_>>()})).unwrap_or(Value::Null);
                inspect["recentFiles"] = json!(self.preferences.recent);
                inspect["dataResults"] = json!(self.data_results);
                inspect["dataResultKind"] = json!(self.data_result_kind);
                inspect["tasks"] = json!(self.engine.tasks());
                inspect["dataRaw"] = self.data_raw.clone();
                inspect["advancedBounds"]=json!(self.advanced_bounds.iter().map(|(key,b)|(key.clone(),json!({"x":f32::from(b.origin.x),"y":f32::from(b.origin.y),"width":f32::from(b.size.width),"height":f32::from(b.size.height)}))).collect::<serde_json::Map<_,_>>());
                inspect["sheetCount"] = json!(self.view().map(|v| v.workbook.sheets.len()));
                inspect["review"]=self.view().map(|v|json!({"comments":v.sheet().map(|s|s.comments.iter().map(|(at,c)|json!({"cell":at.a1(),"author":c.author,"text":c.text,"resolved":c.resolved,"replies":c.replies.len(),"threaded":c.threaded})).collect::<Vec<_>>()),"sheetProtected":v.sheet().is_some_and(|s|s.protection.is_some()),"structureProtected":v.workbook.protected_structure,"names":v.workbook.names.iter().filter(|n|!n.hidden).map(|n|&n.name).collect::<Vec<_>>(),"scriptsTrusted":v.scripts_trusted,"recording":v.recording})).unwrap_or(Value::Null);
                inspect["decodedImages"] = json!(self.object_images.borrow().values().filter(|(_, image)| image.is_some()).count());
                inspect["selectedObject"] = json!(self.selected_object);
                inspect["selectedObjects"] = json!(self.object_selection().into_iter().map(|(k, _)| k).collect::<Vec<_>>());
                inspect["formulaReferences"] = json!(
                    self.draft
                        .as_ref()
                        .map(|d| crate::formula_feedback::references(&d.text))
                        .unwrap_or_default()
                        .iter()
                        .map(|r| json!({"label":r.label,"color":r.color,"range":r.range.a1(),"sheet":r.sheet}))
                        .collect::<Vec<_>>()
                );
                inspect["formulaHint"] = self
                    .formula_hint(cx)
                    .map(|h| json!({"signature":h.signature,"description":h.description,"active":[h.active.start,h.active.end]}))
                    .unwrap_or(Value::Null);
                inspect["outlineText"] = json!({"fontsReady":crate::outline_text::ready(),"cachedBlocks":crate::outline_text::cached()});
                inspect["nativeWindows"] = self.native_windows(cx);
                inspect["arrangement"] = self.last_arrangement.clone();
                inspect["keyTrace"] = json!(self.key_trace);
                inspect["focus"] = json!({"grid":self.editor.read(cx).focus_handle(cx).is_focused(window),"formula":self.formula.read(cx).focus_handle(cx).is_focused(window),"name":self.name.read(cx).focus_handle(cx).is_focused(window),"sheets":self.sheets_focus.is_focused(window)});
                inspect["printStatus"] = json!(self.print_status);
                inspect["printWork"] = json!({"exporting":self.print_jobs.len(),"rendering":self.print_rendering,"dialogOpen":self.print_dialog_generation.is_some(),"current":self.preview_current()});
                inspect["printPreview"] = self
                    .preview
                    .as_ref()
                    .map(|p| json!({"pages":p.document.pages.len(),"page":p.page+1,"pdf":p.document.pdf,"images":p.document.pages}))
                    .unwrap_or(Value::Null);
                inspect["objectBounds"]=json!(self.view().zip(self.geometry.as_ref()).map(|(v,g)|crate::objects::entries(v).into_iter().map(|(o,a)|{
                    let b=crate::objects::rect(&a,&self.layout,g,v.sheet().and_then(|s|s.freeze).unwrap_or((0,0)))+self.grid_bounds.origin;
                    json!({"kind":o.kind,"id":o.id,"x":f32::from(b.origin.x),"y":f32::from(b.origin.y),"width":f32::from(b.size.width),"height":f32::from(b.size.height)})
                }).collect::<Vec<_>>()).unwrap_or_default());
                inspect["drawingObjects"]=self.view().and_then(|v|v.sheet()).map(|s|json!({"charts":s.charts.iter().map(|o|json!({"id":o.id,"anchor":o.anchor})).collect::<Vec<_>>(),"images":s.images.iter().map(|o|json!({"id":o.id,"anchor":o.anchor})).collect::<Vec<_>>(),"shapes":s.shapes.iter().map(|o|json!({"id":o.id,"anchor":o.anchor})).collect::<Vec<_>>()})).unwrap_or(Value::Null);
                inspect["formulaText"] = json!(self.formula.read(cx).value().to_string());
                inspect["cellEditorText"] = json!(self.editor.read(cx).value().to_string());
                inspect["dark"] = json!(self.preferences.dark);
                inspect["settings"] = json!(self.preferences);
                inspect["ribbonTab"] = json!(self.ribbon_tab);
                inspect["panel"] = json!(self.daily_group);
                inspect["composing"] = json!(self.composing(window, cx));
                inspect["inputGeometry"] = self.input_geometry(window, cx);
                inspect["display"] = json!({"scaleFactor":window.scale_factor(),"viewportWidth":f32::from(window.viewport_size().width),"viewportHeight":f32::from(window.viewport_size().height)});
                inspect["error"] = json!(self.error);
                inspect["dialog"] = json!(if self.validation_prompt.is_some() {
                    "validationConfirmation"
                } else if self.trust_request.is_some() {
                    "scriptTrust"
                } else if self.confirm_close {
                    "saveChanges"
                } else if self.palette_open {
                    "commands"
                } else {
                    ""
                });
                inspect["capabilities"] = json!({"screenshot":cfg!(feature="automation"),"nativeImeVerified":false,"gpuiInput":true,"versionFences":true,"lifecycleCompletion":true,"scriptCancellation":"betweenCommands","mcpTargetedScreenshots":true});
                call.respond(Ok(inspect));
            }
            "ui.screenshot" => {
                window.activate_window();
                if self.captures.len() >= 4 {
                    call.respond(Err("at most four screenshots may be outstanding".into()));
                    return;
                }
                self.captures.push(Capture {
                    call,
                    frame: self.frame_number,
                    deadline: std::time::Instant::now() + std::time::Duration::from_secs(7),
                });
                cx.notify();
            }
            "ui.key" | "ui.text" | "ui.click" | "ui.drag" | "ui.move" | "ui.cell.click" | "ui.cell.drag" | "ui.scroll" => {
                let events = self.control_events(&call.method, params);
                match events {
                    Err(error) => call.respond(Err(error)),
                    Ok(events) => window.defer(cx, move |window, cx| {
                        for event in events {
                            match event {
                                Synthetic::Key(key) => {
                                    if key.key_char.is_some() {
                                        window.dispatch_keystroke(key, cx);
                                    } else {
                                        // Non-text native keys must not run GPUI's simulated IME:
                                        // it would insert a newline into a marked composition on Enter.
                                        window.dispatch_event(
                                            PlatformInput::KeyDown(KeyDownEvent { keystroke: key, is_held: false, prefer_character_input: false }),
                                            cx,
                                        );
                                    }
                                }
                                Synthetic::Pointer(event) => {
                                    window.dispatch_event(event, cx);
                                }
                            }
                        }
                        call.respond(Ok(json!({"dispatched":true})));
                    }),
                }
            }
            "ui.import.preview" => {
                self.preview_import(window, cx);
                call.respond(Ok(json!({"submitted":true})));
            }
            "ui.print.dialog" => {
                let errors = self.error_count;
                self.system_print(cx);
                if self.error_count > errors {
                    call.respond(Err(self.message.clone()));
                } else {
                    call.respond(Ok(json!({"submitted":true})));
                }
            }
            "ui.print.close" => {
                self.close_print_preview(window, cx);
                call.respond(Ok(json!({"closed":true})));
            }
            "ui.command" | "ui.menu.invoke" | "command" => {
                if let Some(command) = params
                    .get("command")
                    .or_else(|| params.get("id"))
                    .and_then(Value::as_str)
                    .filter(|id| gridcraft_engine::find_command(id).is_some())
                {
                    let arguments = params.get("params").cloned().unwrap_or_else(|| json!({}));
                    if !arguments.is_object() {
                        call.respond(Err("parameters must be an object".into()));
                        return;
                    }
                    if command == "view.arrangeAll" {
                        if self.confirm_close || self.validation_prompt.is_some() || self.trust_request.is_some() {
                            call.respond(Err("Finish the active confirmation first".into()));
                            return;
                        }
                        self.issue(command, arguments, After::None, cx);
                        call.respond(Ok(json!({"submitted":true})));
                        return;
                    }
                    let after = match command {
                        "file.close" | "window.close" => After::Close(false),
                        _ => After::Command(command.into(), arguments),
                    };
                    self.commit(after, window, cx);
                    call.respond(Ok(json!({"submitted":true})));
                } else {
                    call.respond(Err("unknown command".into()));
                }
            }
            "ui.edit.begin" => {
                let text = params.get("text").and_then(Value::as_str).map(str::to_string);
                self.begin_edit(text, params.get("inline").and_then(Value::as_bool).unwrap_or(true), window, cx);
                call.respond(Ok(json!({"editing":true})));
            }
            "ui.edit.text" => match params.get("text").and_then(Value::as_str) {
                Some(text) if text.len() <= joysheet_protocol::MAX_TEXT_BYTES => {
                    self.begin_edit(Some(text.to_string()), true, window, cx);
                    call.respond(Ok(json!({"editing":true})));
                }
                _ => call.respond(Err("missing text or text exceeds 1 MiB".into())),
            },
            "ui.edit.commit" => {
                self.commit(After::None, window, cx);
                call.respond(Ok(json!({"submitted":true})));
            }
            "ui.edit.cancel" => {
                self.cancel_edit(window, cx);
                call.respond(Ok(json!({"cancelled":true})));
            }
            "ui.resize" => {
                let dimensions = number(params, "width").and_then(|w| number(params, "height").map(|h| (w, h)));
                match dimensions {
                    Ok((w, h)) if (320.0..=8192.).contains(&w) && (240.0..=8192.).contains(&h) => {
                        window.resize(size(px(w), px(h)));
                        cx.notify();
                        call.respond(Ok(json!({"resized":true})));
                    }
                    _ => call.respond(Err("invalid window dimensions".into())),
                }
            }
            "ui.focus" => {
                window.activate_window();
                self.focus_grid(window, cx);
                call.respond(Ok(json!({"focused":true})));
            }
            "ui.set" => {
                let result = self.set_options(params, cx);
                call.respond(result);
            }
            "ui.dialog" => {
                if let Some(name) = params.get("name").and_then(Value::as_str).map(str::to_string) {
                    self.open_command(&name, window, cx);
                    call.respond(Ok(json!({"opened":name})));
                } else {
                    call.respond(Err("missing dialog name".into()));
                }
            }
            "ui.dialog.cancel" => {
                if self.close_saving {
                    call.respond(Err("Save is still pending".into()));
                    return;
                }
                self.confirm_close = false;
                self.trust_request = None;
                self.validation_prompt = None;
                self.palette_open = false;
                self.focus_grid(window, cx);
                cx.notify();
                call.respond(Ok(json!({"cancelled":true})));
            }
            "ui.dialog.set" => {
                let mut current = serde_json::from_str::<Value>(&self.params.read(cx).value()).unwrap_or_else(|_| json!({}));
                if !current.is_object() {
                    current = json!({});
                }
                if let Some(field) = params.get("field").and_then(Value::as_str) {
                    if self.selected_command.as_deref().is_some_and(|id| {
                        crate::advanced::multiline(id, field) || field == "text" && ["data.getData", "data.fromTextCsv"].contains(&id)
                    }) {
                        let text = params.get("value").and_then(Value::as_str).unwrap_or("").to_string();
                        self.import_text.update(cx, |input, cx| input.set_value(text, window, cx));
                    }
                    for entry in &self.command_fields {
                        if entry.key == field {
                            let value = params.get("value").cloned().unwrap_or(Value::Null);
                            let text = value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string());
                            entry.value.update(cx, |input, cx| input.set_value(text, window, cx));
                        }
                    }
                    current[field] = params.get("value").cloned().unwrap_or(Value::Null);
                    self.params.update(cx, |input, cx| input.set_value(current.to_string(), window, cx));
                    call.respond(Ok(json!({"updated":true})));
                } else {
                    call.respond(Err("missing field".into()));
                }
            }
            "ui.dialog.confirm" => {
                self.run_selected_command(window, cx);
                call.respond(Ok(json!({"submitted":true})));
            }
            #[cfg(feature = "automation")]
            "ui.ime.mark" => {
                if let Some(text) = params.get("text").and_then(Value::as_str).filter(|s| s.len() <= joysheet_protocol::MAX_TEXT_BYTES) {
                    let text = text.to_string();
                    let editor = self.active_text_input(window, cx);
                    window.defer(cx, move |window, cx| {
                        let caret = text.encode_utf16().count();
                        editor.update(cx, |input, cx| input.replace_and_mark_text_in_range(None, &text, Some(caret..caret), window, cx));
                        call.respond(Ok(json!({"composing":true})));
                    });
                } else {
                    call.respond(Err("invalid composition text".into()));
                }
            }
            #[cfg(feature = "automation")]
            "ui.ime.unmark" => {
                self.active_text_input(window, cx).update(cx, |input, cx| input.unmark_text(window, cx));
                call.respond(Ok(json!({"composing":false})));
            }
            "ui.perf" => {
                let stages = self.performance.report(flag(params, "raw"));
                let recovery = cx.global::<crate::recovery::RecoveryRuntime>().service.profile();
                let mut frames = self.frame_samples.clone();
                frames.sort_by(f64::total_cmp);
                let p95 = frames.get(frames.len() * 95 / 100).copied().unwrap_or_default();
                let mut edits = self.edit_samples.clone();
                edits.sort_by(f64::total_cmp);
                let edit_p95 = edits.get(edits.len() * 95 / 100).copied();
                if flag(params, "reset") {
                    self.frame_samples.clear();
                    self.edit_samples.clear();
                    self.performance.reset();
                }
                call.respond(Ok(json!({"kind":"CPU worksheet paint submission","samples":frames.len(),"paintP95Ms":p95,"paintedCells":self.painted_cells,"gpuPresentationMeasured":false,"editSamples":edits.len(),"editP95Ms":edit_p95,"stages":stages,"recovery":recovery,"editScope":"GPUI commit event through engine completion to worksheet CPU paint; excludes GPU presentation"})));
            }
            "app.quit" => {
                let force = flag(params, "force");
                if !force && (self.snapshot.windows.iter().any(|v| v.dirty) || !cx.global::<crate::desktop::NativeWindows>().1.is_empty()) {
                    call.respond(Err("unsaved workbooks or draft; save them or explicitly pass force".into()));
                    return;
                }
                let windows: Vec<_> = self.snapshot.windows.iter().map(|v| v.window).collect();
                for id in windows {
                    match self.engine.submit("window.close", json!({"window":id,"force":force}), Default::default()) {
                        Ok(ticket) => self.pending.push(crate::desktop::Pending { ticket, after: After::None, commit: false, clipboard: false }),
                        Err(error) => self.alert(error),
                    }
                }
                call.respond(Ok(json!({"closing":true})));
            }
            _ => {
                let error = format!("UI method not implemented: {}", call.method);
                call.respond(Err(error));
            }
        }
    }
    fn control_events(&self, method: &str, params: &Value) -> Result<Vec<Synthetic>, String> {
        let modifiers = modifiers(params);
        if method == "ui.key" {
            let key = params.get("key").and_then(Value::as_str).ok_or("missing key")?.to_ascii_lowercase();
            let key = match key.as_str() {
                "arrowup" => "up",
                "arrowdown" => "down",
                "arrowleft" => "left",
                "arrowright" => "right",
                _ => &key,
            };
            return Ok(vec![Synthetic::Key(Keystroke { modifiers, key: key.to_string(), key_char: None })]);
        }
        if method == "ui.text" {
            let text = params.get("text").and_then(Value::as_str).ok_or("missing text")?;
            if text.len() > joysheet_protocol::MAX_TEXT_BYTES {
                return Err("text exceeds 1 MiB".into());
            }
            let mut key = Keystroke::parse("a").map_err(|e| e.to_string())?;
            key.key_char = Some(text.into());
            return Ok(vec![Synthetic::Key(key)]);
        }
        let cell_point = |name: &str| -> Result<Point<Pixels>, String> {
            let cell = params
                .get(name)
                .and_then(Value::as_str)
                .and_then(gridcraft_engine::core::CellRef::parse)
                .ok_or_else(|| format!("missing valid {name}"))?;
            let view = self.view().ok_or("no document")?;
            let geo = self.geometry.as_ref().ok_or("grid not laid out")?;
            let rect = self.layout.cell_rect(cell, geo, view.sheet().and_then(|s| s.freeze).unwrap_or((0, 0)));
            let x = rect.x + rect.width / 2.;
            let y = rect.y + rect.height / 2.;
            if self.layout.hit(x, y) != Some(cell) {
                return Err("cell is outside the viewport; use edit.goTo first".into());
            }
            Ok(self.grid_bounds.origin + point(px(x), px(y)))
        };
        let position = if method == "ui.cell.click" {
            cell_point("cell")?
        } else if method == "ui.cell.drag" {
            cell_point("from")?
        } else {
            point(px(number(params, "x")?), px(number(params, "y")?))
        };
        if method == "ui.scroll" {
            return Ok(vec![Synthetic::Pointer(PlatformInput::Scroll(ScrollEvent {
                position,
                delta: ScrollDelta::Pixels(point(
                    px(params.get("dx").and_then(Value::as_f64).unwrap_or(0.) as f32),
                    px(params.get("dy").and_then(Value::as_f64).unwrap_or(0.) as f32),
                )),
                modifiers,
                touch_phase: TouchPhase::Moved,
                source: ScrollSource::Pointer,
            }))]);
        }
        let mut events = vec![Synthetic::Pointer(PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: None, modifiers }))];
        if method == "ui.move" {
            return Ok(events);
        }
        let count = params.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 2) as usize;
        events.push(Synthetic::Pointer(PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers,
            click_count: count,
            first_mouse: false,
        })));
        let to = if method == "ui.cell.drag" {
            cell_point("to")?
        } else if method == "ui.drag" {
            point(px(number(params, "toX")?), px(number(params, "toY")?))
        } else {
            position
        };
        if to != position {
            events.push(Synthetic::Pointer(PlatformInput::MouseMove(MouseMoveEvent {
                position: to,
                pressed_button: Some(MouseButton::Left),
                modifiers,
            })));
        }
        events.push(Synthetic::Pointer(PlatformInput::MouseUp(MouseUpEvent {
            position: to,
            button: MouseButton::Left,
            modifiers,
            click_count: count,
        })));
        Ok(events)
    }
}
enum Synthetic {
    Key(Keystroke),
    Pointer(PlatformInput),
}

#[cfg(feature = "automation")]
fn screenshot(window: &mut Window, params: &Value) -> Result<Value, String> {
    use image::ImageEncoder;
    let viewport = window.viewport_size();
    let scale = window.scale_factor();
    let bytes = f64::from(f32::from(viewport.width)) * f64::from(f32::from(viewport.height)) * f64::from(scale).powi(2) * 4.;
    if bytes > 128. * 1024. * 1024. {
        return Err("screenshot exceeds 128 MiB".into());
    }
    let image = window.render_to_image().map_err(|e| e.to_string())?;
    let mut writer = LimitedPng(Vec::new());
    image::codecs::png::PngEncoder::new(&mut writer)
        .write_image(image.as_raw(), image.width(), image.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    if let Some(path) = params.get("path").and_then(Value::as_str) {
        std::fs::write(path, &writer.0).map_err(|e| e.to_string())?;
        Ok(json!({"path":path,"width":image.width(),"height":image.height()}))
    } else {
        if writer.0.len() > 48 * 1024 * 1024 {
            return Err("base64 screenshot exceeds 64 MiB".into());
        }
        Ok(json!({"base64":gridcraft_engine::io::base64_encode(&writer.0),"width":image.width(),"height":image.height()}))
    }
}
#[cfg(feature = "automation")]
struct LimitedPng(Vec<u8>);
#[cfg(feature = "automation")]
impl std::io::Write for LimitedPng {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > 64 * 1024 * 1024 {
            return Err(std::io::Error::other("encoded screenshot exceeds 64 MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[cfg(not(feature = "automation"))]
fn screenshot(_: &mut Window, _: &Value) -> Result<Value, String> {
    Err("build with --features automation to enable native GPU readback".into())
}
