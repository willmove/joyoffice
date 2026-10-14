//! Usage: line_scene <font-file> <face-index>
//! An explicit-space-break example, preserving all spaces. The fixed page
//! and 18pt baseline spacing are illustrative, not Word pagination rules.

use std::error::Error;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{FeatureSet, LineLimits, LoadedFont, PageScene, layout_paragraph};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: line_scene <font-file> <face-index>".into());
    }
    let bytes: Arc<[u8]> = std::fs::read(&args[1])?.into();
    let face = args[2].parse::<u32>()?;
    let font = LoadedFont::from_bytes(bytes, face).map_err(|e| format!("font: {e:?}"))?;
    let text = "JoyOffice  preserves text and shapes each candidate line again.";
    // The caller supplies opportunities after ASCII spaces, not a UAX #14
    // implementation. No trimming, hyphenation or implicit emergency breaks.
    let breaks: Vec<usize> = text
        .char_indices()
        .filter_map(|(at, ch)| (ch == ' ').then_some(at + 1))
        .collect();
    let result = layout_paragraph(
        &font,
        text,
        Emu::new(12 * Emu::PER_POINT),
        FeatureSet::default(),
        Emu::new(120 * Emu::PER_POINT),
        &breaks,
        LineLimits::default(),
    )
    .map_err(|e| format!("layout: {e:?}"))?;
    let scene = PageScene {
        width: Emu::new(210 * Emu::PER_MM),
        height: Emu::new(297 * Emu::PER_MM),
        runs: result
            .lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                line.to_glyph_run(
                    Emu::new(72 * Emu::PER_POINT),
                    Emu::new((90 + i as i64 * 18) * Emu::PER_POINT),
                )
            })
            .collect(),
    };
    println!(
        "font {} face {} sha256 {}; {} scene runs; diagnostics {:?}, omitted {}; stats {:?}",
        args[1],
        face,
        font.key().hash_hex(),
        scene.runs.len(),
        result.diagnostics,
        result.omitted_diagnostics,
        result.stats
    );
    for line in &result.lines {
        println!(
            "bytes {:?}, scalars {:?}, width {} EMU: {:?}",
            line.source.bytes,
            line.source.scalars,
            line.run.width.raw(),
            line.run.text
        );
    }
    Ok(())
}
