//! Opaque body blocks take no geometry. Full and incremental layout agree,
//! including after an edit and when a table forces the full path.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutInvalidation, LayoutLimits,
    LayoutReuse, LayoutSnapshot, LoadedFont, ParagraphLayoutCache, SingleFontProfile,
    layout_document, layout_incremental,
};
use joyword_model::{
    Block, BlockSeq, BorderEdge, BorderKind, Cell, CellBorders, Color, Document, Measure,
    OpaqueBlock, PageSetup, Paragraph, Preserved, Row, Sdt, SdtProps, Shading, Table,
    VerticalMerge,
};

fn font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap()
}

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    }
}

fn text_block(document: &mut Document, text: &str) -> Block {
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text(text, None, Arc::default())
        .expect("text");
    paragraph.into()
}

fn opaque_block(document: &mut Document, name: &str) -> Block {
    OpaqueBlock {
        id: document.ids().node(),
        preserved: Preserved {
            element: Arc::from(name),
            handle: document.ids().preserved(),
        },
        source: None,
    }
    .into()
}

fn install(document: &mut Document, blocks: Vec<Block>) {
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
}

fn lay(document: &Document) -> LayoutSnapshot {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .expect("layout")
}

fn lay_incremental(
    document: &Document,
    cache: &mut ParagraphLayoutCache,
    previous: Option<&LayoutSnapshot>,
    invalidation: &LayoutInvalidation,
) -> LayoutSnapshot {
    layout_incremental(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        LayoutReuse {
            cache,
            previous,
            invalidation,
        },
    )
    .expect("incremental")
}

fn assert_same(full: &LayoutSnapshot, incremental: &LayoutSnapshot) {
    assert_eq!(full.pages(), incremental.pages());
    assert_eq!(full.lines(), incremental.lines());
    assert_eq!(full.omitted_blocks(), incremental.omitted_blocks());
    assert_eq!(
        full.omitted_diagnostics(),
        incremental.omitted_diagnostics()
    );
    assert_eq!(full.key().content_hash(), incremental.key().content_hash());
}

fn mixed() -> Document {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let blocks = vec![
        text_block(&mut document, "Alpha"),
        opaque_block(&mut document, "jw:annotation"),
        text_block(&mut document, "Beta"),
    ];
    install(&mut document, blocks);
    document
}

#[test]
fn opaque_blocks_are_skipped_counted_and_fingerprinted() {
    let mut plain = Document::new(PageSetup::a4_zh_cn());
    let plain_blocks = vec![
        text_block(&mut plain, "Alpha"),
        text_block(&mut plain, "Beta"),
    ];
    install(&mut plain, plain_blocks);
    let mut with_opaque = Document::new(PageSetup::a4_zh_cn());
    let mut opaque_blocks = vec![
        text_block(&mut with_opaque, "Alpha"),
        text_block(&mut with_opaque, "Beta"),
    ];
    opaque_blocks.push(opaque_block(&mut with_opaque, "jw:annotation"));
    install(&mut with_opaque, opaque_blocks);

    let plain_layout = lay(&plain);
    let opaque_layout = lay(&with_opaque);
    assert_eq!(plain_layout.omitted_blocks(), 0);
    assert_eq!(opaque_layout.omitted_blocks(), 1);
    assert_eq!(plain_layout.lines().len(), opaque_layout.lines().len());
    for (left, right) in plain_layout.lines().iter().zip(opaque_layout.lines()) {
        assert_eq!(left.top, right.top);
        assert_eq!(left.page, right.page);
    }
    assert_ne!(
        plain_layout.key().content_hash(),
        opaque_layout.key().content_hash()
    );

    let mut only = Document::new(PageSetup::a4_zh_cn());
    let only_block = opaque_block(&mut only, "jw:annotation");
    install(&mut only, vec![only_block]);
    let only_layout = lay(&only);
    assert_eq!(only_layout.omitted_blocks(), 1);
    assert!(only_layout.lines().is_empty());
    assert_eq!(only_layout.pages().len(), 1);
}

