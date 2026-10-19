//! Font and paragraph dialogs, and ruler drags, for T205 slice 2.
//!
//! Cancel produces no commands. Apply is one undoable transaction.

use std::collections::BTreeMap;
use std::sync::Arc;

use joyword_engine::Session;
use joyword_model::{
    BlockSeq, Document, Indent, Justification, LineSpacing, Measure, Paragraph, Spacing, TabAlign,
    TabLeader, TabStop,
};
use joyword_shell::command::ArgValue;
use joyword_shell::dialogs::{
    CHINESE_SIZES, Field, FontDraft, IndentMarker, ParagraphDraft, chinese_size, dragged_indent,
    dragged_tabs, font_draft_from_decision, inherited_custom_tabs, paragraph_draft_from_decision,
};
use joyword_shell::input::{InputLimits, TextInput};

fn session(text: &str) -> Session {
    let mut doc = Document::new_blank();
    let mut blocks = Vec::new();
    for line in text.split('\n') {
        let mut paragraph = Paragraph::new(doc.ids().node());
        paragraph.push_text(line, None, Arc::default()).unwrap();
        blocks.push(paragraph.into());
    }
    let body = doc.body_id();
    doc.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    Session::new(doc)
}

fn input(text: &str) -> TextInput {
    TextInput::new(session(text), InputLimits::default()).unwrap()
}

