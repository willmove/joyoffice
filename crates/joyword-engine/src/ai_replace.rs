//! One AI-authored plain-text replacement (T402, AI.md §3, SPEC §8).
//!
//! Offsets are Unicode scalars in the body's final-view text: paragraph
//! final text joined by `\n`, the same projection an agent snapshot uses
//! when the host builds it from [`body_final_paragraphs`]. A range that
//! crosses a paragraph, touches a non-paragraph block, includes an inline
//! object, or is not one contiguous run of final-view scalars is refused
//! before any command runs. The replacement itself is ordinary text: controls
//! other than tab, line separators and the inline-object marker are refused.
//!
//! The committed transaction is [`Origin::Agent`], [`UndoGroup::Standalone`],
//! and tracked even when `w:trackRevisions` is off. Both the deleted span and
//! the inserted text carry [`AI_REVISION_AUTHOR`], not the session's user
//! author. Undo restores the previous text in one step.

use std::sync::Arc;

use joyword_model::{
    Bias, Block, Document, DocumentVersion, NodeId, Paragraph, Position, RevisionView, ScalarOffset,
};

use crate::command::{Command, check_text};
use crate::map::Selection;
use crate::session::{Origin, Outcome, Session, Transaction, TxError, UndoGroup};

/// `w:author` stored on revisions created by [`apply_ai_text_replace`].
pub const AI_REVISION_AUTHOR: &str = "JoyWord AI";

/// Final-view text of each body paragraph, in order. `Err` when the body
/// contains a non-paragraph block or a paragraph whose runs do not cover its
/// text; callers must not guess a projection in that case.
pub fn body_final_paragraphs(document: &Document) -> Result<Vec<String>, TxError> {
    let mut out = Vec::new();
    for paragraph in body_paragraphs(document)? {
        let chars = final_chars(paragraph).map_err(|_| TxError::AiRangeRefused)?;
        out.push(chars.into_iter().map(|(_, ch)| ch).collect());
    }
    Ok(out)
}

/// Replace the final-view character span `[start, end)` with `replacement`.
/// `base_version` must be the session's current version. Failure leaves the
/// session, its version and its undo stack untouched.
pub fn apply_ai_text_replace(
    session: &mut Session,
    base_version: DocumentVersion,
    start: u64,
    end: u64,
    replacement: &str,
) -> Result<Outcome, TxError> {
    if session.version() != base_version {
        return Err(TxError::StaleVersion {
            base: base_version,
            current: session.version(),
        });
    }
    if session.policy().read_only {
        return Err(TxError::ReadOnly);
    }
    if !session.policy().agent_writes {
        return Err(TxError::AgentWritesDisabled);
    }
    if !replacement_allowed(replacement) {
        return Err(TxError::AiReplacementRefused);
    }
    let span = locate(session.document(), start, end)?;
    let story = session.document().body_id();
    let at = Position {
        story,
        paragraph: span.paragraph,
        offset: ScalarOffset(span.scalar_start),
        bias: Bias::After,
    };
    let to = Position {
        story,
        paragraph: span.paragraph,
        offset: ScalarOffset(span.scalar_end),
        bias: Bias::Before,
    };
    let mut commands = vec![Command::DeleteRange { start: at, end: to }];
    if !replacement.is_empty() {
        commands.push(Command::InsertText {
            at,
            text: replacement.to_owned(),
        });
    }
    let mut transaction = Transaction::new(base_version, commands);
    transaction.origin = Origin::Agent;
    transaction.undo_group = UndoGroup::Standalone;
    transaction.tracked_author = Some(Arc::from(AI_REVISION_AUTHOR));
    transaction.selection = Some(Selection {
        anchor: at,
        head: to,
    });
    session.apply(transaction)
}

struct Span {
    paragraph: NodeId,
    scalar_start: usize,
    scalar_end: usize,
}

