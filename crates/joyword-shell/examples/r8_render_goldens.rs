//! Render the same fixed DOCX layouts to PNG and PDF, and independently inspect the PDF.
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache,
    InterpreterSettings, Paint, PathDrawMode, SoftMask, font::Glyph, interpret_page,
};
use joyoffice_opc::{Method, Package, write_archive};
use joyword_layout::{
    LayoutCache, LayoutOptions,
    display::{DisplayOptions, Draw, page_display},
};
use joyword_pdf::PdfOptions;
use joyword_render::RenderOptions;
use kurbo::{Affine, BezPath, Point, Rect};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf};

const DPI: u16 = 144;
const FONT: &[u8] = include_bytes!("../../../assets/fonts/SourceSans3-Regular.ttf");
const FAMILY: &str = "Source Sans 3";
const FACE: u32 = 0;
const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fixture() -> Result<Vec<u8>> {
    let mixed = Package::read(
        include_bytes!("../../../assets/samples/mixed-media.docx"),
        &Default::default(),
    )?;
    let image = &mixed
        .part("/word/media/blocks.png")
        .ok_or("fixture image")?
        .data;
    let xml = std::str::from_utf8(
        &mixed
            .part("/word/document.xml")
            .ok_or("fixture document")?
            .data,
    )?;
    let (_, after) = xml
        .split_once("<w:drawing>")
        .ok_or("fixture drawing start")?;
    let (drawing, _) = after
        .split_once("</w:drawing>")
        .ok_or("fixture drawing end")?;
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
      <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
      <Default Extension="png" ContentType="image/png"/>
      <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
      <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
      <Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
    </Types>"#;
    let root = format!(
        r#"<Relationships xmlns="{RELS}"><Relationship Id="doc" Type="{R}/officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let rels = format!(
        r#"<Relationships xmlns="{RELS}">
      <Relationship Id="styles" Type="{R}/styles" Target="styles.xml"/>
      <Relationship Id="header" Type="{R}/header" Target="header1.xml"/>
      <Relationship Id="rIdImage" Type="{R}/image" Target="media/blocks.png"/>
    </Relationships>"#
    );
    let styles = format!(
        r#"<w:styles xmlns:w="{W}">
      <w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{FAMILY}" w:hAnsi="{FAMILY}" w:eastAsia="{FAMILY}" w:cs="{FAMILY}"/><w:sz w:val="24"/></w:rPr></w:rPrDefault></w:docDefaults>
    </w:styles>"#
    );
    let header = format!(
        r#"<w:hdr xmlns:w="{W}"><w:p>
      <w:r><w:t xml:space="preserve">Proof page </w:t></w:r>
      <w:fldSimple w:instr="PAGE"><w:r><w:t>1</w:t></w:r></w:fldSimple>
    </w:p></w:hdr>"#
    );
    let document = format!(
        r#"<w:document xmlns:w="{W}" xmlns:r="{R}"
      xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
      xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
      xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>
      <w:p><w:r><w:t>Layout proof</w:t></w:r></w:p>
      <w:tbl>
        <w:tblPr><w:tblW w:w="3840" w:type="dxa"/><w:tblBorders>
          <w:top w:val="single" w:sz="8"/><w:bottom w:val="single" w:sz="8"/>
          <w:left w:val="single" w:sz="8"/><w:right w:val="single" w:sz="8"/>
          <w:insideV w:val="single" w:sz="8"/>
        </w:tblBorders></w:tblPr>
        <w:tblGrid><w:gridCol w:w="1920"/><w:gridCol w:w="1920"/></w:tblGrid>
        <w:tr>
          <w:tc><w:tcPr><w:shd w:val="clear" w:fill="DCEBFA"/></w:tcPr><w:p><w:r><w:t>Cell left</w:t></w:r></w:p></w:tc>
          <w:tc><w:p><w:r><w:t>Cell right</w:t></w:r></w:p></w:tc>
        </w:tr>
      </w:tbl>
      <w:p><w:pPr><w:spacing w:before="120"/></w:pPr><w:r><w:drawing>{drawing}</w:drawing></w:r></w:p>
      <w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:r><w:t xml:space="preserve">Review </w:t></w:r>
        <w:del w:id="1" w:author="R8" w:date="2026-11-04T00:00:00Z"><w:r><w:delText xml:space="preserve">removed </w:delText></w:r></w:del>
        <w:ins w:id="2" w:author="R8" w:date="2026-11-04T00:00:00Z"><w:r><w:t>inserted</w:t></w:r></w:ins>
      </w:p>
      <w:sectPr><w:headerReference w:type="default" r:id="header"/>
        <w:pgSz w:w="7200" w:h="5760"/><w:pgMar w:top="960" w:right="720" w:bottom="720" w:left="720" w:header="360" w:footer="360" w:gutter="0"/>
      </w:sectPr>
    </w:body></w:document>"#
    );
    Ok(write_archive(&[
        ("[Content_Types].xml", types.as_bytes(), Method::Stored),
        ("_rels/.rels", root.as_bytes(), Method::Stored),
        (
            "word/_rels/document.xml.rels",
            rels.as_bytes(),
            Method::Stored,
        ),
        ("word/document.xml", document.as_bytes(), Method::Stored),
        ("word/styles.xml", styles.as_bytes(), Method::Stored),
        ("word/header1.xml", header.as_bytes(), Method::Stored),
        ("word/media/blocks.png", image, Method::Stored),
    ])?)
}

