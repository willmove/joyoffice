//! T202 column slice: `w:cols/@w:sep` separator rules painted into the shared
//! PageScene, and `w:type="nextColumn"` section breaks that advance a column.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutInvalidation, LayoutLimits, LayoutNote,
    LayoutReuse, LayoutSnapshot, LoadedFont, ParagraphLayoutCache, SingleFontProfile,
    layout_document, layout_incremental,
};
use joyword_model::{
    Block, BlockSeq, Document, EqualColumns, Measure, NewspaperColumn, Paragraph, ScalarOffset,
    SectionBreak, UnequalColumns,
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

fn page_for(lines: i64) -> joyword_model::PageSetup {
    let mut page = joyword_model::PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(100).raw());
    page.height = Measure::Emu(pt(12 + 18 * lines).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.header = Measure::Emu(pt(2).raw());
    page.footer = Measure::Emu(pt(2).raw());
    page.gutter = Measure::Emu(0);
    page
}

fn document_with(texts: &[&str]) -> Document {
    let mut document = Document::new(page_for(2));
    let mut blocks = Vec::new();
    for text in texts {
        let mut paragraph = Paragraph::new(document.ids().node());
        if !text.is_empty() {
            paragraph.push_text(text, None, Arc::default()).unwrap();
        }
        blocks.push(paragraph.into());
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    document
}

fn layout(document: &Document) -> LayoutSnapshot {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap()
}

/// Close the current section after body block `last_block` and start a clone of
/// it, so the caller can tune the new section's break kind and columns.
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

/// A paragraph whose text is split by an inline column break at `offset`.
fn insert_column_break(document: &mut Document, index: usize, offset: usize) {
    let id = document.ids().node();
    let body = document.body_id();
    document
        .story_mut(body)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(paragraph) = block else {
                unreachable!()
            };
            let paragraph = Arc::make_mut(paragraph);
            paragraph
                .insert_inline(
                    ScalarOffset(offset),
                    joyword_model::InlineObject {
                        id,
                        kind: joyword_model::InlineKind::Break(joyword_model::BreakKind::Column),
                        source: None,
                    },
                    None,
                    Arc::default(),
                )
                .unwrap();
        });
}

fn body_text(snapshot: &LayoutSnapshot, page: usize) -> String {
    snapshot
        .lines()
        .iter()
        .filter(|line| line.page == page)
        .fold(String::new(), |mut text, line| {
            let run = &snapshot.pages()[line.page].runs[line.run];
            text.push_str(&run.text);
            text
        })
}

fn two_equal_columns(document: &mut Document, space: i64) {
    document.sections_mut()[0].columns = Some(EqualColumns { count: 2, space });
}

#[test]
fn equal_column_separator_paints_one_centered_rule() {
    let mut document = document_with(&["AB"]);
    two_equal_columns(&mut document, 720);
    document.sections_mut()[0].column_separator = true;
    // Force both glyphs into distinct columns so the page uses two columns.
    insert_column_break(&mut document, 0, 1);
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 1);

    let geometry = snapshot.page_geometry()[0];
    let left = geometry.left.raw();
    let content_width = geometry.width.raw() - left - geometry.right.raw();
    let gap = 720 * Emu::PER_TWIP;
    let column_width = (content_width - gap) / 2;
    let right_edge = left + column_width;
    let left_edge = left + column_width + gap;
    let center = (right_edge + left_edge) / 2;
    let thickness = Emu::PER_POINT;
    let expected_x = center - thickness / 2;
    let body_height = geometry.height.raw() - geometry.bottom.raw() - geometry.top.raw();

    let rects = &snapshot.pages()[0].rects;
    assert_eq!(rects.len(), 1, "{rects:?}");
    let rule = rects[0];
    assert_eq!((rule.red, rule.green, rule.blue), (0, 0, 0));
    assert_eq!(rule.x.raw(), expected_x);
    assert_eq!(rule.y.raw(), geometry.top.raw());
    assert_eq!(rule.width.raw(), thickness);
    assert_eq!(rule.height.raw(), body_height);

    // Two columns put one glyph in each, straddling the rule.
    let runs = &snapshot.pages()[0].runs;
    assert_eq!(runs.len(), 2);
    assert!(runs[0].origin_x.raw() < expected_x);
    assert!(runs[1].origin_x.raw() > expected_x);
}

