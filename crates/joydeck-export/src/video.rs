use joydeck_model::document::{Deck, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub width: u32,
    pub fps: u32,
    pub seconds_per_page: u32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            width: 960,
            fps: 15,
            seconds_per_page: 2,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub fps: u32,
    pub duration_seconds: u32,
    pub scope: &'static str,
}
pub fn plan(d: &Deck, pages: &[usize], options: &Options) -> Result<Plan> {
    d.validate()?;
    let height =
        ((options.width as f64 * d.height as f64 / d.width as f64 / 2.).round() * 2.) as u32;
    let frames = pages.len() as u64 * options.fps as u64 * options.seconds_per_page as u64;
    if !(320..=1920).contains(&options.width)
        || !options.width.is_multiple_of(2)
        || !(2..=1080).contains(&height)
        || !(1..=30).contains(&options.fps)
        || !(1..=30).contains(&options.seconds_per_page)
        || pages.is_empty()
        || pages.iter().any(|&p| p >= d.slides.len())
        || frames > 1800
        || frames == 0
    {
        return Err(
            "MP4参数超出预算：偶数宽320–1920、高≤1080、1–30fps、每页1–30秒、总计≤1800帧".into(),
        );
    }
    Ok(Plan {
        width: options.width,
        height,
        frames: frames as u32,
        fps: options.fps,
        duration_seconds: pages.len() as u32 * options.seconds_per_page,
        scope: "static pages; silent H.264 Main MP4; no transitions, animation, embedded media or recording",
    })
}
/// A sidecar of page-title cues using exactly the static video page durations. Speaker notes stay private.
pub fn captions(d: &Deck, pages: &[usize], options: &Options) -> Result<String> {
    plan(d, pages, options)?;
    let stamp = |seconds: u32| {
        format!(
            "{:02}:{:02}:{:02}.000",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    };
    let mut text = String::from("WEBVTT\n\n");
    for (i, &page) in pages.iter().enumerate() {
        let title = d.slides[page]
            .title
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        text.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            stamp(i as u32 * options.seconds_per_page),
            stamp((i + 1) as u32 * options.seconds_per_page),
            title
        ));
    }
    Ok(text)
}
