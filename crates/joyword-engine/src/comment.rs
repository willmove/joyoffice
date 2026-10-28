//! Comment commands (SPEC REVIEW-005, REVIEW-006, REVIEW-008).
//!
//! A comment is an annotation plus a comment story. Resolve changes a flag;
//! it does not detach the anchor or delete the story. Deleting the anchored
//! text detaches the comment through the session's position map.

use std::sync::Arc;

use joyword_model::{
    Anchor, AnnotationKind, Bias, Block, Comment, Document, NodeId, Paragraph, StoryKind, TextError,
};

use crate::command::{CommandError, check_text};
use crate::locate;
use crate::session::ChangeSet;

const MAX_AUTHOR: usize = 255;
const MAX_INITIALS: usize = 64;
const MAX_DATE: usize = 64;
const MAX_TEXT: usize = 32_768;

pub(crate) struct Draft<'a> {
    pub author: &'a str,
    pub date: Option<&'a str>,
    pub initials: Option<&'a str>,
    pub text: &'a str,
}

pub(crate) fn add(
    document: &mut Document,
    start: &joyword_model::Position,
    end: &joyword_model::Position,
    draft: Draft<'_>,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let author = checked_author(draft.author)?;
    let date = checked_optional(draft.date, MAX_DATE)?;
    let initials = checked_optional(draft.initials, MAX_INITIALS)?;
    checked_body(draft.text)?;
    if start.story != end.story {
        return Err(CommandError::CrossContainer);
    }
    checked_point(document, start)?;
    checked_point(document, end)?;
    let start_key = position_key(document, start)?;
    let end_key = position_key(document, end)?;
    if start_key > end_key {
        return Err(CommandError::ReversedRange);
    }
    let mut stored_start = *start;
    stored_start.bias = Bias::Before;
    let mut stored_end = *end;
    stored_end.bias = Bias::After;
    let (annotation, story, paragraph) = allocate(
        document,
        author,
        date,
        initials,
        draft.text,
        None,
        Anchor::Range {
            start: stored_start,
            end: stored_end,
        },
    )?;
    changes.annotations.insert(annotation);
    changes.stories.insert(story);
    changes.inserted.insert(paragraph);
    Ok(())
}

pub(crate) fn reply(
    document: &mut Document,
    parent: joyword_model::AnnotationId,
    draft: Draft<'_>,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let author = checked_author(draft.author)?;
    let date = checked_optional(draft.date, MAX_DATE)?;
    let initials = checked_optional(draft.initials, MAX_INITIALS)?;
    checked_body(draft.text)?;
    let (anchor, parent_para) = {
        let annotation = document
            .annotation(parent)
            .ok_or(CommandError::OrphanReply(parent))?;
        let AnnotationKind::Comment(comment) = &annotation.kind else {
            return Err(CommandError::OrphanReply(parent));
        };
        let Some(para_id) = comment.para_id.clone() else {
            return Err(CommandError::CommentThreadUnavailable);
        };
        (annotation.anchor.clone(), para_id)
    };
    let (annotation, story, paragraph) = allocate(
        document,
        author,
        date,
        initials,
        draft.text,
        Some(parent_para),
        anchor,
    )?;
    changes.annotations.insert(annotation);
    changes.stories.insert(story);
    changes.inserted.insert(paragraph);
    Ok(())
}

pub(crate) fn resolve(
    document: &mut Document,
    id: joyword_model::AnnotationId,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    set_resolved(document, id, true, changes)
}

pub(crate) fn reopen(
    document: &mut Document,
    id: joyword_model::AnnotationId,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    set_resolved(document, id, false, changes)
}

fn set_resolved(
    document: &mut Document,
    id: joyword_model::AnnotationId,
    resolved: bool,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let annotation = document
        .annotation_mut(id)
        .ok_or(CommandError::UnknownComment(id))?;
    let AnnotationKind::Comment(comment) = &mut annotation.kind else {
        return Err(CommandError::UnknownComment(id));
    };
    if comment.resolved == resolved {
        return Err(if resolved {
            CommandError::CommentAlreadyResolved(id)
        } else {
            CommandError::CommentNotResolved(id)
        });
    }
    comment.resolved = resolved;
    changes.annotations.insert(id);
    Ok(())
}

