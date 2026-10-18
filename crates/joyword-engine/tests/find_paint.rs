//! Find, replace and format painter. Positions are grapheme clusters; locked
//! and preserved content is skipped; each replace-all or paint is one undo.

use std::sync::Arc;

use joyword_engine::{
    Command, EditPolicy, FindError, FindQuery, FormatBrush, MAX_HITS, PaintError, RunChange,
    Session, Transaction, capture, paint_commands, replace_commands, search,
};
use joyword_model::{
    Bias, Block, BlockSeq, BreakKind, Color, Document, InlineKind, InlineObject, Justification,
    NodeId, Paragraph, Position, RunProps, ScalarOffset, Sdt, SdtLock, SdtProps, Style, StyleId,
    StyleKind, ThemeColorSlot,
};

fn document(lines: &[&str]) -> Document {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    for line in lines {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text(line, None, Arc::default()).unwrap();
        blocks.push(Block::from(paragraph));
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    document
}

fn texts(session: &Session) -> Vec<String> {
    fn walk(block: &Block, out: &mut Vec<String>) {
        match block {
            Block::Paragraph(paragraph) => out.push(paragraph.plain_text(|_| "□".into())),
            Block::Table(table) => {
                for cell in table.rows.iter().flat_map(|row| row.cells.iter()) {
                    for inner in cell.blocks.iter() {
                        walk(inner, out);
                    }
                }
            }
            Block::Sdt(sdt) => {
                for inner in sdt.blocks.iter() {
                    walk(inner, out);
                }
            }
            Block::Opaque(_) => {}
        }
    }
    let mut out = Vec::new();
    for block in session.document().body().blocks.iter() {
        walk(block, &mut out);
    }
    out
}

fn body_paragraph(session: &Session, index: usize) -> &Paragraph {
    session
        .document()
        .body()
        .blocks
        .get(index)
        .and_then(Block::as_paragraph)
        .unwrap()
}

fn pos(session: &Session, paragraph: NodeId, offset: usize) -> Position {
    Position {
        story: session.document().body_id(),
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::After,
    }
}

fn query(text: &str, case_sensitive: bool, whole_word: bool) -> FindQuery {
    FindQuery {
        text: text.into(),
        case_sensitive,
        whole_word,
    }
}

fn apply(session: &mut Session, commands: Vec<Command>) {
    session
        .apply(Transaction::new(session.version(), commands))
        .unwrap();
}

#[test]
fn grapheme_case_and_cjk_word_boundaries() {
    let clustered = document(&["a\u{0301}x"]);
    let sensitive = search(&clustered, &query("a", true, false)).unwrap();
    assert!(sensitive.hits.is_empty(), "a cluster is not split");
    let composed = search(&clustered, &query("a\u{0301}", true, false)).unwrap();
    assert_eq!(composed.hits.len(), 1);
    assert_eq!(composed.hits[0].start.0, 0);

    let document = document(&["Straße 中国 category A12"]);
    let folded = search(&document, &query("straße", false, false)).unwrap();
    assert_eq!(folded.hits.len(), 1);
    let expanded = search(&document, &query("STRASSE", false, false)).unwrap();
    assert!(expanded.hits.is_empty(), "ß does not match two S clusters");

    let inside = search(&document, &query("中", true, true)).unwrap();
    assert_eq!(inside.hits.len(), 1);
    let latin = search(&document, &query("cat", false, true)).unwrap();
    assert!(latin.hits.is_empty());
    let latin_part = search(&document, &query("cat", false, false)).unwrap();
    assert_eq!(latin_part.hits.len(), 1);
    let digits = search(&document, &query("A1", true, true)).unwrap();
    assert!(digits.hits.is_empty());

    assert_eq!(
        search(&document, &query("", true, false)).unwrap_err(),
        FindError::Empty
    );
    let long = "a".repeat(1025);
    assert_eq!(
        search(&document, &query(&long, true, false)).unwrap_err(),
        FindError::TooLong
    );
}

#[test]
fn a_match_does_not_cross_an_inline_and_does_cross_runs() {
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("ab", None, Arc::default()).unwrap();
    paragraph
        .push_text(
            "cd",
            None,
            Arc::new(RunProps {
                bold: Some(true),
                ..RunProps::default()
            }),
        )
        .unwrap();
    paragraph.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::Break(BreakKind::Line),
            source: None,
        },
        None,
        Arc::default(),
    );
    paragraph.push_text("cd", None, Arc::default()).unwrap();
    let id = paragraph.id();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let report = search(&document, &query("bc", true, false)).unwrap();
    assert_eq!(report.hits.len(), 1);
    assert_eq!((report.hits[0].start.0, report.hits[0].end.0), (1, 3));

    let mut session = Session::new(document);
    let plan = replace_commands(session.document(), &report.hits, "X").unwrap();
    assert_eq!(plan.replaced, 1);
    apply(&mut session, plan.commands);
    assert_eq!(texts(&session), vec!["aXd□cd".to_owned()]);
    let paragraph = body_paragraph(&session, 0);
    assert_eq!(paragraph.runs()[0].props.bold, None);
    assert_eq!(paragraph.runs()[1].props.bold, Some(true));
    session.undo().unwrap();
    assert_eq!(texts(&session), vec!["abcd□cd".to_owned()]);
    let _ = id;
}

