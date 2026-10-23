//! One AI-authored plain-text replacement (T402, AI.md §3, SPEC §8).
//!
//! Offsets are Unicode scalars in the body's final-view text: paragraph
//! final text joined by `\n`, the same projection an agent snapshot uses
//! when the host builds it from [`body_final_paragraphs`]. A paragraph
//! whose mark carries a delete revision joins with the next paragraph
//! without a separator. A range that touches a non-paragraph block,
//! includes an inline object, hides part of a covered paragraph's runs, or
//! is not one contiguous run of final-view scalars is refused before any
//! command runs. The replacement itself is one line of ordinary text:
//! controls other than tab, line separators and the inline-object marker
//! are refused, so the replace never creates or removes paragraph marks
//! beyond the ones inside the covered range.
//!
//! A range may cross paragraph marks. The delete then records the covered
//! runs and every paragraph mark between the first and last paragraph as
//! `JoyWord AI` deletions, exactly like a tracked Word delete of a
//! multi-paragraph selection; accepting the revision merges the
//! paragraphs. The committed transaction is [`Origin::Agent`],
//! [`UndoGroup::Standalone`], and tracked even when `w:trackRevisions` is
//! off. Both the deleted span and the inserted text carry
//! [`AI_REVISION_AUTHOR`], not the session's user author. Undo restores
//! the previous text in one step.

use std::sync::Arc;

use joyword_model::{
    Bias, Block, Document, DocumentVersion, NodeId, Paragraph, Position, RevisionKind,
    RevisionView, ScalarOffset,
};

use crate::command::{Command, check_text};
use crate::map::Selection;
use crate::session::{Origin, Outcome, Session, Transaction, TxError, UndoGroup};

/// `w:author` stored on revisions created by [`apply_ai_text_replace`].
pub const AI_REVISION_AUTHOR: &str = "JoyWord AI";

/// Final-view text of the body, one entry per final-view paragraph:
/// neighbouring paragraphs whose mark carries a delete revision are
/// already joined into one entry. `Err` when the body contains a
/// non-paragraph block or a paragraph whose runs do not cover its text;
/// callers must not guess a projection in that case.
pub fn body_final_paragraphs(document: &Document) -> Result<Vec<String>, TxError> {
    let mut groups: Vec<String> = Vec::new();
    let mut join_next = false;
    for paragraph in body_paragraphs(document)? {
        let chars = final_chars(paragraph).map_err(|_| TxError::AiRangeRefused)?;
        let text: String = chars.iter().map(|(_, ch)| ch).collect();
        if join_next && let Some(last) = groups.last_mut() {
            last.push_str(&text);
        } else {
            groups.push(text);
        }
        join_next = mark_deleted(paragraph);
    }
    Ok(groups)
}

/// Replace the final-view character span `[start, end)` with `replacement`.
/// The span may cover paragraph marks; the replacement itself must be one
/// line. `base_version` must be the session's current version. Failure
/// leaves the session, its version and its undo stack untouched.
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
        paragraph: span.end_paragraph,
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
    end_paragraph: NodeId,
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

fn mark_deleted(paragraph: &Paragraph) -> bool {
    paragraph
        .mark_revision
        .as_ref()
        .is_some_and(|revision| revision.kind == RevisionKind::Delete)
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

/// Maps a final-view `[start, end)` span onto the first and last paragraph
/// positions. Inside one paragraph the covered text must be one contiguous
/// run of visible scalars. When the span crosses paragraph marks, the
/// first paragraph's covered suffix must reach its end, every paragraph in
/// between must be fully visible, and the last paragraph's covered prefix
/// must start at its beginning; hidden runs or inline objects inside a
/// covered paragraph refuse the range.
/// One paragraph's projection inside [`locate`]: the paragraph and its
/// final visible chars with scalar offsets.
struct Row<'a> {
    paragraph: &'a Paragraph,
    chars: Vec<(usize, char)>,
}

