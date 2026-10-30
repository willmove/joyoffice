//! Reproducible model/layout benchmark, NOT GUI input-to-presentation timing.
//! Usage: m7_performance <10|100|1000> <samples 1..100> <events 2..2000>
//! JSON Lines on stdout; every edit is checked against full layout outside
//! the timed interval. The embedded original OFL font needs no system fonts.
use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use joyword_engine::{ChangeSet, Command, Outcome, Session, Transaction};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutInvalidation, LayoutLimits,
    LayoutReuse, LayoutSnapshot, LoadedFont, ParagraphLayoutCache, SingleFontProfile,
    layout_document, layout_incremental,
};
use joyword_model::{Bias, Block, BlockSeq, Document, Paragraph, Position, ScalarOffset};
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const FONT: &[u8] = include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf");
const TEXT: &str = "JoyWordOriginalParagraph ";

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    }
}

fn corpus(pages: usize) -> Result<(Document, String)> {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    let mut hash = Sha256::new();
    // One explicitly separated paragraph per page. This is a fixed pure-text
    // workload; it does not stand in for reference-software mixed documents.
    for index in 0..pages {
        let text = format!("Page {index:04}. {}", TEXT.repeat(20));
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text(&text, None, Arc::default())?;
        Arc::make_mut(&mut paragraph.props).page_break_before = Some(index != 0);
        blocks.push(Block::from(paragraph));
    }
    let story = document.body_id();
    document.story_mut(story).ok_or("body")?.blocks = BlockSeq::from_vec(blocks);
    Ok((document, format!("{:x}", hash.finalize())))
}

fn full(
    document: &Document,
    font: &LoadedFont,
    token: &LayoutCancellation,
) -> Result<LayoutSnapshot> {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(font),
        profile(),
        LayoutLimits::default(),
        token,
    )
    .map_err(|error| format!("layout: {error:?}").into())
}

fn incremental(
    document: &Document,
    font: &LoadedFont,
    cache: &mut ParagraphLayoutCache,
    previous: Option<&LayoutSnapshot>,
    changes: &LayoutInvalidation,
) -> Result<LayoutSnapshot> {
    layout_incremental(
        &document.snapshot(),
        FaceSet::uniform(font),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        LayoutReuse {
            cache,
            previous,
            invalidation: changes,
        },
    )
    .map_err(|error| format!("incremental: {error:?}").into())
}

fn invalidation(changes: &ChangeSet) -> LayoutInvalidation {
    LayoutInvalidation {
        paragraphs: changes.paragraphs.clone(),
        inserted: changes.inserted.clone(),
        removed: changes.removed.clone(),
        moved: changes.moved.clone(),
        styles: changes.styles.clone(),
        defaults_changed: changes.defaults_changed,
    }
}

fn same(expected: &LayoutSnapshot, actual: &LayoutSnapshot) -> Result<()> {
    if expected.page_geometry() != actual.page_geometry()
        || expected.lines() != actual.lines()
        || expected.pages().len() != actual.pages().len()
        || expected
            .pages()
            .iter()
            .zip(actual.pages())
            .any(|(a, b)| a.runs != b.runs || a.rects != b.rects || a.images != b.images)
    {
        return Err("incremental/full scene mismatch".into());
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: m7_performance <10|100|1000> <samples> <events>".into());
    }
    let pages = args[1].parse::<usize>()?;
    let samples = args[2].parse::<usize>()?;
    let events = args[3].parse::<usize>()?;
    if ![10, 100, 1000].contains(&pages)
        || !(1..=100).contains(&samples)
        || !(2..=2000).contains(&events)
    {
        return Err("arguments exceed benchmark bounds".into());
    }
    let (document, corpus_hash) = corpus(pages)?;
    let font =
        LoadedFont::from_bytes(Arc::from(FONT), 0).map_err(|error| format!("font: {error:?}"))?;
    println!(
        "{{\"kind\":\"identity\",\"schema\":1,\"pages\":{pages},\"corpus_sha256\":\"{corpus_hash}\",\"font_sha256\":\"{:x}\",\"font_face\":0}}",
        Sha256::digest(FONT)
    );
    for sample in 0..samples {
        let start = Instant::now();
        let scene = full(&document, &font, &LayoutCancellation::default())?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if scene.pages().len() != pages {
            return Err("fixed page count changed".into());
        }
        println!(
            "{{\"kind\":\"full_layout\",\"sample\":{sample},\"ms\":{elapsed:.6},\"pages\":{pages}}}"
        );
    }
    let token = LayoutCancellation::default();
    token.cancel();
    let start = Instant::now();
    let result = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font),
        profile(),
        LayoutLimits::default(),
        &token,
    );
    if !matches!(result, Err(LayoutError::Cancelled)) {
        return Err("cancelled layout published a scene".into());
    }
    println!(
        "{{\"kind\":\"pre_cancel\",\"ms\":{:.6}}}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    let mut session = Session::new(document);
    let mut cache = ParagraphLayoutCache::default();
    let mut scene = incremental(
        session.document(),
        &font,
        &mut cache,
        None,
        &LayoutInvalidation::default(),
    )?;
    for end in ["first", "last"] {
        let block = if end == "first" { 0 } else { pages - 1 };
        let paragraph = session
            .document()
            .body()
            .blocks
            .get(block)
            .ok_or("paragraph")?
            .id();
        for event in 0..events {
            let start = Instant::now();
            let changes = if event % 2 == 0 {
                let position = Position {
                    story: session.document().body_id(),
                    paragraph,
                    offset: ScalarOffset(0),
                    bias: Bias::Before,
                };
                let outcome = session
                    .apply(Transaction::new(
                        session.version(),
                        vec![Command::InsertText {
                            at: position,
                            text: "x".into(),
                        }],
                    ))
                    .map_err(|error| format!("edit: {error:?}"))?;
                let Outcome::Applied(applied) = outcome else {
                    return Err("edit was not applied".into());
                };
                applied.changes
            } else {
                session
                    .undo()
                    .map_err(|error| format!("undo: {error:?}"))?
                    .changes
            };
            let next = incremental(
                session.document(),
                &font,
                &mut cache,
                Some(&scene),
                &invalidation(&changes),
            )?;
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            // Full comparison is outside the sample, but deliberately affects
            // subsequent cache/thermal conditions: these are warm CPU timings.
            same(
                &full(session.document(), &font, &LayoutCancellation::default())?,
                &next,
            )?;
            println!(
                "{{\"kind\":\"{end}_edit_layout\",\"sample\":{event},\"operation\":\"{}\",\"ms\":{elapsed:.6},\"cache_hits\":{},\"shape_calls\":{}}}",
                if event % 2 == 0 { "insert" } else { "undo" },
                next.stats().cache_hits,
                next.stats().shape_calls
            );
            scene = next;
        }
        // Keep the first/last cohorts at the same original document state.
        if events % 2 != 0 {
            let changes = session
                .undo()
                .map_err(|error| format!("undo: {error:?}"))?
                .changes;
            scene = incremental(
                session.document(),
                &font,
                &mut cache,
                Some(&scene),
                &invalidation(&changes),
            )?;
        }
    }
    println!(
        "{{\"kind\":\"complete\",\"pages\":{pages},\"scene_equivalence_checks\":{}}}",
        events * 2
    );
    Ok(())
}