#[test]
fn unequal_column_separator_paints_a_rule_per_gap() {
    let mut document = document_with(&["i i i i i i i i i i i i"]);
    document.sections_mut()[0].page.height = page_for(8).height;
    document.sections_mut()[0].unequal_columns = Some(UnequalColumns {
        columns: vec![
            NewspaperColumn {
                width: 400,
                space: 120,
            },
            NewspaperColumn {
                width: 500,
                space: 120,
            },
            NewspaperColumn {
                width: 600,
                space: 0,
            },
        ],
    });
    document.sections_mut()[0].column_separator = true;
    let snapshot = layout(&document);
    // Three columns have two interior gaps, so exactly two rules.
    let rects: Vec<_> = snapshot.pages()[0]
        .rects
        .iter()
        .filter(|rect| {
            rect.width.raw() == Emu::PER_POINT && (rect.red, rect.green, rect.blue) == (0, 0, 0)
        })
        .collect();
    assert_eq!(rects.len(), 2, "{rects:?}");
    // Rules are strictly ordered left to right and inside the content box.
    let geometry = snapshot.page_geometry()[0];
    let left = geometry.left.raw();
    let right_limit = geometry.width.raw() - geometry.right.raw();
    assert!(rects[0].x.raw() < rects[1].x.raw());
    assert!(rects[0].x.raw() >= left);
    assert!(rects[1].x.raw() + rects[1].width.raw() <= right_limit);
}

#[test]
fn separator_off_paints_no_rule() {
    let mut document = document_with(&["AB"]);
    two_equal_columns(&mut document, 720);
    document.sections_mut()[0].column_separator = false;
    insert_column_break(&mut document, 0, 1);
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 1);
    assert!(
        snapshot.pages()[0].rects.is_empty(),
        "{:?}",
        snapshot.pages()[0].rects
    );
}

#[test]
fn incremental_layout_matches_full_with_separators() {
    let mut document = document_with(&["AB"]);
    two_equal_columns(&mut document, 720);
    document.sections_mut()[0].column_separator = true;
    insert_column_break(&mut document, 0, 1);
    let full = layout(&document);

    let mut cache = ParagraphLayoutCache::with_budget(1 << 20);
    let incremental = layout_incremental(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        LayoutReuse {
            cache: &mut cache,
            previous: None,
            invalidation: &LayoutInvalidation::default(),
        },
    )
    .unwrap();
    assert_eq!(full.pages(), incremental.pages());
    assert_eq!(full.page_geometry(), incremental.page_geometry());
}

#[test]
fn next_column_section_break_advances_one_column() {
    // Section 0 fills column 0; section 1 starts with a nextColumn break and
    // shares the same two-column geometry, so it lands in column 1 of the same
    // page rather than on a new page.
    let mut document = document_with(&["A", "B"]);
    two_equal_columns(&mut document, 720);
    close_section(&mut document, 0);
    document.sections_mut()[1].break_kind = SectionBreak::NextColumn;
    // close_section cloned section 0, so section 1 already has two columns.
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 1, "{:?}", snapshot.pages().len());
    assert_eq!(body_text(&snapshot, 0), "AB");
    let runs = &snapshot.pages()[0].runs;
    let a = runs.iter().find(|run| run.text.contains('A')).unwrap();
    let b = runs.iter().find(|run| run.text.contains('B')).unwrap();
    assert!(
        b.origin_x.raw() > a.origin_x.raw(),
        "B should sit in the next column: A={} B={}",
        a.origin_x.raw(),
        b.origin_x.raw()
    );
    // No page split note: the break stayed on the shared page.
    assert!(
        !snapshot
            .notes()
            .iter()
            .any(|note| matches!(note, LayoutNote::ContinuousPageSplit { .. })),
        "{:?}",
        snapshot.notes()
    );
}

#[test]
fn next_column_with_different_columns_splits_the_page() {
    // Section 1 uses a different column count, so the nextColumn break cannot
    // continue the previous column flow and starts on a fresh page.
    let mut document = document_with(&["A", "B"]);
    two_equal_columns(&mut document, 720);
    close_section(&mut document, 0);
    document.sections_mut()[1].break_kind = SectionBreak::NextColumn;
    document.sections_mut()[1].columns = Some(EqualColumns {
        count: 3,
        space: 360,
    });
    let snapshot = layout(&document);
    assert!(snapshot.pages().len() >= 2, "{:?}", snapshot.pages().len());
    assert!(
        snapshot
            .notes()
            .iter()
            .any(|note| matches!(note, LayoutNote::ContinuousPageSplit { .. })),
        "{:?}",
        snapshot.notes()
    );
}
