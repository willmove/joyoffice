//! T208: incremental layout matches a from-scratch layout, including the T207
//! paths (quote compression, both-justification, single-spacing grid).

use std::sync::Arc;

use joyword_engine::{Command, Outcome, ParagraphChange, Session, Transaction};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutInvalidation, LayoutLimits,
    LayoutReuse, LayoutSnapshot, LoadedFont, ParagraphLayoutCache, SingleFontProfile,
    layout_document, layout_incremental,
};
use joyword_model::{
    Bias, BlockSeq, Document, Justification, LineSpacing, Measure, PageSetup, Paragraph, Position,
    ScalarOffset, Spacing, StoryId,
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

fn be_u16(data: &[u8], offset: usize) -> usize {
    usize::from(u16::from_be_bytes([data[offset], data[offset + 1]]))
}

fn font_named(family: &str) -> LoadedFont {
    let mut data =
        include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").to_vec();
    let tables = be_u16(&data, 4);
    let mut name_at = None;
    for index in 0..tables {
        let entry = 12 + index * 16;
        if &data[entry..entry + 4] == b"name" {
            name_at = Some(
                usize::try_from(u32::from_be_bytes(
                    data[entry + 8..entry + 12].try_into().unwrap(),
                ))
                .unwrap(),
            );
        }
    }
    let name_at = name_at.expect("name table");
    let records = be_u16(&data, name_at + 2);
    let storage = name_at + be_u16(&data, name_at + 4);
    let encoded: Vec<u8> = family.encode_utf16().flat_map(u16::to_be_bytes).collect();
    let mut patched = false;
    for index in 0..records {
        let record = name_at + 6 + index * 12;
        let platform = be_u16(&data, record);
        let name_id = be_u16(&data, record + 6);
        let length = be_u16(&data, record + 8);
        let offset = be_u16(&data, record + 10);
        if platform == 3 && name_id == 1 && length >= encoded.len() {
            let start = storage + offset;
            data[start..start + encoded.len()].copy_from_slice(&encoded);
            let len = u16::try_from(encoded.len()).unwrap();
            data[record + 8..record + 10].copy_from_slice(&len.to_be_bytes());
            patched = true;
            break;
        }
    }
    assert!(patched, "Windows family name");
    LoadedFont::from_bytes(Arc::from(data), 0).unwrap()
}

fn paragraph(document: &mut Document, text: &str) -> Paragraph {
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text(text, None, Arc::default())
        .expect("text");
    paragraph
}

fn install(document: &mut Document, paragraphs: Vec<Paragraph>) {
    let story = document.body_id();
    document.story_mut(story).expect("body").blocks = BlockSeq::from_vec(
        paragraphs
            .into_iter()
            .map(joyword_model::Block::from)
            .collect(),
    );
}

fn assert_same(full: &LayoutSnapshot, incremental: &LayoutSnapshot, step: &str) {
    assert_eq!(full.pages(), incremental.pages(), "{step} pages");
    assert_eq!(full.lines(), incremental.lines(), "{step} lines");
    assert_eq!(
        full.diagnostics(),
        incremental.diagnostics(),
        "{step} diagnostics"
    );
    assert_eq!(
        full.omitted_diagnostics(),
        incremental.omitted_diagnostics(),
        "{step} omitted diagnostics"
    );
    assert_eq!(full.geometry(), incremental.geometry(), "{step} geometry");
    assert_eq!(
        full.stats().glyphs,
        incremental.stats().glyphs,
        "{step} glyphs"
    );
}

fn lay_full(document: &Document, font: &LoadedFont) -> Result<LayoutSnapshot, LayoutError> {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(font),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
}

fn lay_incremental(
    document: &Document,
    font: &LoadedFont,
    cache: &mut ParagraphLayoutCache,
    previous: Option<&LayoutSnapshot>,
    invalidation: &LayoutInvalidation,
) -> Result<LayoutSnapshot, LayoutError> {
    layout_incremental(
        &document.snapshot(),
        FaceSet::uniform(font),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        LayoutReuse {
            cache,
            previous,
            invalidation,
        },
    )
}

fn invalidation_of(changes: &joyword_engine::ChangeSet) -> LayoutInvalidation {
    LayoutInvalidation {
        paragraphs: changes.paragraphs.clone(),
        inserted: changes.inserted.clone(),
        removed: changes.removed.clone(),
        moved: changes.moved.clone(),
        styles: changes.styles.clone(),
        defaults_changed: changes.defaults_changed,
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() as usize) % n
        }
    }
}

