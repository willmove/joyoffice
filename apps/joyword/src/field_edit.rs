//! Single-line text edited inside the window (Save As path, Open path, find
//! fields, format dialog fields).
//!
//! These fields are not the document. Document shortcuts are bound only while
//! `JoyWordField` is absent from the key-context stack, and a focused field
//! adds that context so Backspace, Delete, arrows, Home, End and the editing
//! chords edit the field.

use gpui::prelude::*;
use gpui::{App, Keystroke, div, px, rgb};

/// Identifier a focused in-app field adds to the key-context stack.
pub(crate) const FIELD_CONTEXT: &str = "JoyWordField";

/// What a key did to a [`FieldBuffer`]. Clipboard effects are applied by the
/// caller, which owns the platform clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldEffect {
    /// Caret moved or text changed.
    Handled,
    Submit,
    Cancel,
    Copy(String),
    Cut(String),
    Paste,
    /// Not a field key. The caller leaves it for other handlers.
    Ignored,
}

/// A string with a caret and an anchor, both byte offsets on char boundaries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FieldBuffer {
    text: String,
    caret: usize,
    anchor: usize,
}

impl FieldBuffer {
    pub(crate) fn from_text(text: impl Into<String>) -> Self {
        let mut buffer = Self::default();
        buffer.set_text(text);
        buffer
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Replace the text and put the caret at the end, with no selection.
    pub(crate) fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.caret = self.text.len();
        self.anchor = self.caret;
    }

    /// Insert `piece` in place of the selection.
    pub(crate) fn insert(&mut self, piece: &str) {
        let (start, end) = self.selection();
        self.text.replace_range(start..end, piece);
        self.caret = start + piece.len();
        self.anchor = self.caret;
    }

    pub(crate) fn apply_key(&mut self, stroke: &Keystroke) -> FieldEffect {
        let mods = &stroke.modifiers;
        if mods.alt || mods.platform || mods.function {
            return FieldEffect::Ignored;
        }
        let key = stroke.key.as_str();
        if mods.control {
            return self.apply_control(key, mods.shift);
        }
        let extend = mods.shift;
        match key {
            "enter" => FieldEffect::Submit,
            "escape" => FieldEffect::Cancel,
            "backspace" => {
                self.delete_backward(false);
                FieldEffect::Handled
            }
            "delete" => {
                self.delete_forward(false);
                FieldEffect::Handled
            }
            "left" => {
                self.move_caret(false, extend);
                FieldEffect::Handled
            }
            "right" => {
                self.move_caret(true, extend);
                FieldEffect::Handled
            }
            "home" | "up" | "pageup" => {
                self.place(0, extend);
                FieldEffect::Handled
            }
            "end" | "down" | "pagedown" => {
                self.place(self.text.len(), extend);
                FieldEffect::Handled
            }
            _ => {
                // A real key event carries `key_char` ("A" for shift-a). The
                // key name is the fallback for a plain letter or space, which
                // is also what `Keystroke::parse` produces in tests.
                let piece = stroke.key_char.as_deref().or(match key {
                    "space" => Some(" "),
                    other if other.chars().count() == 1 => Some(other),
                    _ => None,
                });
                let Some(piece) = piece else {
                    return FieldEffect::Ignored;
                };
                if piece.chars().any(char::is_control) {
                    return FieldEffect::Ignored;
                }
                self.insert(piece);
                FieldEffect::Handled
            }
        }
    }

    fn apply_control(&mut self, key: &str, extend: bool) -> FieldEffect {
        match key {
            "a" => {
                self.anchor = 0;
                self.caret = self.text.len();
                FieldEffect::Handled
            }
            "c" => {
                let text = self.selected_text();
                if text.is_empty() {
                    FieldEffect::Handled
                } else {
                    FieldEffect::Copy(text)
                }
            }
            "x" => {
                let text = self.selected_text();
                if text.is_empty() {
                    FieldEffect::Handled
                } else {
                    self.insert("");
                    FieldEffect::Cut(text)
                }
            }
            "v" => FieldEffect::Paste,
            "left" => {
                let next = word_edge(&self.text, self.caret, false);
                self.place(next, extend);
                FieldEffect::Handled
            }
            "right" => {
                let next = word_edge(&self.text, self.caret, true);
                self.place(next, extend);
                FieldEffect::Handled
            }
            "backspace" => {
                self.delete_backward(true);
                FieldEffect::Handled
            }
            "delete" => {
                self.delete_forward(true);
                FieldEffect::Handled
            }
            "home" | "up" => {
                self.place(0, extend);
                FieldEffect::Handled
            }
            "end" | "down" => {
                self.place(self.text.len(), extend);
                FieldEffect::Handled
            }
            _ => FieldEffect::Ignored,
        }
    }

