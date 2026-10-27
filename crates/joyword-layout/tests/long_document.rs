//! T500: long-document scale. A mixed multi-section document of a hundred
//! pages lays out in one pass, a cancelled token returns before any page is
//! published, and edits at either end of a long document reuse the paragraph
//! cache — the stable page tail is copied when the placement anchor matches,
//! and the result still equals a from-scratch layout.

use std::sync::Arc;

use joyword_engine::{Command, Outcome, Session, Transaction};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutInvalidation, LayoutLimits,
    LayoutReuse, LayoutSnapshot, LoadedFont, ParagraphLayoutCache, SingleFontProfile,
    layout_document, layout_incremental,
};
use joyword_model::{
    AnchorWrap, Block, BlockSeq, Cell, CellBorders, Document, EqualColumns, ImageAnchor,
    InlineKind, InlineObject, Measure, PageSetup, Paragraph, Position, Row, ScalarOffset,
    SectionBreak, SquareWrap, Table, VerticalMerge, WrapSide, sample_png_1x1,
};

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

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
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    }
}

fn a4_page() -> PageSetup {
    let mut page = PageSetup::a4_zh_cn();
    page.left = Measure::Twips(1440);
    page.right = Measure::Twips(1440);
    page.top = Measure::Twips(1440);
    page.bottom = Measure::Twips(1440);
    page.header = Measure::Twips(720);
    page.footer = Measure::Twips(720);
    page.gutter = Measure::Twips(0);
    page
}

/// At least three body lines at the test profile on the portrait text width,
/// so each paragraph occupies a stable multi-line block of the page.
fn body_paragraph(document: &mut Document, index: usize) -> Paragraph {
    body_paragraph_with(
        document,
        &format!(
            "第{index:04}段：长文档排版验证保持段落多行高度，第一行排满正文宽度之后继续排入后续行，行距与字号保持一致，段落文本足够长以覆盖三行以上，从而让每个段落占据页面上一块稳定的行块，用于观察长文档的分页、缓存复用与页尾稳定边界的拷贝行为。"
        ),
    )
}

fn body_paragraph_with(document: &mut Document, text: &str) -> Paragraph {
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    paragraph
}

/// Close the current section after body block `last_block` and push a clone of
/// it for the caller to retune (columns, orientation, break kind). Requires
/// the body blocks to be installed, like the T202 column tests.
fn close_section(document: &mut Document, last_block: usize) {
    let boundary = document.body().blocks.get(last_block).unwrap().id();
    let mut next = document.sections().last().unwrap().clone();
    next.id = document.ids().node();
    next.last_block = None;
    next.title_page = None;
    next.headers.clear();
    next.footers.clear();
    let closed = document.sections().len() - 1;
    document.sections_mut()[closed].last_block = Some(boundary);
    document.sections_mut().push(next);
}

fn cell(document: &mut Document, texts: &[&str]) -> Cell {
    Cell {
        id: document.ids().node(),
        grid_span: 1,
        vertical_merge: VerticalMerge::None,
        width: Some(Measure::Twips(4000)),
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(
            texts
                .iter()
                .map(|text| body_paragraph_with(document, text))
                .map(Block::from)
                .collect(),
        ),
        preserved: None,
        props_revision: None,
    }
}

/// A cross-page table: a header row that repeats, then `rows` body rows of
/// two-paragraph cells so the table spans several pages.
fn cross_page_table(document: &mut Document, rows: usize) -> Table {
    let header_row = Row {
        id: document.ids().node(),
        grid_before: 0,
        grid_after: 0,
        cells: vec![cell(document, &["表头列甲"]), cell(document, &["表头列乙"])],
        header: true,
        cant_split: false,
        height: None,
        preserved: None,
        props_revision: None,
    };
    let mut table_rows = vec![header_row];
    for index in 0..rows {
        table_rows.push(Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![
                cell(
                    document,
                    &[&format!("数据行{index}左列"), "第二段保持两行高度"],
                ),
                cell(
                    document,
                    &[&format!("数据行{index}右列"), "第二段保持两行高度"],
                ),
            ],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
            props_revision: None,
        });
    }
    Table {
        id: document.ids().node(),
        grid: vec![Measure::Twips(4000); 2],
        rows: table_rows,
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        props_revision: None,
        source: None,
    }
}