fn locate(document: &Document, start: u64, end: u64) -> Result<Span, TxError> {
    if start >= end {
        return Err(TxError::AiRangeRefused);
    }
    let mut first: Option<(usize, usize)> = None;
    let mut last: Option<(usize, usize)> = None;
    let mut cursor: u64 = 0;
    let mut rows: Vec<Row<'_>> = Vec::new();
    for paragraph in body_paragraphs(document)? {
        let chars = final_chars(paragraph).map_err(|_| TxError::AiRangeRefused)?;
        let para_len = u64::try_from(chars.len()).map_err(|_| TxError::AiRangeRefused)?;
        let para_start = cursor;
        let para_end = para_start
            .checked_add(para_len)
            .ok_or(TxError::AiRangeRefused)?;
        let separator = !mark_deleted(paragraph);
        let index = rows.len();
        if first.is_none() {
            if start >= para_start && start < para_end {
                first = Some((
                    index,
                    usize::try_from(start - para_start).map_err(|_| TxError::AiRangeRefused)?,
                ));
            } else if start == para_end && separator && end > para_end {
                // The range begins on this paragraph's mark: an empty span
                // whose mark deletion is part of the replace.
                first = Some((index, chars.len()));
            }
        }
        if end > para_start && end <= para_end {
            last = Some((
                index,
                usize::try_from(end - para_start).map_err(|_| TxError::AiRangeRefused)?,
            ));
        } else if separator && end == para_end.checked_add(1).ok_or(TxError::AiRangeRefused)? {
            // The range ends right after this paragraph's mark: the next
            // paragraph contributes an empty span.
            last = Some((index + 1, 0));
        }
        rows.push(Row { paragraph, chars });
        cursor = para_end + u64::from(separator);
    }
    let (first_row, first_index) = first.ok_or(TxError::AiRangeRefused)?;
    let (last_row, last_index) = last.ok_or(TxError::AiRangeRefused)?;
    let first = rows.get(first_row).ok_or(TxError::AiRangeRefused)?;
    let last = rows.get(last_row).ok_or(TxError::AiRangeRefused)?;
    if first_row == last_row {
        let span = contiguous_span(&first.chars, first_index, last_index)?;
        return Ok(Span {
            paragraph: first.paragraph.id(),
            scalar_start: span.0,
            end_paragraph: first.paragraph.id(),
            scalar_end: span.1,
        });
    }
    // Crossing: the suffix of the first paragraph and the prefix of the
    // last one are covered to their paragraph edges. Hidden delete or
    // move-from runs are structurally covered too (the engine keeps other
    // authors' deletions and reuses its own); inline objects are refused
    // because a deleted revision cannot carry one.
    let scalar_start = if first_index == first.chars.len() {
        first.paragraph.len()
    } else {
        visible_suffix(first.paragraph, &first.chars, first_index)?
    };
    for row in first_row + 1..last_row {
        let row = rows.get(row).ok_or(TxError::AiRangeRefused)?;
        if !row.paragraph.inlines().is_empty() {
            return Err(TxError::AiRangeRefused);
        }
    }
    let scalar_end = if last_index == 0 {
        0
    } else {
        visible_prefix(last.paragraph, &last.chars, last_index)?
    };
    Ok(Span {
        paragraph: first.paragraph.id(),
        scalar_start,
        end_paragraph: last.paragraph.id(),
        scalar_end,
    })
}

/// One contiguous visible span `[index_start, index_end)` inside a single
/// paragraph, as its scalar offsets.
fn contiguous_span(
    chars: &[(usize, char)],
    index_start: usize,
    index_end: usize,
) -> Result<(usize, usize), TxError> {
    if index_start >= index_end || index_end > chars.len() {
        return Err(TxError::AiRangeRefused);
    }
    let Some((scalar_start, _)) = chars.get(index_start).copied() else {
        return Err(TxError::AiRangeRefused);
    };
    for (offset, (scalar, ch)) in chars[index_start..index_end].iter().enumerate() {
        if *scalar != scalar_start + offset
            || *ch == '\u{FFFC}'
            || *ch == joyword_model::OBJECT_MARKER
        {
            return Err(TxError::AiRangeRefused);
        }
    }
    let Some((last_scalar, _)) = chars.get(index_end - 1).copied() else {
        return Err(TxError::AiRangeRefused);
    };
    let scalar_end = last_scalar.checked_add(1).ok_or(TxError::AiRangeRefused)?;
    Ok((scalar_start, scalar_end))
}

/// Scalar offset of the covered suffix start at final-view index `index`;
/// the suffix must be contiguous and the paragraph free of inline objects.
fn visible_suffix(
    paragraph: &Paragraph,
    chars: &[(usize, char)],
    index: usize,
) -> Result<usize, TxError> {
    let (scalar_start, _) = contiguous_span(chars, index, chars.len())?;
    if !paragraph.inlines().is_empty() {
        return Err(TxError::AiRangeRefused);
    }
    Ok(scalar_start)
}

/// Scalar offset just past the covered prefix ending at final-view index
/// `index`; the prefix must start at position 0, be contiguous, and the
/// paragraph must be free of inline objects.
fn visible_prefix(
    paragraph: &Paragraph,
    chars: &[(usize, char)],
    index: usize,
) -> Result<usize, TxError> {
    let (_, scalar_end) = contiguous_span(chars, 0, index)?;
    if chars
        .first()
        .copied()
        .is_some_and(|(scalar, _)| scalar != 0)
        || !paragraph.inlines().is_empty()
    {
        return Err(TxError::AiRangeRefused);
    }
    Ok(scalar_end)
}