    fn selection(&self) -> (usize, usize) {
        if self.caret <= self.anchor {
            (self.caret, self.anchor)
        } else {
            (self.anchor, self.caret)
        }
    }

    fn selected_text(&self) -> String {
        let (start, end) = self.selection();
        self.text[start..end].to_owned()
    }

    fn place(&mut self, next: usize, extend: bool) {
        self.caret = next;
        if !extend {
            self.anchor = next;
        }
    }

    fn move_caret(&mut self, forward: bool, extend: bool) {
        if !extend && self.caret != self.anchor {
            let (start, end) = self.selection();
            self.place(if forward { end } else { start }, false);
            return;
        }
        let next = if forward {
            self.text[self.caret..]
                .chars()
                .next()
                .map(|ch| self.caret + ch.len_utf8())
                .unwrap_or(self.caret)
        } else {
            self.text[..self.caret]
                .chars()
                .next_back()
                .map(|ch| self.caret - ch.len_utf8())
                .unwrap_or(0)
        };
        self.place(next, extend);
    }

    fn delete_backward(&mut self, word: bool) {
        if self.caret != self.anchor {
            self.insert("");
            return;
        }
        if self.caret == 0 {
            return;
        }
        let start = if word {
            word_edge(&self.text, self.caret, false)
        } else {
            self.text[..self.caret]
                .chars()
                .next_back()
                .map(|ch| self.caret - ch.len_utf8())
                .unwrap_or(0)
        };
        self.anchor = start;
        self.insert("");
    }

    fn delete_forward(&mut self, word: bool) {
        if self.caret != self.anchor {
            self.insert("");
            return;
        }
        if self.caret >= self.text.len() {
            return;
        }
        let end = if word {
            word_edge(&self.text, self.caret, true)
        } else {
            self.text[self.caret..]
                .chars()
                .next()
                .map(|ch| self.caret + ch.len_utf8())
                .unwrap_or(self.caret)
        };
        self.anchor = end;
        self.insert("");
    }
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || (ch > '\u{7f}' && !ch.is_whitespace())
}

/// The next word edge at or beside `caret`. A word is a run of letters and
/// digits (including non-ASCII), or a run of punctuation.
fn word_edge(text: &str, caret: usize, forward: bool) -> usize {
    if forward {
        let mut index = caret;
        let Some(first) = text[index..].chars().next() else {
            return index;
        };
        if first.is_whitespace() {
            for ch in text[index..].chars() {
                if !ch.is_whitespace() {
                    break;
                }
                index += ch.len_utf8();
            }
            return index;
        }
        let word = is_word(first);
        for ch in text[index..].chars() {
            if ch.is_whitespace() || is_word(ch) != word {
                break;
            }
            index += ch.len_utf8();
        }
        index
    } else {
        let mut index = caret;
        let Some(first) = text[..index].chars().next_back() else {
            return 0;
        };
        if first.is_whitespace() {
            while let Some(ch) = text[..index].chars().next_back() {
                if !ch.is_whitespace() {
                    break;
                }
                index -= ch.len_utf8();
            }
            return index;
        }
        let word = is_word(first);
        while let Some(ch) = text[..index].chars().next_back() {
            if ch.is_whitespace() || is_word(ch) != word {
                break;
            }
            index -= ch.len_utf8();
        }
        index
    }
}

/// The field's text, with the selection highlighted and a caret bar.
pub(crate) fn field_view(buffer: &FieldBuffer, placeholder: &str) -> gpui::Div {
    let caret = div().w(px(1.0)).h(px(16.0)).flex_none().bg(rgb(0x163b65));
    if buffer.text.is_empty() {
        return div().flex().items_center().w_full().child(caret).child(
            div()
                .text_color(rgb(0x8896a3))
                .child(placeholder.to_owned()),
        );
    }
    let text = buffer.text.as_str();
    let at = buffer.caret;
    let anchor = buffer.anchor;
    let row = div().flex().flex_wrap().items_center().w_full();
    if at == anchor {
        return row
            .child(text[..at].to_owned())
            .child(caret)
            .child(text[at..].to_owned());
    }
    let marked = |slice: &str| {
        div()
            .bg(rgb(0xb9d4f0))
            .text_color(rgb(0x263247))
            .child(slice.to_owned())
    };
    if at < anchor {
        row.child(text[..at].to_owned())
            .child(caret)
            .child(marked(&text[at..anchor]))
            .child(text[anchor..].to_owned())
    } else {
        row.child(text[..anchor].to_owned())
            .child(marked(&text[anchor..at]))
            .child(caret)
            .child(text[at..].to_owned())
    }
}

