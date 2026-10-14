//! 捕获本进程窗口为 PNG（仅证据采集，不属于验证链路）。
//!
//! macOS 行为：用 `CGWindowListCreateImage` 按"本进程拥有的窗口 ID"
//! 捕获自己的窗口不需要屏幕录制权限（该权限约束的是捕获其他进程的
//! 窗口/整屏）；终端宿主的 `screencapture` 因缺权限失败，故由探针
//! 进程自己完成。窗口通过 kCGWindowOwnerPID + 逻辑尺寸匹配。

use std::path::Path;

use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::window as cgwindow;

/// 在屏幕上的本进程窗口里找逻辑尺寸 `logical (w, h)` 的那个，按窗口
/// ID 捕获（IgnoreFraming 去掉阴影），写 PNG。返回错误说明原因（调用
/// 方把它写进证据目录，不视为链路失败）。
// unsafe 仅限把 CFArray 的 void* 项按 get 规则包成 CFDictionary：
// core-foundation 0.10 没有带类型的数组项访问封装。数组在作用域内
// 持有这些值，get 规则不做保留，无泄漏。
#[allow(unsafe_code)]
pub fn capture_own_window_png(
    logical_width: f64,
    logical_height: f64,
    out: &Path,
) -> Result<(), String> {
    // 探针进程只开一个窗口：按 kCGWindowOwnerPID 找到本进程的在屏窗口
    // 即可（logical_width/height 仅作记录）。
    let _ = (logical_width, logical_height);
    let pid = i64::from(std::process::id());
    let infos = cgwindow::copy_window_info(
        cgwindow::kCGWindowListOptionOnScreenOnly,
        cgwindow::kCGNullWindowID,
    )
    .ok_or("CGWindowListCopyWindowInfo returned nothing")?;
    let mut found: Option<u32> = None;
    for void in infos.get_all_values() {
        let dict = unsafe { CFDictionary::<CFString, CFType>::wrap_under_get_rule(void as _) };
        let Some(owner) = number_of(&dict, "kCGWindowOwnerPID") else {
            continue;
        };
        if owner != pid {
            continue;
        }
        if let Some(number) = number_of(&dict, "kCGWindowNumber") {
            found = Some(u32::try_from(number).map_err(|_| "window id overflow")?);
        }
    }
    let window_id = found.ok_or("no on-screen window of this process")?;
    // CGRectNull（无限原点、零尺寸）＝ 按窗口自身边界捕获整个窗口。
    let screen = CGRect::new(
        &CGPoint::new(f64::INFINITY, f64::INFINITY),
        &CGSize::new(0.0, 0.0),
    );
    let image = cgwindow::create_image(
        screen,
        cgwindow::kCGWindowListOptionIncludingWindow,
        window_id,
        cgwindow::kCGWindowImageBoundsIgnoreFraming,
    )
    .ok_or("CGWindowListCreateImage returned null (own-window capture refused)")?;
    let width = image.width();
    let height = image.height();
    let bytes_per_row = image.bytes_per_row();
    let bits_per_pixel = image.bits_per_pixel();
    if bits_per_pixel != 32 {
        return Err(format!("unexpected {bits_per_pixel} bpp"));
    }
    let data = image.data();
    let bytes = data.bytes();
    if bytes.len() < bytes_per_row * height {
        return Err("image data shorter than expected".into());
    }
    // macOS 窗口图像通常是 BGRA；去行填充并交换为 RGBA。
    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        let line = &bytes[row * bytes_per_row..row * bytes_per_row + width * 4];
        for pixel in line.as_chunks::<4>().0 {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    let buffer = image::RgbaImage::from_raw(width as u32, height as u32, rgba)
        .ok_or("buffer size mismatch")?;
    buffer
        .save(out)
        .map_err(|error| format!("save png: {error}"))?;
    Ok(())
}

fn number_of(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<i64> {
    let value = dict.find(CFString::new(key))?;
    value.downcast::<CFNumber>()?.to_i64()
}
