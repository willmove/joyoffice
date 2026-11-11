//! Adapt JoyOffice's AccessKit TextRun projection to this Reader's UTF-16 context.
//! The native input bridge still owns all document changes.
use super::*;
use gpui::{
    AccessibleAction,
    accesskit::{ActionData, Node, NodeId, Role, TextPosition, TextSelection},
};
use std::{cell::Cell, rc::Rc};

fn scalar_index(text: &str, offset: usize) -> Option<usize> {
    let mut utf16 = 0;
    for (index, ch) in text.chars().enumerate() {
        if utf16 == offset {
            return Some(index);
        }
        utf16 += ch.len_utf16();
    }
    (utf16 == offset).then(|| text.chars().count())
}

fn utf16_offset(text: &str, character: usize) -> Option<usize> {
    let mut chars = text.chars();
    let mut offset = 0;
    for _ in 0..character {
        offset += chars.next()?.len_utf16();
    }
    Some(offset)
}

impl JoyWord {
    pub(super) fn accessible_text(
        &self,
        viewport: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let reader = self.canvas.reader.as_ref();
        let read_only = reader.is_some_and(|r| r.engine().view.read_mode);
        let projection = self
            .canvas
            .a11y_active
            .then(|| reader?.input_context())
            .flatten();
        let expected = reader.map(|r| {
            (
                r.engine().document_id(),
                r.presentation_revision(),
                r.engine().sel.clone(),
            )
        });
        let node_id = Rc::new(Cell::new(None::<NodeId>));
        let published_id = node_id.clone();
        let text = projection.as_ref().map(|p| p.text.clone());
        let focus_owner = cx.weak_entity();
        let scroll_owner = focus_owner.clone();
        let select_owner = focus_owner.clone();
        viewport
            .role(Role::MultilineTextInput)
            .aria_label(self.shell.tr("文档正文", "Document text"))
            .aria_description(self.shell.tr(
                "随光标读取段落。用方向键或翻页键浏览。",
                "Reads the current paragraph. Navigate with the arrow or page keys.",
            ))
            .track_focus(&self.focus)
            .tab_stop(true)
            .a11y_synthetic_children(move |builder| {
                if read_only {
                    builder.parent_node().set_read_only();
                }
                let Some(context) = projection else { return };
                let id = builder.synthetic_node_id("current-paragraph");
                let mut run = Node::new(Role::TextRun);
                run.set_value(context.text.as_str());
                run.set_character_lengths(
                    context
                        .text
                        .chars()
                        .map(|ch| ch.len_utf8() as u8)
                        .collect::<Vec<_>>(),
                );
                builder.parent_node().set_value(context.text.as_str());
                let position = |offset| {
                    scalar_index(&context.text, offset).map(|character_index| TextPosition {
                        node: id,
                        character_index,
                    })
                };
                let (start, end) = (
                    position(context.selection.start),
                    position(context.selection.end),
                );
                if let (Some(start), Some(end)) = (start, end) {
                    let (anchor, focus) = if context.reversed {
                        (end, start)
                    } else {
                        (start, end)
                    };
                    builder
                        .parent_node()
                        .set_text_selection(TextSelection { anchor, focus });
                }
                if builder.push_child(id, run) {
                    published_id.set(Some(id));
                }
            })
            .on_a11y_action(AccessibleAction::Focus, move |_, w, cx| {
                let _ = focus_owner.update(cx, |v, cx| w.focus(&v.focus, cx));
            })
            .on_a11y_action(AccessibleAction::ScrollIntoView, move |_, _, cx| {
                let _ = scroll_owner.update(cx, |v, cx| {
                    v.reveal_input = true;
                    cx.notify();
                });
            })
            .on_a11y_action(AccessibleAction::SetTextSelection, move |data, w, cx| {
                let _ = select_owner.update(cx, |v, cx| {
                    let Some(ActionData::SetTextSelection(selection)) = data else {
                        return;
                    };
                    let Some(text) = &text else { return };
                    let Some(id) = node_id.get() else { return };
                    if selection.anchor.node != id || selection.focus.node != id {
                        return;
                    }
                    let (Some(anchor), Some(focus)) = (
                        utf16_offset(text, selection.anchor.character_index),
                        utf16_offset(text, selection.focus.character_index),
                    ) else {
                        return;
                    };
                    let Some(r) = v.canvas.reader.as_ref() else {
                        return;
                    };
                    if r.is_composing()
                        || expected.as_ref()
                            != Some(&(
                                r.engine().document_id(),
                                r.presentation_revision(),
                                r.engine().sel.clone(),
                            ))
                    {
                        return;
                    }
                    v.apply_input(w, cx, |r| {
                        let (a, b) = r.native_positions(anchor.min(focus)..anchor.max(focus))?;
                        if anchor <= focus {
                            r.select_positions(a, b)
                        } else {
                            r.select_positions(b, a)
                        }
                    });
                    w.focus(&v.focus, cx);
                });
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_run_uses_scalar_indices_and_exact_utf16_boundaries() {
        let text = "甲😀e\u{301}";
        assert_eq!(scalar_index(text, 3), Some(2));
        assert_eq!(scalar_index(text, 2), None);
        assert_eq!(utf16_offset(text, 4), Some(5));
        assert_eq!(utf16_offset(text, 5), None);
    }
}
