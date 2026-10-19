//! T300: revision markup on header and footer bands of the shared page scene.

use std::sync::Arc;

use joyword_engine::{Command, Session, Transaction};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutInvalidation, LayoutLimits, LayoutReuse,
    LayoutSnapshot, LoadedFont, ParagraphLayoutCache, SingleFontProfile, layout_document_viewed,
    layout_incremental_viewed,
};
use joyword_model::{
    Block, BlockSeq, Document, HeaderFooterKind, InlineKind, InlineObject, Measure, PageField,
    PageFieldKind, PageSetup, Paragraph, RevisionKind, RevisionView, StoryId, StoryKind,
    TextRevision,
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

fn document() -> Document {
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(100).raw());
    page.height = Measure::Emu(pt(12 + 18 * 6).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.header = Measure::Emu(pt(2).raw());
    page.footer = Measure::Emu(pt(2).raw());
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut body = Paragraph::new(document.ids().node());
    body.push_text("BODY", None, Arc::default()).unwrap();
    let body_id = document.body_id();
    document.story_mut(body_id).unwrap().blocks = BlockSeq::from_vec(vec![body.into()]);
    document
}

fn mark(kind: RevisionKind, id: u32) -> TextRevision {
    TextRevision {
        id,
        kind,
        author: Arc::from("Ada"),
        date: None,
    }
}

fn attach(document: &mut Document, header: bool, paragraph: Paragraph) -> StoryId {
    let id = document.insert_story(
        if header {
            StoryKind::Header
        } else {
            StoryKind::Footer
        },
        vec![paragraph.into()],
    );
    let refs = if header {
        &mut document.sections_mut()[0].headers
    } else {
        &mut document.sections_mut()[0].footers
    };
    refs.push((HeaderFooterKind::Default, id));
    id
}

fn scene_text(snapshot: &LayoutSnapshot) -> String {
    snapshot.pages()[0]
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect()
}

fn layout(document: &Document, view: RevisionView) -> LayoutSnapshot {
    layout_document_viewed(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        view,
    )
    .unwrap()
}

fn story_plain(session: &Session, story: StoryId) -> String {
    session
        .document()
        .story(story)
        .unwrap()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect()
}

fn revision_ids(session: &Session, story: StoryId) -> Vec<u32> {
    session
        .document()
        .story(story)
        .unwrap()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|paragraph| paragraph.runs().iter())
        .filter_map(|run| run.revision.as_ref().map(|mark| mark.id))
        .collect()
}

