//! T504: Advanced CJK Layout & GB/T 9704 Official Document Verification.
//!
//! Validates:
//! 1. GB/T 9704-2012 document layout geometry, margins, linesAndChars grid (22 lines x 28 chars).
//! 2. Advanced CJK inline objects: Ruby (phonetic guide), CombineCharacters (two-lines-in-one),
//!    Emphasis marks, and Enclosed/circled characters.
//! 3. Manuscript paper (稿纸) specs (400 words / 500 words).
//! 4. BiDi and complex script diagnostics under PLATFORM-004 / LAYOUT-014 fidelity rules.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::cjk::{
    ComplexScriptDiagnostic, Gbt9704Issue, Gbt9704Spec, ManuscriptPaperSpec,
    create_gbt9704_document_sample, scan_complex_scripts, verify_gbt9704_section,
};
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, SingleFontProfile,
    layout_document,
};
use joyword_model::{
    CombineBrackets, CombineCharacters, DocGrid, DocGridKind, DumpOptions, EmphasisMark,
    EncloseShape, EnclosedCharacter, InlineKind, InlineObject, Measure, PageSetup, Paragraph, Ruby,
    RubyAlign,
};

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    }
}

fn test_font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap()
}

#[test]
fn test_gbt9704_spec_and_verification() {
    let doc = create_gbt9704_document_sample();
    let section = &doc.sections()[0];

    // Verify compliance
    assert!(verify_gbt9704_section(section).is_ok());

    // Verify grid parameters
    let grid = section.doc_grid.as_ref().unwrap();
    assert_eq!(grid.kind, Some(DocGridKind::LinesAndChars));
    assert_eq!(grid.line_pitch, Some(Gbt9704Spec::LINE_PITCH_TWIPS));
    assert_eq!(grid.char_space, Some(Gbt9704Spec::CHAR_PITCH_TWIPS));

    // Verify print area calculation: 210mm - 28mm - 26mm = 156mm
    assert_eq!(Gbt9704Spec::PRINT_AREA_WIDTH_TWIPS, 8844);
    // 297mm - 37mm - 35mm = 225mm
    assert_eq!(Gbt9704Spec::PRINT_AREA_HEIGHT_TWIPS, 12756);
    assert_eq!(Gbt9704Spec::LINES_PER_PAGE, 22);
    assert_eq!(Gbt9704Spec::CHARS_PER_LINE, 28);

    // Negative check 1: bad margins
    let mut bad_margin = section.clone();
    bad_margin.page.top = Measure::Twips(1000);
    let err = verify_gbt9704_section(&bad_margin).unwrap_err();
    assert!(matches!(
        err,
        Gbt9704Issue::MarginMismatch { edge: "top", .. }
    ));

    // Negative check 2: missing grid
    let mut no_grid = section.clone();
    no_grid.doc_grid = None;
    let err = verify_gbt9704_section(&no_grid).unwrap_err();
    assert_eq!(err, Gbt9704Issue::GridMissing);

    // Negative check 3: wrong grid kind
    let mut wrong_grid = section.clone();
    wrong_grid.doc_grid = Some(DocGrid {
        kind: Some(DocGridKind::Lines),
        line_pitch: Some(580),
        char_space: None,
    });
    let err = verify_gbt9704_section(&wrong_grid).unwrap_err();
    assert_eq!(err, Gbt9704Issue::GridKindMismatch);
}

