use joyword_layout::view::{ViewMode, ViewOptions, project_paragraph};
use joyword_model::{Block, Document, Paragraph, RunProps, Style, StyleKind};
use std::sync::Arc;

#[test]
fn test_view_mode_switching_does_not_mutate_document() {
    let mut doc = Document::new_blank();
    let body_id = doc.body_id();

    // Style for Heading 1
    let style = Style::new("Heading1", StyleKind::Paragraph, Some("heading 1"));
    let h1_style_id = style.id.clone();
    doc.styles_mut().insert(style).unwrap();

    let h1_id = doc.ids().node();
    let mut h1 = Paragraph::new(h1_id);
    h1.style = Some(h1_style_id);
    let _ = h1.push_text(
        "Chapter 1: Architecture",
        None,
        Arc::new(RunProps::default()),
    );

    let body_para_id = doc.ids().node();
    let mut body_para = Paragraph::new(body_para_id);
    let _ = body_para.push_text(
        "This is the main body text describing the system.",
        None,
        Arc::new(RunProps::default()),
    );

    if let Some(story) = doc.story_mut(body_id) {
        story.blocks = joyword_model::BlockSeq::from_vec(vec![
            Block::Paragraph(Arc::new(h1.clone())),
            Block::Paragraph(Arc::new(body_para.clone())),
        ]);
    }

    // Capture initial document state snapshot
    let initial_annotations_count = doc.annotations().count();
    let initial_stories_count = doc.body().blocks.len();

    // 1. Print View
    let print_view = ViewOptions {
        mode: ViewMode::Print,
        ..Default::default()
    };
    let proj_h1_print = project_paragraph(&doc, &h1, print_view).unwrap();
    let proj_body_print = project_paragraph(&doc, &body_para, print_view).unwrap();
    assert_eq!(proj_h1_print.text, "Chapter 1: Architecture");
    assert_eq!(
        proj_body_print.text,
        "This is the main body text describing the system."
    );

    // 2. Draft View
    let draft_view = ViewOptions {
        mode: ViewMode::Draft,
        ..Default::default()
    };
    let proj_h1_draft = project_paragraph(&doc, &h1, draft_view).unwrap();
    let proj_body_draft = project_paragraph(&doc, &body_para, draft_view).unwrap();
    assert_eq!(proj_h1_draft.text, "Chapter 1: Architecture");
    assert_eq!(
        proj_body_draft.text,
        "This is the main body text describing the system."
    );

    // 3. Outline View: Heading is kept, Body text is collapsed/omitted
    let outline_view = ViewOptions {
        mode: ViewMode::Outline,
        ..Default::default()
    };
    let proj_h1_outline = project_paragraph(&doc, &h1, outline_view).unwrap();
    let proj_body_outline = project_paragraph(&doc, &body_para, outline_view).unwrap();
    assert_eq!(proj_h1_outline.text, "Chapter 1: Architecture");
    assert_eq!(proj_body_outline.text, ""); // Body text omitted in outline view

    // 4. Reading View
    let reading_view = ViewOptions {
        mode: ViewMode::Reading,
        ..Default::default()
    };
    let proj_h1_reading = project_paragraph(&doc, &h1, reading_view).unwrap();
    let proj_body_reading = project_paragraph(&doc, &body_para, reading_view).unwrap();
    assert_eq!(proj_h1_reading.text, "Chapter 1: Architecture");
    assert_eq!(
        proj_body_reading.text,
        "This is the main body text describing the system."
    );

    // Verify document was NEVER modified during view switching
    assert_eq!(doc.annotations().count(), initial_annotations_count);
    assert_eq!(doc.body().blocks.len(), initial_stories_count);
    assert_eq!(doc.body_id(), body_id);
}