/// A floating wrap-square picture pinned to the page, as T203 imports it.
fn floating_picture_paragraph(document: &mut Document) -> Paragraph {
    let resource = document.ids().resource();
    document
        .resources_mut()
        .insert(resource, "image/png", Arc::from(sample_png_1x1()), None);
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::Image {
                resource,
                width: Measure::Emu(pt(72).raw()),
                height: Measure::Emu(pt(72).raw()),
                anchor: Some(ImageAnchor {
                    x: Measure::Emu(pt(312).raw()),
                    y: Measure::Emu(pt(180).raw()),
                    wrap: AnchorWrap::Square(SquareWrap {
                        top: 0,
                        bottom: 0,
                        left: 0,
                        right: 0,
                        side: WrapSide::Larger,
                    }),
                }),
            },
            source: None,
        },
        None,
        Arc::default(),
    );
    paragraph
        .push_text(
            "带浮动图片的段落，文字绕排在锚定矩形两侧。",
            None,
            Arc::default(),
        )
        .unwrap();
    paragraph
}

/// The mixed long document: portrait paragraphs and (optionally) a cross-page
/// repeating table, a landscape section that opens with a floating
/// wrap-square picture, and an even-page section in two separated columns.
fn mixed_long_document(section_paragraphs: usize, with_table: bool) -> Document {
    let mut document = Document::new(a4_page());
    let mut blocks: Vec<Block> = Vec::new();
    for index in 0..section_paragraphs {
        blocks.push(body_paragraph(&mut document, index).into());
    }
    if with_table {
        blocks.push(cross_page_table(&mut document, 40).into());
    }
    let section0_last = blocks.len() - 1;
    blocks.push(floating_picture_paragraph(&mut document).into());
    for index in section_paragraphs..2 * section_paragraphs {
        blocks.push(body_paragraph(&mut document, index).into());
    }
    let section1_last = blocks.len() - 1;
    for index in 2 * section_paragraphs..3 * section_paragraphs {
        blocks.push(body_paragraph(&mut document, index).into());
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    // Close on the installed blocks, then retune each cloned section: the
    // clones are made from the portrait original before any retuning.
    close_section(&mut document, section0_last);
    close_section(&mut document, section1_last);
    let portrait = a4_page();
    let landscape = &mut document.sections_mut()[1];
    landscape.break_kind = SectionBreak::NextPage;
    landscape.page = PageSetup {
        width: portrait.height,
        height: portrait.width,
        ..portrait
    };
    let columns = &mut document.sections_mut()[2];
    columns.break_kind = SectionBreak::EvenPage;
    columns.columns = Some(EqualColumns {
        count: 2,
        space: 720,
    });
    columns.column_separator = true;
    document
}

fn lay(document: &Document) -> Result<LayoutSnapshot, LayoutError> {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
}

#[test]
fn a_hundred_page_mixed_section_document_lays_out() {
    // 3 × 500 multi-line paragraphs plus a 41-row two-line-cell table that
    // repeats its header: comfortably over one hundred pages.
    let document = mixed_long_document(500, true);
    let snapshot = lay(&document).expect("mixed hundred-page layout");
    assert!(
        snapshot.pages().len() >= 100,
        "{} pages",
        snapshot.pages().len()
    );
    // Portrait pages exist, and the landscape section turns the page box.
    let geometries = snapshot.page_geometry();
    assert!(
        geometries
            .iter()
            .any(|geometry| geometry.width.raw() < geometry.height.raw())
    );
    assert!(
        geometries
            .iter()
            .any(|geometry| geometry.width.raw() > geometry.height.raw()),
        "no landscape pages"
    );
    // The repeating header reaches more than one page of the table.
    let header_pages = snapshot
        .pages()
        .iter()
        .filter(|page| page.runs.iter().any(|run| run.text.contains("表头列甲")))
        .count();
    assert!(header_pages >= 2, "header on {header_pages} pages");
    // The column section paints its separator rule and fills both sides.
    let column_pages: Vec<_> = snapshot
        .pages()
        .iter()
        .filter(|page| {
            page.rects
                .iter()
                .any(|rect| rect.width == Emu::new(Emu::PER_POINT))
        })
        .collect();
    assert!(!column_pages.is_empty(), "no separator rules painted");
    let column_page = column_pages[0];
    let rule = column_page
        .rects
        .iter()
        .find(|rect| rect.width == Emu::new(Emu::PER_POINT))
        .unwrap();
    let left = column_page
        .runs
        .iter()
        .any(|run| run.origin_x.raw() < rule.x.raw());
    let right = column_page
        .runs
        .iter()
        .any(|run| run.origin_x.raw() > rule.x.raw() + rule.width.raw());
    assert!(left && right, "runs missing beside the rule");
    // The floating picture is present as a page image on the landscape pages.
    assert!(snapshot.pages().iter().any(|page| !page.images.is_empty()));
}

#[test]
fn a_cancelled_hundred_page_layout_returns_without_publishing_pages() {
    let document = mixed_long_document(500, true);
    let token = LayoutCancellation::default();
    let other = token.clone();
    other.cancel();
    assert!(token.is_cancelled());
    assert!(matches!(
        layout_document(
            &document.snapshot(),
            FaceSet::uniform(&font()),
            profile(),
            LayoutLimits::default(),
            &token
        ),
        Err(LayoutError::Cancelled)
    ));
    // The token stays monotonic; a retry with a fresh token completes.
    assert!(token.is_cancelled());
    lay(&document).expect("retry with a fresh token");
}

fn assert_same(full: &LayoutSnapshot, incremental: &LayoutSnapshot, step: &str) {
    assert_eq!(
        full.pages().len(),
        incremental.pages().len(),
        "{step} pages"
    );
    assert_eq!(full.geometry(), incremental.geometry(), "{step} geometry");
    assert_eq!(
        full.page_geometry(),
        incremental.page_geometry(),
        "{step} boxes"
    );
    assert_eq!(full.lines(), incremental.lines(), "{step} lines");
    for (index, (full_page, warm_page)) in full.pages().iter().zip(incremental.pages()).enumerate()
    {
        assert_eq!(full_page.runs, warm_page.runs, "{step} page {index} runs");
        assert_eq!(
            full_page.rects, warm_page.rects,
            "{step} page {index} rects"
        );
        assert_eq!(
            full_page.images, warm_page.images,
            "{step} page {index} images"
        );
    }
}

fn lay_incremental(
    document: &Document,
    cache: &mut ParagraphLayoutCache,
    previous: Option<&LayoutSnapshot>,
    invalidation: &LayoutInvalidation,
) -> Result<LayoutSnapshot, LayoutError> {
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

fn insert_at(document: &Document, paragraph: joyword_model::NodeId, offset: usize) -> Position {
    Position {
        story: document.body_id(),
        paragraph,
        offset: ScalarOffset(offset),
        bias: joyword_model::Bias::Before,
    }
}

#[test]
fn long_document_edits_reuse_fits_and_the_stable_page_tail() {
    // Table-less long document so the incremental paragraph path applies (a
    // table would take the full placement), with sections, columns and a
    // separator rule still in the flow.
    let document = mixed_long_document(450, false);
    let paragraph_count = document.body().blocks.len();
    let mut cache = ParagraphLayoutCache::default();
    let warm = lay_incremental(&document, &mut cache, None, &LayoutInvalidation::default())
        .expect("warm incremental");
    assert_same(&lay(&document).unwrap(), &warm, "warm");
    assert!(warm.pages().len() >= 65, "{} pages", warm.pages().len());

    // An unchanged relayout shapes nothing.
    let again = lay_incremental(
        &document,
        &mut cache,
        Some(&warm),
        &LayoutInvalidation::default(),
    )
    .expect("unchanged incremental");
    assert_eq!(again.stats().shape_calls, 0);
    assert!(again.stats().cache_hits >= paragraph_count);
    // The tail copy fires despite the column separator rules: the pages of
    // the landscape section that carry the page-anchored picture are placed
    // (their image is not line-attached), and everything after them — the
    // whole separated column section included — is copied from the anchor.
    assert!(again.stats().stable_boundary.is_some());

    // Edit the first paragraph: one refit whose height is unchanged, so the
    // placement anchor matches and the whole remaining tail is copied without
    // being placed again.
    let first = document.body().blocks.get(0).unwrap().id();
    let mut session = Session::new(document);
    let outcome = session
        .apply(Transaction::new(
            session.version(),
            vec![Command::InsertText {
                at: insert_at(session.document(), first, 4),
                text: "增".into(),
            }],
        ))
        .unwrap();
    let Outcome::Applied(result) = outcome else {
        panic!("first-paragraph insert rejected");
    };
    let incremental = lay_incremental(
        session.document(),
        &mut cache,
        Some(&again),
        &invalidation_of(&result.changes),
    )
    .expect("first-paragraph incremental");
    let full = lay(session.document()).expect("first-paragraph full");
    assert_same(&full, &incremental, "first paragraph");
    assert!(incremental.stats().shape_calls <= 4);
    assert!(incremental.stats().cache_hits >= paragraph_count - 1);
    // The tail copy starts after the pages that carry the anchored picture;
    // the boundary sits past the edited paragraph, before the columns.
    assert!(
        incremental
            .stats()
            .stable_boundary
            .is_some_and(|index| index > 1)
    );
    assert_eq!(incremental.pages().len(), again.pages().len());

    // Edit the last paragraph: the edited paragraph itself is refitted, no
    // suffix copy fires (its fit is not "same"), and everything before it is
    // a cache hit. The pages still equal a full layout.
    let last = session
        .document()
        .body()
        .blocks
        .get(session.document().body().blocks.len() - 1)
        .unwrap()
        .id();
    let before_last = incremental;
    let outcome = session
        .apply(Transaction::new(
            session.version(),
            vec![Command::InsertText {
                at: insert_at(session.document(), last, 4),
                text: "末".into(),
            }],
        ))
        .unwrap();
    let Outcome::Applied(result) = outcome else {
        panic!("last-paragraph insert rejected");
    };
    let incremental = lay_incremental(
        session.document(),
        &mut cache,
        Some(&before_last),
        &invalidation_of(&result.changes),
    )
    .expect("last-paragraph incremental");
    let full = lay(session.document()).expect("last-paragraph full");
    assert_same(&full, &incremental, "last paragraph");
    // One paragraph's refit may try several candidate lines; everything else
    // is a cache hit, and no suffix copy fires for the edited paragraph.
    // Only the edited paragraph is refit: every other paragraph is a cache
    // hit, no suffix copy fires for the edited paragraph itself, and the
    // shaping is a small fraction of a from-scratch layout.
    assert_eq!(incremental.stats().cache_hits, paragraph_count - 1);
    assert!(
        incremental.stats().shape_calls * 10 < full.stats().shape_calls,
        "incremental {} vs full {}",
        incremental.stats().shape_calls,
        full.stats().shape_calls
    );
    assert_eq!(incremental.stats().stable_boundary, None);
    assert_eq!(incremental.pages().len(), before_last.pages().len());
}
