//! T202: explicit page breaks, section page boxes, and primary headers.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutInvalidation, LayoutLimits,
    LayoutNote, LayoutReuse, LayoutSnapshot, LoadedFont, ParagraphLayoutCache, SingleFontProfile,
    ViewOptions, layout_document, layout_incremental, layout_incremental_with_view,
    layout_with_view,
};
use joyword_model::{
    Bias, Block, BlockSeq, BreakKind, Document, EqualColumns, HeaderFooterKind, InlineKind,
    InlineObject, Measure, NumberFormat, Orientation, PageField, PageFieldKind, PageNumbering,
    PageSetup, Paragraph, Position, Preserved, ScalarOffset, SectionBreak, StoryKind,
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
    attach(document, 0, true, HeaderFooterKind::Default, text, field)
}

fn fit_one_line(document: &mut Document) {
    let height = page_for(1).height;
    for section in document.sections_mut() {
        section.page.height = height;
    }
}

fn attach(
    document: &mut Document,
    section: usize,
    header: bool,
    kind: HeaderFooterKind,
    text: &str,
    field: Option<PageFieldKind>,
) -> joyword_model::StoryId {
    let mut paragraph = Paragraph::new(document.ids().node());
    if !text.is_empty() {
        paragraph.push_text(text, None, Arc::default()).unwrap();
    }
    if let Some(field) = field {
        paragraph.push_inline(
            InlineObject {
                id: document.ids().node(),
                kind: InlineKind::PageField(PageField::new(field)),
                source: None,
            },
            None,
            Arc::default(),
        );
    }
    let id = document.insert_story(
        if header {
            StoryKind::Header
        } else {
            StoryKind::Footer
        },
        vec![paragraph.into()],
    );
    let refs = if header {
        &mut document.sections_mut()[section].headers
    } else {
        &mut document.sections_mut()[section].footers
    };
    refs.push((kind, id));
    id
}

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
            kind: InlineKind::PageField(PageField::new(PageFieldKind::Page)),
            source: None,
        },
        None,
        Arc::default(),
    );
    paragraph.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::PageField(PageField::new(PageFieldKind::NumPages)),
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
fn first_even_and_default_headers_and_footers_paint_on_their_pages() {
    let mut document = document_with(&["A", "B", "C"]);
    fit_one_line(&mut document);
    document.sections_mut()[0].title_page = Some(true);
    document.settings_mut().even_and_odd_headers = Some(true);
    attach(
        &mut document,
        0,
        true,
        HeaderFooterKind::First,
        "H1",
        Some(PageFieldKind::NumPages),
    );
    attach(
        &mut document,
        0,
        true,
        HeaderFooterKind::Even,
        "H2",
        Some(PageFieldKind::Page),
    );
    attach(
        &mut document,
        0,
        true,
        HeaderFooterKind::Default,
        "H0",
        None,
    );
    attach(&mut document, 0, false, HeaderFooterKind::First, "F1", None);
    attach(&mut document, 0, false, HeaderFooterKind::Even, "F2", None);
    attach(
        &mut document,
        0,
        false,
        HeaderFooterKind::Default,
        "F0",
        None,
    );
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 3);
    let page0 = page_text(&snapshot, 0);
    assert!(page0.contains("H1"));
    assert!(page0.contains('3'));
    assert!(page0.contains("F1"));
    assert!(!page0.contains("H0"));
    assert!(!page0.contains("H2"));
    assert!(!page0.contains("F0"));
    assert!(!page0.contains("F2"));
    let page1 = page_text(&snapshot, 1);
    assert!(page1.contains("H2"));
    assert!(page1.contains('2'));
    assert!(page1.contains("F2"));
    assert!(!page1.contains("H1"));
    assert!(!page1.contains("H0"));
    assert!(!page1.contains("F1"));
    assert!(!page1.contains("F0"));
    let page2 = page_text(&snapshot, 2);
    assert!(page2.contains("H0"));
    assert!(page2.contains("F0"));
    assert!(!page2.contains("H1"));
    assert!(!page2.contains("H2"));
    assert!(!page2.contains("F1"));
    assert!(!page2.contains("F2"));
}

