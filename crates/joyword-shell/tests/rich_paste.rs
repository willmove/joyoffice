//! Hostile HTML and RTF become plain formatted runs, or plain text.

use std::sync::Arc;

use joyword_engine::Session;
use joyword_model::{BlockSeq, Document, Paragraph, Underline};
use joyword_shell::input::{
    ClipboardOffer, InputLimits, PasteChoice, TextInput, choose_paste, plain_text_offer,
};
use joyword_shell::rich_paste::{MAX_RICH_BYTES, ParsedRich, parse_html, parse_rtf};

fn input(text: &str) -> TextInput {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    for line in text.split('\n') {
        let mut paragraph = Paragraph::new(document.ids().node());
        if !line.is_empty() {
            paragraph.push_text(line, None, Arc::default()).unwrap();
        }
        blocks.push(paragraph.into());
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    TextInput::new(Session::new(document), InputLimits::default()).unwrap()
}

fn body(editor: &TextInput) -> String {
    editor
        .backend()
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .map(|paragraph| paragraph.plain_text(|_| "\u{FFFC}".into()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn html_keeps_bold_italic_underline_and_drops_active_content() {
    let parsed = parse_html(
        "<p>before<script>alert(1)</script><style>body{}</style><b>B</b><i>I</i><u>U</u>after</p>",
    );
    let ParsedRich::Formatted(document) = parsed else {
        panic!("{parsed:?}");
    };
    let text: String = document
        .paragraphs
        .iter()
        .flat_map(|paragraph| paragraph.runs.iter().map(|run| run.text.as_str()))
        .collect();
    assert_eq!(text, "beforeBIUafter");
    assert!(!text.contains("alert"));
    assert!(!text.contains("body"));
    let bold = document.paragraphs[0]
        .runs
        .iter()
        .find(|run| run.text == "B")
        .unwrap();
    assert!(bold.bold && !bold.italic && !bold.underline);

    let remote = parse_html(
        r#"<p>a<img src="http://example.invalid/x.png">b<iframe src="http://example.invalid/"></iframe></p>"#,
    );
    let ParsedRich::Formatted(document) = remote else {
        panic!("{remote:?}");
    };
    let text: String = document.paragraphs[0]
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(text, "ab");

    let encoded = parse_html("<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>");
    let ParsedRich::Formatted(document) = encoded else {
        panic!("{encoded:?}");
    };
    assert_eq!(
        document.paragraphs[0].runs[0].text,
        "<script>alert(1)</script>"
    );
}

#[test]
fn rtf_keeps_formatting_and_drops_objects_pictures_and_field_instructions() {
    let parsed = parse_rtf(r"{\rtf1\b bold\b0 \i italic\i0 \ul under\ulnone \par next}");
    let ParsedRich::Formatted(document) = parsed else {
        panic!("{parsed:?}");
    };
    assert_eq!(document.paragraphs.len(), 2);
    assert_eq!(document.paragraphs[0].runs[0].text, "bold");
    assert!(document.paragraphs[0].runs[0].bold);
    assert!(
        document.paragraphs[0]
            .runs
            .iter()
            .any(|run| run.italic && run.text.contains("italic"))
    );
    assert!(
        document.paragraphs[0]
            .runs
            .iter()
            .any(|run| run.underline && run.text.contains("under"))
    );
    assert_eq!(document.paragraphs[1].runs[0].text.trim(), "next");

    let hostile = parse_rtf(
        r#"{\rtf1{\object\objdata 0100}{\pict\pngblip 89504e47}{\field{\*\fldinst HYPERLINK "javascript:alert(1)"}{\fldrslt click}}end}"#,
    );
    let ParsedRich::Formatted(document) = hostile else {
        panic!("{hostile:?}");
    };
    let text: String = document
        .paragraphs
        .iter()
        .flat_map(|paragraph| paragraph.runs.iter().map(|run| run.text.as_str()))
        .collect();
    assert!(text.contains("click"), "{text}");
    assert!(text.contains("end"), "{text}");
    assert!(!text.contains("javascript"), "{text}");
    assert!(!text.contains("0100"), "{text}");
    assert!(!text.contains("89504e47"), "{text}");
}

#[test]
fn oversize_markup_is_refused_and_deep_markup_falls_back_to_plain_text() {
    let huge = "x".repeat(MAX_RICH_BYTES + 1);
    assert!(matches!(parse_html(&huge), ParsedRich::Refuse(_)));
    assert!(matches!(
        parse_rtf(&format!("{{\\rtf1 {huge}}}")),
        ParsedRich::Refuse(_)
    ));

    let mut html = String::new();
    for _ in 0..40 {
        html.push_str("<div>");
    }
    html.push_str("deep<script>alert(1)</script>");
    for _ in 0..40 {
        html.push_str("</div>");
    }
    match parse_html(&html) {
        ParsedRich::Plain(text) => {
            assert!(text.contains("deep"), "{text}");
            assert!(!text.contains("alert"), "{text}");
        }
        other => panic!("{other:?}"),
    }

    let mut rtf = String::from("{\\rtf1");
    for _ in 0..40 {
        rtf.push('{');
    }
    rtf.push_str("deep");
    for _ in 0..40 {
        rtf.push('}');
    }
    rtf.push('}');
    assert!(matches!(parse_rtf(&rtf), ParsedRich::Plain(_)));
}

#[test]
fn pasted_html_becomes_formatted_runs_and_one_undo() {
    let ParsedRich::Formatted(document) = parse_html("<p><b>Hi</b> <i>there</i></p><p>next</p>")
    else {
        panic!("formatted");
    };
    let mut editor = input("");
    editor.paste_rich(&document).unwrap();
    assert_eq!(body(&editor), "Hi there\nnext");
    let paragraph = editor
        .backend()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    let bold = paragraph
        .runs()
        .iter()
        .find(|run| run.props.bold == Some(true))
        .expect("bold run");
    assert_eq!(bold.len, 2);
    let italic = paragraph
        .runs()
        .iter()
        .find(|run| run.props.italic == Some(true))
        .expect("italic run");
    assert_eq!(italic.len, 5);
    assert!(paragraph.runs().iter().any(|run| {
        run.props.underline == Some(Underline::None) && run.props.bold == Some(false)
    }));
    assert!(paragraph.link_at(0).is_none());
    editor.undo().unwrap();
    assert_eq!(body(&editor), "");
}

#[test]
fn html_text_is_chosen_ahead_of_plain_text_and_paths_stay_closed() {
    let html = "<p>hi</p>";
    assert_eq!(
        choose_paste(&[ClipboardOffer::Text(html)]),
        PasteChoice::Html(html)
    );
    assert!(plain_text_offer(&[ClipboardOffer::Text(html)]).is_none());
    assert_eq!(
        choose_paste(&[ClipboardOffer::Text("plain"), ClipboardOffer::Paths]),
        PasteChoice::Text("plain")
    );
    let rtf = r"{\rtf1 plain}";
    assert_eq!(
        choose_paste(&[ClipboardOffer::Text(rtf)]),
        PasteChoice::Rtf(rtf)
    );
}
