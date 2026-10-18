//! T202: explicit page breaks, section page boxes, and primary headers.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutInvalidation, LayoutLimits,
    LayoutNote, LayoutReuse, LayoutSnapshot, LoadedFont, ParagraphLayoutCache, SingleFontProfile,
    layout_document, layout_incremental,
};
use joyword_model::{
    Bias, Block, BlockSeq, BreakKind, Document, HeaderFooterKind, InlineKind, InlineObject,
    Measure, Orientation, PageFieldKind, PageSetup, Paragraph, Position, Preserved, ScalarOffset,
    StoryKind,
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

fn page_for(lines: i64) -> PageSetup {
    let mut page = PageSetup::a4_zh_cn();
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

fn change(document: &mut Document, index: usize, f: impl FnOnce(&mut Paragraph)) {
    let body = document.body_id();
    document
        .story_mut(body)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(paragraph) = block else {
                unreachable!()
            };
            f(Arc::make_mut(paragraph));
        });
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

fn page_text(snapshot: &LayoutSnapshot, page: usize) -> String {
    snapshot.pages()[page]
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect()
}

fn insert_break(document: &mut Document, index: usize, offset: usize) {
    let id = document.ids().node();
    change(document, index, |paragraph| {
        paragraph
            .insert_inline(
                ScalarOffset(offset),
                InlineObject {
                    id,
                    kind: InlineKind::Break(BreakKind::Page),
                    source: None,
                },
                None,
                Arc::default(),
            )
            .unwrap();
    });
}

fn header(
    document: &mut Document,
    text: &str,
    field: Option<PageFieldKind>,
) -> joyword_model::StoryId {
    let mut paragraph = Paragraph::new(document.ids().node());
    if !text.is_empty() {
        paragraph.push_text(text, None, Arc::default()).unwrap();
    }
    if let Some(kind) = field {
        paragraph.push_inline(
            InlineObject {
                id: document.ids().node(),
                kind: InlineKind::PageField(kind),
                source: None,
            },
            None,
            Arc::default(),
        );
    }
    let id = document.insert_story(StoryKind::Header, vec![paragraph.into()]);
    document.sections_mut()[0]
        .headers
        .push((HeaderFooterKind::Default, id));
    id
}

#[test]
fn explicit_page_breaks_split_pages_and_keep_caret_offsets() {
    let mut document = document_with(&["AV"]);
    insert_break(&mut document, 0, 1);
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 2);
    assert_eq!(body_text(&snapshot, 0), "A");
    assert_eq!(body_text(&snapshot, 1), "V");
    let paragraph = document.body().blocks.get(0).unwrap().id();
    let at = |offset, bias| {
        snapshot
            .caret(Position {
                story: document.body_id(),
                paragraph,
                offset: ScalarOffset(offset),
                bias,
            })
            .unwrap()
    };
    assert_eq!(at(0, Bias::After).page, 0);
    assert_eq!(at(1, Bias::Before).page, 0);
    assert_eq!(at(2, Bias::After).page, 1);
    assert_eq!(at(3, Bias::Before).page, 1);
}

#[test]
fn a_leading_break_and_consecutive_breaks_leave_blank_pages() {
    let mut leading = document_with(&["A"]);
    insert_break(&mut leading, 0, 0);
    let snapshot = layout(&leading);
    assert_eq!(snapshot.pages().len(), 2);
    assert_eq!(body_text(&snapshot, 0), "");
    assert_eq!(body_text(&snapshot, 1), "A");

    let mut consecutive = document_with(&["AB"]);
    insert_break(&mut consecutive, 0, 1);
    insert_break(&mut consecutive, 0, 2);
    let snapshot = layout(&consecutive);
    assert_eq!(snapshot.pages().len(), 3);
    assert_eq!(body_text(&snapshot, 0), "A");
    assert_eq!(body_text(&snapshot, 1), "");
    assert_eq!(body_text(&snapshot, 2), "B");
}

