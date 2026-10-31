use crate::desktop::{After, Desktop, Draft, NativeWindows, Pending};
use gpui_kit::{prelude::*, *};
use joysheet_engine::{core::CellRef, display};
use serde_json::json;

impl Desktop {
    pub fn composing(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.editor.update(cx, |s, cx| s.marked_text_range(window, cx).is_some())
            || self.formula.update(cx, |s, cx| s.marked_text_range(window, cx).is_some())
    }
    pub fn guard_composition(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.composing(window, cx) {
            self.alert("请先完成或取消输入法组合输入 / Finish or cancel IME composition first");
            cx.notify();
            true
        } else {
            false
        }
    }
    pub fn capture_input_navigation(&mut self, key: &str, shift: bool, command: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft.is_none() && self.editor.read(cx).focus_handle(cx).is_focused(window) {
            self.key_down(
                &KeyDownEvent {
                    keystroke: Keystroke {
                        key: key.into(),
                        key_char: None,
                        modifiers: Modifiers {
                            shift,
                            control: command && !cfg!(target_os = "macos"),
                            platform: command && cfg!(target_os = "macos"),
                            ..Default::default()
                        },
                    },
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
        }
    }
    pub fn track_draft(&self, cx: &mut Context<Self>) {
        if let Some(id) = self.window_id {
            cx.update_global::<crate::recovery::RecoveryRuntime, _>(|r, _| {
                if let Some(d) = self.draft.as_ref().filter(|d| d.text != d.original) {
                    r.drafts.insert(id, json!({"doc":d.doc,"sheet":d.sheet,"sheetName":d.sheet_name,"cell":d.cell.a1(),"text":d.text}));
                } else {
                    r.drafts.remove(&id);
                }
            });
            cx.update_global::<NativeWindows, _>(|windows, _| {
                if self.draft.as_ref().is_some_and(|d| d.text != d.original) {
                    windows.1.insert(id);
                } else {
                    windows.1.remove(&id);
                }
            });
        }
    }
    pub fn capture_text_command(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirm_close {
            cx.stop_propagation();
            return;
        }
        if self.preview.is_some() {
            cx.stop_propagation();
            return;
        }
        if self.draft.is_some() || !self.editor.read(cx).focus_handle(cx).is_focused(window) {
            return;
        }
        let params = if command == "edit.paste" {
            self.clipboard.read_external().map(|text| json!({"text":text})).unwrap_or_else(|| json!({}))
        } else {
            json!({"html":true})
        };
        self.issue(command, params, After::None, cx);
        cx.stop_propagation();
    }
    pub fn begin_edit(&mut self, replacement: Option<String>, inline: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.guard_composition(window, cx) {
            return;
        }
        if self.committing
            || self.confirm_close
            || self.trust_request.is_some()
            || self.validation_prompt.is_some()
            || self.palette_open
            || self.preview.is_some()
        {
            return;
        }
        if self.selected_anchor().is_some() {
            self.alert("按 Esc 选择单元格，或用对象属性编辑 / Press Esc for cells, or edit object properties");
            cx.notify();
            return;
        }
        if self.draft.is_none() {
            let Some(view) = self.view() else {
                return;
            };
            let Some(sheet) = view.sheet() else {
                return;
            };
            let cell = self.selection().active;
            let original = display::edit_text(&view.workbook, sheet, cell);
            self.draft = Some(Draft {
                window: view.window,
                doc: view.doc,
                sheet: view.sheet,
                sheet_name: sheet.name.clone(),
                cell,
                text: original.clone(),
                original,
                inline,
            });
        }
        if let Some(draft) = self.draft.as_mut() {
            draft.inline = inline;
            if let Some(text) = replacement {
                draft.text = text;
            }
        }
        let text = self.draft.as_ref().map(|d| d.text.clone()).unwrap_or_default();
        self.track_draft(cx);
        self.formula.update(cx, |input, cx| input.set_value(text.clone(), window, cx));
        self.editor.update(cx, |input, cx| input.set_value(text, window, cx));
        if inline {
            self.focus_grid(window, cx);
        } else {
            self.formula.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }
    pub fn begin_formula(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.update(cx, |s, cx| s.marked_text_range(window, cx).is_some()) {
            self.focus_grid(window, cx);
            return;
        }
        if self.committing {
            return;
        }
        if let Some(draft) = self.draft.as_mut() {
            draft.inline = false;
            cx.notify();
            return;
        }
        let Some(view) = self.view() else {
            return;
        };
        let Some(sheet) = view.sheet() else {
            return;
        };
        let cell = view.selection.active;
        let original = display::edit_text(&view.workbook, sheet, cell);
        self.draft = Some(Draft {
            window: view.window,
            doc: view.doc,
            sheet: view.sheet,
            sheet_name: sheet.name.clone(),
            cell,
            text: original.clone(),
            original,
            inline: false,
        });
        self.track_draft(cx);
        cx.notify();
    }
    pub fn input_changed(&mut self, text: String, inline: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.committing {
            return;
        }
        if self.draft.is_none() {
            let Some(view) = self.view() else {
                return;
            };
            let Some(sheet) = view.sheet() else {
                return;
            };
            let cell = view.selection.active;
            let original = display::edit_text(&view.workbook, sheet, cell);
            self.draft = Some(Draft {
                window: view.window,
                doc: view.doc,
                sheet: view.sheet,
                sheet_name: sheet.name.clone(),
                cell,
                text: text.clone(),
                original,
                inline,
            });
        }
        if let Some(draft) = self.draft.as_mut() {
            draft.text = text.clone();
            draft.inline = inline;
        }
        self.track_draft(cx);
        // set_value is silent: the active editor retains its selection and IME transaction.
        if inline {
            self.formula.update(cx, |input, cx| input.set_value(text, window, cx));
        } else {
            self.editor.update(cx, |input, cx| input.set_value(text, window, cx));
        }
        cx.notify();
    }
    pub fn commit(&mut self, after: After, window: &mut Window, cx: &mut Context<Self>) {
        let commit_started = std::time::Instant::now();
        if self.guard_composition(window, cx) {
            return;
        }
        if self.committing || self.confirm_close || self.trust_request.is_some() || self.validation_prompt.is_some() {
            return;
        }
        let Some(draft) = self.draft.as_ref() else {
            self.after(after, window, cx);
            return;
        };
        if draft.text == draft.original {
            self.draft = None;
            self.track_draft(cx);
            self.sync_inputs(window, cx);
            self.after(after, window, cx);
            return;
        }
        if draft.text.len() > joysheet_protocol::MAX_TEXT_BYTES {
            self.alert("单元格输入超过 1 MiB / Input exceeds 1 MiB");
            cx.notify();
            return;
        }
        let current = self.snapshot.window(draft.window).filter(|v| v.doc == draft.doc).and_then(|v| {
            v.workbook.sheets.get(draft.sheet).filter(|s| s.name == draft.sheet_name).map(|s| display::edit_text(&v.workbook, s, draft.cell))
        });
        if current.as_deref() != Some(draft.original.as_str()) {
            self.alert("单元格已被其他操作修改；按 Esc 重新载入 / Cell changed externally; press Esc to reload");
            cx.notify();
            return;
        }
        if !self.validation_accepted
            && let Some(view) = self.snapshot.window(draft.window)
            && let Some((rule, message)) = joysheet_engine::cmd::data::check_validation(&view.workbook, draft.sheet, draft.cell, &draft.text)
            && rule.show_error
            && rule.error_style != joysheet_engine::model::ErrorStyle::Stop
        {
            self.validation_prompt = Some(crate::validation::ValidationPrompt { after, message, revision: view.revision });
            self.trust_focus.focus(window, cx);
            cx.notify();
            return;
        }
        let target = joysheet_protocol::CommandTarget { window: Some(draft.window), doc: None };
        let revision = self.snapshot.window(draft.window).map(|v| v.revision);
        if self.snapshot.window(draft.window).is_some_and(|v| v.sheet != draft.sheet) {
            for (command, params) in [("sheet.activate", json!({"sheet":draft.sheet})), ("selection.set", json!({"range":draft.cell.a1()}))] {
                if let Err(e) = self.engine.submit(command, params, target) {
                    self.alert(e);
                    cx.notify();
                    return;
                }
            }
        }
        let edit_started = std::time::Instant::now();
        match self.engine.submit_guarded(
            "__joysheet.validatedSet",
            json!({"sheet":draft.sheet,"cell":draft.cell.a1(),"input":draft.text,"validationAccepted":self.validation_accepted}),
            target,
            revision,
        ) {
            Ok(ticket) => {
                self.performance.begin(
                    ticket.id,
                    draft.window,
                    draft.doc,
                    draft.sheet,
                    commit_started,
                    edit_started.duration_since(commit_started).as_secs_f64() * 1000.,
                );
                self.edit_clock = Some((edit_started, draft.doc));
                self.edit_fence = None;
                self.committing = true;
                self.pending.push(Pending { ticket, after, commit: true, clipboard: false });
            }
            Err(error) => self.alert(error),
        }
        cx.notify();
    }
    pub fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.guard_composition(window, cx) {
            return;
        }
        if self.committing {
            return;
        }
        self.draft = None;
        self.track_draft(cx);
        self.sync_inputs(window, cx);
        self.focus_grid(window, cx);
        cx.notify();
    }
    pub fn reference_text(&self, range: &str) -> String {
        if let (Some(draft), Some(view)) = (&self.draft, self.view())
            && draft.sheet != view.sheet
            && let Some(sheet) = view.sheet()
        {
            return format!("'{}'!{range}", sheet.name.replace('\'', "''"));
        }
        range.into()
    }
    pub fn insert_reference(&mut self, reference: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.as_ref() else {
            return;
        };
        let inline = draft.inline;
        let input = if inline { self.editor.clone() } else { self.formula.clone() };
        input.update(cx, |input, cx| {
            input.replace_text_in_range(None, reference, window, cx);
            input.focus(window, cx);
        });
    }
    pub fn zoom(&mut self, delta: i32, cx: &mut Context<Self>) {
        let current = self.view().and_then(|v| v.sheet()).map(|s| s.zoom).unwrap_or(100);
        self.issue("view.zoom", json!({"percent":(current as i32+delta).clamp(10,400)}), After::None, cx);
    }
    pub fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if cx.global::<crate::control::ControlRuntime>().diagnostics
            && matches!(event.keystroke.key.as_str(), "left" | "right" | "up" | "down" | "escape" | "tab" | "enter" | "f2" | "f6")
        {
            self.key_trace.push(format!(
                "{} shift={} palette={} grid={} formula={}",
                event.keystroke.key,
                event.keystroke.modifiers.shift,
                self.palette_open,
                self.editor.read(cx).focus_handle(cx).is_focused(window),
                self.formula.read(cx).focus_handle(cx).is_focused(window)
            ));
            if self.key_trace.len() > 16 {
                self.key_trace.remove(0);
            }
        }
        if self.snapshot.active_window != self.window_id
            && let Some(id) = self.window_id
        {
            self.issue("window.activate", json!({"window":id}), After::None, cx);
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let command = if cfg!(target_os = "macos") { modifiers.platform } else { modifiers.control };
        if self.validation_prompt.is_some() {
            match key {
                "escape" => {
                    self.validation_prompt = None;
                    self.focus_grid(window, cx);
                    cx.notify();
                }
                "enter" if self.trust_focus.is_focused(window) => self.accept_validation(window, cx),
                "tab" | "enter" => return,
                _ => {}
            }
            cx.stop_propagation();
            return;
        }
        if self.trust_request.is_some() {
            if key == "escape" {
                self.trust_request = None;
                self.focus_grid(window, cx);
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        if self.confirm_close {
            match key {
                "escape" => self.cancel_close(window, cx),
                "enter" if self.close_focus.is_focused(window) => self.save_then_close(window, cx),
                "s" if command => self.save_then_close(window, cx),
                "tab" | "enter" => return,
                _ => {}
            }
            cx.stop_propagation();
            return;
        }
        let grid_focus = self.editor.read(cx).focus_handle(cx).is_focused(window);
        let formula_focus = self.formula.read(cx).focus_handle(cx).is_focused(window);
        // A native composition must finish before spreadsheet navigation or submission.
        let composing = if grid_focus {
            self.editor.update(cx, |input, cx| input.marked_text_range(window, cx).is_some())
        } else if formula_focus {
            self.formula.update(cx, |input, cx| input.marked_text_range(window, cx).is_some())
        } else {
            false
        };
        if composing {
            return;
        }
        if self.preview.is_some() {
            match key {
                "escape" => {
                    self.close_print_preview(window, cx);
                }
                "tab" => return,
                "pageup" | "left" => {
                    if let Some(p) = &mut self.preview {
                        p.page = p.page.saturating_sub(1);
                    }
                }
                "pagedown" | "right" => {
                    if let Some(p) = &mut self.preview {
                        p.page = (p.page + 1).min(p.document.pages.len().saturating_sub(1));
                    }
                }
                "p" if command => self.system_print(cx),
                _ => {}
            }
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if key == "f6" && !self.palette_open {
            let band = if grid_focus {
                0
            } else if formula_focus {
                1
            } else if self.name.read(cx).focus_handle(cx).is_focused(window) {
                2
            } else if self.sheets_focus.is_focused(window) {
                4
            } else {
                3
            };
            self.focus_band = (band + if modifiers.shift { 4 } else { 1 }) % 5;
            if self.focus_band == 1 && !self.preferences.formula_bar {
                self.focus_band = if modifiers.shift { 0 } else { 2 };
            }
            match self.focus_band {
                0 => self.focus_grid(window, cx),
                1 => self.formula.update(cx, |s, cx| s.focus(window, cx)),
                2 => self.name.update(cx, |s, cx| s.focus(window, cx)),
                4 => self.sheets_focus.focus(window, cx),
                _ => self.ribbon_focus.focus(window, cx),
            }
            cx.stop_propagation();
            return;
        }
        if command {
            match key {
                "1" => {
                    self.commit(After::Command("home.formatCells".into(), json!({})), window, cx);
                    cx.stop_propagation();
                    return;
                }
                "," => {
                    self.daily_group = 20;
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "f" | "h" if self.draft.is_none() => {
                    self.open_command(if key == "f" { "edit.find" } else { "edit.replace" }, window, cx);
                    cx.stop_propagation();
                    return;
                }
                "k" => {
                    self.palette_open = true;
                    self.search.update(cx, |input, cx| input.focus(window, cx));
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "s" => {
                    self.commit(After::Save(modifiers.shift), window, cx);
                    cx.stop_propagation();
                    return;
                }
                "p" => {
                    self.commit(After::Command("file.printPreview".into(), json!({})), window, cx);
                    cx.stop_propagation();
                    return;
                }
                "o" => {
                    self.pick_open(window, cx);
                    cx.stop_propagation();
                    return;
                }
                "n" => {
                    self.commit(After::Command("file.new".into(), json!({})), window, cx);
                    cx.stop_propagation();
                    return;
                }
                "w" => {
                    self.commit(After::Close(false), window, cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        if key == "escape" {
            if self.preview.take().is_some() {
                cx.notify();
                cx.stop_propagation();
                return;
            }
            self.selected_object = None;
            self.selected_objects.clear();
            self.object_drag = None;
            if self.palette_open {
                self.palette_open = false;
                self.selected_command = None;
                self.focus_grid(window, cx);
            } else if self.confirm_close {
                self.confirm_close = false;
            } else if self.daily_group != 0 && self.draft.is_none() {
                self.daily_group = 0;
                self.focus_grid(window, cx);
            } else {
                self.cancel_edit(window, cx);
            }
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if !grid_focus && !formula_focus {
            return;
        }
        if key == "space" && grid_focus && self.draft.is_none() && (command || modifiers.shift) {
            let at = self.selection().active;
            let range = if command {
                let c = joysheet_engine::core::col_to_letters(at.col);
                format!("{c}:{c}")
            } else {
                format!("{0}:{0}", at.row + 1)
            };
            self.issue("selection.set", json!({"range":range}), After::None, cx);
            cx.stop_propagation();
            return;
        }
        if key == "space"
            && grid_focus
            && !command
            && !modifiers.shift
            && self.draft.is_none()
            && self
                .view()
                .and_then(|v| v.sheet().map(|s| (v, s)))
                .is_some_and(|(v, s)| v.workbook.styles.get(s.style_id(self.selection().active)).num_fmt.as_str() == "checkbox")
        {
            self.issue("cell.toggleCheckbox", json!({}), After::None, cx);
            cx.stop_propagation();
            return;
        }
        if self.draft.is_none() && grid_focus && self.selected_anchor().is_some() {
            let step = if modifiers.shift { 10. } else { 1. };
            let movement = match key {
                "left" => Some((-step, 0.)),
                "right" => Some((step, 0.)),
                "up" => Some((0., -step)),
                "down" => Some((0., step)),
                _ => None,
            };
            if !command && let Some((dx, dy)) = movement {
                self.nudge_objects(dx, dy, cx);
                cx.stop_propagation();
                return;
            }
            if key == "tab" {
                self.cycle_object(modifiers.shift, cx);
                cx.stop_propagation();
                return;
            }
            if key == "enter" || key == "f2" {
                self.daily_group = 12;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            if command && key == "a" {
                self.selected_objects = self.view().map(crate::objects::entries).unwrap_or_default().into_iter().take(256).map(|(k, _)| k).collect();
                cx.notify();
                cx.stop_propagation();
                return;
            }
        }
        if key == "enter" || key == "tab" {
            if key == "tab" && self.try_complete_formula(window, cx) {
                cx.stop_propagation();
                return;
            }
            let step = if modifiers.shift { -1 } else { 1 };
            self.commit(if key == "tab" { After::Move(0, step) } else { After::Move(step, 0) }, window, cx);
            self.focus_grid(window, cx);
            cx.stop_propagation();
            return;
        }
        if self.draft.is_some() {
            return;
        }
        if !grid_focus {
            return;
        }
        if self.selected_anchor().is_some() && matches!(key, "backspace" | "delete") {
            self.delete_selected_object(cx);
            cx.stop_propagation();
            return;
        }
        if command {
            let action = match key {
                "z" => Some(if modifiers.shift { "edit.redo" } else { "edit.undo" }),
                "y" => Some("edit.redo"),
                "c" => Some("edit.copy"),
                "x" => Some("edit.cut"),
                "b" => Some("home.bold"),
                "i" => Some("home.italic"),
                "u" => Some("home.underline"),
                "a" => Some("edit.selectAll"),
                _ => None,
            };
            if let Some(action) = action {
                self.issue(action, json!({"html":true}), After::None, cx);
                cx.stop_propagation();
                return;
            }
            if key == "v" {
                let text = self.clipboard.read_external();
                let params = text.map(|text| json!({"text":text})).unwrap_or_else(|| json!({}));
                self.issue("edit.paste", params, After::None, cx);
                cx.stop_propagation();
                return;
            }
            if key == "g" {
                self.name.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
                cx.stop_propagation();
                return;
            }
            if key == "f" {
                self.open_command("edit.find", window, cx);
                cx.stop_propagation();
                return;
            }
        }
        let movement = match key {
            "up" => Some((-1, 0)),
            "down" => Some((1, 0)),
            "left" => Some((0, -1)),
            "right" => Some((0, 1)),
            "pageup" => Some((-(self.layout.rows.len().saturating_sub(2) as i64).max(1), 0)),
            "pagedown" => Some(((self.layout.rows.len().saturating_sub(2) as i64).max(1), 0)),
            _ => None,
        };
        if let Some((dr, dc)) = movement {
            self.move_selection(dr, dc, modifiers.shift, command, cx);
            cx.stop_propagation();
            return;
        }
        match key {
            "f2" => self.begin_edit(None, true, window, cx),
            "backspace" | "delete" => self.issue("edit.clearContents", json!({}), After::None, cx),
            "home" => {
                let row = if command { 0 } else { self.selection().active.row };
                self.select(CellRef::new(row, 0), modifiers.shift, cx);
            }
            "end" if command => {
                let cell = self.view().and_then(|v| v.sheet()).and_then(|s| s.used_range()).map(|r| r.end).unwrap_or_default();
                self.select(cell, modifiers.shift, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }
    fn try_complete_formula(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.preferences.autocomplete {
            return false;
        }
        let Some(draft) = self.draft.as_ref() else {
            return false;
        };
        if !draft.text.starts_with('=') {
            return false;
        }
        let input = if draft.inline { self.editor.clone() } else { self.formula.clone() };
        let caret = input.read(cx).selected_range().end.min(draft.text.len());
        let before = draft.text.get(..caret).unwrap_or("");
        let start =
            before.char_indices().rfind(|(_, c)| !c.is_ascii_alphanumeric() && *c != '.' && *c != '_').map(|(i, c)| i + c.len_utf8()).unwrap_or(0);
        let Some(prefix) = draft.text.get(start..caret) else {
            return false;
        };
        if prefix.is_empty() {
            return false;
        }
        let prefix = prefix.to_ascii_uppercase();
        if let Some(function) = joysheet_functions::all().iter().find(|function| function.name.starts_with(&prefix)) {
            let end = caret
                + draft
                    .text
                    .get(caret..)
                    .unwrap_or("")
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_')
                    .map(char::len_utf8)
                    .sum::<usize>();
            let add_paren = !draft.text.get(end..).is_some_and(|s| s.starts_with('('));
            let text = format!("{}{}", function.name, if add_paren { "(" } else { "" });
            input.update(cx, |state, cx| {
                state.set_selected_range(start..end, cx);
                state.replace_text_in_range(None, &text, window, cx);
            });
            return true;
        }
        false
    }
    pub fn formula_hint(&self, cx: &App) -> Option<crate::formula_feedback::Hint> {
        let text = &self.draft.as_ref()?.text;
        if !text.starts_with('=') {
            return None;
        }
        let draft = self.draft.as_ref()?;
        let input = if draft.inline { &self.editor } else { &self.formula };
        let caret = input.read(cx).selected_range().end.min(text.len());
        if let Some((name, argument)) = crate::formula_feedback::context(text, caret)
            && let Some(f) = joysheet_functions::all().iter().find(|f| f.name.eq_ignore_ascii_case(&name))
        {
            return Some(crate::formula_feedback::Hint {
                signature: f.signature.into(),
                description: format!("参数 {} · {}", argument + 1, f.description),
                active: crate::formula_feedback::argument_span(f.signature, argument),
            });
        }
        let text = text.get(..caret)?;
        let mut opens = Vec::new();
        let mut quoted = false;
        for (i, c) in text.char_indices() {
            match c {
                '"' => quoted = !quoted,
                '(' if !quoted => opens.push(i),
                ')' if !quoted => {
                    opens.pop();
                }
                _ => {}
            }
        }
        if quoted {
            return None;
        }
        if let Some(open) = opens.last() {
            let before = text.get(..*open)?;
            let name = before.rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_').next()?.to_ascii_uppercase();
            if let Some(f) = joysheet_functions::all().iter().find(|f| f.name == name) {
                return Some(crate::formula_feedback::Hint { signature: f.signature.into(), description: f.description.into(), active: 0..0 });
            }
        }
        let prefix = text.rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_').next()?.to_ascii_uppercase();
        if prefix.is_empty() {
            return None;
        }
        let names: Vec<_> = joysheet_functions::all().iter().filter(|f| f.name.starts_with(&prefix)).take(6).map(|f| f.name).collect();
        if names.is_empty() {
            None
        } else {
            Some(crate::formula_feedback::Hint { signature: format!("{} · Tab", names.join("   ")), description: String::new(), active: 0..0 })
        }
    }
}
