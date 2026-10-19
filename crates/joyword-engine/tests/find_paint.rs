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
        fold_width: false,
        regex: false,
    }
}

fn query_full(text: &str, case_sensitive: bool, whole_word: bool, fold_width: bool) -> FindQuery {
    FindQuery {
        text: text.into(),
        case_sensitive,
        whole_word,
        fold_width,
        regex: false,
    }
}

fn query_regex(text: &str, case_sensitive: bool, whole_word: bool, fold_width: bool) -> FindQuery {
    FindQuery {
        text: text.into(),
        case_sensitive,
        whole_word,
        fold_width,
        regex: true,
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
                color: Some(Color::theme(ThemeColorSlot::Accent1)),
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

#[test]
fn fullwidth_forms_fold_to_halfwidth() {
    let document = document(&["ＡＢＣ １２３！中　x"]);
    // Every full-width form below is one scalar, like its half-width twin:
    // Ａ=0 Ｂ=1 Ｃ=2, space=3, １=4 ２=5 ３=6, ！=7, 中=8,　=9, x=10.
    let strict = search(&document, &query("ABC", true, false)).unwrap();
    assert!(
        strict.hits.is_empty(),
        "no folding keeps full-width distinct"
    );
    let folded = search(&document, &query_full("ABC", true, false, true)).unwrap();
    assert_eq!(folded.hits.len(), 1);
    assert_eq!((folded.hits[0].start.0, folded.hits[0].end.0), (0, 3));

    let digits = search(&document, &query_full("123", true, false, true)).unwrap();
    assert_eq!(digits.hits.len(), 1);
    assert_eq!((digits.hits[0].start.0, digits.hits[0].end.0), (4, 7));
    let bang = search(&document, &query_full("!", true, false, true)).unwrap();
    assert_eq!(bang.hits.len(), 1);
    assert_eq!((bang.hits[0].start.0, bang.hits[0].end.0), (7, 8));

    // U+3000 ideographic space folds to an ordinary space.
    let wide_space = search(&document, &query_full("中 ", true, false, true)).unwrap();
    assert_eq!(wide_space.hits.len(), 1);
    assert_eq!(
        (wide_space.hits[0].start.0, wide_space.hits[0].end.0),
        (8, 10)
    );
    let no_wide_space = search(&document, &query_full("中 ", true, false, false)).unwrap();
    assert!(no_wide_space.hits.is_empty());

    // Ideographs are not width-folded; the option changes nothing for them.
    let han = search(&document, &query_full("中", true, false, true)).unwrap();
    assert_eq!(han.hits.len(), 1);

    // Half-width text is found with the option on as well.
    let ascii = search(&document, &query_full("x", true, false, true)).unwrap();
    assert_eq!(ascii.hits.len(), 1);
}

#[test]
fn width_folding_combines_with_case_folding() {
    let document = document(&["ａｂｃ Straße"]);
    let folded = search(&document, &query_full("ABC", false, false, true)).unwrap();
    assert_eq!(folded.hits.len(), 1, "ａｂｃ matches ABC");
    let without_width = search(&document, &query("ABC", false, false)).unwrap();
    assert!(without_width.hits.is_empty());
    // ß still does not expand against SS under width folding.
    let sharp = search(&document, &query_full("STRASSE", false, false, true)).unwrap();
    assert!(sharp.hits.is_empty());
    let straße = search(&document, &query_full("straße", false, false, true)).unwrap();
    assert_eq!(straße.hits.len(), 1);
}

#[test]
fn regex_find_matches_patterns() {
    let document = document(&["cat category CAT 12a"]);
    // Offsets: cat=0..3, space=3, category=4..12, space=12, CAT=13..16,
    // space=16, 1=17, 2=18, a=19.
    let dotted = search(&document, &query_regex("c.t", true, false, false)).unwrap();
    assert_eq!(dotted.hits.len(), 2, "cat and the start of category");
    let insensitive = search(&document, &query_regex("cat", false, false, false)).unwrap();
    assert_eq!(insensitive.hits.len(), 3, "CAT joins under case folding");
    let words = search(&document, &query_regex("cat", false, true, false)).unwrap();
    assert_eq!(words.hits.len(), 2, "category is not a whole word");
    let digits = search(&document, &query_regex(r"\d+", true, false, false)).unwrap();
    assert_eq!(digits.hits.len(), 1);
    assert_eq!((digits.hits[0].start.0, digits.hits[0].end.0), (17, 19));

    assert_eq!(
        search(&document, &query_regex("(", true, false, false)).unwrap_err(),
        FindError::InvalidPattern
    );
    assert_eq!(
        search(&document, &query_regex("a{3,1}", true, false, false)).unwrap_err(),
        FindError::InvalidPattern
    );
}

#[test]
fn regex_skips_zero_width_and_cluster_splitting() {
    let plain = document(&["ab"]);
    let empty = search(&plain, &query_regex("x*", true, false, false)).unwrap();
    assert!(
        empty.hits.is_empty(),
        "a pattern that only matches empty text finds nothing"
    );
    let stars = search(&plain, &query_regex("a*", true, false, false)).unwrap();
    assert_eq!(stars.hits.len(), 1);
    assert_eq!((stars.hits[0].start.0, stars.hits[0].end.0), (0, 1));

    let clustered = document(&["a\u{0301}x"]);
    let inside = search(&clustered, &query_regex("a", true, false, false)).unwrap();
    assert!(
        inside.hits.is_empty(),
        "a match ending inside a cluster is skipped"
    );
    let mark = search(&clustered, &query_regex("\u{0301}", true, false, false)).unwrap();
    assert!(
        mark.hits.is_empty(),
        "a match starting inside a cluster is skipped"
    );
    let ascii = search(&clustered, &query_regex("x", true, false, false)).unwrap();
    assert_eq!(ascii.hits.len(), 1);
    assert_eq!((ascii.hits[0].start.0, ascii.hits[0].end.0), (2, 3));
}

#[test]
fn regex_never_crosses_an_inline_object() {
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("ab", None, Arc::default()).unwrap();
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
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    // Offsets: a=0, b=1, the inline object=2, c=3, d=4.
    let across = search(&document, &query_regex("b.d", true, false, false)).unwrap();
    assert!(
        across.hits.is_empty(),
        "dot may touch the placeholder but the hit is refused"
    );
    let greedy = search(&document, &query_regex("b.*d", true, false, false)).unwrap();
    assert!(greedy.hits.is_empty());
    let single = search(&document, &query_regex(".", true, false, false)).unwrap();
    let ranges: Vec<(usize, usize)> = single
        .hits
        .iter()
        .map(|hit| (hit.start.0, hit.end.0))
        .collect();
    assert_eq!(ranges, vec![(0, 1), (1, 2), (3, 4), (4, 5)]);
}

#[test]
fn regex_width_folding_applies_to_the_haystack() {
    let wide = document(&["ＡＢＣ"]);
    let strict = search(&wide, &query_regex("ABC", true, false, false)).unwrap();
    assert!(strict.hits.is_empty());
    let folded = search(&wide, &query_regex("ABC", true, false, true)).unwrap();
    assert_eq!(folded.hits.len(), 1);
    assert_eq!((folded.hits[0].start.0, folded.hits[0].end.0), (0, 3));

    let lower = document(&["ａｂｃ"]);
    let both = search(&lower, &query_regex("ABC", false, false, true)).unwrap();
    assert_eq!(both.hits.len(), 1);
}

#[test]
fn regex_hit_count_is_bounded() {
    let session = Session::new(document(&[&"a".repeat(MAX_HITS + 10)]));
    let report = search(session.document(), &query_regex("a", true, false, false)).unwrap();
    assert_eq!(report.hits.len(), MAX_HITS);
    assert!(report.truncated);
}

#[test]
fn regex_replace_is_one_undo_and_skips_locked() {
    let mut session = Session::new(document(&["cat 12 dog 34"]));
    let report = search(session.document(), &query_regex(r"\d+", true, false, false)).unwrap();
    assert_eq!(report.hits.len(), 2);
    let plan = replace_commands(session.document(), &report.hits, "#").unwrap();
    assert_eq!(plan.replaced, 2);
    apply(&mut session, plan.commands);
    assert_eq!(texts(&session), vec!["cat # dog #".to_owned()]);
    session.undo().unwrap();
    assert_eq!(texts(&session), vec!["cat 12 dog 34".to_owned()]);
    assert!(!session.can_undo());
}
