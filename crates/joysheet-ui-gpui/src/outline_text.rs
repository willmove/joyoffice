//! Rotated text uses font outlines; ordinary text stays in GPUI's glyph cache.
use gpui_kit::*;
use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{Arc, OnceLock},
};

static FONTS: OnceLock<Result<Arc<usvg::fontdb::Database>, String>> = OnceLock::new();
static STARTED: OnceLock<()> = OnceLock::new();
thread_local! {
    static CACHE: RefCell<HashMap<String, Arc<Vec<u8>>>> = RefCell::new(HashMap::new());
}
pub fn start() {
    STARTED.get_or_init(|| {
        if let Err(error) = std::thread::Builder::new().name("joysheet-outline-fonts".into()).spawn(|| {
            let mut fonts = usvg::fontdb::Database::new();
            fonts.load_system_fonts();
            fonts.set_sans_serif_family(if cfg!(target_os = "windows") { "Segoe UI" } else { "sans-serif" });
            let _ = FONTS.set(Ok(Arc::new(fonts)));
        }) {
            let _ = FONTS.set(Err(error.to_string()));
        }
    });
}
pub fn ready() -> bool {
    FONTS.get().is_some()
}
pub fn cached() -> usize {
    CACHE.with_borrow(|c| c.len())
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}
/// Bounding box of a rotated rectangle, relative to its rotation anchor.
pub fn rotated_box(x: f32, y: f32, w: f32, h: f32, angle: f32) -> [f32; 4] {
    let (s, c) = angle.sin_cos();
    let corners = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)].map(|(x, y)| (x * c - y * s, x * s + y * c));
    let left = corners.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let top = corners.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    let right = corners.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
    let bottom = corners.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
    [left, top, (right - left).max(1.), (bottom - top).max(1.)]
}
pub struct TextBlock<'a> {
    pub lines: &'a [String],
    pub font: &'a Font,
    pub size: f32,
    pub angle: f32,
    pub underline: bool,
    pub strike: bool,
    pub outline: f32,
}
impl TextBlock<'_> {
    pub fn dimensions(&self, window: &Window) -> (f32, f32) {
        let width = self
            .lines
            .iter()
            .map(|line| {
                f32::from(
                    window
                        .text_system()
                        .shape_line(
                            line.clone().into(),
                            px(self.size),
                            &[TextRun { len: line.len(), font: self.font.clone(), ..Default::default() }],
                            None,
                        )
                        .width(),
                )
            })
            .fold(1., f32::max);
        (width, self.size * 1.22 * self.lines.len().max(1) as f32)
    }
    pub fn paint(&self, origin: Point<Pixels>, dx: f32, dy: f32, ink: u32, window: &mut Window, cx: &mut App) -> bool {
        let Some(Ok(fonts)) = FONTS.get() else { return false };
        let (width, height) = self.dimensions(window);
        let [left, top, w, h] = rotated_box(dx, dy, width, height, self.angle);
        if w > 8192. || h > 8192. || !w.is_finite() || !h.is_finite() {
            return false;
        }
        let decoration = match (self.underline, self.strike) {
            (true, true) => "underline line-through",
            (true, false) => "underline",
            (false, true) => "line-through",
            _ => "none",
        };
        let mut content = String::new();
        for (i, line) in self.lines.iter().take(256).enumerate() {
            content.push_str(&format!("<text x=\"0\" y=\"{}\">{}</text>", self.size + i as f32 * self.size * 1.22, escape(line)));
        }
        let outline = self.outline;
        let fill = if outline > 0. { "none" } else { "black" };
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\"><g fill=\"{fill}\" stroke=\"black\" stroke-width=\"{outline}\" stroke-linejoin=\"round\" transform=\"translate({} {}) rotate({}) translate({dx} {dy})\" font-family=\"{},sans-serif\" font-size=\"{}\" font-weight=\"{}\" font-style=\"{}\" text-decoration=\"{decoration}\">{content}</g></svg>",
            w + 2.,
            h + 2.,
            1. - left,
            1. - top,
            self.angle.to_degrees(),
            escape(self.font.family.as_ref()),
            self.size,
            if self.font.weight == FontWeight::BOLD { "bold" } else { "normal" },
            if self.font.style == FontStyle::Italic { "italic" } else { "normal" }
        );
        let bytes = CACHE.with_borrow_mut(|cache| {
            if let Some(value) = cache.get(&svg) {
                return Some(value.clone());
            }
            let options = usvg::Options { fontdb: fonts.clone(), ..Default::default() };
            let tree = usvg::Tree::from_str(&svg, &options).ok()?;
            let outlines = Arc::new(tree.to_string(&usvg::WriteOptions::default()).into_bytes());
            if outlines.len() > 4 * 1024 * 1024 {
                return None;
            }
            if cache.len() >= 128 || cache.values().map(|v| v.len()).sum::<usize>() + outlines.len() > 16 * 1024 * 1024 {
                cache.clear();
            }
            cache.insert(svg.clone(), outlines.clone());
            Some(outlines)
        });
        let Some(bytes) = bytes else { return false };
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        svg.hash(&mut hash);
        let bounds = Bounds::new(origin + point(px(left - 1.), px(top - 1.)), size(px(w + 2.), px(h + 2.)));
        window
            .paint_svg(bounds, format!("joysheet-outline/{}", hash.finish()).into(), Some(&bytes), TransformationMatrix::unit(), rgb(ink).into(), cx)
            .is_ok()
    }
}
#[cfg(test)]
#[path = "tests/outline_text_tests.rs"]
mod tests;