fn args(pairs: &[(&'static str, &str)]) -> BTreeMap<&'static str, ArgValue> {
    let mut map = BTreeMap::new();
    map.insert("commit", ArgValue::Text("apply".into()));
    for (name, value) in pairs {
        map.insert(*name, ArgValue::Text((*value).to_owned()));
    }
    map
}

fn caret_props(input: &TextInput) -> joyword_model::RunProps {
    let head = input.view().selection().head;
    let document = input.backend().document();
    let paragraph = document.find_paragraph(head.story, head.paragraph).unwrap();
    let (style, props) = paragraph.format_at(head.offset);
    document
        .styles()
        .resolve_run(paragraph.style.as_ref(), style.as_ref(), props.as_ref())
        .props
}

#[test]
fn chinese_sizes_follow_the_office_point_table() {
    assert_eq!(CHINESE_SIZES.len(), 16);
    assert_eq!(chinese_size("初号").unwrap().half_points, 84);
    assert_eq!(chinese_size("小初").unwrap().half_points, 72);
    assert_eq!(chinese_size("二号").unwrap().half_points, 44);
    assert_eq!(chinese_size("三号").unwrap().half_points, 32);
    assert_eq!(chinese_size("五号").unwrap().half_points, 21);
    assert_eq!(chinese_size("六号").unwrap().half_points, 15);
    assert_eq!(chinese_size("七号").unwrap().half_points, 11);
    assert_eq!(chinese_size("八号").unwrap().half_points, 10);
    assert_eq!(chinese_size("小四").unwrap().points_tenths, 120);
}

#[test]
fn a_cancelled_font_dialog_does_not_open_a_transaction() {
    let mut editor = input("甲乙");
    editor.select_all().unwrap();
    let version = editor.backend().version();
    assert!(font_draft_from_decision(None).unwrap().is_none());
    assert_eq!(editor.backend().version(), version);
    assert!(!editor.can_undo());
}

#[test]
fn font_apply_is_one_undo_and_an_empty_selection_is_not() {
    let mut editor = input("甲乙");
    let version = editor.backend().version();
    let draft = font_draft_from_decision(Some(&args(&[
        ("family", "宋体"),
        ("size", "五号"),
        ("bold", "true"),
    ])))
    .unwrap()
    .unwrap();
    editor.apply_font(&draft).unwrap();
    assert_eq!(editor.backend().version(), version);
    assert!(!editor.can_undo());

    editor.select_all().unwrap();
    editor.apply_font(&draft).unwrap();
    assert!(editor.can_undo());
    let props = caret_props(&editor);
    assert_eq!(
        props.fonts.east_asia.as_ref().map(|font| match font {
            joyword_model::FontRef::Named(name) => name.as_ref(),
            joyword_model::FontRef::Theme(_) => "",
        }),
        Some("宋体")
    );
    assert_eq!(props.size, Some(Measure::HalfPoints(21)));
    assert_eq!(props.bold, Some(true));
    editor.undo().unwrap();
    let props = caret_props(&editor);
    assert!(props.fonts.east_asia.is_none());
    assert!(props.size.is_none());
    assert!(props.bold.is_none());
    assert!(!editor.can_undo());
}

#[test]
fn paragraph_indent_spacing_and_line_are_one_undo() {
    let mut editor = input("甲乙");
    editor.select_all().unwrap();
    let draft = paragraph_draft_from_decision(Some(&args(&[
        ("start", "720"),
        ("first", "360"),
        ("before", "120"),
        ("after", "80"),
        ("line", "auto:360"),
    ])))
    .unwrap()
    .unwrap();
    editor.apply_paragraph(&draft).unwrap();
    let head = editor.view().selection().head;
    let props = editor
        .backend()
        .document()
        .find_paragraph(head.story, head.paragraph)
        .unwrap()
        .props
        .clone();
    assert_eq!(props.indent.start, Some(Measure::Twips(720)));
    assert_eq!(props.indent.first_line, Some(Measure::Twips(360)));
    assert_eq!(props.spacing.before, Some(Measure::Twips(120)));
    assert_eq!(props.spacing.line, Some(LineSpacing::Auto(360)));
    editor.undo().unwrap();
    let props = editor
        .backend()
        .document()
        .find_paragraph(head.story, head.paragraph)
        .unwrap()
        .props
        .clone();
    assert_eq!(props.indent, Indent::default());
    assert_eq!(props.spacing, Spacing::default());
}

#[test]
fn justify_with_indent_is_refused_and_the_document_stays() {
    let mut editor = input("甲乙");
    editor.select_all().unwrap();
    let draft = ParagraphDraft {
        alignment: Field::Set(Some(Justification::Center)),
        indent: Field::Set(Indent {
            start: Some(Measure::Twips(720)),
            ..Indent::default()
        }),
        spacing: Field::Leave,
        tabs: Field::Leave,
    };
    let error = editor.apply_paragraph(&draft).unwrap_err();
    assert!(matches!(
        error,
        joyword_shell::input::InputError::Rejected("dialog.paragraph.justify_indent")
    ));
    assert!(!editor.can_undo());
}

#[test]
fn first_line_drag_keeps_the_body_edge() {
    let current = Indent {
        start: Some(Measure::Twips(720)),
        ..Indent::default()
    };
    let body = Measure::Twips(720).absolute().unwrap().raw();
    let further = body + Measure::Twips(360).absolute().unwrap().raw();
    let indent = dragged_indent(&current, IndentMarker::FirstLine, further, None).unwrap();
    assert_eq!(indent.start, Some(Measure::Twips(720)));
    assert_eq!(indent.first_line, Some(Measure::Twips(360)));
    assert!(indent.hanging.is_none());
}

#[test]
fn dragging_a_generated_stop_adds_a_direct_left_stop() {
    let added = dragged_tabs(&[], 720 * 635, Some(1440 * 635)).unwrap();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].align, TabAlign::Left);
    assert_eq!(added[0].leader, TabLeader::None);
    assert_eq!(added[0].position, Measure::Twips(1440));
}

#[test]
fn inherited_style_tabs_are_detected() {
    let resolved = [TabStop {
        position: Measure::Twips(720),
        align: TabAlign::Left,
        leader: TabLeader::None,
    }];
    assert!(inherited_custom_tabs(&[], &resolved));
    assert!(!inherited_custom_tabs(&resolved, &resolved));
}

#[test]
fn underline_and_srgb_do_not_record_a_paint_refusal() {
    let mut editor = input("甲");
    editor.select_all().unwrap();
    let notes = editor
        .apply_font(&FontDraft {
            family: None,
            size_half_points: None,
            bold: None,
            italic: None,
            underline: Some(joyword_model::Underline::Single),
            color: Some(joyword_model::Color::Rgb(180, 20, 20)),
        })
        .unwrap();
    assert!(!notes.contains(&"dialog.font.paint"));
}
