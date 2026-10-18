//! Real-font tests. Set JOYWORD_TEST_FONT=path:face for CJK and
//! JOYWORD_LINE_LATIN_FONT=path:face for Latin/ligatures. The default Latin
//! face is the pinned, OFL-licensed repository fixture. An explicit path
//! failing to load is an error. Missing automatic candidates print SKIPPED;
//! JOYWORD_REQUIRE_TEST_FONT makes either missing font an error.

use std::path::Path;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    BreakError, FeatureSet, LineDiagnostic, LineError, LineLimit, LineLimits, LoadedFont,
    OverflowKind, PageScene, ParagraphLines, layout_paragraph, layout_paragraph_auto, shape,
};

const SIZE: Emu = Emu::new(12 * Emu::PER_POINT);

fn font(latin: bool) -> Option<LoadedFont> {
    let variable = if latin {
        "JOYWORD_LINE_LATIN_FONT"
    } else {
        "JOYWORD_TEST_FONT"
    };
    let candidates = match std::env::var(variable) {
        Ok(spec) => {
            let (path, index) = spec.rsplit_once(':').expect("font spec must be path:face");
            vec![(path.to_owned(), index.parse().expect("numeric face index"))]
        }
        Err(_) if latin => vec![(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/fonts/joyoffice-test-serif.ttf")
                .to_string_lossy()
                .into_owned(),
            0,
        )],
        Err(_) => vec![
            ("/System/Library/Fonts/Supplemental/Songti.ttc".into(), 0),
            (
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
                2,
            ),
            ("C:\\Windows\\Fonts\\simsun.ttc".into(), 0),
        ],
    };
    for (path, face) in candidates {
        if Path::new(&path).is_file() || std::env::var_os(variable).is_some() {
            let bytes: Arc<[u8]> = std::fs::read(&path)
                .expect("readable requested font")
                .into();
            let font = LoadedFont::from_bytes(bytes, face).expect("usable requested font face");
            eprintln!(
                "EXECUTED font {path} face {face} sha256 {}",
                font.key().hash_hex()
            );
            return Some(font);
        }
    }
    assert!(
        !latin && std::env::var_os("JOYWORD_REQUIRE_TEST_FONT").is_none(),
        "required {variable} unavailable"
    );
    eprintln!("SKIPPED real-font test: {variable} unavailable");
    None
}

fn fit(font: &LoadedFont, text: &str, width: Emu, breaks: &[usize]) -> ParagraphLines {
    layout_paragraph(
        font,
        text,
        SIZE,
        FeatureSet::default(),
        width,
        breaks,
        LineLimits::default(),
    )
    .unwrap()
}

fn cluster_breaks(font: &LoadedFont, text: &str, features: FeatureSet) -> Vec<usize> {
    let mut breaks: Vec<usize> = shape(font, text, SIZE, features)
        .glyphs
        .iter()
        .map(|g| g.cluster)
        .filter(|&at| at > 0)
        .collect();
    breaks.dedup();
    breaks
}

