//! Opt-in native I/O acceptance controls. Normal builds and normal launches
//! never pause jobs. The validation feature plus an explicit environment flag
//! exposes channel gates around the actual production open/save workers.
use crate::JoyWord;
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};
use joyword_shell::pause::StepGate;

#[derive(Default)]
pub(crate) struct IoValidation {
    #[cfg(feature = "validation")]
    enabled: bool,
    #[cfg(feature = "validation")]
    pending: Vec<(&'static str, joyword_shell::pause::StepController)>,
}

impl IoValidation {
    pub(crate) fn from_environment() -> Self {
        #[cfg(feature = "validation")]
        return Self {
            enabled: std::env::var("JOYWORD_VALIDATION_IO_GATE").is_ok_and(|value| value == "1"),
            pending: Vec::new(),
        };
        #[cfg(not(feature = "validation"))]
        Self::default()
    }

    fn gate(&mut self, _phase: &'static str) -> Option<StepGate> {
        #[cfg(feature = "validation")]
        if self.enabled && self.pending.len() < 8 {
            let (controller, gate) = StepGate::pair();
            self.pending.push((_phase, controller));
            eprintln!(
                "validation.io queued phase={_phase} pending={}",
                self.pending.len()
            );
            return Some(gate);
        }
        None
    }

    fn pending(&self) -> Option<(&'static str, usize)> {
        #[cfg(feature = "validation")]
        return self
            .pending
            .first()
            .map(|(phase, _)| (*phase, self.pending.len()));
        #[cfg(not(feature = "validation"))]
        None
    }

    fn release(&mut self, _newest: bool) {
        #[cfg(feature = "validation")]
        if !self.pending.is_empty() {
            let index = if _newest { self.pending.len() - 1 } else { 0 };
            // Drop sends on the channel; the real worker then continues.
            drop(self.pending.remove(index));
            eprintln!("validation.io released pending={}", self.pending.len());
        }
    }
}

impl JoyWord {
    pub(crate) fn validation_io_gate(
        &mut self,
        phase: &'static str,
        cx: &mut Context<Self>,
    ) -> Option<StepGate> {
        let gate = self.io_validation.gate(phase);
        if gate.is_some() {
            cx.notify();
        }
        gate
    }

    pub(crate) fn validation_io_panel(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let (phase, count) = self.io_validation.pending()?;
        let button = |newest, key| {
            let entity = cx.entity();
            crate::ui::controls::button(
                key,
                self.label(key),
                self.label(key),
                true,
                false,
                false,
                std::rc::Rc::new(move |window, cx| {
                    entity.update(cx, |this, cx| {
                        this.io_validation.release(newest);
                        window.focus(&this.focus_handle, cx);
                        cx.notify();
                    });
                }),
            )
        };
        Some(
            div()
                .id("validation-io-panel")
                .role(gpui::Role::Status)
                .flex()
                .items_center()
                .gap_2()
                .px_4()
                .h(px(40.0))
                .bg(rgb(crate::ui::theme::NOTICE))
                .text_sm()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(format!("{} ({count})", self.label(phase))),
                )
                .child(
                    div()
                        .w(px(120.0))
                        .child(button(false, "validation.io.oldest")),
                )
                .child(
                    div()
                        .w(px(120.0))
                        .child(button(true, "validation.io.newest")),
                )
                .into_any_element(),
        )
    }
}