#[test]
fn header_and_footer_markup_paints_and_projections_omit_it() {
    let mut document = document();
    let mut header = Paragraph::new(document.ids().node());
    header.push_text("KEEP", None, Arc::default()).unwrap();
    header
        .push_text_marked("NEW", None, Arc::default(), mark(RevisionKind::Insert, 3))
        .unwrap();
    header
        .push_text_marked("OLD", None, Arc::default(), mark(RevisionKind::Delete, 4))
        .unwrap();
    let mut moved = Paragraph::new(document.ids().node());
    moved
        .push_text_marked("SRC", None, Arc::default(), mark(RevisionKind::MoveFrom, 5))
        .unwrap();
    moved
        .push_text_marked("DST", None, Arc::default(), mark(RevisionKind::MoveTo, 6))
        .unwrap();
    let header_id = document.insert_story(StoryKind::Header, vec![header.into(), moved.into()]);
    document.sections_mut()[0]
        .headers
        .push((HeaderFooterKind::Default, header_id));
    let mut footer = Paragraph::new(document.ids().node());
    footer.push_text("FOOT", None, Arc::default()).unwrap();
    footer
        .push_text_marked("ADD", None, Arc::default(), mark(RevisionKind::Insert, 8))
        .unwrap();
    footer.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::PageField(PageField::new(PageFieldKind::Page)),
            source: None,
        },
        None,
        Arc::default(),
    );
    attach(&mut document, false, footer);

    let face = font();
    let faces = FaceSet::uniform(&face);
    let profile = profile();
    let cancellation = LayoutCancellation::default();
    let limits = LayoutLimits::default();
    for view in [
        RevisionView::FinalWithMarkup,
        RevisionView::Final,
        RevisionView::Original,
    ] {
        let full = layout_document_viewed(
            &document.snapshot(),
            faces,
            profile,
            limits,
            &cancellation,
            view,
        )
        .unwrap();
        let mut cache = ParagraphLayoutCache::default();
        let invalidation = LayoutInvalidation::default();
        let incremental = layout_incremental_viewed(
            &document.snapshot(),
            faces,
            profile,
            limits,
            &cancellation,
            LayoutReuse {
                cache: &mut cache,
                previous: None,
                invalidation: &invalidation,
            },
            view,
        )
        .unwrap();
        assert_eq!(full.pages(), incremental.pages(), "{view:?}");
    }

    let markup = layout(&document, RevisionView::FinalWithMarkup);
    let text = scene_text(&markup);
    assert!(
        text.contains("KEEP")
            && text.contains("NEW")
            && text.contains("OLD")
            && text.contains("SRC")
            && text.contains("DST")
            && text.contains("FOOT")
            && text.contains("ADD")
            && text.contains("BODY"),
        "{text}"
    );
    let content_left = markup.page_geometry()[0].left.raw();
    let body_baseline = markup.lines()[0].baseline.raw();
    let colored: Vec<_> = markup.pages()[0]
        .runs
        .iter()
        .filter(|run| run.color != [0, 0, 0])
        .collect();
    assert!(
        colored
            .iter()
            .any(|run| run.baseline_y.raw() < body_baseline),
        "header band has no author colour: {text}"
    );
    assert!(
        colored
            .iter()
            .any(|run| run.baseline_y.raw() > body_baseline + pt(40).raw()),
        "footer band has no author colour: {text}"
    );
    let bars: Vec<_> = markup.pages()[0]
        .rects
        .iter()
        .filter(|rect| rect.x.raw().saturating_add(rect.width.raw()) < content_left)
        .collect();
    assert!(
        bars.len() >= 3,
        "expected a margin bar on each revised header and footer line, got {}",
        bars.len()
    );
    assert!(
        bars.iter()
            .all(|rect| rect.width.raw() > 0 && rect.height.raw() > 0)
    );
    let rules = markup.pages()[0].rects.iter().filter(|rect| {
        rect.x.raw().saturating_add(rect.width.raw()) >= content_left
            && (rect.red != 0 || rect.green != 0 || rect.blue != 0)
            && rect.red != 128
    });
    assert!(
        rules.count() >= 5,
        "insertion underline and deletion strike were not both painted"
    );

    let final_view = layout(&document, RevisionView::Final);
    let original = layout(&document, RevisionView::Original);
    let final_text = scene_text(&final_view);
    let original_text = scene_text(&original);
    assert!(
        final_text.contains("NEW")
            && final_text.contains("DST")
            && final_text.contains("ADD")
            && !final_text.contains("OLD")
            && !final_text.contains("SRC"),
        "{final_text}"
    );
    assert!(
        original_text.contains("OLD")
            && original_text.contains("SRC")
            && !original_text.contains("NEW")
            && !original_text.contains("DST")
            && !original_text.contains("ADD"),
        "{original_text}"
    );
    for (name, scene) in [("final", &final_view), ("original", &original)] {
        assert!(
            scene.pages()[0]
                .runs
                .iter()
                .all(|run| run.color == [0, 0, 0]),
            "{name} coloured a run"
        );
        let outside = scene.pages()[0]
            .rects
            .iter()
            .any(|rect| rect.x.raw().saturating_add(rect.width.raw()) < content_left);
        assert!(!outside, "{name} painted a margin bar");
    }
}

#[test]
fn accepting_or_rejecting_a_header_revision_is_one_undo() {
    let mut document = document();
    let mut header = Paragraph::new(document.ids().node());
    header.push_text("KEEP", None, Arc::default()).unwrap();
    header
        .push_text_marked("NEW", None, Arc::default(), mark(RevisionKind::Insert, 3))
        .unwrap();
    header
        .push_text_marked("OLD", None, Arc::default(), mark(RevisionKind::Delete, 4))
        .unwrap();
    let story = attach(&mut document, true, header);
    let mut session = Session::new(document);
    assert_eq!(story_plain(&session, story), "KEEPNEWOLD");

    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::AcceptAllRevisions],
        ))
        .unwrap();
    assert_eq!(session.undo_depth(), 1);
    assert_eq!(story_plain(&session, story), "KEEPNEW");
    assert!(revision_ids(&session, story).is_empty());
    session.undo().unwrap();
    assert_eq!(session.undo_depth(), 0);
    assert_eq!(story_plain(&session, story), "KEEPNEWOLD");
    assert_eq!(revision_ids(&session, story), vec![3, 4]);

    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::RejectAllRevisions],
        ))
        .unwrap();
    assert_eq!(session.undo_depth(), 1);
    assert_eq!(story_plain(&session, story), "KEEPOLD");
    assert!(revision_ids(&session, story).is_empty());
    session.undo().unwrap();
    assert_eq!(story_plain(&session, story), "KEEPNEWOLD");
    assert_eq!(revision_ids(&session, story), vec![3, 4]);
}
