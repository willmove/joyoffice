//! Deterministic static-page MP4 export with a bounded stream to a platform encoder.
use crate::document::{Deck, MAX_FILE, Result, check_cancel};
pub use joydeck_export::video::*;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::AtomicBool,
        mpsc::{self, SyncSender, TrySendError},
    },
    time::{Duration, Instant},
};
fn helper() -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Err("MP4编码适配器仅在macOS可用".into());
    }
    joydeck_platform::helpers::find("video")
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Encoder {
    child: Child,
    send: Option<SyncSender<Vec<u8>>>,
    writer: Option<std::thread::JoinHandle<std::io::Result<()>>>,
}
impl Drop for Encoder {
    fn drop(&mut self) {
        self.send.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }
}
pub fn export(
    path: &Path,
    d: &Deck,
    pages: &[usize],
    options: &Options,
    cancel: &AtomicBool,
) -> Result<Plan> {
    check_cancel(cancel)?;
    let plan = plan(d, pages, options)?;
    let helper = helper()?;
    let dir = crate::storage::private_directory("video")?;
    let scratch = Scratch(dir);
    let encoded = scratch.0.join("encoded.mp4");
    let mut child = Command::new(helper)
        .arg(&encoded)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("缺少编码输入")?;
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
    let header = [plan.width, plan.height, plan.fps, plan.frames]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let writer = std::thread::spawn(move || {
        stdin.write_all(&header)?;
        for bytes in rx {
            stdin.write_all(&bytes)?;
        }
        stdin.flush()
    });
    let mut encoder = Encoder {
        child,
        send: Some(tx),
        writer: Some(writer),
    };
    let start = Instant::now();
    let bounded = |cancel: &AtomicBool| -> Result<()> {
        check_cancel(cancel)?;
        if start.elapsed() > Duration::from_secs(600) {
            Err("MP4编码超过10分钟预算".into())
        } else {
            Ok(())
        }
    };
    for &page in pages {
        bounded(cancel)?;
        let frame = crate::preview::render(d, page, plan.width, cancel)?;
        let bytes = if frame.height == plan.height {
            frame.bgra
        } else {
            image::imageops::resize(
                &image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
                    .ok_or("无效帧")?,
                plan.width,
                plan.height,
                image::imageops::FilterType::Triangle,
            )
            .into_raw()
        };
        for _ in 0..options.fps * options.seconds_per_page {
            let mut pending = bytes.clone();
            loop {
                bounded(cancel)?;
                match encoder
                    .send
                    .as_ref()
                    .ok_or("MP4编码输入已关闭")?
                    .try_send(pending)
                {
                    Ok(()) => break,
                    Err(TrySendError::Full(bytes)) => {
                        pending = bytes;
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(TrySendError::Disconnected(_)) => return Err("MP4编码进程中断".into()),
                }
            }
        }
    }
    encoder.send.take();
    loop {
        bounded(cancel)?;
        if let Some(status) = encoder.child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err("系统MP4编码失败".into());
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    encoder
        .writer
        .take()
        .ok_or("MP4输入线程缺失")?
        .join()
        .map_err(|_| "MP4输入线程异常")?
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    std::fs::File::open(&encoded)
        .map_err(|e| e.to_string())?
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    crate::export::atomic(path, &bytes, None, cancel)?;
    Ok(plan)
}