#[test]
fn title_page_and_even_odd_off_keep_the_default_story() {
    let mut document = document_with(&["A", "B", "C"]);
    fit_one_line(&mut document);
    attach(&mut document, 0, true, HeaderFooterKind::First, "H1", None);
    attach(&mut document, 0, true, HeaderFooterKind::Even, "H2", None);
    attach(
        &mut document,
        0,
        true,
        HeaderFooterKind::Default,
        "H0",
        None,
    );
    let omitted = layout(&document);
    assert_eq!(omitted.pages().len(), 3);
    for page in 0..3 {
        let text = page_text(&omitted, page);
        assert!(text.contains("H0"), "{text}");
        assert!(!text.contains("H1"), "{text}");
        assert!(!text.contains("H2"), "{text}");
    }

    document.sections_mut()[0].title_page = Some(false);
    document.settings_mut().even_and_odd_headers = Some(false);
    let explicit = layout(&document);
    for page in 0..3 {
        let text = page_text(&explicit, page);
        assert!(text.contains("H0"), "{text}");
        assert!(!text.contains("H1"), "{text}");
        assert!(!text.contains("H2"), "{text}");
    }

    document.settings_mut().even_and_odd_headers = Some(true);
    let even_only = layout(&document);
    assert!(page_text(&even_only, 0).contains("H0"));
    assert!(!page_text(&even_only, 0).contains("H1"));
    assert!(page_text(&even_only, 1).contains("H2"));
    assert!(!page_text(&even_only, 1).contains("H0"));
    assert!(page_text(&even_only, 2).contains("H0"));
}

#[test]
fn an_omitted_first_or_even_story_does_not_fall_back_to_default() {
    let mut document = document_with(&["A", "B", "C"]);
    fit_one_line(&mut document);
    document.sections_mut()[0].title_page = Some(true);
    document.settings_mut().even_and_odd_headers = Some(true);
    attach(
        &mut document,
        0,
        true,
        HeaderFooterKind::Default,
        "H0",
        None,
    );
    attach(
        &mut document,
        0,
        false,
        HeaderFooterKind::Default,
        "F0",
        None,
    );
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 3);
    assert!(!page_text(&snapshot, 0).contains("H0"));
    assert!(!page_text(&snapshot, 0).contains("F0"));
    assert!(!page_text(&snapshot, 1).contains("H0"));
    assert!(!page_text(&snapshot, 1).contains("F0"));
    assert!(page_text(&snapshot, 2).contains("H0"));
    assert!(page_text(&snapshot, 2).contains("F0"));
}

#[test]
fn omitted_first_and_even_stories_inherit_until_an_empty_story() {
    let mut document = document_with(&["A", "B", "C", "D", "E"]);
    fit_one_line(&mut document);
    document.sections_mut()[0].title_page = Some(true);
    document.settings_mut().even_and_odd_headers = Some(true);
    attach(&mut document, 0, true, HeaderFooterKind::First, "H1", None);
    attach(&mut document, 0, true, HeaderFooterKind::Even, "H2", None);
    attach(
        &mut document,
        0,
        true,
        HeaderFooterKind::Default,
        "H0",
        None,
    );
    attach(&mut document, 0, false, HeaderFooterKind::First, "F1", None);
    attach(&mut document, 0, false, HeaderFooterKind::Even, "F2", None);
    attach(
        &mut document,
        0,
        false,
        HeaderFooterKind::Default,
        "F0",
        None,
    );
    close_section(&mut document, 1);
    document.sections_mut()[1].title_page = Some(true);
    let inherited = layout(&document);
    assert_eq!(inherited.pages().len(), 5);
    assert!(page_text(&inherited, 0).contains("H1"));
    assert!(page_text(&inherited, 0).contains("F1"));
    assert!(page_text(&inherited, 1).contains("H2"));
    assert!(page_text(&inherited, 1).contains("F2"));
    assert!(!page_text(&inherited, 1).contains("H0"));
    assert!(page_text(&inherited, 2).contains("H1"));
    assert!(page_text(&inherited, 2).contains("F1"));
    assert!(!page_text(&inherited, 2).contains("H2"));
    assert!(page_text(&inherited, 3).contains("H2"));
    assert!(page_text(&inherited, 3).contains("F2"));
    assert!(page_text(&inherited, 4).contains("H0"));
    assert!(page_text(&inherited, 4).contains("F0"));
    assert!(!page_text(&inherited, 4).contains("H2"));

    let empty = Paragraph::new(document.ids().node());
    let empty_id = document.insert_story(StoryKind::Header, vec![empty.into()]);
    document.sections_mut()[1]
        .headers
        .push((HeaderFooterKind::First, empty_id));
    let blocked = layout(&document);
    assert!(page_text(&blocked, 0).contains("H1"));
    assert!(!page_text(&blocked, 2).contains("H1"));
    assert!(page_text(&blocked, 2).contains("F1"));
    assert!(page_text(&blocked, 3).contains("H2"));
    assert!(page_text(&blocked, 4).contains("H0"));

    let empty_footer = Paragraph::new(document.ids().node());
    let empty_footer_id = document.insert_story(StoryKind::Footer, vec![empty_footer.into()]);
    document.sections_mut()[1]
        .footers
        .push((HeaderFooterKind::Even, empty_footer_id));
    let stopped = layout(&document);
    assert!(page_text(&stopped, 1).contains("F2"));
    assert!(!page_text(&stopped, 3).contains("F2"));
    assert!(page_text(&stopped, 3).contains("H2"));
    assert!(page_text(&stopped, 4).contains("F0"));
}