fn check_integrity(font: &LoadedFont, text: &str, features: FeatureSet, result: &ParagraphLines) {
    let mut bytes = 0;
    let mut scalars = 0;
    let mut reconstructed = String::new();
    for line in &result.lines {
        assert_eq!(line.source.bytes.start, bytes);
        assert_eq!(line.source.scalars.start, scalars);
        assert_eq!(line.run.text, text[line.source.bytes.clone()]);
        assert_eq!(
            line.run,
            shape(font, &line.run.text, SIZE, features),
            "line must be independently shaped"
        );
        assert_eq!(line.run.font, font.key());
        let placed = line.to_glyph_run(Emu::new(100), Emu::new(200));
        assert_eq!(placed.font, font.key());
        let mut local_glyph = 0;
        let mut cluster_byte = bytes;
        let mut cluster_scalar = scalars;
        for cluster in &line.clusters {
            assert_eq!(cluster.glyphs.start, local_glyph);
            assert_eq!(cluster.source.bytes.start, cluster_byte);
            assert_eq!(cluster.source.scalars.start, cluster_scalar);
            assert_eq!(
                cluster.source.scalars.len(),
                text[cluster.source.bytes.clone()].chars().count()
            );
            for index in cluster.glyphs.clone() {
                let range = placed.cluster_range(index);
                assert_eq!(range.start + bytes..range.end + bytes, cluster.source.bytes);
            }
            local_glyph = cluster.glyphs.end;
            cluster_byte = cluster.source.bytes.end;
            cluster_scalar = cluster.source.scalars.end;
        }
        assert_eq!(local_glyph, line.run.glyphs.len());
        assert_eq!(cluster_byte, line.source.bytes.end);
        assert_eq!(cluster_scalar, line.source.scalars.end);
        if !text.is_empty() {
            assert!(!line.source.bytes.is_empty());
            let remaining = shape(font, &text[bytes..], SIZE, features);
            assert!(
                line.source.bytes.end == text.len()
                    || remaining
                        .glyphs
                        .iter()
                        .any(|g| g.cluster + bytes == line.source.bytes.end)
            );
        }
        bytes = line.source.bytes.end;
        scalars = line.source.scalars.end;
        reconstructed.push_str(&line.run.text);
    }
    assert_eq!(bytes, text.len());
    assert_eq!(scalars, text.chars().count());
    assert_eq!(reconstructed, text);
}

#[test]
fn cjk_breaks_fit_exact_emu_width_and_convert_to_page_scene() {
    let Some(font) = font(false) else { return };
    let text = "甲乙丙丁戊己庚辛";
    let width = shape(&font, "甲乙", SIZE, FeatureSet::default()).width;
    let result = fit(&font, text, width, &[3, 6, 9, 12, 15, 18, 21]);
    assert_eq!(
        result
            .lines
            .iter()
            .map(|l| l.run.text.as_str())
            .collect::<Vec<_>>(),
        ["甲乙", "丙丁", "戊己", "庚辛"]
    );
    assert!(result.diagnostics.is_empty());
    assert!(
        result
            .lines
            .iter()
            .all(|line| line.run.width.raw() <= width.raw())
    );
    check_integrity(&font, text, FeatureSet::default(), &result);
    let scene = PageScene {
        width: Emu::new(210 * Emu::PER_MM),
        height: Emu::new(297 * Emu::PER_MM),
        rects: Vec::new(),
        runs: result
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                line.to_glyph_run(Emu::new(100), Emu::new(200 + index as i64 * SIZE.raw()))
            })
            .collect(),
    };
    assert_eq!(scene.runs.len(), 4);
    assert_eq!(
        scene
            .runs
            .iter()
            .map(|r| r.text.as_str())
            .collect::<String>(),
        text
    );
}

#[test]
fn empty_paragraph_is_one_empty_line_with_font_identity() {
    let Some(font) = font(false) else { return };
    let result = fit(&font, "", Emu::new(1), &[]);
    assert_eq!(result.lines.len(), 1);
    assert_eq!(result.lines[0].run.width, Emu::new(0));
    assert_eq!(result.stats.shape_calls, 1);
    assert!(result.diagnostics.is_empty());
    check_integrity(&font, "", FeatureSet::default(), &result);
}

#[test]
fn continuous_whitespace_combining_marks_and_emoji_never_lose_text() {
    let Some(font) = font(false) else { return };
    for text in [
        "   ",
        "甲  乙   丙 ",
        "甲e\u{301}👩\u{200d}💻乙😀",
        "\u{301}e\u{301}乙",
    ] {
        let breaks = cluster_breaks(&font, text, FeatureSet::default());
        for width in [Emu::new(1), SIZE, Emu::new(SIZE.raw() * 3)] {
            let result = fit(&font, text, width, &breaks);
            check_integrity(&font, text, FeatureSet::default(), &result);
            for line in &result.lines {
                if line.run.width.raw() > width.raw() {
                    assert!(result.diagnostics.iter().any(|d| matches!(d, LineDiagnostic::Overflow { source, .. } if source == &line.source)));
                }
            }
        }
    }
    let error = layout_paragraph(
        &font,
        "e\u{301}",
        SIZE,
        FeatureSet::default(),
        SIZE,
        &[1],
        LineLimits::default(),
    );
    assert_eq!(
        error,
        Err(LineError::InvalidBreak {
            offset: 1,
            reason: BreakError::InsideCluster
        })
    );
}

