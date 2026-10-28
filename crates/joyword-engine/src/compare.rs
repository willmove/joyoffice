//! T303 first slice: headless semantic comparison of two documents. The
//! result is a copy of the original with tracked insertions and deletions
//! (`w:ins` / `w:del` runs, inserted and deleted paragraph marks) such that
//! accepting everything reproduces the revised document's body text and
//! rejecting everything restores the original's — the REVIEW-007 projection
//! invariants, pinned by tests.
//!
//! The slice is deliberately narrow and refuses rather than guesses:
//!
//! * Either input still carries pending revisions, move markers, comments or
//!   footnote-style stories — choosing the input projection is explicit and
//!   comes later (SPEC: 文档比较遇到已有待决修订时要求明确选择输入投影，不静默合并历史).
//! * Tables and inline objects are not compared yet.
//! * Paragraph alignment is a longest-common-subsequence over body paragraph
//!   text, bounded by a table-size budget. A gap between two kept matches is
//!   resolved three ways: paragraphs only the revised side has are inserted
//!   as tracked paragraphs after the survivor; paragraphs only the original
//!   has are tracked-deleted through the merge machinery; one deleted and
//!   one added paragraph become an in-paragraph text diff (common prefix and
//!   suffix trimmed, the middle deleted and reinserted inside the surviving
//!   paragraph — its formatting is the original's, a formatting-only change
//!   is therefore not detected). A gap with several paragraphs on both sides
//!   has no faithful tracked representation and is refused.
//! * A run of paragraphs added or deleted at the very beginning of the body
//!   has no surviving paragraph to anchor on and is refused.
//!
//! Header and footer stories must have the same texts on both sides (the
//! result keeps the original's); differing ones are refused. Section
//! properties are carried over from the original unchanged; differing
//! section counts are refused.
//!
//! The deletion representation follows the merge machinery: for a run of
//! paragraphs `P_j..P_k` that the revised side dropped after a surviving
//! `P_{j-1}`, the tracked range runs from the end of `P_{j-1}`'s text to the
//! end of `P_k`'s text. That deletes the run's text and the marks
//! `P_{j-1}..P_{k-1}` — every break the revised side lost — while `P_k`'s
//! own mark stays, so the acceptance chain absorbs the empty remainder and
//! ends exactly at the surviving break. Insertions split the surviving
//! paragraph at its end: each split's tail mark carries `w:ins`, and the
//! text goes in as a tracked insertion, so rejecting joins the new paragraph
//! into the next one, which keeps its identity and formatting.
//!
//! The deletion representation follows the merge machinery: for a run of
//! paragraphs `P_j..P_k` that the revised side dropped after a surviving
//! `P_{j-1}`, the tracked range runs from the end of `P_{j-1}`'s text to the
//! end of `P_k`'s text. That deletes the run's text and the marks
//! `P_{j-1}..P_{k-1}` — every break the revised side lost — while `P_k`'s
//! own mark stays, so the acceptance chain absorbs the empty remainder and
//! ends exactly at the surviving break. Insertions split the surviving
//! paragraph at its end: each split's tail mark carries `w:ins`, and the
//! text goes in as a tracked insertion, so rejecting joins the new paragraph
//! into the next one, which keeps its identity and formatting.

use std::sync::Arc;

use joyword_model::{Bias, Block, Document, Paragraph, Position, ScalarOffset, StoryKind};

use crate::command::{Command, CommandError};
use crate::session::{Session, Transaction};

/// Why a comparison could not be produced. Every variant is a refusal: the
/// caller decides what to do, nothing is silently merged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompareError {
    /// Either input still carries pending revisions or move markers. Choosing
    /// the input projection is an explicit decision this slice does not make.
    PendingRevisions,
    /// The documents differ in something this slice does not compare.
    Unsupported(&'static str),
    /// The engine refused one of the comparison edits.
    Rejected(String),
    /// The paragraph alignment table exceeds the bounded budget.
    TooLarge,
}

impl std::fmt::Display for CompareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompareError::PendingRevisions => {
                write!(f, "document still has pending revisions")
            }
            CompareError::Unsupported(reason) => write!(f, "compare unsupported: {reason}"),
            CompareError::Rejected(reason) => write!(f, "compare edit rejected: {reason}"),
            CompareError::TooLarge => write!(f, "document too large to compare"),
        }
    }
}

impl std::error::Error for CompareError {}