#[test]
fn a_later_section_first_page_uses_its_own_story_on_an_even_page() {
    let mut document = document_with(&["A", "B", "C"]);
    fit_one_line(&mut document);
    document.settings_mut().even_and_odd_headers = Some(true);
    attach(
        &mut document,
        0,
        true,
        HeaderFooterKind::Default,
        "H0",
        None,
    );
    attach(&mut document, 0, true, HeaderFooterKind::Even, "H2", None);
    close_section(&mut document, 0);
    document.sections_mut()[1].title_page = Some(true);
    attach(&mut document, 1, true, HeaderFooterKind::First, "H1", None);
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 3);
    assert!(page_text(&snapshot, 0).contains("H0"));
    assert!(!page_text(&snapshot, 0).contains("H1"));
    assert!(page_text(&snapshot, 1).contains("H1"));
    assert!(!page_text(&snapshot, 1).contains("H2"));
    assert!(!page_text(&snapshot, 1).contains("H0"));
    assert!(page_text(&snapshot, 2).contains("H0"));
    assert!(!page_text(&snapshot, 2).contains("H1"));
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

fn push_field(document: &mut Document, index: usize, kind: PageFieldKind, cache: Option<&str>) {
    let id = document.ids().node();
    change(document, index, |paragraph| {
        let field = match cache {
            Some(text) => PageField::with_cache(kind, text),
            None => PageField::new(kind),
        };
        paragraph.push_inline(
            InlineObject {
                id,
                kind: InlineKind::PageField(field),
                source: None,
            },
            None,
            Arc::default(),
        );
    });
}

fn scenes_match(full: &LayoutSnapshot, incremental: &LayoutSnapshot) {
    assert_eq!(full.pages().len(), incremental.pages().len());
    for (left, right) in full.pages().iter().zip(incremental.pages()) {
        let left_text: Vec<_> = left.runs.iter().map(|run| run.text.as_str()).collect();
        let right_text: Vec<_> = right.runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(left_text, right_text);
    }
    assert_eq!(full.notes(), incremental.notes());
}

fn layout_both(document: &Document) -> (LayoutSnapshot, LayoutSnapshot) {
    let full = layout(document);
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
    (full, incremental)
}

#[test]
fn a_body_page_field_uses_the_displayed_number() {
    let mut document = document_with(&["A", "B"]);
    fit_one_line(&mut document);
    push_field(&mut document, 0, PageFieldKind::Page, Some("9"));
    push_field(&mut document, 0, PageFieldKind::NumPages, None);
    push_field(&mut document, 1, PageFieldKind::Page, None);
    let (full, incremental) = layout_both(&document);
    scenes_match(&full, &incremental);
    assert_eq!(full.pages().len(), 2);
    assert_eq!(body_text(&full, 0), "A12");
    assert_eq!(body_text(&full, 1), "B2");
    assert!(full.notes().is_empty());
}

#[test]
fn field_codes_replace_body_page_fields_and_match_the_cache() {
    let mut document = document_with(&["A", "B"]);
    fit_one_line(&mut document);
    push_field(&mut document, 0, PageFieldKind::Page, Some("9"));
    push_field(&mut document, 0, PageFieldKind::NumPages, None);
    push_field(&mut document, 1, PageFieldKind::SectionPages, None);
    let view = ViewOptions {
        field_codes: true,
        ..ViewOptions::default()
    };
    let full = layout_with_view(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        view,
    )
    .unwrap();
    let mut cache = ParagraphLayoutCache::with_budget(1 << 20);
    let incremental = layout_incremental_with_view(
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
        view,
    )
    .unwrap();
    scenes_match(&full, &incremental);
    assert_eq!(body_text(&full, 0), "APAGENUMPAGES");
    assert_eq!(body_text(&full, 1), "BSECTIONPAGES");
    assert_ne!(full.key(), layout(&document).key());
    assert_eq!(
        full.key().content_hash(),
        layout(&document).key().content_hash()
    );
}

#[test]
fn an_unformattable_page_field_falls_back_to_its_cache() {
    let mut document = document_with(&["A"]);
    document.sections_mut()[0].numbering = PageNumbering {
        format: NumberFormat::UpperRoman,
        start: Some(0),
    };
    push_field(&mut document, 0, PageFieldKind::Page, Some("iv"));
    let snapshot = layout(&document);
    assert_eq!(body_text(&snapshot, 0), "Aiv");
    assert!(
        snapshot
            .notes()
            .iter()
            .any(|note| { matches!(note, LayoutNote::PageFieldCached { .. }) })
    );
}

