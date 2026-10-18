//! T003 unified-scene check. Shapes the original contract seed with one font
//! file, places it on an A4 page scene, writes `scene.pdf` and the expected
//! geometry `expected.json`; `scripts/verify_unified_pdf.py` then reads the
//! PDF back with an independent tool and compares text and positions.
//!
//! Usage: unified_scene <font-file> <face-index> <out-dir> [split-third-line]
//!
//! `split-third-line` breaks the third line after ten characters, giving
//! `scripts/compare_pdf_lines.py` a known line-break difference to detect.
//!
//! Line spacing here is a fixed 1.5 × size, not Word's rule; the check is
//! about one scene driving the PDF exactly, not about Word pagination.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{FeatureSet, GlyphRun, LoadedFont, PageScene, shape, to_points};

const SEED: &str = include_str!("../../../tests/fixtures/text/cjk-contract.txt");

fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (font_path, index, out_dir, split) = match args.as_slice() {
        [_, font, index, out] => (font, index, out, false),
        [_, font, index, out, variant] if variant == "split-third-line" => (font, index, out, true),
        _ => {
            eprintln!("usage: unified_scene <font-file> <face-index> <out-dir> [split-third-line]");
            return ExitCode::from(2);
        }
    };
    let Ok(index) = index.parse::<u32>() else {
        eprintln!("face index must be a number");
        return ExitCode::from(2);
    };
    let bytes: Arc<[u8]> = match std::fs::read(font_path) {
        Ok(bytes) => bytes.into(),
        Err(error) => {
            eprintln!("cannot read {font_path}: {error}");
            return ExitCode::from(1);
        }
    };
    let font = match LoadedFont::from_bytes(bytes, index) {
        Ok(font) => font,
        Err(error) => {
            eprintln!("unusable font: {error:?}");
            return ExitCode::from(1);
        }
    };

    let size = Emu::new(12 * Emu::PER_POINT);
    let margin = Emu::new(72 * Emu::PER_POINT);
    let leading = Emu::new(18 * Emu::PER_POINT);
    let mut scene = PageScene {
        width: Emu::new(210 * Emu::PER_MM),
        height: Emu::new(297 * Emu::PER_MM),
        runs: Vec::new(),
        rects: Vec::new(),
    };
    let mut lines_json = Vec::new();
    let mut missing = 0;
    let mut lines: Vec<String> = SEED.lines().map(str::to_owned).collect();
    if split {
        let third = lines.remove(2);
        let cut = third
            .char_indices()
            .nth(10)
            .map_or(third.len(), |(at, _)| at);
        lines.insert(2, third[cut..].to_owned());
        lines.insert(2, third[..cut].to_owned());
    }
    for (line_index, line) in lines.iter().map(String::as_str).enumerate() {
        let run = shape(&font, line, size, FeatureSet::default());
        missing += run.missing.len();
        let baseline = Emu::new(margin.raw() + leading.raw() * (line_index as i64 + 1));
        let placed = GlyphRun::place(&run, margin, baseline);
        let glyphs: Vec<String> = (0..placed.glyphs.len())
            .map(|i| {
                format!(
                    "[{},{:.4}]",
                    placed.glyphs[i].cluster,
                    to_points(placed.glyph_x(i))
                )
            })
            .collect();
        lines_json.push(format!(
            "{{\"text\":{},\"x_pt\":{:.4},\"baseline_pt\":{:.4},\"width_pt\":{:.4},\"glyphs\":[{}]}}",
            json_string(line),
            to_points(margin),
            to_points(baseline),
            to_points(run.width),
            glyphs.join(",")
        ));
        scene.runs.push(placed);
    }

    let pdf = match joyword_pdf::render(std::slice::from_ref(&scene), std::slice::from_ref(&font)) {
        Ok(pdf) => pdf,
        Err(error) => {
            eprintln!("PDF output failed: {error:?}");
            return ExitCode::from(1);
        }
    };
    let out = PathBuf::from(out_dir);
    if let Err(error) = std::fs::create_dir_all(&out)
        .and_then(|()| std::fs::write(out.join("scene.pdf"), &pdf))
        .and_then(|()| {
            let json = format!(
                "{{\"font\":{},\"font_sha256\":\"{}\",\"face\":{},\"size_pt\":{:.4},\"missing_glyphs\":{},\"lines\":[\n{}\n]}}\n",
                json_string(font_path),
                font.key().hash_hex(),
                index,
                to_points(size),
                missing,
                lines_json.join(",\n")
            );
            std::fs::write(out.join("expected.json"), json)
        })
    {
        eprintln!("cannot write output: {error}");
        return ExitCode::from(1);
    }
    println!(
        "font {} face {} sha256 {}; {} lines, {} missing glyphs; PDF {} bytes",
        font_path,
        index,
        font.key().hash_hex(),
        scene.runs.len(),
        missing,
        pdf.len()
    );
    ExitCode::SUCCESS
}
