use joydeck_model::document::{Deck, Result, check_cancel};
use std::{io::Write, sync::atomic::AtomicBool};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}
#[derive(Clone)]
pub struct Options {
    pub pages: Vec<usize>,
    pub dpi: u32,
    pub transparent: bool,
    pub notes: bool,
}
impl Options {
    pub fn validate(&self, deck: &Deck) -> Result<()> {
        deck.validate()?;
        if !(36..=600).contains(&self.dpi) || self.pages.is_empty() || self.pages.len() > 100 {
            return Err("导出DPI须36–600，页面须1–100页".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for p in &self.pages {
            if *p >= deck.slides.len() || !seen.insert(*p) {
                return Err("导出页不存在或重复".into());
            }
        }
        Ok(())
    }
}
pub fn pages(deck: &Deck, range: &str, hidden: bool) -> Result<Vec<usize>> {
    let mut out = vec![];
    let mut seen = std::collections::BTreeSet::new();
    let range = range.trim();
    if range.is_empty() {
        for (i, s) in deck.slides.iter().enumerate() {
            if hidden || !s.hidden {
                out.push(i);
            }
        }
    } else {
        for item in range.split(',') {
            let item = item.trim();
            let (a, b) = if let Some((a, b)) = item.split_once('-') {
                (
                    a.trim().parse::<usize>().map_err(|_| "页范围无效")?,
                    b.trim().parse::<usize>().map_err(|_| "页范围无效")?,
                )
            } else {
                let a = item.parse::<usize>().map_err(|_| "页范围无效")?;
                (a, a)
            };
            if a == 0 || b < a || b > deck.slides.len() {
                return Err("页范围越界".into());
            }
            for i in a - 1..b {
                if !seen.insert(i) {
                    return Err("页范围重复".into());
                }
                if hidden || !deck.slides[i].hidden {
                    out.push(i);
                }
            }
        }
    }
    if out.is_empty() {
        return Err("筛选后无可导出页面".into());
    }
    Ok(out)
}
pub fn page_deck(deck: &Deck, page: usize, notes: bool) -> Result<Deck> {
    let mut d = deck.clone();
    d.source = None;
    if notes && !d.slides.get(page).ok_or("页不存在")?.notes.is_empty() {
        use unicode_segmentation::UnicodeSegmentation;
        let count = (d.width / 16.).floor().max(1.) as usize;
        let text = d.slides[page].notes.clone();
        let mut lines = vec![];
        for line in text.lines() {
            let clusters: Vec<_> = line.graphemes(true).collect();
            if clusters.is_empty() {
                lines.push(String::new());
            } else {
                for chunk in clusters.chunks(count) {
                    lines.push(chunk.concat());
                }
            }
        }
        if lines.len() > 40 {
            return Err("备注输出超过40行，请缩短备注或单独导出".into());
        }
        let top = d.height;
        d.height += (lines.len() as f32 * 20.).max(100.) + 40.;
        if d.height > 4096. {
            return Err("备注页面高度超限".into());
        }
        let mut o = joydeck_model::identity::object("text");
        o.id = format!("notes-export-{page}");
        o.x = 20.;
        o.y = top + 20.;
        o.width = d.width - 40.;
        o.height = d.height - top - 20.;
        o.font_size = 14.;
        o.fill = "#202b3a".into();
        o.text = lines.join("\n");
        d.slides[page].objects.push(o);
    }
    Ok(d)
}
pub fn raster(
    deck: &Deck,
    page: usize,
    dpi: u32,
    transparent: bool,
    cancel: &AtomicBool,
) -> Result<image::RgbaImage> {
    check_cancel(cancel)?;
    if !(36..=600).contains(&dpi) {
        return Err("DPI超限".into());
    }
    let scale = dpi as f32 / 72.;
    let w = (deck.width * scale).round().max(1.) as u32;
    let h = (deck.height * scale).round().max(1.) as u32;
    if w > 8192 || h > 8192 || u64::from(w) * u64::from(h) > 16 * 1024 * 1024 {
        return Err("导出尺寸超过8192边/16M像素预算，请降低DPI".into());
    }
    let mut svg = crate::preview::svg(deck, page, cancel)?;
    if transparent {
        let a = svg.find("<rect").ok_or("缺少背景")?;
        let b = svg[a..].find("/>").ok_or("背景无效")? + a + 2;
        svg.replace_range(a..b, "");
    }
    let tree =
        resvg::usvg::Tree::from_str(&svg, &crate::preview::options()).map_err(|e| e.to_string())?;
    let mut pix = resvg::tiny_skia::Pixmap::new(w, h).ok_or("输出分配失败")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(w as f32 / deck.width, h as f32 / deck.height),
        &mut pix.as_mut(),
    );
    check_cancel(cancel)?;
    let mut rgba = pix.take();
    for p in rgba.as_chunks_mut::<4>().0 {
        let a = p[3] as u32;
        for c in &mut p[..3] {
            *c = (*c as u32 * 255 + a / 2)
                .checked_div(a)
                .unwrap_or(0)
                .min(255) as u8;
        }
    }
    image::RgbaImage::from_raw(w, h, rgba).ok_or("输出尺寸错误".into())
}
pub fn encoded(image: &image::RgbaImage, format: ImageFormat, dpi: u32) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    match format {
        ImageFormat::Png => {
            let mut encoder = png::Encoder::new(&mut bytes, image.width(), image.height());
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let ppm = (dpi as f64 / 0.0254).round() as u32;
            encoder.set_pixel_dims(Some(png::PixelDimensions {
                xppu: ppm,
                yppu: ppm,
                unit: png::Unit::Meter,
            }));
            encoder
                .write_header()
                .map_err(|e| e.to_string())?
                .write_image_data(image.as_raw())
                .map_err(|e| e.to_string())?;
        }
        ImageFormat::Jpeg => {
            let rgb = image::DynamicImage::ImageRgba8(image.clone()).into_rgb8();
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 92);
            encoder.set_pixel_density(image::codecs::jpeg::PixelDensity::dpi(dpi as u16));
            encoder
                .encode(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(bytes)
}
fn utf16(s: &str) -> String {
    let mut out = "FEFF".to_owned();
    for c in s.encode_utf16() {
        out.push_str(&format!("{c:04X}"));
    }
    out
}
fn zip_bytes(data: &[u8]) -> Result<Vec<u8>> {
    let mut z = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
    z.write_all(data).map_err(|e| e.to_string())?;
    z.finish().map_err(|e| e.to_string())
}
struct Pdf {
    objects: Vec<Vec<u8>>,
}
impl Pdf {
    fn add(&mut self, b: Vec<u8>) -> usize {
        self.objects.push(b);
        self.objects.len()
    }
    fn stream(&mut self, dict: &str, data: &[u8]) -> usize {
        let mut bytes = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
        bytes.extend_from_slice(data);
        bytes.extend_from_slice(b"\nendstream");
        self.add(bytes)
    }
    fn finish(self) -> Vec<u8> {
        let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = vec![0];
        for (i, obj) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(obj);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes(),
        );
        for offset in &offsets[1..] {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        out
    }
}
/// Visible page is raster; invisible ActualText maps object lines for extraction. Not vector/Tagged PDF.
pub fn pdf_bytes(deck: &Deck, options: &Options, cancel: &AtomicBool) -> Result<Vec<u8>> {
    options.validate(deck)?;
    if options.transparent {
        return Err("PDF当前使用不透明页面".into());
    }
    let mut pdf = Pdf {
        objects: vec![vec![], vec![], vec![]],
    };
    let font = pdf.add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());
    let mut pages = vec![];
    let mut titles = vec![];
    for page in &options.pages {
        check_cancel(cancel)?;
        let d = page_deck(deck, *page, options.notes)?;
        let rgba = raster(&d, *page, options.dpi, false, cancel)?;
        let rgb = image::DynamicImage::ImageRgba8(rgba).into_rgb8();
        let compressed = zip_bytes(rgb.as_raw())?;
        let image=pdf.stream(&format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",rgb.width(),rgb.height()),&compressed);
        let mut content = format!("q {} 0 0 {} 0 0 cm /Im0 Do Q\n", d.width, d.height);
        let s = crate::design::resolve_slide(&d, *page)?;
        let objects = crate::table::expand(&d, *page, &s.objects)?;
        for o in &objects {
            if o.kind != "text" && o.kind != "placeholder" {
                continue;
            }
            let angle = -o.rotation.to_radians();
            let (sin, cos) = angle.sin_cos();
            for (i, line) in o.text.split('\n').enumerate() {
                if line.is_empty() {
                    continue;
                }
                let y = d.height - o.y - o.font_size * (1. + i as f32 * 1.3);
                let scale = (o.width / (line.chars().count().max(1) as f32 * o.font_size * 0.5))
                    .clamp(0.1, 10.)
                    * 100.;
                let hidden = "x".repeat(line.chars().count());
                content.push_str(&format!("/Span << /ActualText <{}> >> BDC BT /F1 {} Tf 3 Tr {} Tz {} {} {} {} {} {} Tm ({hidden}) Tj ET EMC\n",utf16(line),o.font_size,scale,cos,sin,-sin,cos,o.x,y));
            }
        }
        let stream = pdf.stream("", content.as_bytes());
        let p=pdf.add(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /XObject << /Im0 {image} 0 R >> /Font << /F1 {font} 0 R >> >> /Contents {stream} 0 R >>",d.width,d.height).into_bytes());
        pages.push(p);
        titles.push(s.title);
        if pdf.objects.iter().map(Vec::len).sum::<usize>() > 64 * 1024 * 1024 {
            return Err("PDF超过64 MiB预算".into());
        }
    }
    let first = pdf.objects.len() + 1;
    for (i, page) in pages.iter().enumerate() {
        let previous = if i > 0 {
            format!("/Prev {} 0 R", first + i - 1)
        } else {
            String::new()
        };
        let next = if i + 1 < pages.len() {
            format!("/Next {} 0 R", first + i + 1)
        } else {
            String::new()
        };
        pdf.add(
            format!(
                "<< /Title <{}> /Parent 3 0 R /Dest [{page} 0 R /Fit] {previous} {next} >>",
                utf16(&titles[i])
            )
            .into_bytes(),
        );
    }
    pdf.objects[0] = b"<< /Type /Catalog /Pages 2 0 R /Outlines 3 0 R >>".to_vec();
    pdf.objects[1] = format!(
        "<< /Type /Pages /Count {} /Kids [{}] >>",
        pages.len(),
        pages
            .iter()
            .map(|p| format!("{p} 0 R"))
            .collect::<Vec<_>>()
            .join(" ")
    )
    .into_bytes();
    pdf.objects[2] = format!(
        "<< /Type /Outlines /First {first} 0 R /Last {} 0 R /Count {} >>",
        first + pages.len() - 1,
        pages.len()
    )
    .into_bytes();
    Ok(pdf.finish())
}
