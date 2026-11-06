//! Read the standard external rich flavors beside GPUI's existing clipboard data.
#[cfg(not(target_os = "linux"))]
use joydeck_model::TextBody;
#[cfg(target_os = "linux")]
#[path = "native_clipboard_linux.rs"]
mod linux;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use joydeck_shell::clipboard_import::{self, MAX_INPUT};
#[cfg(target_os = "linux")]
pub(super) use linux::rich_text;

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn parse(html: Option<Vec<u8>>, rtf: Option<Vec<u8>>) -> Option<TextBody> {
    html.as_deref()
        .and_then(clipboard_import::import_html)
        .or_else(|| rtf.as_deref().and_then(clipboard_import::import_rtf))
}

#[cfg(target_os = "macos")]
pub(super) fn rich_text(_compositor: &str) -> Option<TextBody> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;
    let board = NSPasteboard::generalPasteboard();
    let stamp = board.changeCount();
    let read = |kind| {
        let data = board.dataForType(&NSString::from_str(kind))?;
        (data.len() <= MAX_INPUT).then(|| data.to_vec())
    };
    let html = read("public.html");
    let rtf = read("public.rtf");
    if board.changeCount() != stamp {
        return None;
    }
    parse(html, rtf)
}

#[cfg(target_os = "windows")]
pub(super) fn rich_text(_compositor: &str) -> Option<TextBody> {
    let (html, rtf) = {
        let _board = clipboard_win::Clipboard::new_attempts(2).ok()?;
        let read = |kind| {
            let format = clipboard_win::register_format(kind)?.get();
            let size = clipboard_win::size(format)?.get();
            if size > MAX_INPUT {
                return None;
            }
            let mut bytes = vec![0; size];
            let read = clipboard_win::raw::get(format, &mut bytes).ok()?;
            bytes.truncate(read);
            Some(bytes)
        };
        (read("HTML Format"), read("Rich Text Format"))
    };
    parse(html, rtf)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub(super) fn rich_text(_compositor: &str) -> Option<TextBody> {
    None
}
