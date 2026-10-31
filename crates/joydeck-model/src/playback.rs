//! Content addressed audiovisual assets plus an owned, kill-on-drop macOS adapter process.
use crate::document::{Deck, MAX_PART, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Info {
    pub profiles: Vec<String>,
    pub fps: f64,
    pub duration: f64,
    pub codecs: Vec<String>,
    pub width: u32,
    pub height: u32,
    pub adapter: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub extension: String,
    pub info: Info,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bytes: Vec<u8>,
}
impl Asset {
    pub fn id(&self) -> String {
        format!("{:x}", Sha256::digest(&self.bytes))
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub id: String,
    pub slide: String,
    pub asset: String,
    pub start: f64,
    pub end: f64,
    pub volume: f32,
    pub looping: bool,
    pub autoplay: bool,
    pub cross_slide: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Playback {
    pub assets: BTreeMap<String, Arc<Asset>>,
    pub clips: Vec<Clip>,
}
impl Playback {
    pub fn validate(&self, d: &Deck) -> Result<()> {
        if self.clips.len() > 64 || self.assets.len() > 64 {
            return Err("媒体数量超过64".into());
        }
        let mut total = 0;
        let mut ids = std::collections::BTreeSet::new();
        for (id, a) in &self.assets {
            total += a.bytes.len();
            if a.bytes.is_empty()
                || a.bytes.len() > MAX_PART
                || &a.id() != id
                || !matches!(a.extension.as_str(), "wav" | "mp3" | "mp4")
                || !a.info.duration.is_finite()
                || a.info.duration <= 0.
                || a.info.duration > 3600.
                || a.info.width > 1920
                || a.info.height > 1080
                || !a.info.fps.is_finite()
                || a.info.fps > 30.01
                || (a.info.width > 0
                    && !a
                        .info
                        .profiles
                        .iter()
                        .all(|s| s == "H264-Baseline-8bit" || s == "H264-Main-8bit"))
            {
                return Err("媒体资源、hash或预算无效".into());
            }
        }
        if total > 32 * 1024 * 1024 {
            return Err("媒体总量超过32MiB".into());
        }
        for c in &self.clips {
            let a = self.assets.get(&c.asset).ok_or("缺少媒体资源")?;
            if (c.cross_slide && a.info.width > 0)
                || c.id.is_empty()
                || c.id.len() > 128
                || !ids.insert(&c.id)
                || !d.slides.iter().any(|s| s.id == c.slide)
                || !c.start.is_finite()
                || !c.end.is_finite()
                || c.start < 0.
                || c.end <= c.start
                || c.end > a.info.duration + 0.01
                || !c.volume.is_finite()
                || !(0. ..=1.).contains(&c.volume)
            {
                return Err("媒体参数或锚点无效".into());
            }
        }
        Ok(())
    }
}
pub fn reconcile(d: &mut Deck) {
    d.playback
        .clips
        .retain(|c| d.slides.iter().any(|s| s.id == c.slide));
    d.playback
        .assets
        .retain(|id, _| d.playback.clips.iter().any(|c| &c.asset == id));
}