/// The alignment table budget in cells of `u32`: the product of both
/// paragraph counts plus one each, about 16 MiB at the cap. Real reports sit
/// far below it, and a larger comparison refuses instead of trading
/// unbounded memory for a guess.
const ALIGNMENT_CELLS: usize = 4_000_000;

/// Compares `original` against `revised` and returns a document carrying the
/// differences as tracked revisions authored by `author`. Accepting every
/// revision reproduces `revised`'s body paragraph text; rejecting every
/// revision restores `original`'s.
pub fn compare_documents(
    original: &Document,
    revised: &Document,
    author: &str,
    date: Option<Arc<str>>,
) -> Result<Document, CompareError> {
    validate_inputs(original, revised)?;
    let edits = plan(original, revised)?;
    apply_plan(original, &edits, author, date)
}

fn validate_inputs(original: &Document, revised: &Document) -> Result<(), CompareError> {
    for document in [original, revised] {
        if document.body().blocks.iter().next().is_none() {
            return Err(CompareError::Unsupported("empty document body"));
        }
        for block in document.body().blocks.iter() {
            let Block::Paragraph(paragraph) = block else {
                return Err(CompareError::Unsupported("tables are not compared yet"));
            };
            if !paragraph.inlines().is_empty() {
                return Err(CompareError::Unsupported(
                    "inline objects are not compared yet",
                ));
            }
        }
        if has_pending_revisions(document) {
            return Err(CompareError::PendingRevisions);
        }
        if document.annotations().next().is_some() {
            return Err(CompareError::Unsupported("comments are not compared yet"));
        }
    }
    if story_texts(original, StoryKind::Header) != story_texts(revised, StoryKind::Header)
        || story_texts(original, StoryKind::Footer) != story_texts(revised, StoryKind::Footer)
    {
        return Err(CompareError::Unsupported("header or footer stories differ"));
    }
    for kind in [StoryKind::Footnote, StoryKind::Endnote, StoryKind::TextBox] {
        if !story_texts(original, kind).is_empty() || !story_texts(revised, kind).is_empty() {
            return Err(CompareError::Unsupported(
                "footnote, endnote and text-box stories are not compared yet",
            ));
        }
    }
    if original.sections().len() != revised.sections().len() {
        return Err(CompareError::Unsupported("section structure differs"));
    }
    Ok(())
}

fn has_pending_revisions(document: &Document) -> bool {
    if !document.move_markers().is_empty() {
        return true;
    }
    document.stories().any(|story| {
        story.blocks.iter().any(|block| {
            let Some(paragraph) = block.as_paragraph() else {
                return false;
            };
            paragraph.runs().iter().any(|run| run.revision.is_some())
                || paragraph.mark_revision.is_some()
                || paragraph.props_revision.is_some()
        })
    })
}

/// The concatenated plain texts of one non-body story kind, in document
/// order. Only the flat story text is compared; table blocks inside a story
/// contribute nothing and are refused elsewhere for the body.
fn story_texts(document: &Document, kind: StoryKind) -> Vec<String> {
    document
        .stories()
        .filter(|story| story.kind == kind)
        .map(|story| {
            let mut text = String::new();
            for block in story.blocks.iter() {
                let Some(paragraph) = block.as_paragraph() else {
                    continue;
                };
                text.push_str(&paragraph.plain_text(|_| String::new()));
            }
            text
        })
        .collect()
}

fn body_paragraphs(document: &Document) -> Vec<&Paragraph> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .collect()
}

fn body_texts(document: &Document) -> Vec<String> {
    body_paragraphs(document)
        .iter()
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect()
}