#[test]
fn stored_page_size_is_used_without_swapping_orientation() {
    let mut document = document_with(&["A"]);
    let page = &mut document.sections_mut()[0].page;
    page.width = Measure::Twips(16_838);
    page.height = Measure::Twips(11_906);
    page.orientation = Orientation::Landscape;
    page.left = Measure::Twips(720);
    page.right = Measure::Twips(720);
    page.top = Measure::Twips(720);
    page.bottom = Measure::Twips(720);
    let snapshot = layout(&document);
    let width = Measure::Twips(16_838).absolute().unwrap();
    let height = Measure::Twips(11_906).absolute().unwrap();
    assert_eq!(snapshot.pages()[0].width, width);
    assert_eq!(snapshot.pages()[0].height, height);
    assert_eq!(snapshot.page_geometry()[0].width, width);
}

#[test]
fn a_later_section_uses_its_own_page_box() {
    let mut document = document_with(&["A", "B"]);
    let boundary = document.body().blocks.get(0).unwrap().id();
    let mut next = document.sections()[0].clone();
    next.id = document.ids().node();
    next.last_block = None;
    next.page.width = Measure::Twips(10_000);
    next.page.height = Measure::Twips(8_000);
    next.page.left = Measure::Twips(720);
    next.page.right = Measure::Twips(720);
    next.page.top = Measure::Twips(720);
    next.page.bottom = Measure::Twips(720);
    document.sections_mut()[0].last_block = Some(boundary);
    document.sections_mut().push(next);
    let snapshot = layout(&document);
    assert!(snapshot.pages().len() >= 2);
    let second = snapshot
        .page_geometry()
        .iter()
        .find(|geometry| geometry.width == Measure::Twips(10_000).absolute().unwrap())
        .expect("section page");
    assert_eq!(second.height, Measure::Twips(8_000).absolute().unwrap());
    assert_ne!(snapshot.pages()[0].width, second.width);
}

#[test]
fn default_header_paints_page_numbers_on_the_shared_scene() {
    let mut document = document_with(&["A"]);
    insert_break(&mut document, 0, 1);
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("H", None, Arc::default()).unwrap();
    paragraph.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::PageField(PageFieldKind::Page),
            source: None,
        },
        None,
        Arc::default(),
    );
    paragraph.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::PageField(PageFieldKind::NumPages),
            source: None,
        },
        None,
        Arc::default(),
    );
    let story = document.insert_story(StoryKind::Header, vec![paragraph.into()]);
    document.sections_mut()[0]
        .headers
        .push((HeaderFooterKind::Default, story));
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 2);
    assert!(page_text(&snapshot, 0).contains('H'));
    assert!(page_text(&snapshot, 0).contains('1'));
    assert!(page_text(&snapshot, 1).contains('2'));
    assert!(page_text(&snapshot, 0).contains('2') || page_text(&snapshot, 1).contains('2'));
    assert!(
        snapshot
            .pages()
            .iter()
            .all(|page| page_text_of(page).contains('2'))
    );
}

fn page_text_of(page: &joyword_layout::PageScene) -> String {
    page.runs.iter().map(|run| run.text.as_str()).collect()
}

#[test]
fn first_page_and_even_headers_are_noted_and_the_default_story_still_paints() {
    let mut document = document_with(&["A"]);
    header(&mut document, "DEF", None);
    let mut first = Paragraph::new(document.ids().node());
    first.push_text("FIRST", None, Arc::default()).unwrap();
    let first_id = document.insert_story(StoryKind::Header, vec![first.into()]);
    document.sections_mut()[0]
        .headers
        .push((HeaderFooterKind::First, first_id));
    document.sections_mut()[0].title_page = Some(true);
    document.settings_mut().even_and_odd_headers = Some(true);
    let snapshot = layout(&document);
    let details: Vec<_> = snapshot
        .notes()
        .iter()
        .filter_map(|note| match note {
            LayoutNote::HeaderVariantNotApplied { detail } => Some(*detail),
            _ => None,
        })
        .collect();
    assert!(details.iter().any(|detail| detail.contains("first")));
    assert!(details.iter().any(|detail| detail.contains("odd")));
    assert!(page_text(&snapshot, 0).contains("DEF"));
    assert!(!page_text(&snapshot, 0).contains("FIRST"));
}