#[test]
fn no_break_and_single_cluster_overflow_are_distinct_and_preserved() {
    let Some(font) = font(false) else { return };
    for (text, kind) in [
        ("甲", OverflowKind::SingleCluster),
        ("甲乙丙丁", OverflowKind::UnbreakableSpan),
    ] {
        let result = fit(&font, text, Emu::new(1), &[]);
        assert_eq!(result.lines.len(), 1);
        assert!(
            matches!(&result.diagnostics[0], LineDiagnostic::Overflow { kind: actual, .. } if *actual == kind)
        );
        check_integrity(&font, text, FeatureSet::default(), &result);
    }
    // A long unbreakable input still produces one complete, diagnosed line.
    let text = "甲".repeat(2048);
    let result = fit(&font, &text, Emu::new(1), &[]);
    assert_eq!(result.lines.len(), 1);
    check_integrity(&font, &text, FeatureSet::default(), &result);
}

#[test]
fn missing_glyph_diagnostics_use_absolute_source_and_are_bounded() {
    let Some(font) = font(false) else { return };
    let text = "甲\u{f0000}乙\u{f0001}丙";
    let breaks = cluster_breaks(&font, text, FeatureSet::default());
    let result = fit(&font, text, SIZE, &breaks);
    let missing: Vec<_> = result
        .diagnostics
        .iter()
        .filter_map(|d| match d {
            LineDiagnostic::MissingGlyph { source } => Some(source.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(missing.len(), 2);
    assert_eq!(missing[0].bytes, 3..7);
    assert_eq!(missing[0].scalars, 1..2);
    assert_eq!(missing[1].bytes, 10..14);
    check_integrity(&font, text, FeatureSet::default(), &result);
    for cap in [0, 1] {
        let bounded = layout_paragraph(
            &font,
            text,
            SIZE,
            FeatureSet::default(),
            SIZE,
            &breaks,
            LineLimits {
                max_diagnostics: cap,
                ..LineLimits::default()
            },
        )
        .unwrap();
        assert_eq!(bounded.diagnostics.len(), cap);
        assert_eq!(bounded.omitted_diagnostics + cap, result.diagnostics.len());
        assert_eq!(bounded.lines, result.lines);
    }
}

#[test]
fn budget_exhaustion_returns_error_instead_of_a_partial_success() {
    let Some(font) = font(false) else { return };
    for (limits, expected) in [
        (
            LineLimits {
                max_shape_calls: 1,
                ..LineLimits::default()
            },
            LineLimit::ShapeCalls,
        ),
        (
            LineLimits {
                max_shaped_bytes: 6,
                ..LineLimits::default()
            },
            LineLimit::ShapedBytes,
        ),
        (
            LineLimits {
                max_lines: 1,
                ..LineLimits::default()
            },
            LineLimit::Lines,
        ),
        (
            LineLimits {
                max_lines: 0,
                ..LineLimits::default()
            },
            LineLimit::Lines,
        ),
    ] {
        assert_eq!(
            layout_paragraph(
                &font,
                "甲乙",
                SIZE,
                FeatureSet::default(),
                Emu::new(1),
                &[3],
                limits
            ),
            Err(LineError::LimitExceeded(expected))
        );
    }
}

#[test]
fn ligatures_are_atomic_and_every_output_line_is_reshaped() {
    let Some(font) = font(true) else { return };
    let features = FeatureSet {
        kerning: true,
        standard_ligatures: true,
        contextual_alternates: true,
    };
    let text = "fi fi";
    let first = shape(&font, "fi", SIZE, features);
    assert_eq!(
        first.glyphs.len(),
        1,
        "Latin test face must support fi ligature"
    );
    assert_eq!(
        layout_paragraph(
            &font,
            text,
            SIZE,
            features,
            first.width,
            &[1],
            LineLimits::default()
        ),
        Err(LineError::InvalidBreak {
            offset: 1,
            reason: BreakError::InsideCluster
        })
    );
    let result = layout_paragraph(
        &font,
        text,
        SIZE,
        features,
        first.width,
        &[2, 3],
        LineLimits::default(),
    )
    .unwrap();
    assert_eq!(result.lines[0].run, first);
    check_integrity(&font, text, features, &result);
}

#[test]
fn kerning_at_a_line_end_differs_from_a_paragraph_glyph_slice() {
    let Some(font) = font(true) else { return };
    let features = FeatureSet {
        kerning: true,
        ..FeatureSet::default()
    };
    let full = shape(&font, "AV", SIZE, features);
    let first = shape(&font, "A", SIZE, features);
    assert_ne!(
        full.glyphs[0].advance, first.glyphs[0].advance,
        "Latin test face must kern AV"
    );
    let result = layout_paragraph(
        &font,
        "AV",
        SIZE,
        features,
        first.width,
        &[1],
        LineLimits::default(),
    )
    .unwrap();
    assert_eq!(result.lines.len(), 2);
    assert_eq!(result.lines[0].run, first);
    check_integrity(&font, "AV", features, &result);
}

#[test]
fn automatic_cjk_wrap_obeys_default_open_and_close_punctuation_rules() {
    let Some(font) = font(false) else { return };
    let text = "第一条：甲方（负责人）确认。第二条：乙方履行；双方协商！";
    let width = Emu::new(SIZE.raw() * 3);
    let result = layout_paragraph_auto(
        &font,
        text,
        SIZE,
        FeatureSet::default(),
        width,
        LineLimits::default(),
    )
    .unwrap();
    assert!(result.lines.len() > 3);
    for line in &result.lines {
        assert!(!line.run.text.starts_with([
            '，', '。', '！', '？', '；', '：', '、', '）', '】', '》', '」', '』'
        ]));
        assert!(!line.run.text.ends_with(['（', '【', '《', '「', '『']));
    }
    check_integrity(&font, text, FeatureSet::default(), &result);
}

#[test]
fn automatic_nonbreaking_space_and_word_joiner_remain_indivisible() {
    let Some(font) = font(false) else { return };
    let text = "A\u{a0}V A\u{2060}V fi";
    let result = layout_paragraph_auto(
        &font,
        text,
        SIZE,
        FeatureSet::default(),
        Emu::new(1),
        LineLimits::default(),
    )
    .unwrap();
    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.run.text.as_str())
            .collect::<Vec<_>>(),
        ["A\u{a0}V ", "A\u{2060}V ", "fi"]
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| matches!(d, LineDiagnostic::Overflow { .. }))
    );
    check_integrity(&font, text, FeatureSet::default(), &result);
}

