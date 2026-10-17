//! Same immutable document scenes -> CPU PNGs and searchable PDF.
//! Usage: cpu_scene <font-file> <face-index> <out-dir>
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, SingleFontProfile,
    layout_document,
};
use joyword_model::{BlockSeq, Document, Measure, PageSetup, Paragraph};
use joyword_render::{Projection, RasterCancellation, RasterLimits, rasterize};
use std::path::Path;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: cpu_scene <font-file> <face-index> <out-dir>".into());
    }
    let font = LoadedFont::from_bytes(std::fs::read(&args[1])?.into(), args[2].parse()?)
        .map_err(|e| format!("font: {e:?}"))?;
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(148 * Emu::PER_MM);
    page.height = Measure::Emu(140 * Emu::PER_MM);
    page.left = Measure::Emu(18 * Emu::PER_MM);
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    let mut doc = Document::new(page);
    let mut blocks = Vec::new();
    for text in include_str!("../../../tests/fixtures/text/cjk-contract.txt")
        .lines()
        .cycle()
        .take(20)
    {
        let mut p = Paragraph::new(doc.ids().node());
        p.push_text(text, None, Arc::default())?;
        blocks.push(p.into());
    }
    let story = doc.body_id();
    doc.story_mut(story).unwrap().blocks = BlockSeq::from_vec(blocks);
    let result = layout_document(
        &doc.snapshot(),
        FaceSet::uniform(&font),
        SingleFontProfile {
            size: Emu::new(12 * Emu::PER_POINT),
            leading: Emu::new(18 * Emu::PER_POINT),
            baseline: Emu::new(14 * Emu::PER_POINT),
            features: FeatureSet::default(),
        },
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )?;
    if !result.diagnostics().is_empty() || result.omitted_diagnostics() != 0 {
        return Err(format!("diagnostics: {:?}", result.diagnostics()).into());
    }
    let out = Path::new(&args[3]);
    std::fs::create_dir_all(out)?;
    for (index, page) in result.pages().iter().enumerate() {
        let raster = rasterize(
            page,
            std::slice::from_ref(&font),
            Projection::new(144.0, 1.0)?,
            RasterLimits::default(),
            &RasterCancellation::default(),
        )?;
        std::fs::write(out.join(format!("cpu-{}.png", index + 1)), raster.png()?)?;
        println!(
            "page {}: {}x{} {:?}",
            index + 1,
            raster.width(),
            raster.height(),
            raster.stats()
        );
    }
    let pdf = joyword_pdf::render(result.pages(), std::slice::from_ref(&font))
        .map_err(|e| format!("PDF: {e:?}"))?;
    std::fs::write(out.join("reference.pdf"), pdf)?;
    println!(
        "{} pages / {} lines; font {} face {}",
        result.pages().len(),
        result.lines().len(),
        font.key().hash_hex(),
        font.key().index()
    );
    Ok(())
}