/// The common prefix and suffix lengths of two texts, never overlapping.
fn common_edges(original: &str, revised: &str) -> (usize, usize) {
    let a: Vec<char> = original.chars().collect();
    let b: Vec<char> = revised.chars().collect();
    let mut prefix = 0usize;
    while prefix < a.len() && prefix < b.len() && a[prefix] == b[prefix] {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < a.len() - prefix
        && suffix < b.len() - prefix
        && a[a.len() - 1 - suffix] == b[b.len() - 1 - suffix]
    {
        suffix += 1;
    }
    (prefix, suffix)
}

/// Longest-common-subsequence alignment over the paragraph texts. Returns the
/// matched index pairs in document order.
fn align(original: &[String], revised: &[String]) -> Result<Vec<(usize, usize)>, CompareError> {
    let width = revised.len() + 1;
    if (original.len() + 1).saturating_mul(width) > ALIGNMENT_CELLS {
        return Err(CompareError::TooLarge);
    }
    let mut table = vec![0u32; (original.len() + 1) * width];
    for a in (0..original.len()).rev() {
        for b in (0..revised.len()).rev() {
            table[a * width + b] = if original[a] == revised[b] {
                table[(a + 1) * width + b + 1] + 1
            } else {
                table[(a + 1) * width + b].max(table[a * width + b + 1])
            };
        }
    }
    let mut matches = Vec::new();
    let mut a = 0usize;
    let mut b = 0usize;
    while a < original.len() && b < revised.len() {
        if original[a] == revised[b] {
            matches.push((a, b));
            a += 1;
            b += 1;
        } else if table[(a + 1) * width + b] >= table[a * width + b + 1] {
            a += 1;
        } else {
            b += 1;
        }
    }
    Ok(matches)
}

/// The planned edits, in forward document order. Every edit anchors at nodes
/// of the original document's body; they stay stable while other gaps insert
/// or delete around them, so the apply phase is order-independent.
fn plan(original: &Document, revised: &Document) -> Result<Vec<Edit>, CompareError> {
    let original_texts = body_texts(original);
    let revised_texts = body_texts(revised);
    let paragraphs_a = body_paragraphs(original);
    let nodes: Vec<joyword_model::NodeId> = paragraphs_a
        .iter()
        .map(|paragraph| paragraph.id())
        .collect();
    let matches = align(&original_texts, &revised_texts)?;
    if matches.first() != Some(&(0, 0)) {
        return Err(CompareError::Unsupported(
            "changes at the very beginning of the body are not compared yet",
        ));
    }
    let mut edits = Vec::new();
    let mut previous_a = 0usize;
    let mut previous_b = 0usize;
    for (a, b) in matches
        .iter()
        .copied()
        .chain(std::iter::once((original_texts.len(), revised_texts.len())))
    {
        let deleted = a - previous_a;
        let added = b - previous_b;
        if deleted == 0 && added == 0 {
            continue;
        }
        // The first `min(deleted, added)` orphan pairs become in-paragraph
        // text diffs; the remaining original paragraphs are tracked-deleted
        // (their shells are absorbed by the acceptance chain) and the
        // remaining revised paragraphs are inserted after the last original
        // orphan, which is where they belong once the shells are gone.
        let paired = deleted.min(added);
        let mut group = Vec::new();
        if deleted > paired {
            let start = if paired > 0 {
                previous_a + paired - 1
            } else {
                // Pure deletion: the survivor before the run carries the
                // first deleted break.
                previous_a - 1
            };
            group.push(Edit::DeleteRun {
                survivor: nodes[start],
                last: nodes[a - 1],
            });
        }
        for index in 0..paired {
            let old_text = &original_texts[previous_a + index];
            let fresh_text = &revised_texts[previous_b + index];
            let (prefix, suffix) = common_edges(old_text, fresh_text);
            let old: Vec<char> = old_text.chars().collect();
            let fresh: Vec<char> = fresh_text.chars().collect();
            let removed = (old.len() - suffix > prefix).then_some((prefix, old.len() - suffix));
            let inserted = (fresh.len() - suffix > prefix).then_some((
                prefix,
                fresh[prefix..fresh.len() - suffix]
                    .iter()
                    .collect::<String>(),
            ));
            if removed.is_some() || inserted.is_some() {
                group.push(Edit::ReplaceParagraph {
                    node: nodes[previous_a + index],
                    removed,
                    inserted,
                });
            }
        }
        if added > paired {
            group.push(Edit::InsertRun {
                survivor: nodes[a - 1],
                texts: revised_texts[previous_b + paired..b].to_vec(),
            });
        }
        if !group.is_empty() {
            edits.push(Edit::Group(group));
        }
        previous_a = a;
        previous_b = b;
    }
    Ok(edits)
}

/// One planned edit. Anchors are node ids of the original document's body
/// paragraphs; they stay stable while other gaps insert or delete around
/// them, so the apply phase is order-independent.
enum Edit {
    /// One alignment gap: the edits inside run in document order (deletion
    /// first, in-paragraph replacements next, insertions last) because the
    /// deletion range must not span the inserted paragraphs.
    Group(Vec<Edit>),
    /// Delete the original paragraphs after `survivor` through `last`
    /// (inclusive), anchored at the end of the survivor's text.
    DeleteRun {
        survivor: joyword_model::NodeId,
        last: joyword_model::NodeId,
    },
    /// Insert `texts` as new tracked paragraphs after the surviving paragraph.
    InsertRun {
        survivor: joyword_model::NodeId,
        texts: Vec<String>,
    },
    /// Replace one paragraph's text in place: the middle between the common
    /// edges is tracked-deleted and the revised middle inserted at the same
    /// offset. The paragraph and its mark are untouched.
    ReplaceParagraph {
        node: joyword_model::NodeId,
        removed: Option<(usize, usize)>,
        inserted: Option<(usize, String)>,
    },
}

fn position(document: &Document, paragraph: joyword_model::NodeId, offset: usize) -> Position {
    Position {
        story: document.body_id(),
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::Before,
    }
}

fn len_of(document: &Document, paragraph: joyword_model::NodeId) -> usize {
    document
        .find_paragraph(document.body_id(), paragraph)
        .map(Paragraph::len)
        .unwrap_or(0)
}

/// The body paragraph immediately after `node`, or `None` when `node` is the
/// last one.
fn paragraph_after(
    document: &Document,
    node: joyword_model::NodeId,
) -> Option<joyword_model::NodeId> {
    let mut seen = false;
    for block in document.body().blocks.iter() {
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        if seen {
            return Some(paragraph.id());
        }
        seen = paragraph.id() == node;
    }
    None
}

fn apply(
    session: &mut Session,
    commands: Vec<Command>,
    author: &Arc<str>,
) -> Result<(), CompareError> {
    let mut transaction = Transaction::new(session.version(), commands);
    transaction.tracked_author = Some(Arc::clone(author));
    session
        .apply(transaction)
        .map(|_| ())
        .map_err(|error| match error {
            crate::session::TxError::Command { error, .. } => CompareError::Rejected(match error {
                CommandError::CrossContainer => "edit crosses a container".to_owned(),
                CommandError::ReversedRange => "edit range is reversed".to_owned(),
                other => format!("{other:?}"),
            }),
            other => CompareError::Rejected(format!("{other:?}")),
        })
}

fn apply_plan(
    original: &Document,
    edits: &[Edit],
    author: &str,
    date: Option<Arc<str>>,
) -> Result<Document, CompareError> {
    let author: Arc<str> = Arc::from(author);
    let mut session = Session::new(original.clone());
    session.set_revision_author(author.as_ref());
    session.set_revision_date(date);
    // Edits are applied from the end of the document backwards. Every index
    // an edit references lies at or before its own anchor, so blocks added by
    // later insertions never shift an earlier edit's anchors.
    for edit in edits.iter().rev() {
        apply_edit(&mut session, edit, &author)?;
    }
    Ok(session.document().clone())
}

fn apply_edit(session: &mut Session, edit: &Edit, author: &Arc<str>) -> Result<(), CompareError> {
    match edit {
        Edit::DeleteRun { survivor, last } => {
            let document = session.document();
            let start = position(document, *survivor, len_of(document, *survivor));
            let end = position(document, *last, len_of(document, *last));
            apply(session, vec![Command::DeleteRange { start, end }], author)?;
        }
        Edit::InsertRun { survivor, texts } => {
            let mut after = *survivor;
            for text in texts.iter() {
                let document = session.document();
                let at = position(document, after, len_of(document, after));
                apply(session, vec![Command::SplitParagraph { at }], author)?;
                let Some(new) = paragraph_after(session.document(), after) else {
                    return Err(CompareError::Unsupported("split produced no paragraph"));
                };
                if !text.is_empty() {
                    let at = position(session.document(), new, 0);
                    apply(
                        session,
                        vec![Command::InsertText {
                            at,
                            text: text.clone(),
                        }],
                        author,
                    )?;
                }
                after = new;
            }
        }
        Edit::ReplaceParagraph {
            node,
            removed,
            inserted,
        } => {
            let mut commands = Vec::new();
            if let Some((start, end)) = removed {
                let document = session.document();
                commands.push(Command::DeleteRange {
                    start: position(document, *node, *start),
                    end: position(document, *node, *end),
                });
            }
            if let Some((at, text)) = inserted {
                let document = session.document();
                commands.push(Command::InsertText {
                    at: position(document, *node, *at),
                    text: text.clone(),
                });
            }
            if !commands.is_empty() {
                apply(session, commands, author)?;
            }
        }
        // A gap's edits run in document order: the deletion range must not
        // span paragraphs inserted by the same gap.
        Edit::Group(group) => {
            for edit in group {
                apply_edit(session, edit, author)?;
            }
        }
    }
    Ok(())
}