fn check_faces(draws: &[Draw]) -> Result<usize> {
    let mut count = 0;
    for draw in draws {
        match draw {
            Draw::Glyphs { face, glyphs, .. } => {
                if face.data() != FONT || face.index() != FACE || glyphs.iter().any(|g| g.0 == 0) {
                    return Err(format!("unexpected glyph or font face: {face:?}").into());
                }
                count += glyphs.len();
            }
            Draw::Figure { draws, .. }
            | Draw::Turned { items: draws, .. }
            | Draw::Rotated { items: draws, .. }
            | Draw::Clip { items: draws, .. } => count += check_faces(draws)?,
            _ => {}
        }
    }
    Ok(count)
}

#[derive(Default)]
struct PdfPage {
    text: String,
    origins: Vec<[i64; 2]>,
    images: usize,
}

fn centipoints(v: f64) -> i64 {
    (v * 100.0).round() as i64
}

impl Device<'_> for PdfPage {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'_>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn draw_glyph(
        &mut self,
        g: &Glyph<'_>,
        transform: Affine,
        local: Affine,
        _: &Paint<'_>,
        mode: &GlyphDrawMode,
    ) {
        if matches!(mode, GlyphDrawMode::Stroke(_)) {
            return;
        }
        match g.as_unicode() {
            Some(hayro_cmap::BfString::Char(ch)) => self.text.push(ch),
            Some(hayro_cmap::BfString::String(value)) => self.text.push_str(&value),
            None => self.text.push('\u{fffd}'),
        }
        let origin = (transform * local) * Point::ORIGIN;
        self.origins
            .push([centipoints(origin.x), centipoints(origin.y)]);
    }
    fn draw_image(&mut self, _: Image<'_, '_>, _: Affine) {
        self.images += 1;
    }
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

