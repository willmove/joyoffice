use std::sync::Arc;

use joyword_engine::fields::{
    FieldError, FieldInstruction, parse_field_instruction, update_fields,
};
use joyword_engine::notes::{
    get_note_text, insert_endnote, insert_footnote, list_notes, remove_note,
};
use joyword_model::{
    Anchor, AnnotationKind, Bias, Document, Paragraph, Position, RunProps, ScalarOffset, Style,
    StyleKind,
};

#[test]
fn test_footnotes_and_endnotes_workflow() {
    let mut doc = Document::new_blank();
    let para_id = doc
        .body()
        .blocks
        .iter()
        .next()
        .unwrap()
        .as_paragraph()
        .unwrap()
        .id();

    let pos = Position {
        story: doc.body_id(),
        paragraph: para_id,
        offset: ScalarOffset(0),
        bias: Bias::After,
    };

    // 1. Insert Footnote
    let (fn_anno_id, fn_story_id) =
        insert_footnote(&mut doc, pos, "First footnote annotation content");
    assert_eq!(
        get_note_text(&doc, fn_story_id).as_deref(),
        Some("First footnote annotation content")
    );

    // 2. Insert Endnote
    let (ed_anno_id, ed_story_id) = insert_endnote(&mut doc, pos, "End of document reference");
    assert_eq!(
        get_note_text(&doc, ed_story_id).as_deref(),
        Some("End of document reference")
    );

    // 3. List notes
    let footnotes = list_notes(&doc, false);
    assert_eq!(footnotes.len(), 1);
    assert_eq!(footnotes[0].0, fn_anno_id);

    let endnotes = list_notes(&doc, true);
    assert_eq!(endnotes.len(), 1);
    assert_eq!(endnotes[0].0, ed_anno_id);

    // 4. Remove note
    let removed = remove_note(&mut doc, fn_anno_id).unwrap();
    assert_eq!(removed, fn_story_id);
    assert!(doc.story(fn_story_id).is_none());
    assert_eq!(list_notes(&doc, false).len(), 0);
}

#[test]
fn test_field_instruction_parsing() {
    assert_eq!(
        parse_field_instruction("TOC \\o \"1-3\" \\h"),
        FieldInstruction::Toc {
            min_level: 1,
            max_level: 3,
            hyperlinks: true,
        }
    );

    assert_eq!(
        parse_field_instruction("REF \"section_intro\""),
        FieldInstruction::Ref {
            bookmark: "section_intro".into(),
        }
    );

    assert_eq!(
        parse_field_instruction("PAGEREF target_bm"),
        FieldInstruction::PageRef {
            bookmark: "target_bm".into(),
        }
    );

    assert_eq!(
        parse_field_instruction("SEQ Figure"),
        FieldInstruction::Seq {
            identifier: "Figure".into(),
        }
    );
}

#[test]
fn test_field_seq_and_ref_evaluation() {
    let mut doc = Document::new_blank();
    let body_id = doc.body_id();
    let para_id = doc
        .body()
        .blocks
        .iter()
        .next()
        .unwrap()
        .as_paragraph()
        .unwrap()
        .id();

    // Set text in first paragraph
    if let Some(story) = doc.story_mut(body_id) {
        let mut para = Paragraph::new(para_id);
        let _ = para.push_text(
            "Referenced Section Title",
            None,
            Arc::new(RunProps::default()),
        );
        story.blocks = joyword_model::BlockSeq::from_vec(vec![joyword_model::Block::Paragraph(
            Arc::new(para),
        )]);
    }

    // Add bookmark "intro" covering the paragraph
    let bm_pos_start = Position {
        story: body_id,
        paragraph: para_id,
        offset: ScalarOffset(0),
        bias: Bias::After,
    };
    let bm_pos_end = Position {
        story: body_id,
        paragraph: para_id,
        offset: ScalarOffset(24),
        bias: Bias::Before,
    };
    doc.insert_annotation(
        AnnotationKind::Bookmark {
            name: "intro".into(),
        },
        Anchor::Range {
            start: bm_pos_start,
            end: bm_pos_end,
        },
    );

    // Create a paragraph with SEQ and REF fields
    let field_para_id = doc.ids().node();
    let mut field_para = Paragraph::new(field_para_id);
    let _ = field_para.push_text(
        "SEQ Figure:  REF intro: ",
        None,
        Arc::new(RunProps::default()),
    );
    let new_block = joyword_model::Block::Paragraph(Arc::new(field_para));
    if let Some(story) = doc.story_mut(body_id) {
        let mut blocks = story.blocks.iter().cloned().collect::<Vec<_>>();
        blocks.push(new_block);
        story.blocks = joyword_model::BlockSeq::from_vec(blocks);
    }

    // Insert SEQ field
    doc.insert_annotation(
        AnnotationKind::Field {
            separator: Some(Position {
                story: body_id,
                paragraph: field_para_id,
                offset: ScalarOffset(10),
                bias: Bias::After,
            }),
        },
        Anchor::Range {
            start: Position {
                story: body_id,
                paragraph: field_para_id,
                offset: ScalarOffset(0),
                bias: Bias::After,
            },
            end: Position {
                story: body_id,
                paragraph: field_para_id,
                offset: ScalarOffset(11),
                bias: Bias::Before,
            },
        },
    );

    // Insert REF field
    doc.insert_annotation(
        AnnotationKind::Field {
            separator: Some(Position {
                story: body_id,
                paragraph: field_para_id,
                offset: ScalarOffset(22),
                bias: Bias::After,
            }),
        },
        Anchor::Range {
            start: Position {
                story: body_id,
                paragraph: field_para_id,
                offset: ScalarOffset(13),
                bias: Bias::After,
            },
            end: Position {
                story: body_id,
                paragraph: field_para_id,
                offset: ScalarOffset(23),
                bias: Bias::Before,
            },
        },
    );

    let stats = update_fields(&mut doc, None, 10).expect("update fields");
    assert!(stats.fields_evaluated >= 2);
}

