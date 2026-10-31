//! Shared diagnostics use the same GPUI input layout as the platform IME handler.
use crate::desktop::Desktop;
use gpui_kit::{component::input::InputState, *};
use serde_json::{Value, json};
impl Desktop {
    pub fn refresh_ime_geometry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Ordinary caret changes are handled by GPUI. Only an active composition
        // needs our extra invalidation after viewport or DPI geometry changes.
        if self.draft.is_none() || !self.composing(window, cx) {
            self.ime_geometry = None;
            return;
        }
        let stamp = (self.active_text_input(window, cx).read(cx).input_bounds(), window.scale_factor());
        if self.ime_geometry != Some(stamp) {
            self.ime_geometry = Some(stamp);
            window.invalidate_character_coordinates();
        }
    }
    pub fn active_text_input(&self, window: &Window, cx: &App) -> Entity<InputState> {
        if self.draft.as_ref().is_some_and(|d| !d.inline) || self.formula.read(cx).focus_handle(cx).is_focused(window) {
            self.formula.clone()
        } else {
            self.editor.clone()
        }
    }
    pub fn input_geometry(&self, window: &mut Window, cx: &mut Context<Self>) -> Value {
        let input = self.active_text_input(window, cx);
        let formula = input == self.formula;
        input.update(cx, |s, cx| {
            let selected = s.selected_text_range(false, window, cx).map(|r| r.range).unwrap_or(0..0);
            let marked = s.marked_text_range(window, cx);
            let bounds = s.input_bounds();
            let text = s.text_bounds().unwrap_or(bounds);
            let candidate = s.bounds_for_range(selected.clone(), text, window, cx);
            let rect = |r: Bounds<Pixels>| json!({"x":f32::from(r.origin.x),"y":f32::from(r.origin.y),"width":f32::from(r.size.width),"height":f32::from(r.size.height)});
            json!({"source":"GPUI EntityInputHandler UTF-16 layout","formula":formula,"scaleFactor":window.scale_factor(),"input":rect(bounds),"candidate":candidate.map(rect),"selectionUtf16":[selected.start,selected.end],"markedUtf16":marked.map(|r|vec![r.start,r.end])})
        })
    }
}
