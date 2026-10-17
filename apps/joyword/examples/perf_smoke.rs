//! T107 performance smoke baseline, headless (no window, no GPU).
//!
//! Usage: perf_smoke <font-file> <face-index>
//!
//! Measures, with an explicit CJK-capable font and the startup DOCX fixture:
//! - open latency of the startup sample (`DocxSession::open_default`),
//! - first full layout of a 20+ page document built from the sample lines,
//! - input latency: one committed character insert followed by a full relayout
//!   (the current M1 reality before T208 incremental layout).
//!
//! `cargo run -p joyword --example perf_smoke --locked --release -- FONT 0`
//! Numbers are environment-specific; record the machine and font next to
//! results. The checked-in joyoffice-test-serif.ttf subset is Latin-only and
//! cannot take the CJK sample through layout; pass a CJK-capable OFL font.

use std::sync::Arc;
use std::time::{Duration, Instant};

use joyword_engine::{Command, Transaction};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, SingleFontProfile,
    layout_document,
};
use joyword_model::{Bias, BlockSeq, Document, PageSetup, Paragraph, Position, ScalarOffset};

const SAMPLE_DOCX: &[u8] = include_bytes!("../../../tests/fixtures/docx/DOCX-SAMPLE-001.docx");
const TARGET_PAGES: usize = 20;
const PARAGRAPH_CAP: usize = 2_000;
const OPEN_RUNS: usize = 7;
const INPUT_RUNS: usize = 5;

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    }
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort();
    values
        .get(values.len() / 2)
        .copied()
        .unwrap_or(Duration::ZERO)
}

fn ms(duration: Duration) -> String {
    format!("{:.3}", duration.as_secs_f64() * 1_000.0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: perf_smoke <font-file> <face-index>".into());
    }
    let font_data: Arc<[u8]> = std::fs::read(&args[1])?.into();
    let face: u32 = args[2].parse()?;
    let font = LoadedFont::from_bytes(font_data, face).map_err(|e| format!("font: {e:?}"))?;
    let profile = profile();
    let cancel = LayoutCancellation::default();

    // Open latency on the committed startup fixture.
    let mut open = Vec::new();
    for _ in 0..OPEN_RUNS {
        let start = Instant::now();
        let session = joyword_shell::docx::DocxSession::open_default(SAMPLE_DOCX)?;
        open.push(start.elapsed());
        std::hint::black_box(&session);
    }

    // Source lines from the same fixture, repeated into a 20+ page document.
    let session = joyword_shell::docx::DocxSession::open_default(SAMPLE_DOCX)?;
    let mut lines: Vec<String> = Vec::new();
    for block in session.session().document().body().blocks.iter() {
        if let Some(paragraph) = block.as_paragraph() {
            let text = paragraph.plain_text(|_| String::new());
            if !text.trim().is_empty() {
                lines.push(text);
            }
        }
    }
    if lines.is_empty() {
        return Err("startup sample has no text lines".into());
    }

    let mut document = Document::new(PageSetup::a4_zh_cn());
    let story = document.body_id();
    let mut paragraphs: Vec<joyword_model::Block> = Vec::new();
    let mut pages = 0usize;
    let build_start = Instant::now();
    while pages < TARGET_PAGES && paragraphs.len() < PARAGRAPH_CAP {
        for line in &lines {
            let mut paragraph = Paragraph::new(document.ids().node());
            paragraph.push_text(line, None, Arc::default())?;
            paragraphs.push(paragraph.into());
        }
        document.story_mut(story).ok_or("body story exists")?.blocks =
            BlockSeq::from_vec(paragraphs.clone());
        let layout = layout_document(
            &document.snapshot(),
            FaceSet::uniform(&font),
            profile,
            LayoutLimits::default(),
            &cancel,
        )
        .map_err(|e| format!("layout: {e:?}"))?;
        if !layout.diagnostics().is_empty() || layout.omitted_diagnostics() != 0 {
            return Err("layout reported diagnostics for the sample font".into());
        }
        pages = layout.pages().len();
    }
    let build_ms = build_start.elapsed();
    if pages < TARGET_PAGES {
        return Err(format!(
            "could not reach {TARGET_PAGES} pages within {PARAGRAPH_CAP} paragraphs (got {pages})"
        )
        .into());
    }

    // First full layout of the 20+ page document.
    let layout_start = Instant::now();
    let laid_out = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font),
        profile,
        LayoutLimits::default(),
        &cancel,
    )
    .map_err(|e| format!("layout: {e:?}"))?;
    let first_layout = layout_start.elapsed();
    if laid_out.pages().len() != pages {
        return Err("layout page count changed between identical runs".into());
    }

    // Input latency: commit one character, then full relayout, median of runs.
    let first = document
        .body()
        .blocks
        .get(0)
        .and_then(|block| block.as_paragraph())
        .ok_or("first block is a paragraph")?
        .id();
    let mut commit = Vec::new();
    let mut relayout = Vec::new();
    for _ in 0..INPUT_RUNS {
        let at = Position {
            story,
            paragraph: first,
            offset: ScalarOffset(0),
            bias: Bias::After,
        };
        let mut session = joyword_engine::Session::new(document.clone());
        let commit_start = Instant::now();
        session.apply(Transaction::new(
            session.version(),
            vec![Command::InsertText {
                at,
                text: "校".into(),
            }],
        ))?;
        commit.push(commit_start.elapsed());
        let relayout_start = Instant::now();
        let again = layout_document(
            &session.document().snapshot(),
            FaceSet::uniform(&font),
            profile,
            LayoutLimits::default(),
            &cancel,
        )
        .map_err(|e| format!("layout: {e:?}"))?;
        relayout.push(relayout_start.elapsed());
        std::hint::black_box(&again);
    }

    println!("workload pages={pages} paragraphs={}", paragraphs.len());
    println!(
        "open_median_ms={} open_min_ms={} open_runs={OPEN_RUNS}",
        ms(median(open.clone())),
        ms(open.into_iter().min().unwrap_or(Duration::ZERO)),
    );
    println!(
        "build_20p_ms={} (untimed part of the baseline record)",
        ms(build_ms)
    );
    println!("layout_first_ms={}", ms(first_layout));
    println!(
        "input_commit_median_ms={} input_relayout_median_ms={} input_runs={INPUT_RUNS}",
        ms(median(commit)),
        ms(median(relayout)),
    );
    println!(
        "font_sha256={} face={}",
        font.key().hash_hex(),
        font.key().index()
    );
    Ok(())
}