#[test]
fn test_cjk_inline_structures_and_dump() {
    let mut doc = joyword_model::Document::new(PageSetup::a4_gbt9704());
    let mut p = Paragraph::new(doc.ids().node());

    // 1. Ruby
    p.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Ruby(Ruby {
                base: "文字".to_string(),
                text: "wén zì".to_string(),
                align: RubyAlign::Center,
                font_size_hps: Some(16),
                raise_hps: Some(8),
            }),
            source: None,
        },
        None,
        Arc::default(),
    );

    // 2. CombineCharacters
    p.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Combine(CombineCharacters {
                text: "双行合一".to_string(),
                brackets: CombineBrackets::Round,
            }),
            source: None,
        },
        None,
        Arc::default(),
    );

    // 3. EmphasisMark
    p.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Emphasis {
                text: "着重内容".to_string(),
                mark: EmphasisMark::UnderDot,
            },
            source: None,
        },
        None,
        Arc::default(),
    );

    // 4. EnclosedCharacter
    p.push_inline(
        InlineObject {
            id: doc.ids().node(),
            kind: InlineKind::Enclosed(EnclosedCharacter {
                text: "甲".to_string(),
                shape: EncloseShape::Square,
            }),
            source: None,
        },
        None,
        Arc::default(),
    );

    let body = doc.body_id();
    doc.story_mut(body).unwrap().blocks = joyword_model::BlockSeq::from_vec(vec![p.into()]);

    let dump = doc.canonical_dump(DumpOptions { node_ids: false });
    assert!(dump.contains("ruby:文字[wén zì]"));
    assert!(dump.contains("combine:Round:双行合一"));
    assert!(dump.contains("emphasis:UnderDot:着重内容"));
    assert!(dump.contains("enclosed:Square:甲"));
}

#[test]
fn test_manuscript_paper_specs() {
    let spec400 = ManuscriptPaperSpec::standard_400();
    assert_eq!(spec400.rows, 20);
    assert_eq!(spec400.cols, 20);
    assert_eq!(spec400.rows * spec400.cols, 400);

    let spec500 = ManuscriptPaperSpec::standard_500();
    assert_eq!(spec500.rows, 20);
    assert_eq!(spec500.cols, 25);
    assert_eq!(spec500.rows * spec500.cols, 500);

    let grid = DocGrid::manuscript_20x20();
    assert_eq!(grid.kind, Some(DocGridKind::LinesAndChars));
    assert_eq!(grid.line_pitch, Some(600));
    assert_eq!(grid.char_space, Some(440));
}

#[test]
fn test_complex_scripts_and_bidi_diagnostics() {
    let standard_zh = "北京市人民政府关于加快数字经济标杆城市建设的通知";
    let diag_zh = scan_complex_scripts(standard_zh);
    assert!(
        diag_zh.is_empty(),
        "Standard Chinese text should produce no complex script diagnostics"
    );

    let mixed_arabic = "JoyWord 文档 مرحبا 2026";
    let diag_ar = scan_complex_scripts(mixed_arabic);
    assert_eq!(diag_ar.len(), 1);
    match &diag_ar[0] {
        ComplexScriptDiagnostic::BiDiRtlPreservedWithLimitedLayout { script, range } => {
            assert_eq!(*script, "Arabic/Hebrew BiDi");
            assert_eq!(&mixed_arabic[range.clone()], "مرحبا");
        }
        _ => panic!("Expected BiDiRtlPreservedWithLimitedLayout"),
    }

    let mixed_indic = "公文附件 नमस्ते 规格";
    let diag_indic = scan_complex_scripts(mixed_indic);
    assert_eq!(diag_indic.len(), 1);
    match &diag_indic[0] {
        ComplexScriptDiagnostic::ComplexIndicPreservedWithLimitedLayout { script, range } => {
            assert_eq!(*script, "Indic/Thai Complex");
            assert_eq!(&mixed_indic[range.clone()], "नमस्ते");
        }
        _ => panic!("Expected ComplexIndicPreservedWithLimitedLayout"),
    }
}

#[test]
fn test_gbt9704_document_sample_layout() {
    let doc = create_gbt9704_document_sample();
    let font = test_font();

    let snapshot = layout_document(
        &doc.snapshot(),
        FaceSet::uniform(&font),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();

    assert!(!snapshot.pages().is_empty());
    let first_page = &snapshot.pages()[0];
    assert_eq!(
        first_page.width,
        Emu::from_twips(Gbt9704Spec::PAGE_WIDTH_TWIPS).unwrap()
    );
    assert_eq!(
        first_page.height,
        Emu::from_twips(Gbt9704Spec::PAGE_HEIGHT_TWIPS).unwrap()
    );
}
