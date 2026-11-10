use super::*;
use gpui::{ClipboardEntry, ClipboardItem};
use serde_json::json;

const MAX_CLIPBOARD_BYTES: usize = 16 * 1024 * 1024;

fn copy_marker(document: u64) -> String {
    format!("joyword:{}:{document}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use std::sync::atomic::AtomicBool;

    #[gpui::test]
    fn keep_text_only_reads_the_current_clipboard(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update_in(cx, |v, w, cx| {
            let mut reader = joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
            reader
                .edit_command("document.setText", &json!({"text":"original copy"}))
                .unwrap();
            reader.edit_command("select.all", &json!({})).unwrap();
            let document = reader.engine().document_id();
            v.canvas.reader = Some(reader);
            v.install_input(document, cx);
            v.clipboard("edit.copy", w, cx);

            // A subsequent OS copy replaces the clipboard without replacing the internal fragment.
            cx.write_to_clipboard(ClipboardItem::new_string("external text".into()));
            v.clipboard("edit.pasteText", w, cx);
            let reader = v.canvas.reader.as_ref().unwrap();
            assert_eq!(
                reader.engine().doc.body[0].as_para().unwrap().plain_text(),
                "external text",
            );
        });
    }
}

impl JoyWord {
    pub(super) fn clipboard(&mut self, id: &str, w: &mut Window, cx: &mut Context<Self>) {
        if !self.format_ready(id) {
            return;
        }
        if matches!(id, "edit.copy" | "edit.cut") {
            let mut copied = None;
            self.apply_input(w, cx, |reader| {
                let result = reader.execute(id, &json!({}))?;
                if let Some(text) = result["text"].as_str()
                    && (!text.is_empty() || result["html"].is_string())
                {
                    copied = Some(ClipboardItem::new_string_with_metadata(
                        text.to_owned(),
                        copy_marker(reader.engine().document_id()),
                    ));
                }
                Ok(())
            });
            if let Some(item) = copied {
                cx.write_to_clipboard(item);
            }
            return;
        }
        #[cfg(target_os = "windows")]
        let sequence = clipboard_win::raw::seq_num();
        let item = cx.read_from_clipboard();
        let text = item
            .as_ref()
            .and_then(ClipboardItem::text)
            .filter(|text| text.len() <= MAX_CLIPBOARD_BYTES);
        if id == "edit.pasteText" {
            #[cfg(target_os = "windows")]
            if sequence != clipboard_win::raw::seq_num() {
                return;
            }
            if let Some(text) = text.filter(|text| !text.is_empty()) {
                self.engine_command(id, &json!({"text": text}), w, cx);
            }
            return;
        }
        if let Some(marker) = item.as_ref().and_then(ClipboardItem::metadata)
            && self.canvas.reader.as_ref().is_some_and(|reader| {
                reader.engine().clipboard.is_some()
                    && *marker == copy_marker(reader.engine().document_id())
            })
        {
            self.engine_command(id, &json!({}), w, cx);
            return;
        }

        let (html, rtf) = native_rich();
        #[cfg(target_os = "windows")]
        if sequence != clipboard_win::raw::seq_num() {
            return;
        }
        if text.is_some() || html.is_some() || rtf.is_some() {
            self.engine_command(
                id,
                &json!({"text": text, "html": html, "rtf": rtf, "external": true}),
                w,
                cx,
            );
        } else if id == "edit.paste"
            && let Some(image) = item.as_ref().and_then(|item| {
                item.entries().iter().find_map(|entry| match entry {
                    ClipboardEntry::Image(image) => Some(image),
                    _ => None,
                })
            })
        {
            self.apply_input(w, cx, |reader| reader.paste_clipboard_image(image.bytes()));
        }
    }
}

#[cfg(target_os = "windows")]
fn native_rich() -> (Option<String>, Option<String>) {
    use clipboard_win::{Clipboard, Getter, formats::Html, raw};

    let Ok(_board) = Clipboard::new_attempts(2) else {
        return (None, None);
    };
    let bounded_size = |format| {
        raw::size(format)
            .map(|size| size.get())
            .filter(|size| *size <= MAX_CLIPBOARD_BYTES)
    };
    let html = Html::new().and_then(|format| {
        let size = bounded_size(format.code())?;
        let mut data = Vec::with_capacity(size);
        format.read_clipboard(&mut data).ok()?;
        String::from_utf8(data).ok()
    });
    let rtf = clipboard_win::register_format("Rich Text Format").and_then(|format| {
        let mut data = vec![0; bounded_size(format.get())?];
        let read = raw::get(format.get(), &mut data).ok()?;
        data.truncate(read);
        let end = data
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(data.len());
        data.truncate(end);
        String::from_utf8(data).ok()
    });
    (html, rtf)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn native_rich() -> (Option<String>, Option<String>) {
    let html = arboard::Clipboard::new()
        .ok()
        .and_then(|mut board| board.get().html().ok())
        .filter(|html| html.len() <= MAX_CLIPBOARD_BYTES);
    (html, None)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn native_rich() -> (Option<String>, Option<String>) {
    (None, None)
}