#[test]
fn replace_all_is_one_undo_and_skips_locked_and_preserved() {
    let mut document = Document::new_blank();
    let mut open = Paragraph::new(document.ids().node());
    open.push_text("cat cat", None, Arc::default()).unwrap();
    let preserved_id = document.ids().preserved();
    let mut kept = Paragraph::new(document.ids().node());
    kept.push_text(
        "cat",
        None,
        Arc::new(RunProps {
            preserved: Some(preserved_id),
            ..RunProps::default()
        }),
    )
    .unwrap();
    let mut inner = Paragraph::new(document.ids().node());
    inner.push_text("cat", None, Arc::default()).unwrap();
    let sdt = Sdt {
        id: document.ids().node(),
        props: SdtProps {
            lock: SdtLock::ContentLocked,
            ..SdtProps::default()
        },
        blocks: BlockSeq::from_vec(vec![inner.into()]),
        source: None,
    };
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks =
        BlockSeq::from_vec(vec![open.into(), kept.into(), sdt.into()]);
    let report = search(&document, &query("cat", true, true)).unwrap();
    assert_eq!(report.hits.len(), 2);
    assert_eq!(report.skipped, 2);

    let mut session = Session::new(document);
    let plan = replace_commands(session.document(), &report.hits, "dog").unwrap();
    assert_eq!(plan.replaced, 2);
    apply(&mut session, plan.commands);
    assert_eq!(
        texts(&session),
        vec!["dog dog".to_owned(), "cat".to_owned(), "cat".to_owned()]
    );
    assert!(session.can_undo());
    session.undo().unwrap();
    assert_eq!(
        texts(&session),
        vec!["cat cat".to_owned(), "cat".to_owned(), "cat".to_owned()]
    );
    assert!(!session.can_undo());
}

#[test]
fn replacement_does_not_search_again_and_rejects_a_separator() {
    let mut session = Session::new(document(&["a"]));
    let report = search(session.document(), &query("a", true, false)).unwrap();
    let plan = replace_commands(session.document(), &report.hits, "aa").unwrap();
    apply(&mut session, plan.commands);
    assert_eq!(texts(&session), vec!["aa".to_owned()]);

    let again = search(session.document(), &query("a", true, false)).unwrap();
    let rejected = replace_commands(session.document(), &again.hits, "a\nb");
    assert!(matches!(rejected, Err(FindError::InvalidReplacement('\n'))));
    assert_eq!(texts(&session), vec!["aa".to_owned()]);
}

#[test]
fn search_still_runs_when_replacement_is_read_only() {
    let mut session = Session::new(document(&["cat"]));
    session.set_policy(EditPolicy {
        read_only: true,
        agent_writes: false,
    });
    let report = search(session.document(), &query("cat", true, false)).unwrap();
    assert_eq!(report.hits.len(), 1);
    let plan = replace_commands(session.document(), &report.hits, "dog").unwrap();
    let error = session
        .apply(Transaction::new(session.version(), plan.commands))
        .unwrap_err();
    assert!(matches!(error, joyword_engine::TxError::ReadOnly));
    assert_eq!(texts(&session), vec!["cat".to_owned()]);
}

#[test]
fn hit_count_is_bounded() {
    let session = Session::new(document(&[&"a".repeat(MAX_HITS + 10)]));
    let report = search(session.document(), &query("a", true, false)).unwrap();
    assert_eq!(report.hits.len(), MAX_HITS);
    assert!(report.truncated);
}

#[test]
fn an_emoji_sequence_is_one_cluster() {
    let document = document(&["👨‍👩‍👧‍👦"]);
    let report = search(&document, &query("👨", true, false)).unwrap();
    assert!(report.hits.is_empty());
    let whole = search(&document, &query("👨‍👩‍👧‍👦", true, false)).unwrap();
    assert_eq!(whole.hits.len(), 1);
}