#[test]
fn automatic_wrap_keeps_combining_and_emoji_zwj_sequences_whole() {
    let Some(font) = font(false) else { return };
    let text = "甲e\u{301}乙👩‍💻丙";
    let result = layout_paragraph_auto(
        &font,
        text,
        SIZE,
        FeatureSet::default(),
        Emu::new(1),
        LineLimits::default(),
    )
    .unwrap();
    for unit in ["e\u{301}", "👩‍💻"] {
        let start = text.find(unit).unwrap();
        for line in &result.lines {
            assert!(!(start < line.source.bytes.end && line.source.bytes.end < start + unit.len()));
        }
    }
    check_integrity(&font, text, FeatureSet::default(), &result);
}

#[test]
fn automatic_ligatures_preserve_clusters_and_reuse_exact_fitter_statistics() {
    let font = font(true).unwrap();
    let features = FeatureSet {
        standard_ligatures: true,
        kerning: true,
        ..FeatureSet::default()
    };
    let text = "fi AV fi";
    let width = shape(&font, "fi ", SIZE, features).width;
    let automatic =
        layout_paragraph_auto(&font, text, SIZE, features, width, LineLimits::default()).unwrap();
    let explicit = layout_paragraph(
        &font,
        text,
        SIZE,
        features,
        width,
        &[3, 6],
        LineLimits::default(),
    )
    .unwrap();
    assert_eq!(automatic, explicit);
    assert!(
        automatic
            .lines
            .iter()
            .filter(|line| line.run.text.contains("fi"))
            .all(|line| line.run.glyphs.len() < line.run.text.chars().count())
    );
    check_integrity(&font, text, features, &automatic);
}