#[test]
fn section_pages_count_pages_opened_by_that_section() {
    let mut document = document_with(&["A", "B", "C"]);
    fit_one_line(&mut document);
    close_section(&mut document, 0);
    push_field(&mut document, 0, PageFieldKind::SectionPages, None);
    push_field(&mut document, 2, PageFieldKind::SectionPages, None);
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 3);
    assert_eq!(body_text(&snapshot, 0), "A1");
    assert_eq!(body_text(&snapshot, 2), "C2");
}

#[test]
fn next_even_odd_and_continuous_breaks_place_sections() {
    let mut next = document_with(&["A", "B"]);
    fit_one_line(&mut next);
    close_section(&mut next, 0);
    let next_layout = layout(&next);
    assert_eq!(next_layout.pages().len(), 2);

    let mut even = document_with(&["A", "B"]);
    fit_one_line(&mut even);
    close_section(&mut even, 0);
    even.sections_mut().last_mut().unwrap().break_kind = SectionBreak::EvenPage;
    even.sections_mut().last_mut().unwrap().numbering.start = Some(5);
    header(&mut even, "", Some(PageFieldKind::Page));
    let even_layout = layout(&even);
    assert_eq!(even_layout.pages().len(), 2);
    assert!(page_text(&even_layout, 1).contains('5'));
    assert!(
        even_layout
            .notes()
            .iter()
            .any(|note| { matches!(note, LayoutNote::PageNumberParity { .. }) })
    );

    let mut odd = document_with(&["A", "B"]);
    fit_one_line(&mut odd);
    close_section(&mut odd, 0);
    odd.sections_mut().last_mut().unwrap().break_kind = SectionBreak::OddPage;
    header(&mut odd, "", Some(PageFieldKind::Page));
    let odd_layout = layout(&odd);
    assert_eq!(odd_layout.pages().len(), 3);
    assert!(page_text(&odd_layout, 1).contains('2'));
    assert!(!page_text(&odd_layout, 1).contains('B'));
    assert!(page_text(&odd_layout, 2).contains('3'));
    assert!(page_text(&odd_layout, 2).contains('B'));

    let mut shared = document_with(&["A", "B"]);
    close_section(&mut shared, 0);
    shared.sections_mut().last_mut().unwrap().break_kind = SectionBreak::Continuous;
    let shared_layout = layout(&shared);
    assert_eq!(shared_layout.pages().len(), 1);

    let mut split = document_with(&["A", "B"]);
    close_section(&mut split, 0);
    let section = split.sections_mut().last_mut().unwrap();
    section.break_kind = SectionBreak::Continuous;
    section.page.width = Measure::Emu(pt(80).raw());
    let split_layout = layout(&split);
    assert_eq!(split_layout.pages().len(), 2);
    assert!(
        split_layout
            .notes()
            .iter()
            .any(|note| { matches!(note, LayoutNote::ContinuousPageSplit { .. }) })
    );
}

#[test]
fn equal_columns_and_a_column_break_share_one_page() {
    let mut document = document_with(&["AB"]);
    document.sections_mut()[0].columns = Some(EqualColumns {
        count: 2,
        space: 720,
    });
    let id = document.ids().node();
    change(&mut document, 0, |paragraph| {
        paragraph
            .insert_inline(
                ScalarOffset(1),
                InlineObject {
                    id,
                    kind: InlineKind::Break(BreakKind::Column),
                    source: None,
                },
                None,
                Arc::default(),
            )
            .unwrap();
    });
    let snapshot = layout(&document);
    assert_eq!(snapshot.pages().len(), 1);
    let runs = &snapshot.pages()[0].runs;
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].text, "A");
    assert_eq!(runs[1].text, "B");
    assert!(runs[1].origin_x.raw() > runs[0].origin_x.raw());
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

#[test]
fn incremental_layout_matches_first_and_even_headers() {
    let mut document = document_with(&["A", "B", "C"]);
    fit_one_line(&mut document);
    document.sections_mut()[0].title_page = Some(true);
    document.settings_mut().even_and_odd_headers = Some(true);
    attach(&mut document, 0, true, HeaderFooterKind::First, "H1", None);
    attach(
        &mut document,
        0,
        true,
        HeaderFooterKind::Even,
        "H2",
        Some(PageFieldKind::Page),
    );
    attach(
        &mut document,
        0,
        true,
        HeaderFooterKind::Default,
        "H0",
        None,
    );
    close_section(&mut document, 0);
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
        let left_text: Vec<_> = left.runs.iter().map(|run| run.text.as_str()).collect();
        let right_text: Vec<_> = right.runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(left_text, right_text);
    }
    assert_eq!(full.notes(), incremental.notes());
    assert!(page_text(&full, 0).contains("H1"));
    assert!(page_text(&full, 1).contains("H2"));
    assert!(page_text(&full, 2).contains("H0"));
}