#[test]
fn painting_a_format_is_one_undoable_transaction() {
    let mut document = Document::new_blank();
    let mut heading = Style::new("Heading1", StyleKind::Paragraph, Some("heading 1"));
    heading.run.bold = Some(true);
    document.styles_mut().insert(heading).unwrap();
    let mut source = Paragraph::new(document.ids().node());
    source.style = Some(StyleId::new("Heading1"));
    source.props = Arc::new(joyword_model::ParaProps {
        justification: Some(Justification::Center),
        ..joyword_model::ParaProps::default()
    });
    source
        .push_text(
            "标题",
            None,
            Arc::new(RunProps {
                italic: Some(true),
                ..RunProps::default()
            }),
        )
        .unwrap();
    let mut target = Paragraph::new(document.ids().node());
    target.push_text("正文", None, Arc::default()).unwrap();
    let source_id = source.id();
    let target_id = target.id();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks =
        BlockSeq::from_vec(vec![source.into(), target.into()]);
    let mut session = Session::new(document);
    let brush = capture(
        session.document(),
        pos(&session, source_id, 0),
        pos(&session, source_id, 2),
    )
    .unwrap();
    assert!(brush.paint_paragraph);
    assert_eq!(
        brush.paragraph_style.as_ref().map(StyleId::as_str),
        Some("Heading1")
    );
    assert_eq!(brush.justification, Some(Justification::Center));
    assert!(brush.changes.contains(&RunChange::Italic(Some(true))));

    let commands = paint_commands(
        session.document(),
        pos(&session, target_id, 0),
        pos(&session, target_id, 2),
        &brush,
    )
    .unwrap();
    apply(&mut session, commands);
    let painted = body_paragraph(&session, 1);
    assert_eq!(
        painted.style.as_ref().map(StyleId::as_str),
        Some("Heading1")
    );
    assert_eq!(painted.props.justification, Some(Justification::Center));
    assert_eq!(painted.runs()[0].props.italic, Some(true));
    session.undo().unwrap();
    let restored = body_paragraph(&session, 1);
    assert!(restored.style.is_none());
    assert!(restored.props.justification.is_none());
    assert!(restored.runs()[0].props.italic.is_none());
    assert!(!session.can_undo());
}

#[test]
fn painter_refuses_theme_colour_and_a_preserved_target() {
    let mut document = Document::new_blank();
    let mut source = Paragraph::new(document.ids().node());
    source
        .push_text(
            "色",
            None,
            Arc::new(RunProps {
                color: Some(Color::Theme(ThemeColorSlot::Accent1)),
                ..RunProps::default()
            }),
        )
        .unwrap();
    let source_id = source.id();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![source.into()]);
    let session = Session::new(document);
    let error = capture(
        session.document(),
        pos(&session, source_id, 0),
        pos(&session, source_id, 1),
    )
    .unwrap_err();
    assert!(matches!(error, PaintError::Unexportable(_)));

    let mut document = Document::new_blank();
    let mut plain = Paragraph::new(document.ids().node());
    plain.push_text("甲", None, Arc::default()).unwrap();
    let mut kept = Paragraph::new(document.ids().node());
    kept.push_text(
        "乙",
        None,
        Arc::new(RunProps {
            preserved: Some(document.ids().preserved()),
            ..RunProps::default()
        }),
    )
    .unwrap();
    let plain_id = plain.id();
    let kept_id = kept.id();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![plain.into(), kept.into()]);
    let session = Session::new(document);
    let brush = capture(
        session.document(),
        pos(&session, plain_id, 0),
        pos(&session, plain_id, 0),
    )
    .unwrap();
    let brush = FormatBrush {
        changes: vec![RunChange::Bold(Some(true))],
        paint_paragraph: false,
        ..brush
    };
    let error = paint_commands(
        session.document(),
        pos(&session, kept_id, 0),
        pos(&session, kept_id, 1),
        &brush,
    )
    .unwrap_err();
    assert!(matches!(error, PaintError::Unexportable(_)));
    assert_eq!(
        body_paragraph(&session, 1).plain_text(|_| String::new()),
        "乙"
    );
}

#[test]
fn direct_paint_refuses_a_paragraph_that_already_has_a_style() {
    let mut document = Document::new_blank();
    let mut heading = Style::new("Heading1", StyleKind::Paragraph, Some("heading 1"));
    heading.run.bold = Some(true);
    document.styles_mut().insert(heading).unwrap();
    let mut styled = Paragraph::new(document.ids().node());
    styled.style = Some(StyleId::new("Heading1"));
    styled.push_text("标题", None, Arc::default()).unwrap();
    let id = styled.id();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![styled.into()]);
    let session = Session::new(document);
    let brush = FormatBrush {
        paint_paragraph: false,
        paragraph_style: None,
        justification: None,
        character_style: None,
        changes: vec![RunChange::Italic(Some(true))],
    };
    let error = paint_commands(
        session.document(),
        pos(&session, id, 0),
        pos(&session, id, 2),
        &brush,
    )
    .unwrap_err();
    assert!(matches!(error, PaintError::Unexportable(_)));
    assert!(body_paragraph(&session, 0).runs()[0].props.italic.is_none());
}
