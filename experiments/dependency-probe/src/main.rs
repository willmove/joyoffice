#![forbid(unsafe_code)]
use skrifa::MetadataProvider;
use std::{
    io::{Cursor, Read, Write},
    process::ExitCode,
};

fn probe() -> Result<(), String> {
    let font_path = std::env::args()
        .nth(1)
        .ok_or("usage: joydeck-dependency-probe FONT_FILE [FACE]")?;
    let face: u32 = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "0".into())
        .parse()
        .map_err(|_| "invalid font face")?;
    let metadata = std::fs::metadata(&font_path).map_err(|e| e.to_string())?;
    if metadata.len() > 64 * 1024 * 1024 {
        return Err("font exceeds probe budget".into());
    }
    let bytes = std::fs::read(&font_path).map_err(|e| e.to_string())?;
    let font = skrifa::FontRef::from_index(&bytes, face).map_err(|e| e.to_string())?;
    let mapped = font.charmap().map('A').ok_or("font lacks A")?;
    let data = harfrust::ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str("JoyDeck 中文");
    buffer.guess_segment_properties();
    let glyphs = shaper.shape(buffer, harfrust::ShapeOptions::new());
    if glyphs.is_empty() || glyphs.glyph_infos().iter().any(|g| g.glyph_id == 0) {
        return Err("shaping produced missing glyphs".into());
    }
    let pdf_font = krilla::text::Font::new(bytes.clone().into(), face)
        .ok_or("Krilla rejected the exact font face")?;
    let mut pdf = krilla::Document::new();
    pdf.start_page_with(krilla::page::PageSettings::from_wh(960., 540.).ok_or("invalid page")?)
        .finish();
    let pdf_bytes = pdf.finish().map_err(|e| e.to_string())?;
    if !pdf_bytes.starts_with(b"%PDF-") {
        return Err("invalid PDF header".into());
    }
    drop(pdf_font);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            "document.json",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
    writer
        .write_all(b"{\"product\":\"joydeck\"}")
        .map_err(|e| e.to_string())?;
    let zip_bytes = writer.finish().map_err(|e| e.to_string())?.into_inner();
    let mut archive =
        zip::ZipArchive::new(Cursor::new(zip_bytes.clone())).map_err(|e| e.to_string())?;
    let mut roundtrip = String::new();
    archive
        .by_name("document.json")
        .map_err(|e| e.to_string())?
        .read_to_string(&mut roundtrip)
        .map_err(|e| e.to_string())?;
    if roundtrip != "{\"product\":\"joydeck\"}" {
        return Err("ZIP roundtrip mismatch".into());
    }
    let mut cpu = vello_cpu::RenderContext::new(64, 64);
    cpu.set_paint(vello_cpu::peniko::Color::from_rgba8(166, 61, 36, 255));
    cpu.fill_rect(&kurbo::Rect::new(4., 4., 60., 60.));
    cpu.flush();
    let mut pixmap = vello_cpu::Pixmap::new(64, 64);
    cpu.render(&mut pixmap, &mut vello_cpu::Resources::default());
    let pixel = pixmap
        .data_as_u8_slice()
        .get((32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 4)
        .ok_or("missing raster pixel")?;
    if pixel != [166, 61, 36, 255] {
        return Err(format!("CPU raster mismatch: {pixel:?}"));
    }
    println!(
        "{}",
        serde_json::json!({"result":"pass", "font_bytes":bytes.len(),"face":face,"mapped_glyph":mapped.to_u32(),"shaped_glyphs":glyphs.len(),"krilla":"0.8.2 registry","pdf_bytes":pdf_bytes.len(),"zip":"9.0.0","zip_bytes":zip_bytes.len(),"vello_cpu":"0.2.0","scope":"font read/shaping, PDF writer/font acceptance, ZIP roundtrip, CPU path API; full slide/PDF parity remains R1/R6"})
    );
    Ok(())
}

fn main() -> ExitCode {
    match probe() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Probe failed: {error}");
            ExitCode::FAILURE
        }
    }
}