#[test]
fn incremental_layout_matches_full_around_opaque_blocks() {
    let mut document = mixed();
    let full = lay(&document);
    let mut cache = ParagraphLayoutCache::default();
    let first = lay_incremental(&document, &mut cache, None, &LayoutInvalidation::default());
    assert_same(&full, &first);
    assert_eq!(first.omitted_blocks(), 1);
    let second = lay_incremental(
        &document,
        &mut cache,
        Some(&first),
        &LayoutInvalidation::default(),
    );
    assert_same(&full, &second);
    assert_eq!(second.stats().stable_boundary, Some(0));

    let body = document.body_id();
    let block = document.story_mut(body).unwrap().blocks.get_mut(0).unwrap();
    let Block::Paragraph(paragraph) = block else {
        panic!("first block is a paragraph");
    };
    let id = paragraph.id();
    Arc::make_mut(paragraph)
        .push_text(" more", None, Arc::default())
        .unwrap();
    let invalidation = LayoutInvalidation {
        paragraphs: std::iter::once(id).collect(),
        ..LayoutInvalidation::default()
    };
    let edited = lay(&document);
    let incremental = lay_incremental(&document, &mut cache, Some(&second), &invalidation);
    assert_same(&edited, &incremental);
    assert_eq!(edited.omitted_blocks(), 1);
    assert_ne!(full.key().content_hash(), edited.key().content_hash());
    assert!(
        edited.pages()[0]
            .runs
            .iter()
            .any(|run| run.text.contains("Alpha more"))
    );
}

#[test]
fn section_break_after_an_opaque_block_uses_the_body_index() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let first = text_block(&mut document, "Alpha");
    let marker = opaque_block(&mut document, "jw:annotation");
    let second = text_block(&mut document, "Beta");
    let boundary = marker.id();
    install(&mut document, vec![first, marker, second]);
    let mut next = document.sections()[0].clone();
    document.sections_mut()[0].last_block = Some(boundary);
    next.id = document.ids().node();
    next.last_block = None;
    document.sections_mut().push(next);

    let full = lay(&document);
    assert_eq!(full.omitted_blocks(), 1);
    assert_eq!(full.lines().len(), 2);
    assert_eq!(full.lines()[0].page, 0);
    assert_eq!(full.lines()[1].page, 1);
    let mut cache = ParagraphLayoutCache::default();
    let incremental = lay_incremental(&document, &mut cache, None, &LayoutInvalidation::default());
    assert_same(&full, &incremental);
    let again = lay_incremental(
        &document,
        &mut cache,
        Some(&incremental),
        &LayoutInvalidation::default(),
    );
    assert_same(&full, &again);
}

#[test]
fn a_table_and_an_opaque_block_still_match_full_layout() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let intro = text_block(&mut document, "Before");
    let marker = opaque_block(&mut document, "jw:annotation");
    let mut cell_text = Paragraph::new(document.ids().node());
    cell_text.push_text("Cell", None, Arc::default()).unwrap();
    let borders = CellBorders {
        top: Some(BorderEdge {
            kind: BorderKind::Single,
            size_eighths: 4,
            color: Color::Auto,
        }),
        left: None,
        bottom: None,
        right: None,
    };
    let table = Table {
        id: document.ids().node(),
        grid: vec![Measure::Twips(2400)],
        rows: vec![Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![Cell {
                id: document.ids().node(),
                grid_span: 1,
                vertical_merge: VerticalMerge::None,
                width: Some(Measure::Twips(2400)),
                borders,
                shading: Some(Shading {
                    fill: Color::Rgb(0, 0, 0),
                }),
                margin: None,
                vertical_align: None,
                blocks: BlockSeq::from_vec(vec![cell_text.into()]),
                preserved: None,
            }],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
        }],
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        source: None,
    };
    install(&mut document, vec![intro, marker, table.into()]);
    let full = lay(&document);
    assert_eq!(full.omitted_blocks(), 1);
    assert!(full.pages()[0].runs.iter().any(|run| run.text == "Before"));
    assert!(full.pages()[0].runs.iter().any(|run| run.text == "Cell"));
    let mut cache = ParagraphLayoutCache::default();
    let incremental = lay_incremental(&document, &mut cache, None, &LayoutInvalidation::default());
    assert_same(&full, &incremental);
}

#[test]
fn content_controls_are_still_refused() {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let inner = text_block(&mut document, "Inside");
    let sdt = Sdt {
        id: document.ids().node(),
        props: SdtProps::default(),
        blocks: BlockSeq::from_vec(vec![inner]),
        source: None,
    };
    install(&mut document, vec![sdt.into()]);
    let error = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .expect_err("sdt");
    assert!(matches!(error, LayoutError::Unsupported { .. }));
}
