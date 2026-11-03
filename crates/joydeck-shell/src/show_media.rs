//! Native decoder frames composited into the same slideshow raster. One selected media clip/page.
use crate::{playback::Player, show};
use joydeck_model::{ShapeKind, resolve};
use serde_json::{Value, json};

#[derive(Clone)]
pub struct Control {
    pub enabled: Option<bool>,
    pub muted: bool,
    pub seek: Option<(u64, f64)>,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            enabled: None,
            muted: true,
            seek: None,
        }
    }
}
#[derive(Default)]
pub struct Media {
    key: Option<(usize, joydeck_model::ShapeId)>,
    player: Option<Player>,
    paused: bool,
    muted: bool,
    seek_serial: u64,
}
impl Media {
    pub fn clear(&mut self) {
        self.player = None;
        self.key = None;
    }
    pub fn composite(
        &mut self,
        show: &show::Player,
        control: &Control,
        image: &mut joydeck_render::Image,
    ) -> Result<Value, String> {
        let slide = &show.model.slides[show.state.slide];
        let Some((shape, clip)) = slide.shapes.iter().find_map(|s| {
            if let ShapeKind::Media(c) = &s.kind {
                Some((s, c))
            } else {
                None
            }
        }) else {
            self.clear();
            return Ok(Value::Null);
        };
        if !control.enabled.unwrap_or(clip.autoplay) || show.state.ended {
            self.clear();
            return Ok(json!({"playing":false}));
        }
        let ctx = resolve::Ctx::for_slide(&show.model, slide).ok_or("media master missing")?;
        let rect = resolve::xfrm(&ctx, shape);
        if rect.rot.abs() > 0.001
            || rect.flip_h
            || rect.flip_v
            || clip.play_across_slides
            || clip.full_screen
        {
            return Err("rotated/flipped/fullscreen/across-slide media requires a later playback adapter; poster retained".into());
        }
        let key = (show.state.slide, shape.id);
        if self.key != Some(key) {
            self.clear();
            let item = show
                .model
                .media(clip.media)
                .ok_or("embedded media missing")?;
            if item.link.is_some() {
                return Err("linked media is not fetched automatically".into());
            }
            let extension = std::path::Path::new(&item.name)
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or(if clip.video { "mp4" } else { "mp3" })
                .to_ascii_lowercase();
            let mut player = Player::new(&item.data, &extension)?;
            if clip.trim_start_ms > 0 {
                player.control(
                    json!({"command":"seek","seconds":f64::from(clip.trim_start_ms)/1000.}),
                )?;
            }
            player.control(json!({"command":"volume","value":if control.muted{0.}else{clip.volume.clamp(0.,1.)}}))?;
            player.control(json!({"command":if show.paused{"pause"}else{"play"}}))?;
            self.player = Some(player);
            self.key = Some(key);
            self.paused = show.paused;
            self.muted = control.muted;
            self.seek_serial = 0;
        }
        let player = self.player.as_mut().ok_or("media player missing")?;
        if self.paused != show.paused {
            player.control(json!({"command":if show.paused{"pause"}else{"play"}}))?;
            self.paused = show.paused;
        }
        if self.muted != control.muted {
            player.control(json!({"command":"volume","value":if control.muted{0.}else{clip.volume.clamp(0.,1.)}}))?;
            self.muted = control.muted;
        }
        if let Some((serial, seconds)) = control.seek
            && serial != self.seek_serial
        {
            if !seconds.is_finite() || !(0.0..=3600.).contains(&seconds) {
                return Err("media seek exceeds range".into());
            }
            let seconds = seconds.min(
                player.info["duration"]
                    .as_f64()
                    .filter(|s| *s > 0.)
                    .unwrap_or(3600.),
            );
            player.control(json!({"command":"seek","seconds":seconds}))?;
            self.seek_serial = serial;
        }
        let mut status = player.control(json!({"command":"status"}))?;
        if (clip.trim_end_ms > 0
            && status["seconds"].as_f64().unwrap_or(0.) >= f64::from(clip.trim_end_ms) / 1000.)
            || (status["paused"] == true && !show.paused && clip.loop_play)
        {
            player.control(json!({"command":"pause"}))?;
            if clip.loop_play || clip.rewind {
                player.control(
                    json!({"command":"seek","seconds":f64::from(clip.trim_start_ms)/1000.}),
                )?;
            }
            if clip.loop_play {
                player.control(json!({"command":"play"}))?;
            }
            status = player.control(json!({"command":"status"}))?;
        }
        status["metadata"] = player.info.clone();
        if clip.video
            && show.blank.is_none()
            && let Some(frame) = player.frame()?
        {
            let scale = f64::from(image.width) / show.model.slide_size.width;
            let crop = clip.crop;
            if crop
                .iter()
                .any(|v| !v.is_finite() || !(0.0..1.).contains(v))
                || crop[0] + crop[2] >= 1.
                || crop[1] + crop[3] >= 1.
            {
                return Err("media crop invalid".into());
            }
            let x0 = (rect.x * scale).floor().max(0.) as u32;
            let y0 = (rect.y * scale).floor().max(0.) as u32;
            let x1 = ((rect.x + rect.w) * scale)
                .ceil()
                .clamp(0., f64::from(image.width)) as u32;
            let y1 = ((rect.y + rect.h) * scale)
                .ceil()
                .clamp(0., f64::from(image.height)) as u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let u = ((f64::from(x) / scale - rect.x) / rect.w).clamp(0., 1.);
                    let v = ((f64::from(y) / scale - rect.y) / rect.h).clamp(0., 1.);
                    let sx = ((crop[0] + u * (1. - crop[0] - crop[2])) * f64::from(frame.width))
                        .floor()
                        .clamp(0., f64::from(frame.width - 1))
                        as usize;
                    let sy = ((crop[1] + v * (1. - crop[1] - crop[3])) * f64::from(frame.height))
                        .floor()
                        .clamp(0., f64::from(frame.height - 1))
                        as usize;
                    let src = (sy * frame.width as usize + sx) * 4;
                    let dst = (y as usize * image.width as usize + x as usize) * 4;
                    let a = u32::from(frame.bgra[src + 3]);
                    for (d, s) in [(0, 2), (1, 1), (2, 0)] {
                        image.pixels[dst + d] = (u32::from(frame.bgra[src + s])
                            + u32::from(image.pixels[dst + d]) * (255 - a) / 255)
                            .min(255) as u8;
                    }
                    image.pixels[dst + 3] =
                        (a + u32::from(image.pixels[dst + 3]) * (255 - a) / 255).min(255) as u8;
                }
            }
            status["frame_seconds"] = json!(frame.seconds);
        }
        Ok(status)
    }
}
