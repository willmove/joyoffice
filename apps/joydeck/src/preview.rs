//! Restricted SVG is generated from validated primitives, never from file markup.
use crate::document::*;
use resvg::{tiny_skia, usvg};
use std::sync::{Arc, OnceLock, atomic::AtomicBool};

pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            // LastResort advertises coverage of every Unicode scalar but draws
            // category boxes. It must never preempt genuine fallback fonts.
            let placeholders: Vec<_> = db
                .faces()
                .filter(|face| {
                    face.families.iter().any(|(name, _)| {
                        name.trim_start_matches('.')
                            .eq_ignore_ascii_case("LastResort")
                    })
                })
                .map(|face| face.id)
                .collect();
            for id in placeholders {
                db.remove_face(id);
            }
            db.set_sans_serif_family(if cfg!(target_os = "windows") {
                "Microsoft YaHei"
            } else if cfg!(target_os = "macos") {
                "PingFang SC"
            } else {
                "DejaVu Sans"
            });
            Arc::new(db)
        })
        .clone()
}
fn emoji_family<'a>(cluster: &str, db: &'a usvg::fontdb::Database) -> Option<&'a str> {
    // Select the font for the entire emoji grapheme before SVG shaping. usvg
    // fallback can abandon a mixed run when font substitution changes glyph count.
    if !cluster
        .chars()
        .any(|c| (0x1f000..=0x1ffff).contains(&(c as u32)) || c == '\u{20e3}' || c == '\u{fe0f}')
    {
        return None;
    }
    for family in [
        "Apple Color Emoji",
        "Segoe UI Emoji",
        "Noto Color Emoji",
        "Noto Emoji",
    ] {
        let Some(id) = db.query(&usvg::fontdb::Query {
            families: &[usvg::fontdb::Family::Name(family)],
            ..Default::default()
        }) else {
            continue;
        };
        let covers = db
            .with_face_data(id, |data, index| {
                ttf_parser::Face::parse(data, index)
                    .ok()
                    .is_some_and(|face| {
                        cluster.chars().all(|c| {
                            matches!(c, '\u{200d}' | '\u{fe0e}' | '\u{fe0f}')
                                || face.glyph_index(c).is_some()
                        })
                    })
            })
            .unwrap_or(false);
        if covers {
            return Some(family);
        }
    }
    None
}
fn span_markup(text: &str, style: &crate::text::Style) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    let db = fonts();
    let mut spans: Vec<(String, String)> = vec![];
    for cluster in text.graphemes(true) {
        let family = emoji_family(cluster, &db).unwrap_or(&style.family);
        if let Some((last_family, value)) = spans.last_mut()
            && last_family == family
        {
            value.push_str(cluster);
        } else {
            spans.push((family.to_owned(), cluster.to_owned()));
        }
    }
    let mut out = String::new();
    for (family, text) in spans {
        out.push_str(&format!(r#"<tspan font-family="{}" font-size="{}" font-weight="{}" font-style="{}" fill="{}">{}</tspan>"#, escape(&family), style.size, if style.bold { "bold" } else { "normal" }, if style.italic { "italic" } else { "normal" }, style.color, escape(&text)));
    }
    out
}
pub(crate) fn check_text_budget(slide: &Slide) -> Result<()> {
    let mut total = 0usize;
    for object in &slide.objects {
        let count = object.chart.as_ref().map_or(0, |c| {
            c.title.chars().count()
                + c.categories
                    .iter()
                    .map(|c| c.chars().count())
                    .sum::<usize>()
                + c.series
                    .iter()
                    .map(|s| s.name.chars().count())
                    .sum::<usize>()
        }) + object.diagram.as_ref().map_or(0, |d| {
            d.nodes
                .iter()
                .map(|n| n.text.chars().count())
                .sum::<usize>()
        }) + object.text.chars().count()
            + object.table.as_ref().map_or(0, |t| {
                t.cells
                    .iter()
                    .map(|c| c.text.chars().count())
                    .sum::<usize>()
            });
        total = total.saturating_add(count);
        if count > 50_000 || total > 100_000 {
            return Err("页面文字布局超出预算（单框50000字符/单页100000字符）；内容保留，可保存或缩短文本后重试".into());
        }
    }
    Ok(())
}
pub(crate) fn svg(deck: &Deck, page: usize, cancel: &AtomicBool) -> Result<String> {
    check_cancel(cancel)?;
    let slide = crate::design::resolve_slide(deck, page)?;
    check_text_budget(&slide)?;
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" viewBox="0 0 {} {}"><rect width="100%" height="100%" fill="{}"/>"##,
        deck.width, deck.height, deck.width, deck.height, slide.background
    );
    let mut expanded = vec![];
    for o in &slide.objects {
        if o.kind == "diagram" {
            expanded.extend(crate::diagram::expand(o)?);
        } else {
            expanded.push(o.clone());
        }
    }
    let objects = crate::table::expand(deck, page, &expanded)?;
    for (i, o) in objects.iter().enumerate() {
        check_cancel(cancel)?;
        if o.opacity == 0. {
            continue;
        }
        let (x, y, ow, oh, c) = (o.x, o.y, o.width, o.height, &o.fill);
        svg.push_str(&format!(
            r#"<g transform="rotate({} {} {})" opacity="{}" stroke="{}" stroke-width="{}">"#,
            o.rotation,
            x + ow / 2.,
            y + oh / 2.,
            o.opacity,
            o.stroke,
            o.stroke_width
        ));
        match o.kind.as_str() {
            "chart" => svg.push_str(&crate::chart::svg(deck, page, o)?),
            "image" => svg.push_str(&crate::media::svg(deck, o, i)?),
            "ellipse" => svg.push_str(&format!(
                r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" fill="{c}"/>"#,
                x + ow / 2.,
                y + oh / 2.,
                ow / 2.,
                oh / 2.
            )),
            "line" => svg.push_str(&format!(
                r#"<path d="M{x} {y} L{} {}" fill="none" stroke="{c}" stroke-width="{}"/>"#,
                x + ow,
                y + oh,
                o.stroke_width.max(1.)
            )),
            "rect" | "roundRect" => svg.push_str(&format!(
                r#"<rect x="{x}" y="{y}" width="{ow}" height="{oh}" rx="{}" fill="{c}"/>"#,
                if o.kind == "roundRect" { 12. } else { 0. }
            )),
            "text" | "placeholder" => {
                if o.kind == "text" && o.stroke_width > 0. {
                    svg.push_str(&format!(
                        r#"<rect x="{x}" y="{y}" width="{ow}" height="{oh}" fill="none"/>"#
                    ));
                }
                if o.kind == "placeholder" {
                    svg.push_str(&format!(r##"<rect x="{x}" y="{y}" width="{ow}" height="{oh}" fill="#fff0d5" stroke="{c}"/>"##));
                }
                svg.push_str(&format!(r#"<defs><clipPath id="c{i}"><rect x="{x}" y="{y}" width="{ow}" height="{oh}"/></clipPath></defs><g clip-path="url(#c{i})">"#));
                let runs = crate::text::runs(o);
                let mut offset = 0;
                for (line, text) in o.text.split('\n').enumerate() {
                    let baseline = y + o.font_size * (1. + line as f32 * 1.3);
                    let tx = x + if o.text_align == "center" {
                        ow / 2.
                    } else if o.text_align == "right" {
                        ow
                    } else {
                        0.
                    };
                    let anchor = if o.text_align == "center" {
                        "middle"
                    } else if o.text_align == "right" {
                        "end"
                    } else {
                        "start"
                    };
                    svg.push_str(&format!(r#"<text x="{tx}" y="{baseline}" text-anchor="{anchor}" stroke="none" font-family="sans-serif" font-size="{}" fill="{c}" xml:space="preserve">"#, o.font_size));
                    let end = offset + text.len();
                    for r in &runs {
                        let a = offset.max(r.start);
                        let b = end.min(r.end);
                        if a < b {
                            svg.push_str(&span_markup(&o.text[a..b], &r.style));
                        }
                    }
                    svg.push_str("</text>");
                    offset = end + 1;
                }
                svg.push_str("</g>");
            }
            "arrow" => svg.push_str(&format!(
                r#"<path d="M{x} {} H{} V{y} L{} {} L{} {} V{} H{x} z" fill="{c}"/>"#,
                y + oh * 0.25,
                x + ow * 0.65,
                x + ow,
                y + oh * 0.5,
                x + ow * 0.65,
                y + oh,
                y + oh * 0.75
            )),
            _ => return Err("未验证的绘制对象".into()),
        }
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    Ok(svg)
}
pub(crate) fn options() -> usvg::Options<'static> {
    usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    }
}
pub fn render(deck: &Deck, page: usize, width: u32, cancel: &AtomicBool) -> Result<Raster> {
    let svg = svg(deck, page, cancel)?;
    let scale = (width.clamp(1, 2048) as f32 / deck.width).min(2048. / deck.height);
    let w = (deck.width * scale).ceil().max(1.) as u32;
    let h = (deck.height * scale).ceil().max(1.) as u32;
    check_cancel(cancel)?;
    let options = options();
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|e| format!("预览排版失败：{e}"))?;
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("预览图像分配失败")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    check_cancel(cancel)?;
    // Whole slide has an opaque background, so premultiplied alpha is 255.
    let mut bytes = pixmap.take();
    for px in bytes.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    Ok(Raster {
        width: w,
        height: h,
        bgra: bytes,
    })
}

/// Resolve requested families against the same database used by CPU rendering.
/// Coverage and embedded-font permissions require further diagnostics.
pub fn diagnose_fonts(deck: &mut Deck) {
    let db = fonts();
    let mut missing = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (page, slide) in deck.slides.iter().enumerate() {
        for (index, object) in slide.objects.iter().enumerate() {
            let mut families: Vec<_> = object.runs.iter().map(|r| r.style.family.clone()).collect();
            if let Some(t) = &object.table {
                families.push(crate::table::theme_value(deck, page, &t.style.family, true));
                for cell in &t.cells {
                    if let Some(f) = &cell.style.family {
                        families.push(crate::table::theme_value(deck, page, f, true));
                    }
                    for run in &cell.runs {
                        families.push(run.style.family.clone());
                    }
                }
            }
            for family in &families {
                if matches!(family.as_str(), "sans-serif" | "serif" | "monospace")
                    || !seen.insert((page, family.clone()))
                {
                    continue;
                }
                let query = usvg::fontdb::Query {
                    families: &[usvg::fontdb::Family::Name(family)],
                    ..Default::default()
                };
                if db.query(&query).is_none() {
                    missing.push((
                        page + 1,
                        format!(
                            "document.json#slide/{}/object/{}",
                            slide.id,
                            if object.id.is_empty() {
                                index.to_string()
                            } else {
                                object.id.clone()
                            }
                        ),
                        format!("未安装字体 {family}；显示使用系统替代，原字体名称保留"),
                    ));
                }
            }
        }
    }
    for (page, part, message) in missing {
        deck.warn("missing-font", Some(page), &part, &message);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod font_tests {
    use super::*;
    #[test]
    fn mixed_bold_cjk_emoji_and_accents_use_real_glyphs() {
        let options = usvg::Options {
            fontdb: fonts(),
            ..Default::default()
        };
        let mut o = crate::edit::object("text");
        o.text = "中文😀 e\u{301}".into();
        let mut style = crate::text::Style::for_object(&o);
        style.bold = true;
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="400" height="100"><text x="0" y="40" font-size="24">{}</text></svg>"#,
            span_markup(&o.text, &style)
        );
        let tree = usvg::Tree::from_str(&svg, &options).unwrap();
        fn collect(group: &usvg::Group, tree: &usvg::Tree, text: &mut String) {
            for node in group.children() {
                match node {
                    usvg::Node::Group(g) => collect(g, tree, text),
                    usvg::Node::Text(t) => {
                        for span in t.layouted() {
                            for glyph in &span.positioned_glyphs {
                                assert_ne!(glyph.id.0, 0, "missing glyph {:?}", glyph.text);
                                let face = tree.fontdb().face(glyph.font).unwrap();
                                assert!(
                                    face.families
                                        .iter()
                                        .all(|(name, _)| !name.contains("LastResort")),
                                    "placeholder font used for {:?}",
                                    glyph.text
                                );
                                text.push_str(&glyph.text);
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
        let mut text = String::new();
        collect(tree.root(), &tree, &mut text);
        for part in ["中", "文", "😀", "e\u{301}"] {
            assert!(text.contains(part), "lost {part:?}: {text:?}");
        }
    }
}
