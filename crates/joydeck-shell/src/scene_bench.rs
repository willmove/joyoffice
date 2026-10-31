//! Fixed independent synthetic data, never external slide content.
use crate::document::{Deck, Result};
use joydeck_scene::scene::*;
use resvg::tiny_skia as sk;
use std::sync::{Arc, atomic::AtomicBool};
pub const SEED: u64 = 0x4a6f794465636b04;
pub const FRAMES: usize = 48;
pub fn deck() -> Deck {
    let mut deck = crate::edit::Session::new().deck.as_ref().clone();
    deck.width = 1600.;
    deck.height = 1200.;
    let mut objects = vec![];
    for i in 0..120 {
        let mut o = crate::edit::object(["rect", "ellipse", "roundRect", "arrow", "line"][i % 5]);
        o.id = format!("bench-geometry-{i}");
        o.x = (i % 12) as f32 * 130. + 8.;
        o.y = (i / 12) as f32 * 110. + 8.;
        o.width = 105.;
        o.height = 48.;
        o.stroke_width = 2.;
        o.fill = format!("#{:06x}", 0x108080 + (i * 157) % 0x808080);
        objects.push(o);
    }
    for i in 0..60 {
        let mut o = crate::edit::object("text");
        o.id = format!("bench-text-{i}");
        o.x = (i % 10) as f32 * 158. + 8.;
        o.y = (i / 10) as f32 * 185. + 55.;
        o.width = 150.;
        o.height = 94.;
        o.font_size = 9.;
        // 5 lines per block, approximately 8,000 shaped glyphs across all 60 blocks.
        let line = "JoyDeck中文图文编辑基准测试abcdef0123";
        o.text = std::iter::repeat_n(line, 5).collect::<Vec<_>>().join("\n");
        o.fill = "#16354f".into();
        objects.push(o);
    }
    deck.slides[0].objects = objects;
    deck
}
pub fn edit(cache: &mut SceneCache) -> Result<Scene> {
    let d = deck();
    let mut scene = cache.build(&d, 0, SEED, 0, &AtomicBool::new(false))?;
    let mut pix = sk::Pixmap::new(512, 512).ok_or("benchmark image")?;
    for (i, p) in pix.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let x = i % 512;
        let y = i / 512;
        *p = [
            (x / 2) as u8,
            (y / 2) as u8,
            if (x / 32 + y / 32) % 2 == 0 { 220 } else { 100 },
            255,
        ];
    }
    let base = pix.take();
    for i in 0..20 {
        let mut rgba = base.clone();
        for p in rgba.as_chunks_mut::<4>().0 {
            p[0] = p[0].wrapping_add(i * 7);
            p[2] = p[2].wrapping_add(i * 11);
        }
        let bitmap = Bitmap {
            width: 512,
            height: 512,
            rgba: Arc::new(rgba),
        };
        scene.nodes.push(Node::image(
            format!("bench-image-{i}"),
            ((i % 10) as f32 * 158. + 80., (i / 10) as f32 * 550. + 180.),
            (75., 75.),
            bitmap,
        )?);
    }
    Ok(scene)
}
pub fn effect() -> Result<Scene> {
    let mut nodes = vec![];
    for i in 0..30 {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="180" height="120"><defs><linearGradient id="g"><stop stop-color="#2090e0"/><stop offset="1" stop-color="#f04060"/></linearGradient><filter id="s"><feGaussianBlur stdDeviation="3"/></filter><clipPath id="c"><rect x="10" y="10" width="145" height="95" rx="18"/></clipPath></defs><g opacity="0.55" clip-path="url(#c)"><rect x="20" y="20" width="130" height="80" fill="url(#g)" filter="url(#s)"/><circle cx="110" cy="65" r="38" fill="#40b060"/></g></svg>"##;
        let g = geometry_from_svg(svg, 180., 120., (0., 0.), false, &AtomicBool::new(false))?;
        nodes.push(Node {
            id: format!("effect-{i}"),
            position: ((i % 6) as f32 * 160., (i / 6) as f32 * 100.),
            geometry: Arc::new(g),
        });
    }
    Ok(Scene {
        document: SEED + 1,
        revision: 0,
        slide: "effect".into(),
        width: 1000.,
        height: 620.,
        background: [255, 255, 255, 255],
        nodes,
    })
}
pub fn position(frame: usize) -> (f32, f32) {
    let t = frame as f32 / (FRAMES - 1) as f32;
    (180. * t, 40. * (t * std::f32::consts::TAU).sin())
}
