//! Arrange existing, process-owned native windows through safe Win32 wrappers.
use serde::Serialize;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
#[derive(Serialize)]
pub struct WindowInfo {
    pub dpi: u32,
    pub client: Rect,
    pub handle: u64,
    pub bounds: Rect,
    pub work_area: Rect,
}
pub fn layout(area: Rect, count: usize, mode: &str) -> Result<Vec<Rect>, String> {
    if count == 0 || count > 128 {
        return Err("Arrange between 1 and 128 windows".into());
    }
    let (rows, cols) = match mode {
        "horizontal" => (count, 1),
        "vertical" => (1, count),
        "tiled" => {
            let cols = (count as f64).sqrt().ceil() as usize;
            (count.div_ceil(cols), cols)
        }
        "cascade" => (1, 1),
        _ => return Err("Arrange must be tiled, horizontal, vertical or cascade".into()),
    };
    if area.width < 320 * cols as i32 || area.height < 180 * rows as i32 {
        return Err("Too many windows for this display; arrange only the active workbook or use cascade".into());
    }
    let mut result = Vec::new();
    for i in 0..count {
        if mode == "cascade" {
            let step = 28;
            let depth = ((area.width - 640).min(area.height - 400) / step).max(1) as usize;
            let offset = (i % depth) as i32 * step;
            result.push(Rect {
                x: area.x + offset,
                y: area.y + offset,
                width: (area.width - ((count.min(depth) - 1) as i32 * step)).max(320),
                height: (area.height - ((count.min(depth) - 1) as i32 * step)).max(180),
            });
        } else {
            let row = i / cols;
            let col = i % cols;
            let row_cols = cols.min(count - row * cols);
            let x = area.width * col as i32 / row_cols as i32;
            let right = area.width * (col + 1) as i32 / row_cols as i32;
            let y = area.height * row as i32 / rows as i32;
            let bottom = area.height * (row + 1) as i32 / rows as i32;
            result.push(Rect { x: area.x + x, y: area.y + y, width: right - x, height: bottom - y });
        }
    }
    Ok(result)
}
#[cfg(target_os = "windows")]
fn owned(ids: &[u64]) -> Result<Vec<winsafe::HWND>, String> {
    use winsafe::prelude::*;
    let mut result = Vec::new();
    winsafe::EnumWindows(|window| {
        if ids.contains(&(window.ptr() as usize as u64)) && window.GetWindowThreadProcessId().1 == std::process::id() {
            result.push(window);
        }
        true
    })
    .map_err(|e| e.to_string())?;
    if result.len() != ids.len() {
        return Err("A target window closed or does not belong to this process".into());
    }
    result.sort_by_key(|w| ids.iter().position(|id| *id == w.ptr() as usize as u64));
    Ok(result)
}
#[cfg(target_os = "windows")]
fn info(window: &winsafe::HWND) -> Result<WindowInfo, String> {
    use winsafe::{co, prelude::*};
    let r = window.GetWindowRect().map_err(|e| e.to_string())?;
    let mut monitor = winsafe::MONITORINFOEX::default();
    window.MonitorFromWindow(co::MONITOR::DEFAULTTONEAREST).GetMonitorInfo(&mut monitor).map_err(|e| e.to_string())?;
    let a = monitor.rcWork;
    let client = window.GetClientRect().map_err(|e| e.to_string())?;
    let mut origin = winsafe::POINT::new(0, 0);
    window.ClientToScreen(&mut origin).map_err(|e| e.to_string())?;
    Ok(WindowInfo {
        dpi: window.GetDpiForWindow(),
        client: Rect { x: origin.x, y: origin.y, width: client.right - client.left, height: client.bottom - client.top },
        handle: window.ptr() as usize as u64,
        bounds: Rect { x: r.left, y: r.top, width: r.right - r.left, height: r.bottom - r.top },
        work_area: Rect { x: a.left, y: a.top, width: a.right - a.left, height: a.bottom - a.top },
    })
}
#[cfg(target_os = "windows")]
pub fn inspect(ids: &[u64]) -> Result<Vec<WindowInfo>, String> {
    owned(ids)?.iter().map(info).collect()
}
#[cfg(target_os = "windows")]
pub fn arrange(ids: &[u64], mode: &str) -> Result<Vec<WindowInfo>, String> {
    use winsafe::{co, prelude::*};
    let windows = owned(ids)?;
    let area = info(windows.first().ok_or("No windows to arrange")?)?.work_area;
    let rectangles = layout(area, windows.len(), mode)?;
    for (window, r) in windows.iter().zip(rectangles) {
        if window.IsZoomed() || window.IsIconic() {
            window.ShowWindow(co::SW::RESTORE);
        }
        window
            .SetWindowPos(
                winsafe::HwndPlace::None,
                winsafe::POINT::new(r.x, r.y),
                winsafe::SIZE::new(r.width, r.height),
                co::SWP::NOZORDER | co::SWP::NOACTIVATE,
            )
            .map_err(|e| e.to_string())?;
    }
    windows.iter().map(info).collect()
}
#[cfg(not(target_os = "windows"))]
pub fn inspect(_: &[u64]) -> Result<Vec<WindowInfo>, String> {
    Err("Native arrangement currently supports Windows".into())
}
#[cfg(not(target_os = "windows"))]
pub fn arrange(_: &[u64], _: &str) -> Result<Vec<WindowInfo>, String> {
    Err("Native arrangement currently supports Windows".into())
}
#[cfg(test)]
#[path = "tests/windows_tests.rs"]
mod tests;
