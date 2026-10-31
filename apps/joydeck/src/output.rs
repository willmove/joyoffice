//! Unified scene exports. This module has no UI or shell command strings.
use crate::document::{Deck, Result, check_cancel};
use std::{path::Path, sync::atomic::AtomicBool};
pub fn vector_pdf(path: &Path, d: &Deck, pages: &[usize], cancel: &AtomicBool) -> Result<()> {
    use pdf_writer::{
        Content, Name, Pdf, Rect, Ref, TextStr,
        types::{ActionType, AnnotationType},
    };
    d.validate()?;
    if pages.is_empty() || pages.len() > 500 {
        return Err("输出页数无效".into());
    }
    let mut pdf = Pdf::new();
    let catalog = Ref::new(1);
    let tree = Ref::new(2);
    let ids: Vec<_> = (0..pages.len()).map(|i| Ref::new(3 + i as i32)).collect();
    let mut next = 3 + pages.len() as i32;
    for (index, &p) in pages.iter().enumerate() {
        check_cancel(cancel)?;
        let svg = crate::preview::svg(d, p, cancel)?;
        let tree_svg = svg2pdf::usvg::Tree::from_str(&svg, &crate::preview::options())
            .map_err(|e| e.to_string())?;
        let (chunk, reference) =
            svg2pdf::to_chunk(&tree_svg, svg2pdf::ConversionOptions::default())
                .map_err(|e| e.to_string())?;
        let mut map = std::collections::HashMap::new();
        let renumbered = chunk.renumber(|old| {
            *map.entry(old).or_insert_with(|| {
                let id = Ref::new(next);
                next += 1;
                id
            })
        });
        let form = map[&reference];
        pdf.extend(&renumbered);
        let page = ids[index];
        let contents = Ref::new(next);
        next += 1;
        let mut content = Content::new();
        content
            .save_state()
            .transform([d.width, 0., 0., d.height, 0., 0.])
            .x_object(Name(b"Scene"))
            .restore_state();
        pdf.stream(contents, &content.finish());
        let mut annotations = vec![];
        for o in &d.slides[p].objects {
            if let Some(target) = d
                .presentation
                .links
                .get(&o.id)
                .and_then(|id| pages.iter().position(|p| &d.slides[*p].id == id))
            {
                let id = Ref::new(next);
                next += 1;
                annotations.push(id);
                let r = crate::geometry::union(std::slice::from_ref(o));
                pdf.annotation(id)
                    .subtype(AnnotationType::Link)
                    .rect(Rect::new(
                        r.x,
                        d.height - r.y - r.h,
                        r.x + r.w,
                        d.height - r.y,
                    ))
                    .page(page)
                    .border(0., 0., 0., None)
                    .action()
                    .action_type(ActionType::GoTo)
                    .destination()
                    .page(ids[target])
                    .fit();
            }
        }
        let mut page_writer = pdf.page(page);
        if !annotations.is_empty() {
            page_writer.annotations(annotations);
        }
        page_writer
            .parent(tree)
            .media_box(Rect::new(0., 0., d.width, d.height))
            .contents(contents);
        page_writer
            .resources()
            .x_objects()
            .pair(Name(b"Scene"), form);
    }
    let outlines = Ref::new(next);
    next += 1;
    let outline_ids: Vec<_> = (0..pages.len())
        .map(|i| Ref::new(next + i as i32))
        .collect();
    pdf.outline(outlines)
        .first(outline_ids[0])
        .last(*outline_ids.last().unwrap())
        .count(outline_ids.len() as i32);
    for (i, &p) in pages.iter().enumerate() {
        let mut item = pdf.outline_item(outline_ids[i]);
        item.title(TextStr(&d.slides[p].title)).parent(outlines);
        if i > 0 {
            item.prev(outline_ids[i - 1]);
        }
        if i + 1 < pages.len() {
            item.next(outline_ids[i + 1]);
        }
        item.dest().page(ids[i]).fit();
    }
    pdf.catalog(catalog).pages(tree).outlines(outlines);
    pdf.pages(tree)
        .kids(ids.iter().copied())
        .count(ids.len() as i32);
    let bytes = pdf.finish();
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("PDF超过64MiB".into());
    }
    crate::export::atomic(path, &bytes, None, cancel)
}
pub fn presentation(path: &Path, d: &Deck, kind: &str, cancel: &AtomicBool) -> Result<()> {
    let mut parts = crate::pptx_write::generate(d)?;
    let ct = parts
        .get_mut("[Content_Types].xml")
        .ok_or("缺少content types")?;
    let original =
        "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
    let target = match kind {
        "pptx" => original,
        "potx" => "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml",
        "ppsx" => "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml",
        _ => return Err("未知演示格式".into()),
    };
    *ct = String::from_utf8(ct.clone())
        .map_err(|e| e.to_string())?
        .replace(original, target)
        .into_bytes();
    crate::export::save_parts(path, &parts, cancel)
}
pub fn other(
    path: &Path,
    d: &Deck,
    pages: &[usize],
    kind: &str,
    cancel: &AtomicBool,
) -> Result<()> {
    d.validate()?;
    let mut outline = String::new();
    for &p in pages {
        check_cancel(cancel)?;
        let s = d.slides.get(p).ok_or("页码越界")?;
        outline.push_str(&format!("{}\n", s.title));
        for o in &s.objects {
            if !o.text.is_empty() {
                outline.push_str(&o.text);
                outline.push('\n');
            }
        }
        outline.push('\n');
    }
    let bytes = match kind {
        "outline" => outline.into_bytes(),
        "rtf" => {
            let mut r = String::from("{\\rtf1\\ansi\\uc1 ");
            for c in outline.chars() {
                match c {
                    '\n' => r.push_str("\\par\n"),
                    '\\' | '{' | '}' => {
                        r.push('\\');
                        r.push(c)
                    }
                    c if c.is_ascii() => r.push(c),
                    c => {
                        let mut buf = [0; 2];
                        for u in c.encode_utf16(&mut buf) {
                            r.push_str(&format!("\\u{}?", *u as i16));
                        }
                    }
                }
            }
            r.push('}');
            r.into_bytes()
        }
        "gif" => {
            let mut out = vec![];
            {
                let mut encoder = image::codecs::gif::GifEncoder::new(&mut out);
                for &p in pages {
                    check_cancel(cancel)?;
                    let raster = crate::preview::render(d, p, 640, cancel)?;
                    let mut rgba = raster.bgra;
                    for px in rgba.as_chunks_mut::<4>().0 {
                        px.swap(0, 2);
                    }
                    let im = image::RgbaImage::from_raw(raster.width, raster.height, rgba)
                        .ok_or("无效帧")?;
                    encoder
                        .encode_frame(image::Frame::from_parts(
                            im,
                            0,
                            0,
                            image::Delay::from_numer_denom_ms(1000, 1),
                        ))
                        .map_err(|e| e.to_string())?;
                }
            }
            out
        }
        _ => return Err("未知输出格式".into()),
    };
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("输出超过64MiB".into());
    }
    crate::export::atomic(path, &bytes, None, cancel)
}

pub fn print(d: &Deck, pages: &[usize], cancel: &AtomicBool) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err("该平台请先导出PDF，再使用系统打印".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let helper = [
        exe.parent().unwrap().join("joydeck-print-macos"),
        std::path::PathBuf::from(env!("OUT_DIR")).join("joydeck-print-macos"),
        exe.parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("joydeck-print-macos"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .ok_or("缺少打印适配器；可使用PDF导出")?;
    let path = std::env::temp_dir().join(format!(
        "joydeck-print-{}-{}.pdf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    vector_pdf(&path, d, pages, cancel)?;
    let result = std::process::Command::new(helper)
        .arg(&path)
        .status()
        .map_err(|e| e.to_string());
    let _ = std::fs::remove_file(path);
    match result {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err("系统打印失败".into()),
        Err(e) => Err(e),
    }
}