fn allocate(
    document: &mut Document,
    author: Arc<str>,
    date: Option<Arc<str>>,
    initials: Option<Arc<str>>,
    text: &str,
    parent_para_id: Option<Arc<str>>,
    anchor: Anchor,
) -> Result<(joyword_model::AnnotationId, joyword_model::StoryId, NodeId), CommandError> {
    let source_id = document
        .allocate_comment_id()
        .ok_or(CommandError::CommentThreadUnavailable)?;
    let para_id = document
        .allocate_para_id()
        .ok_or(CommandError::CommentThreadUnavailable)?;
    let durable_id = document
        .allocate_durable_id(&para_id)
        .ok_or(CommandError::CommentThreadUnavailable)?;
    let mut paragraph = Paragraph::new(document.ids().node());
    let paragraph_id = paragraph.id();
    if !text.is_empty() {
        paragraph
            .push_text(text, None, Arc::default())
            .map_err(CommandError::Text)?;
    }
    let story = document.insert_story(StoryKind::Comment, vec![Block::from(paragraph)]);
    let mut comment = Comment::new(story);
    comment.source_id = source_id;
    comment.author = Some(author);
    comment.date = date;
    comment.initials = initials;
    comment.para_id = Some(para_id);
    comment.durable_id = Some(durable_id);
    comment.parent_para_id = parent_para_id;
    // The wild commentsExtensible parts repeat `w:date` verbatim as
    // `w16cex:dateUtc`; a comment created with that shape already knows its
    // UTC stamp, so a reimport that reads the part back compares equal.
    if comment
        .date
        .as_deref()
        .is_some_and(joyword_model::is_strict_utc_date)
    {
        comment.date_utc = comment.date.clone();
    }
    let annotation = document.insert_annotation(AnnotationKind::Comment(comment), anchor);
    Ok((annotation, story, paragraph_id))
}

fn checked_author(author: &str) -> Result<Arc<str>, CommandError> {
    if author.is_empty() || author.chars().count() > MAX_AUTHOR {
        return Err(CommandError::CommentAuthorMissing);
    }
    check_text(author)?;
    Ok(Arc::from(author))
}

fn checked_optional(value: Option<&str>, limit: usize) -> Result<Option<Arc<str>>, CommandError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > limit {
        return Err(CommandError::CommentRangeInvalid);
    }
    check_text(value)?;
    Ok(Some(Arc::from(value)))
}

fn checked_body(text: &str) -> Result<(), CommandError> {
    if text.chars().count() > MAX_TEXT {
        return Err(CommandError::CommentRangeInvalid);
    }
    check_text(text)
}

fn checked_point(document: &Document, at: &joyword_model::Position) -> Result<(), CommandError> {
    if document.story(at.story).is_none() {
        return Err(CommandError::UnknownStory(at.story));
    }
    let location = locate::locate(document, at.story, at.paragraph)
        .ok_or(CommandError::UnknownNode(at.paragraph))?;
    let paragraph =
        locate::paragraph(document, &location).ok_or(CommandError::NotAParagraph(at.paragraph))?;
    if at.offset.0 > paragraph.len() {
        return Err(CommandError::Text(TextError::OutOfBounds {
            offset: at.offset.0,
            len: paragraph.len(),
        }));
    }
    if location.content_locked {
        return Err(CommandError::Locked(at.paragraph));
    }
    Ok(())
}

fn position_key(
    document: &Document,
    at: &joyword_model::Position,
) -> Result<(u32, usize), CommandError> {
    let order = crate::find::paragraph_order(document, at.story, at.paragraph)
        .ok_or(CommandError::UnknownNode(at.paragraph))?;
    Ok((order, at.offset.0))
}