#[test]
fn a_missing_default_header_inherits_and_an_empty_story_does_not() {
    let mut document = document_with(&["A", "B"]);
    header(&mut document, "PREV", None);
    let boundary = document.body().blocks.get(0).unwrap().id();
    let mut next = document.sections()[0].clone();
    next.id = document.ids().node();
    next.last_block = None;
    next.headers.clear();
    document.sections_mut()[0].last_block = Some(boundary);
    document.sections_mut().push(next);
    let inherited = layout(&document);
    assert!(
        inherited
            .pages()
            .iter()
            .all(|page| { page.runs.iter().any(|run| run.text.contains("PREV")) })
    );

    let empty_paragraph = Paragraph::new(document.ids().node());
    let empty_id = document.insert_story(StoryKind::Header, vec![empty_paragraph.into()]);
    document.sections_mut()[1]
        .headers
        .push((HeaderFooterKind::Default, empty_id));
    let snapshot = layout(&document);
    let later = snapshot.pages().last().unwrap();
    assert!(later.runs.iter().all(|run| !run.text.contains("PREV")));
}

#[test]
fn an_unsupported_header_inline_is_skipped_instead_of_guessing_digits() {
    let mut document = document_with(&["A"]);
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("9", None, Arc::default()).unwrap();
    paragraph.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::Preserved(Preserved {
                element: Arc::from("w:object"),
                handle: document.ids().preserved(),
            }),
            source: None,
        },
        None,
        Arc::default(),
    );
    let story = document.insert_story(StoryKind::Header, vec![paragraph.into()]);
    document.sections_mut()[0]
        .headers
        .push((HeaderFooterKind::Default, story));
    let snapshot = layout(&document);
    assert!(
        snapshot
            .notes()
            .iter()
            .any(|note| { matches!(note, LayoutNote::StoryContentSkipped { .. }) })
    );
    assert!(snapshot.pages()[0].runs.iter().all(|run| run.text != "9"));
}

#[test]
fn a_page_field_in_the_body_stays_unsupported() {
    let mut document = document_with(&["A"]);
    let field = document.ids().node();
    change(&mut document, 0, |paragraph| {
        paragraph.push_inline(
            InlineObject {
                id: field,
                kind: InlineKind::PageField(PageFieldKind::Page),
                source: None,
            },
            None,
            Arc::default(),
        );
    });
    let error = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        LayoutError::Unsupported {
            feature: "page field outside header or footer",
            ..
        }
    ));
}

#[test]
fn a_keep_chain_taller_than_the_page_is_diagnosed_and_stays_finite() {
    let mut document = document_with(&["A", "B", "C", "D"]);
    for index in 0..3 {
        change(&mut document, index, |paragraph| {
            Arc::make_mut(&mut paragraph.props).keep_next = Some(true);
        });
    }
    document.sections_mut()[0].page.height = Measure::Emu(pt(12 + 18).raw());
    let snapshot = layout(&document);
    assert!(
        snapshot
            .notes()
            .iter()
            .any(|note| { matches!(note, LayoutNote::KeepTallerThanPage { .. }) })
    );
    assert!(snapshot.pages().len() >= 2);
    assert!(snapshot.pages().len() < 16);
}

#[test]
fn too_many_explicit_breaks_hit_the_page_limit() {
    let mut document = document_with(&[""]);
    for _ in 0..4 {
        let id = document.ids().node();
        change(&mut document, 0, |paragraph| {
            paragraph
                .insert_inline(
                    ScalarOffset(0),
                    InlineObject {
                        id,
                        kind: InlineKind::Break(BreakKind::Page),
                        source: None,
                    },
                    None,
                    Arc::default(),
                )
                .unwrap();
        });
    }
    let limits = LayoutLimits {
        max_pages: 2,
        ..LayoutLimits::default()
    };
    let error = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        limits,
        &LayoutCancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        LayoutError::LimitExceeded(joyword_layout::LayoutLimit::Pages)
    ));
}

#[test]
fn incremental_layout_matches_the_full_scene_including_headers() {
    let mut document = document_with(&["A", "B"]);
    insert_break(&mut document, 0, 1);
    header(&mut document, "H", Some(PageFieldKind::Page));
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
    assert_eq!(full.pages().len(), incremental.pages().len());
    for (left, right) in full.pages().iter().zip(incremental.pages()) {
        assert_eq!(left.width, right.width);
        assert_eq!(left.height, right.height);
        let left_text: Vec<_> = left.runs.iter().map(|run| run.text.as_str()).collect();
        let right_text: Vec<_> = right.runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(left_text, right_text);
    }
    assert_eq!(full.notes(), incremental.notes());
}