#[test]
fn test_field_circular_reference_detection() {
    let mut doc = Document::new_blank();
    let body_id = doc.body_id();
    let para_id = doc
        .body()
        .blocks
        .iter()
        .next()
        .unwrap()
        .as_paragraph()
        .unwrap()
        .id();

    // Create a paragraph containing REF to itself
    if let Some(story) = doc.story_mut(body_id) {
        let mut para = Paragraph::new(para_id);
        let _ = para.push_text("REF loop_mark", None, Arc::new(RunProps::default()));
        story.blocks = joyword_model::BlockSeq::from_vec(vec![joyword_model::Block::Paragraph(
            Arc::new(para),
        )]);
    }

    let pos_start = Position {
        story: body_id,
        paragraph: para_id,
        offset: ScalarOffset(0),
        bias: Bias::After,
    };
    let pos_end = Position {
        story: body_id,
        paragraph: para_id,
        offset: ScalarOffset(13),
        bias: Bias::Before,
    };

    // Bookmark "loop_mark" on the REF field
    doc.insert_annotation(
        AnnotationKind::Bookmark {
            name: "loop_mark".into(),
        },
        Anchor::Range {
            start: pos_start,
            end: pos_end,
        },
    );

    // Field pointing to "loop_mark"
    doc.insert_annotation(
        AnnotationKind::Field {
            separator: Some(pos_end),
        },
        Anchor::Range {
            start: pos_start,
            end: pos_end,
        },
    );

    let err = update_fields(&mut doc, None, 10).unwrap_err();
    assert!(matches!(err, FieldError::CircularReference(ref name) if name == "loop_mark"));
}

#[test]
fn test_toc_generation_from_headings() {
    let mut doc = Document::new_blank();
    let body_id = doc.body_id();

    // Register style "heading 1"
    let style = Style::new("Heading1", StyleKind::Paragraph, Some("heading 1"));
    let heading_style_id = style.id.clone();
    doc.styles_mut().insert(style).unwrap();

    let h1_id = doc.ids().node();
    let mut h1 = Paragraph::new(h1_id);
    h1.style = Some(heading_style_id);
    let _ = h1.push_text(
        "Chapter 1: Foundations",
        None,
        Arc::new(RunProps::default()),
    );

    let toc_id = doc.ids().node();
    let mut toc_p = Paragraph::new(toc_id);
    let _ = toc_p.push_text("TOC \\o \"1-3\" \\h", None, Arc::new(RunProps::default()));

    if let Some(story) = doc.story_mut(body_id) {
        story.blocks = joyword_model::BlockSeq::from_vec(vec![
            joyword_model::Block::Paragraph(Arc::new(toc_p)),
            joyword_model::Block::Paragraph(Arc::new(h1)),
        ]);
    }

    doc.insert_annotation(
        AnnotationKind::Field { separator: None },
        Anchor::Range {
            start: Position {
                story: body_id,
                paragraph: toc_id,
                offset: ScalarOffset(0),
                bias: Bias::After,
            },
            end: Position {
                story: body_id,
                paragraph: toc_id,
                offset: ScalarOffset(15),
                bias: Bias::Before,
            },
        },
    );

    let stats = update_fields(&mut doc, None, 10).expect("update toc");
    assert_eq!(stats.toc_entries, 1);
}
