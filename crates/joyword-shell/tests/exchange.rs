use joyword_shell::exchange::{Format, MAX_INPUT, plan};

#[test]
fn all_output_formats_round_trip_chinese_emoji_and_literal_punctuation() {
    let input = "中文😀 *literal* <b>& \\";
    for target in [
        Format::Txt,
        Format::Markdown,
        Format::Html,
        Format::Rtf,
        Format::Docx,
    ] {
        let exported = plan(input.as_bytes(), Format::Txt, target).unwrap();
        assert!(exported.losses.has_loss());
        let readback = plan(exported.bytes(), target, Format::Txt).unwrap();
        assert_eq!(
            std::str::from_utf8(readback.bytes()).unwrap().trim(),
            input,
            "{target:?}"
        );
    }
}

#[test]
fn review_hidden_encoding_controls_and_work_budgets_are_refused() {
    for (from, input) in [
        (Format::Rtf, br"{\rtf1 public {\v private} tail}".as_slice()),
        (Format::Rtf, br"{\rtf1 \deleted secret}"),
        (Format::Html, b"<p hidden>secret</p>"),
        (Format::Html, b"<p style='display:none'>secret</p>"),
        (Format::Txt, b"\xef\xbb\xbf\xff"),
        (Format::Txt, b"\xff\xfe\x00\xd8"),
        (Format::Txt, b"abc\x00def"),
    ] {
        assert!(plan(input, from, Format::Txt).is_err(), "{from:?}");
    }
    assert!(plan(&vec![b'a'; MAX_INPUT + 1], Format::Txt, Format::Html).is_err());
    assert!(
        plan(
            "line\n".repeat(10_001).as_bytes(),
            Format::Txt,
            Format::Html
        )
        .is_err()
    );
}

#[test]
fn docx_uses_the_production_importer_and_never_falls_back_to_text_for_protected_input() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/docx");
    let plain = std::fs::read(root.join("DOCX-MIN-001.docx")).unwrap();
    let converted = plan(&plain, Format::Docx, Format::Html).unwrap();
    assert!(
        std::str::from_utf8(converted.bytes())
            .unwrap()
            .contains("<p>")
    );
    let protected = std::fs::read(root.join("DOCX-MIN-005.docx")).unwrap();
    assert!(plan(&protected, Format::Docx, Format::Txt).is_err());
}