fn ids(document: &Document) -> Vec<joyword_model::NodeId> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph().map(|paragraph| paragraph.id()))
        .collect()
}

fn at(story: StoryId, paragraph: joyword_model::NodeId, offset: usize) -> Position {
    Position {
        story,
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::After,
    }
}

fn random_command(document: &Document, rng: &mut Rng) -> Option<Command> {
    let paragraphs = ids(document);
    let story = document.body_id();
    let index = rng.below(paragraphs.len());
    let id = paragraphs[index];
    let len = document
        .body()
        .blocks
        .iter()
        .find_map(|block| {
            block
                .as_paragraph()
                .filter(|paragraph| paragraph.id() == id)
                .map(|paragraph| paragraph.len())
        })
        .unwrap_or(0);
    let samples = ["“", "”", "中", "文", "排", "版", "A", " ", "好", "，"];
    match rng.below(6) {
        0 => Some(Command::InsertText {
            at: at(story, id, rng.below(len + 1)),
            text: samples[rng.below(samples.len())].into(),
        }),
        1 if len > 0 => {
            let start = rng.below(len);
            let end = start + 1 + rng.below(len - start);
            Some(Command::DeleteRange {
                start: at(story, id, start),
                end: at(story, id, end),
            })
        }
        2 => Some(Command::SplitParagraph {
            at: at(story, id, rng.below(len + 1)),
        }),
        3 => Some(Command::FormatParagraph {
            story,
            paragraph: id,
            change: ParagraphChange::Justification(match rng.below(4) {
                0 => None,
                1 => Some(Justification::Both),
                2 => Some(Justification::Center),
                _ => Some(Justification::End),
            }),
        }),
        4 => Some(Command::FormatParagraph {
            story,
            paragraph: id,
            change: ParagraphChange::Spacing(Spacing {
                before: (rng.below(3) == 0).then_some(Measure::Twips(120)),
                after: (rng.below(3) == 0).then_some(Measure::Twips(80)),
                line: (rng.below(2) == 0).then_some(LineSpacing::Auto(240)),
                ..Spacing::default()
            }),
        }),
        5 if paragraphs.len() > 1 => Some(Command::RemoveBlock { story, block: id }),
        _ => None,
    }
}

fn cjk_document() -> (Document, LoadedFont) {
    let font = font_named("等线");
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let texts = [
        "“你好，世界。”这是一段需要压缩引号的中文。",
        "中文两端对齐测试文字用于分配行尾余量",
        "单倍行距网格吸附",
        "Hello 世界混合",
        "“第二处引号”仍然走压缩",
        "末段保持较短",
    ];
    let mut paragraphs = Vec::new();
    for (index, text) in texts.iter().enumerate() {
        let mut paragraph = paragraph(&mut document, text);
        let props = Arc::make_mut(&mut paragraph.props);
        props.spacing.line = Some(LineSpacing::Auto(240));
        if index == 0 || index == 1 {
            props.justification = Some(Justification::Both);
        }
        if index == 2 {
            props.keep_next = Some(true);
        }
        if index == 3 {
            props.spacing.before = Some(Measure::Twips(200));
        }
        paragraphs.push(paragraph);
    }
    install(&mut document, paragraphs);
    (document, font)
}

#[test]
fn cached_layout_matches_full_layout_across_random_edits() {
    let (document, font) = cjk_document();
    for seed in [1u64, 7, 19, 42] {
        let mut session = Session::new(document.clone());
        let mut cache = ParagraphLayoutCache::default();
        let mut previous = lay_incremental(
            session.document(),
            &font,
            &mut cache,
            None,
            &LayoutInvalidation::default(),
        )
        .expect("initial incremental");
        assert_same(
            &lay_full(session.document(), &font).expect("initial full"),
            &previous,
            "initial",
        );
        let mut rng = Rng(seed);
        let mut applied = 0usize;
        for step in 0..36 {
            let Some(command) = random_command(session.document(), &mut rng) else {
                continue;
            };
            let outcome = session.apply(Transaction::new(session.version(), vec![command]));
            let Ok(Outcome::Applied(result)) = outcome else {
                continue;
            };
            applied += 1;
            let invalidation = invalidation_of(&result.changes);
            let incremental = lay_incremental(
                session.document(),
                &font,
                &mut cache,
                Some(&previous),
                &invalidation,
            );
            let full = lay_full(session.document(), &font);
            match (full, incremental) {
                (Ok(full), Ok(incremental)) => {
                    assert_same(&full, &incremental, &format!("seed {seed} step {step}"));
                    previous = incremental;
                }
                (Err(full), Err(incremental)) => assert_eq!(full, incremental),
                (Ok(_), Err(error)) => panic!("seed {seed} step {step}: incremental {error:?}"),
                (Err(error), Ok(_)) => panic!("seed {seed} step {step}: full {error:?}"),
            }
        }
        assert!(applied >= 8, "seed {seed} applied only {applied}");
    }
}