fn replacement_allowed(text: &str) -> bool {
    if check_text(text).is_err() {
        return false;
    }
    !text
        .chars()
        .any(|ch| ch == joyword_model::OBJECT_MARKER || ch == '\u{FFFC}')
}

fn body_paragraphs(document: &Document) -> Result<Vec<&Paragraph>, TxError> {
    let mut out = Vec::new();
    for block in document.body().blocks.iter() {
        match block {
            Block::Paragraph(paragraph) => out.push(paragraph.as_ref()),
            _ => return Err(TxError::AiRangeRefused),
        }
    }
    Ok(out)
}

fn final_chars(paragraph: &Paragraph) -> Result<Vec<(usize, char)>, ()> {
    let plain = paragraph.plain_text(|_| "\u{FFFC}".to_owned());
    let mut iter = plain.chars();
    let mut scalar = 0usize;
    let mut visible = Vec::new();
    for run in paragraph.runs() {
        let shown = match &run.revision {
            Some(revision) => revision.kind.visible_in(RevisionView::Final),
            None => true,
        };
        for _ in 0..run.len {
            let Some(ch) = iter.next() else {
                return Err(());
            };
            if shown {
                visible.push((scalar, ch));
            }
            scalar = scalar.checked_add(1).ok_or(())?;
        }
    }
    if iter.next().is_some() || scalar != paragraph.len() {
        return Err(());
    }
    Ok(visible)
}

fn locate(document: &Document, start: u64, end: u64) -> Result<Span, TxError> {
    if start >= end {
        return Err(TxError::AiRangeRefused);
    }
    let mut cursor: u64 = 0;
    let mut seen = false;
    for paragraph in body_paragraphs(document)? {
        if seen {
            if start < cursor.saturating_add(1) && end > cursor {
                return Err(TxError::AiRangeRefused);
            }
            cursor = cursor.checked_add(1).ok_or(TxError::AiRangeRefused)?;
        }
        seen = true;
        let chars = final_chars(paragraph).map_err(|_| TxError::AiRangeRefused)?;
        let para_len = u64::try_from(chars.len()).map_err(|_| TxError::AiRangeRefused)?;
        let para_end = cursor
            .checked_add(para_len)
            .ok_or(TxError::AiRangeRefused)?;
        if start >= cursor && end <= para_end {
            return span_of(paragraph.id(), &chars, start - cursor, end - cursor);
        }
        if start < para_end && end > para_end {
            return Err(TxError::AiRangeRefused);
        }
        cursor = para_end;
    }
    Err(TxError::AiRangeRefused)
}

fn span_of(
    paragraph: NodeId,
    chars: &[(usize, char)],
    local_start: u64,
    local_end: u64,
) -> Result<Span, TxError> {
    let local_start = usize::try_from(local_start).map_err(|_| TxError::AiRangeRefused)?;
    let local_end = usize::try_from(local_end).map_err(|_| TxError::AiRangeRefused)?;
    if local_start >= local_end || local_end > chars.len() {
        return Err(TxError::AiRangeRefused);
    }
    let Some((scalar_start, _)) = chars.get(local_start).copied() else {
        return Err(TxError::AiRangeRefused);
    };
    let Some((last, _)) = chars.get(local_end - 1).copied() else {
        return Err(TxError::AiRangeRefused);
    };
    if last < scalar_start || last - scalar_start + 1 != local_end - local_start {
        return Err(TxError::AiRangeRefused);
    }
    for (offset, (scalar, ch)) in chars[local_start..local_end].iter().enumerate() {
        if *scalar != scalar_start + offset
            || *ch == '\u{FFFC}'
            || *ch == joyword_model::OBJECT_MARKER
        {
            return Err(TxError::AiRangeRefused);
        }
    }
    let Some(scalar_end) = last.checked_add(1) else {
        return Err(TxError::AiRangeRefused);
    };
    Ok(Span {
        paragraph,
        scalar_start,
        scalar_end,
    })
}