#[test]
fn automatic_wrap_preserves_repeated_and_edge_whitespace_without_trimming() {
    let font = font(true).unwrap();
    let text = "  AV   fi  AV  ";
    let width = shape(&font, "AV   ", SIZE, FeatureSet::default()).width;
    let result = layout_paragraph_auto(
        &font,
        text,
        SIZE,
        FeatureSet::default(),
        width,
        LineLimits::default(),
    )
    .unwrap();
    assert!(result.lines.len() > 1);
    check_integrity(&font, text, FeatureSet::default(), &result);
}

#[test]
fn automatic_wrap_charges_breakpoint_and_shaping_budgets_without_retry_reset() {
    let font = font(true).unwrap();
    let text = "AV fi AV fi";
    let width = Emu::new(1);
    for (limits, expected) in [
        (
            LineLimits {
                max_text_bytes: 2,
                ..LineLimits::default()
            },
            LineLimit::TextBytes,
        ),
        (
            LineLimits {
                max_breakpoints: 1,
                ..LineLimits::default()
            },
            LineLimit::Breakpoints,
        ),
        (
            LineLimits {
                max_shape_calls: 1,
                ..LineLimits::default()
            },
            LineLimit::ShapeCalls,
        ),
        (
            LineLimits {
                max_shaped_bytes: text.len(),
                ..LineLimits::default()
            },
            LineLimit::ShapedBytes,
        ),
        (
            LineLimits {
                max_lines: 1,
                ..LineLimits::default()
            },
            LineLimit::Lines,
        ),
    ] {
        assert_eq!(
            layout_paragraph_auto(&font, text, SIZE, FeatureSet::default(), width, limits),
            Err(LineError::LimitExceeded(expected))
        );
    }
    // No internal opportunity is required for a single word or empty text.
    let zero_breaks = LineLimits {
        max_breakpoints: 0,
        ..LineLimits::default()
    };
    assert!(
        layout_paragraph_auto(&font, "AV", SIZE, FeatureSet::default(), width, zero_breaks).is_ok()
    );
    assert!(
        layout_paragraph_auto(&font, "", SIZE, FeatureSet::default(), width, zero_breaks).is_ok()
    );
}

#[test]
fn automatic_wrap_keeps_existing_unsupported_input_contract() {
    let font = font(true).unwrap();
    for text in [
        "A\nV",
        "A\tV",
        "A\u{2028}V",
        "A\u{2067}V",
        "A\u{00ad}V",
        "Aא",
    ] {
        assert!(matches!(
            layout_paragraph_auto(
                &font,
                text,
                SIZE,
                FeatureSet::default(),
                SIZE,
                LineLimits::default()
            ),
            Err(LineError::UnsupportedText { .. })
        ));
    }
    assert_eq!(
        layout_paragraph_auto(
            &font,
            "AV",
            SIZE,
            FeatureSet::default(),
            Emu::new(0),
            LineLimits::default()
        ),
        Err(LineError::NonPositiveWidth)
    );
    assert_eq!(
        layout_paragraph_auto(
            &font,
            "AV",
            Emu::new(0),
            FeatureSet::default(),
            SIZE,
            LineLimits::default()
        ),
        Err(LineError::InvalidSize)
    );
}

#[test]
fn automatic_wrap_is_deterministic_and_outputs_every_byte_across_widths() {
    let Some(font) = font(false) else { return };
    let text = "甲方（共同确认）与乙方，中文 English 与组合e\u{301}、emoji🙂。";
    for points in [1, 12, 24, 36, 48, 72, 120, 300] {
        let width = Emu::new(points * Emu::PER_POINT);
        let first = layout_paragraph_auto(
            &font,
            text,
            SIZE,
            FeatureSet::default(),
            width,
            LineLimits::default(),
        )
        .unwrap();
        let second = layout_paragraph_auto(
            &font,
            text,
            SIZE,
            FeatureSet::default(),
            width,
            LineLimits::default(),
        )
        .unwrap();
        assert_eq!(first, second);
        check_integrity(&font, text, FeatureSet::default(), &first);
    }
}