fn pdf_report(bytes: Vec<u8>, markup: bool) -> Result<Vec<Value>> {
    let pdf = hayro_syntax::Pdf::new(bytes).map_err(|e| format!("PDF read-back: {e:?}"))?;
    if pdf.pages().len() != 2 {
        return Err("expected two PDF pages".into());
    }
    let cache = InterpreterCache::new();
    pdf.pages()
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let mut context = Context::new(
                Affine::IDENTITY,
                Rect::new(0.0, 0.0, 360.0, 288.0),
                &cache,
                pdf.xref(),
                InterpreterSettings::default(),
            );
            let mut found = PdfPage::default();
            interpret_page(page, &mut context, &mut found);
            let text = &found.text;
            let required = if index == 0 {
                ["Proof page 1", "Layout proof", "Cell left", "Cell right"]
            } else {
                [
                    "Proof page 2",
                    "Review",
                    "inserted",
                    if markup { "removed" } else { "inserted" },
                ]
            };
            if !required.iter().all(|word| text.contains(word))
                || (!markup && text.contains("removed"))
                || found.images != usize::from(index == 0)
                || text.contains('\u{fffd}')
            {
                return Err(format!(
                    "unexpected PDF contents on page {}: {text:?}, {} images",
                    index + 1,
                    found.images
                )
                .into());
            }
            let rect = page.media_box();
            let dimensions = [centipoints(rect.width()), centipoints(rect.height())];
            if dimensions != [36000, 28800] {
                return Err(format!("unexpected PDF page dimensions: {dimensions:?}").into());
            }
            Ok(json!({
                "page_centipoints": dimensions, "text": text, "images": found.images,
                "glyph_origins_sha256": sha(&serde_json::to_vec(&found.origins)?),
            }))
        })
        .collect()
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(
        args.next()
            .ok_or("usage: r8_render_goldens NEW_OUTPUT_DIR [--record]")?,
    );
    let record = match args.next() {
        None => false,
        Some(arg) if arg == "--record" && args.next().is_none() => true,
        Some(_) => return Err("usage: r8_render_goldens NEW_OUTPUT_DIR [--record]".into()),
    };
    if record && std::env::var_os("CI").is_some() {
        return Err("CI must compare the reference".into());
    }
    fs::create_dir_all(
        output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new(".")),
    )?;
    fs::create_dir(&output)?;
    joyword_fonts::FontDb::global().set_system_fallback(false);
    let input = fixture()?;
    joyword_shell::reader::preflight_word_package(&input)?;
    let mut doc = joyword_docx::read(&input)?;
    doc.core.created = "2026-11-04T00:00:00Z".into();
    fs::write(output.join("fixture.docx"), &input)?;
    let mut views = serde_json::Map::new();
    for (name, markup) in [("review", true), ("final", false)] {
        let layout = joyword_layout::layout(
            &doc,
            &mut LayoutCache::new(),
            &LayoutOptions {
                hide_deleted: !markup,
                ..Default::default()
            },
        );
        let options = RenderOptions {
            display: DisplayOptions {
                markup,
                dim_header: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut png_report = Vec::new();
        for (index, page) in layout.pages.iter().enumerate() {
            let count = check_faces(&page_display(&doc, page, &options.display))?;
            if count == 0 {
                return Err("scene has no glyphs".into());
            }
            let raster = joyword_render::render_page(&doc, page, f32::from(DPI) / 72.0, &options);
            let png = raster.to_png();
            if png.is_empty() || (raster.width, raster.height) != (720, 576) {
                return Err("unexpected PNG geometry".into());
            }
            fs::write(output.join(format!("{name}-{}.png", index + 1)), &png)?;
            png_report.push(json!({
                "size": [raster.width, raster.height], "glyphs": count,
                "rgba_sha256": sha(&raster.to_straight()),
                "file_sha256": sha(&png),
            }));
        }
        let pdf = joyword_pdf::export_layout(
            &doc,
            &layout,
            &PdfOptions {
                include_markup: markup,
                ..Default::default()
            },
        )?;
        fs::write(output.join(format!("{name}.pdf")), &pdf)?;
        let pdf_sha256 = sha(&pdf);
        views.insert(
            name.into(),
            json!({"png": png_report, "pdf": pdf_report(pdf, markup)?, "pdf_sha256": pdf_sha256}),
        );
    }
    let actual = json!({
        "fixture_sha256": sha(&input), "dpi": DPI,
        "font": {"family": FAMILY, "style": "Regular", "face": FACE, "sha256": sha(FONT)},
        "views": views,
    });
    let summary = output.join("render.json");
    fs::write(&summary, serde_json::to_string_pretty(&actual)? + "\n")?;
    if !record {
        let path = PathBuf::from(ROOT).join("../../tests/fixtures/r8-render/render.json");
        let expected: Value = serde_json::from_slice(&fs::read(&path)?)?;
        if actual != expected {
            return Err(format!(
                "render golden mismatch: compare {} with {}",
                summary.display(),
                path.display()
            )
            .into());
        }
        println!("PNG and PDF goldens match at {DPI} DPI ({FAMILY} face {FACE})");
    }
    Ok(())
}

fn main() -> Result<()> {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(run)?
        .join()
        .map_err(|_| "render worker panicked")?
}
