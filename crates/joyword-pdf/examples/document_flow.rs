//! C05: engine snapshot -> paginated document layout -> unchanged PDF scenes.
//! Usage: document_flow <font-file> <face-index> <out-dir>
//! Explicit preview metrics, not a Word compatibility profile.
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;

use joyword_engine::{Command, Session, Transaction};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, SingleFontProfile,
    layout_document, to_points,
};
use joyword_model::{
    Bias, BlockSeq, Document, Measure, PageSetup, Paragraph, Position, ScalarOffset,
};

const SEED: &str = include_str!("../../../tests/fixtures/text/cjk-contract.txt");

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: document_flow <font-file> <face-index> <out-dir>".into());
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
    let mut document = Document::new(page);
    let mut blocks = Vec::new();
    for text in SEED.lines().cycle().take(20) {
        let mut p = Paragraph::new(document.ids().node());
        p.push_text(text, None, Arc::default())?;
        blocks.push(p.into());
    }
    let story = document.body_id();
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(blocks);
    let start = Position {
        story,
        paragraph: document.body().blocks.get(0).unwrap().id(),
        offset: ScalarOffset(0),
        bias: Bias::Before,
    };
    let mut session = Session::new(document);
    session.apply(Transaction::new(
        session.version(),
        vec![Command::InsertText {
            at: start,
            text: "测试：".into(),
        }],
    ))?;
    let profile = SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    };
    let result = layout_document(
        &session.document().snapshot(),
        FaceSet::uniform(&font),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )?;
    result.ensure_current(&session.document().snapshot(), &font, profile)?;
    if !result.diagnostics().is_empty() || result.omitted_diagnostics() != 0 {
        return Err(format!("layout diagnostics: {:?}", result.diagnostics()).into());
    }
    let mut pages = Vec::new();
    for page in result.pages() {
        let mut lines = Vec::new();
        for run in &page.runs {
            // Reader boxes omit edge whitespace. Keep source/PDF whitespace
            // intact and compare the visible box separately from baselines.
            let visible: Vec<_> = run
                .glyphs
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    run.text[run.cluster_range(*i)]
                        .chars()
                        .any(|c| !c.is_whitespace())
                })
                .map(|(_, g)| g)
                .collect();
            let start = visible.first().map_or(0, |g| g.x.raw());
            let end = visible
                .last()
                .map_or(start, |g| g.x.raw() + g.advance.raw());
            lines.push(format!(
                "{{\"text\":{},\"x_pt\":{:.6},\"baseline_pt\":{:.6},\"width_pt\":{:.6}}}",
                json_string(&run.text),
                to_points(Emu::new(run.origin_x.raw() + start)),
                to_points(run.baseline_y),
                to_points(Emu::new(end - start))
            ));
        }
        pages.push(format!(
            "{{\"width\":{:.6},\"height\":{:.6},\"lines\":[{}]}}",
            to_points(page.width),
            to_points(page.height),
            lines.join(",")
        ));
    }
    let expected = format!(
        "{{\"font_sha256\":\"{}\",\"face\":{},\"version\":{},\"pages\":[{}]}}\n",
        font.key().hash_hex(),
        font.key().index(),
        result.key().version.get(),
        pages.join(",\n")
    );
    let pdf = joyword_pdf::render(result.pages(), std::slice::from_ref(&font))
        .map_err(|e| format!("PDF: {e:?}"))?;
    let out = Path::new(&args[3]);
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("scene.pdf"), pdf)?;
    std::fs::write(out.join("expected.json"), expected)?;
    println!(
        "{} pages, {} lines, 0 diagnostics; font {} face {}; {:?}",
        result.pages().len(),
        result.lines().len(),
        font.key().hash_hex(),
        font.key().index(),
        result.stats()
    );
    Ok(())
}