/// Plain text from the clipboard, with newlines removed. `None` when the
/// clipboard has no string.
pub(crate) fn clipboard_line(cx: &App) -> Option<String> {
    let item = cx.read_from_clipboard()?;
    for entry in item.entries() {
        if let gpui::ClipboardEntry::String(text) = entry {
            let line = text.text.replace(['\n', '\r'], "");
            return Some(line.trim().to_owned());
        }
    }
    None
}

/// Copy, cut or paste, and report whether the field took the key.
pub(crate) fn consume(
    effect: FieldEffect,
    buffer: &mut FieldBuffer,
    cx: &App,
) -> Option<FieldEffect> {
    if matches!(effect, FieldEffect::Ignored) {
        return None;
    }
    settle_clipboard(&effect, buffer, cx);
    Some(effect)
}

/// Apply a copy, cut or paste. Other effects are left for the caller.
fn settle_clipboard(effect: &FieldEffect, buffer: &mut FieldBuffer, cx: &App) {
    match effect {
        FieldEffect::Copy(text) | FieldEffect::Cut(text) => {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
        }
        FieldEffect::Paste => {
            if let Some(line) = clipboard_line(cx) {
                buffer.insert(&line);
            }
        }
        FieldEffect::Handled | FieldEffect::Submit | FieldEffect::Cancel | FieldEffect::Ignored => {
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(spec: &str) -> Keystroke {
        Keystroke::parse(spec).unwrap_or_else(|error| panic!("{spec}: {error}"))
    }

    fn apply(buffer: &mut FieldBuffer, spec: &str) -> FieldEffect {
        buffer.apply_key(&stroke(spec))
    }

    #[test]
    fn editing_and_navigation_keys_edit_the_field() {
        let mut buffer = FieldBuffer::from_text("copy-sample.docx");
        assert_eq!(apply(&mut buffer, "backspace"), FieldEffect::Handled);
        assert_eq!(buffer.text(), "copy-sample.doc");
        assert_eq!(apply(&mut buffer, "home"), FieldEffect::Handled);
        assert_eq!(apply(&mut buffer, "delete"), FieldEffect::Handled);
        assert_eq!(buffer.text(), "opy-sample.doc");
        assert_eq!(apply(&mut buffer, "shift-end"), FieldEffect::Handled);
        assert_eq!(apply(&mut buffer, "backspace"), FieldEffect::Handled);
        assert_eq!(buffer.text(), "");
        assert_eq!(apply(&mut buffer, "a"), FieldEffect::Handled);
        assert_eq!(apply(&mut buffer, "b"), FieldEffect::Handled);
        assert_eq!(buffer.text(), "ab");
        assert_eq!(apply(&mut buffer, "left"), FieldEffect::Handled);
        assert_eq!(apply(&mut buffer, "c"), FieldEffect::Handled);
        assert_eq!(buffer.text(), "acb");
        assert_eq!(apply(&mut buffer, "ctrl-a"), FieldEffect::Handled);
        assert_eq!(apply(&mut buffer, "ctrl-x"), FieldEffect::Cut("acb".into()));
        assert!(buffer.text().is_empty());

        buffer.set_text("甲乙");
        assert_eq!(apply(&mut buffer, "backspace"), FieldEffect::Handled);
        assert_eq!(buffer.text(), "甲");
        buffer.set_text("ab cd");
        assert_eq!(apply(&mut buffer, "home"), FieldEffect::Handled);
        assert_eq!(apply(&mut buffer, "ctrl-right"), FieldEffect::Handled);
        assert_eq!(&buffer.text()[..buffer.caret], "ab");
        assert_eq!(apply(&mut buffer, "ctrl-shift-right"), FieldEffect::Handled);
        assert_eq!(apply(&mut buffer, "backspace"), FieldEffect::Handled);
        assert_eq!(buffer.text(), "abcd");

        assert_eq!(apply(&mut buffer, "enter"), FieldEffect::Submit);
        assert_eq!(apply(&mut buffer, "escape"), FieldEffect::Cancel);
        assert_eq!(apply(&mut buffer, "ctrl-s"), FieldEffect::Ignored);
        assert_eq!(apply(&mut buffer, "ctrl-b"), FieldEffect::Ignored);
    }
}