#[test]
fn editing_the_last_paragraph_shapes_fewer_runs_than_a_full_layout() {
    let font = font_named("等线");
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let mut paragraphs = Vec::new();
    for index in 0..24 {
        let mut paragraph = paragraph(
            &mut document,
            &format!("第{index}段：“中文排版”保持单倍行距。"),
        );
        Arc::make_mut(&mut paragraph.props).spacing.line = Some(LineSpacing::Auto(240));
        paragraphs.push(paragraph);
    }
    install(&mut document, paragraphs);
    let mut cache = ParagraphLayoutCache::default();
    let warm = lay_incremental(
        &document,
        &font,
        &mut cache,
        None,
        &LayoutInvalidation::default(),
    )
    .unwrap();
    let full_warm = lay_full(&document, &font).unwrap();
    assert_same(&full_warm, &warm, "warm");
    assert_eq!(warm.stats().shape_calls, full_warm.stats().shape_calls);

    let again = lay_incremental(
        &document,
        &font,
        &mut cache,
        Some(&warm),
        &LayoutInvalidation::default(),
    )
    .unwrap();
    assert_same(&full_warm, &again, "unchanged");
    assert_eq!(again.stats().shape_calls, 0);
    assert!(again.stats().cache_hits >= 24);

    let last = ids(&document).pop().unwrap();
    let mut session = Session::new(document);
    let outcome = session
        .apply(Transaction::new(
            session.version(),
            vec![Command::InsertText {
                at: at(session.document().body_id(), last, 0),
                text: "增".into(),
            }],
        ))
        .unwrap();
    let Outcome::Applied(result) = outcome else {
        panic!("insert changed the last paragraph");
    };
    let full = lay_full(session.document(), &font).unwrap();
    let incremental = lay_incremental(
        session.document(),
        &font,
        &mut cache,
        Some(&again),
        &invalidation_of(&result.changes),
    )
    .unwrap();
    assert_same(&full, &incremental, "last paragraph");
    assert!(incremental.stats().shape_calls > 0);
    assert!(
        incremental.stats().shape_calls * 4 < full.stats().shape_calls,
        "incremental {} vs full {}",
        incremental.stats().shape_calls,
        full.stats().shape_calls
    );
}

#[test]
fn editing_the_first_paragraph_propagates_and_matches() {
    let font = font_named("等线");
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let mut paragraphs = Vec::new();
    for index in 0..12 {
        let mut paragraph = paragraph(&mut document, &format!("段{index}"));
        let props = Arc::make_mut(&mut paragraph.props);
        props.spacing.line = Some(LineSpacing::Auto(240));
        if index == 0 {
            props.keep_next = Some(true);
        }
        paragraphs.push(paragraph);
    }
    install(&mut document, paragraphs);
    let mut cache = ParagraphLayoutCache::default();
    let warm = lay_incremental(
        &document,
        &font,
        &mut cache,
        None,
        &LayoutInvalidation::default(),
    )
    .unwrap();
    let first = ids(&document)[0];
    let mut session = Session::new(document);
    let long = "甲".repeat(400);
    let outcome = session
        .apply(Transaction::new(
            session.version(),
            vec![Command::InsertText {
                at: at(session.document().body_id(), first, 0),
                text: long,
            }],
        ))
        .unwrap();
    let Outcome::Applied(result) = outcome else {
        panic!("insert changed the first paragraph");
    };
    let full = lay_full(session.document(), &font).unwrap();
    let incremental = lay_incremental(
        session.document(),
        &font,
        &mut cache,
        Some(&warm),
        &invalidation_of(&result.changes),
    )
    .unwrap();
    assert_same(&full, &incremental, "first paragraph");
    assert!(full.pages().len() >= warm.pages().len());
}
