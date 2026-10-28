//! Footnote and Endnote management (T501, LAYOUT-007).

use std::sync::Arc;

use joyword_model::{
    Anchor, AnnotationId, AnnotationKind, Block, Document, Paragraph, Position, RunProps, StoryId,
    StoryKind,
};

/// Insert a footnote into the document anchored at `pos`.
pub fn insert_footnote(doc: &mut Document, pos: Position, text: &str) -> (AnnotationId, StoryId) {
    insert_note_internal(doc, pos, text, StoryKind::Footnote)
}

/// Insert an endnote into the document anchored at `pos`.
pub fn insert_endnote(doc: &mut Document, pos: Position, text: &str) -> (AnnotationId, StoryId) {
    insert_note_internal(doc, pos, text, StoryKind::Endnote)
}

fn insert_note_internal(
    doc: &mut Document,
    pos: Position,
    text: &str,
    kind: StoryKind,
) -> (AnnotationId, StoryId) {
    // Create a story with a single paragraph containing the note text
    let para_id = doc.ids().node();
    let mut para = Paragraph::new(para_id);
    if !text.is_empty() {
        let _ = para.push_text(text, None, Arc::new(RunProps::default()));
    }

    let story_id = doc.insert_story(kind, vec![Block::Paragraph(Arc::new(para))]);

    let anno_kind = match kind {
        StoryKind::Endnote => AnnotationKind::Endnote {
            story: story_id,
            custom_mark: None,
        },
        _ => AnnotationKind::Footnote {
            story: story_id,
            custom_mark: None,
        },
    };

    let anchor = Anchor::Range {
        start: pos,
        end: pos,
    };

    let anno_id = doc.insert_annotation(anno_kind, anchor);
    (anno_id, story_id)
}

/// Remove a footnote or endnote annotation and its backing story.
pub fn remove_note(doc: &mut Document, note_id: AnnotationId) -> Result<StoryId, &'static str> {
    let anno = doc.annotation(note_id).ok_or("annotation not found")?;
    let story_id = match &anno.kind {
        AnnotationKind::Footnote { story, .. } | AnnotationKind::Endnote { story, .. } => *story,
        _ => return Err("not a footnote or endnote annotation"),
    };

    doc.remove_annotation(note_id);
    doc.remove_story(story_id);
    Ok(story_id)
}

/// Retrieve the plain text of a footnote or endnote story.
pub fn get_note_text(doc: &Document, story_id: StoryId) -> Option<String> {
    let story = doc.story(story_id)?;
    let mut text = String::new();
    for block in story.blocks.iter() {
        if let Block::Paragraph(p) = block {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&p.raw_text());
        }
    }
    Some(text)
}

/// List all footnotes or endnotes currently in the document.
pub fn list_notes(doc: &Document, endnote: bool) -> Vec<(AnnotationId, StoryId, String)> {
    let mut notes = Vec::new();
    for anno in doc.annotations() {
        let maybe_story = match &anno.kind {
            AnnotationKind::Footnote { story, .. } if !endnote => Some(*story),
            AnnotationKind::Endnote { story, .. } if endnote => Some(*story),
            _ => None,
        };
        if let Some(story_id) = maybe_story {
            let text = get_note_text(doc, story_id).unwrap_or_default();
            notes.push((anno.id, story_id, text));
        }
    }
    notes
}
