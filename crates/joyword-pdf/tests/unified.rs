//! T003 tests that need a real CJK font. The font comes from
//! `JOYWORD_TEST_FONT=path:index` or a known system location; without one the
//! tests report a skip, unless `JOYWORD_REQUIRE_TEST_FONT` is set (CI sets it
//! on the job that installs Noto CJK), in which case they fail.

use std::path::Path;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{FeatureSet, GlyphRun, LoadedFont, PageScene, shape};

const SEED: &str = include_str!("../../../tests/fixtures/text/cjk-contract.txt");

fn test_font() -> Option<LoadedFont> {
    let spec = std::env::var("JOYWORD_TEST_FONT").ok();
    let candidates: Vec<(String, u32)> = match spec.as_deref().and_then(|s| s.rsplit_once(':')) {
        Some((path, index)) => vec![(path.to_owned(), index.parse().expect("face index"))],
        None => vec![
            (
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
                2,
            ),
            ("/System/Library/Fonts/Supplemental/Songti.ttc".into(), 0),
            ("C:\\Windows\\Fonts\\simsun.ttc".into(), 0),
        ],
    };
    for (path, index) in candidates {
        if Path::new(&path).is_file() {
            let bytes: Arc<[u8]> = std::fs::read(&path).expect("readable font").into();
            let font = LoadedFont::from_bytes(bytes, index).expect("usable font");
            eprintln!("test font {path} #{index} sha256 {}", font.key().hash_hex());
            return Some(font);
        }
    }
    assert!(
        std::env::var_os("JOYWORD_REQUIRE_TEST_FONT").is_none(),
        "JOYWORD_REQUIRE_TEST_FONT is set but no CJK test font was found"
    );
    eprintln!("skipped: no CJK test font (set JOYWORD_TEST_FONT=path:index)");
    None
}

fn scene(font: &LoadedFont) -> PageScene {
    let size = Emu::new(12 * Emu::PER_POINT);
    let runs = SEED
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let run = shape(font, line, size, FeatureSet::default());
            GlyphRun::place(
                &run,
                Emu::new(72 * Emu::PER_POINT),
                Emu::new((90 + 18 * index as i64) * Emu::PER_POINT),
            )
        })
        .collect();
    PageScene {
        width: Emu::new(210 * Emu::PER_MM),
        height: Emu::new(297 * Emu::PER_MM),
        runs,
        rects: Vec::new(),
        images: Vec::new(),
    }
}

#[test]
fn every_cjk_cluster_gets_a_glyph_and_positions_are_monotonic() {
    let Some(font) = test_font() else { return };
    for line in SEED.lines() {
        let run = shape(
            &font,
            line,
            Emu::new(12 * Emu::PER_POINT),
            FeatureSet::default(),
        );
        assert!(
            run.missing.is_empty(),
            "missing glyphs in {line:?}: {:?}",
            run.missing
        );
        assert_eq!(run.text, line);
        // Every character starts or belongs to a cluster, in order.
        let clusters: Vec<usize> = run.glyphs.iter().map(|glyph| glyph.cluster).collect();
        assert!(
            clusters.windows(2).all(|pair| pair[0] <= pair[1]),
            "{clusters:?}"
        );
        assert_eq!(clusters.first(), Some(&0));
        let xs: Vec<i64> = run.glyphs.iter().map(|glyph| glyph.x.raw()).collect();
        assert!(xs.windows(2).all(|pair| pair[0] <= pair[1]));
        let sum: i64 = run.glyphs.iter().map(|glyph| glyph.advance.raw()).sum();
        assert_eq!(
            sum,
            run.width.raw(),
            "advances add up to the run width exactly"
        );
    }
}

#[test]
fn missing_glyphs_are_reported_without_changing_text() {
    let Some(font) = test_font() else { return };
    // U+F0000 is a supplementary private-use character no CJK font maps.
    let text = "甲\u{F0000}乙";
    let run = shape(
        &font,
        text,
        Emu::new(12 * Emu::PER_POINT),
        FeatureSet::default(),
    );
    assert_eq!(run.missing, vec!["甲".len()]);
    assert_eq!(run.text, text);
}

#[test]
fn pdf_is_deterministic_embeds_a_subset_and_keeps_text() {
    let Some(font) = test_font() else { return };
    let scene = scene(&font);
    let first =
        joyword_pdf::render(std::slice::from_ref(&scene), std::slice::from_ref(&font)).unwrap();
    let second =
        joyword_pdf::render(std::slice::from_ref(&scene), std::slice::from_ref(&font)).unwrap();
    assert_eq!(first, second, "same scene, same bytes");
    assert!(first.starts_with(b"%PDF-"));
    let contains = |needle: &[u8]| first.windows(needle.len()).any(|window| window == needle);
    assert!(contains(b"/ToUnicode"), "text must stay extractable");
    assert!(contains(b"/Subtype/Type0"));
    assert!(
        contains(b"/FontFile2") || contains(b"/FontFile3"),
        "font embedded"
    );
    let font_size = font.data().len();
    assert!(
        first.len() < font_size / 4,
        "subset, not the whole {font_size}-byte font"
    );
}

#[test]
fn a_run_with_an_unknown_font_is_rejected() {
    let Some(font) = test_font() else { return };
    let scene = scene(&font);
    let error = joyword_pdf::render(&[scene], &[]).unwrap_err();
    assert_eq!(error, joyword_pdf::PdfError::MissingFont(font.key()));
}

#[test]
fn document_snapshot_pages_render_deterministically_without_mutation_or_reflow() {
    use joyword_layout::{
        FaceSet, LayoutCancellation, LayoutLimits, SingleFontProfile, layout_document,
    };
    use joyword_model::{BlockSeq, Document, DumpOptions, Measure, PageSetup, Paragraph};
    let Some(font) = test_font() else { return };
    let mut setup = PageSetup::a4_zh_cn();
    setup.width = Measure::Twips(4000);
    setup.height = Measure::Twips(1800);
    setup.top = Measure::Twips(240);
    setup.bottom = setup.top;
    setup.left = setup.top;
    setup.right = setup.top;
    let mut document = Document::new(setup);
    let mut paragraphs = Vec::new();
    for text in SEED.lines().cycle().take(10) {
        let mut p = Paragraph::new(document.ids().node());
        p.push_text(text, None, Arc::default()).unwrap();
        paragraphs.push(p.into());
    }
    let story = document.body_id();
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(paragraphs);
    let before = document.canonical_dump(DumpOptions { node_ids: true });
    let result = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font),
        SingleFontProfile {
            size: Emu::new(12 * Emu::PER_POINT),
            leading: Emu::new(18 * Emu::PER_POINT),
            baseline: Emu::new(14 * Emu::PER_POINT),
            features: FeatureSet::default(),
        },
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    assert!(result.pages().len() >= 4);
    assert!(result.diagnostics().is_empty());
    let scenes = result.pages().to_vec();
    let first = joyword_pdf::render(result.pages(), std::slice::from_ref(&font)).unwrap();
    let second = joyword_pdf::render(result.pages(), std::slice::from_ref(&font)).unwrap();
    assert_eq!(first, second);
    assert_eq!(result.pages(), scenes);
    assert_eq!(
        document.canonical_dump(DumpOptions { node_ids: true }),
        before
    );
    assert_eq!(
        result
            .pages()
            .iter()
            .flat_map(|p| &p.runs)
            .map(|r| r.text.as_str())
            .collect::<String>(),
        SEED.lines().cycle().take(10).collect::<String>()
    );
}
