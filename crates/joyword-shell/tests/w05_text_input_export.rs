//! W05: real importer → C07 text input / engine transactions → export →
//! importer, for paragraph splits and merges. An independent string model of
//! the flat text (paragraphs joined by LF, UTF-16 offsets) checks every
//! exported package.

use std::path::PathBuf;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{ExportError, ExportMode, ExportPlan, ImportResult, W, import};
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::input::{Direction, InputError, InputLimits, InputSource, TextInput};
use unicode_segmentation::UnicodeSegmentation;

const SECT: &str = r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr>"#;

fn fixture(id: u8) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}

/// DOCX-MIN-001's parts with another main document body.
fn package(body: &str) -> Vec<u8> {
    let source = fixture(1);
    let package = Package::read(&source, &Budget::default()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:x="urn:w05:unknown"><w:body>{body}{SECT}</w:body></w:document>"#
    );
    let mut items = vec![(
        "[Content_Types].xml",
        package.content_types_xml(),
        Method::Deflated,
    )];
    for part in package.parts() {
        let data = if part.name.as_str() == "/word/document.xml" {
            xml.as_bytes()
        } else {
            &part.data
        };
        items.push((part.name.item_name(), data, part.method));
    }
    write_archive(&items).unwrap()
}

fn p(text: &str) -> String {
    format!(r#"<w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#)
}

fn open(bytes: &[u8]) -> TextInput<DocxSession> {
    let session = DocxSession::open(bytes, &Budget::default()).unwrap();
    TextInput::new(session, InputLimits::default()).unwrap()
}

fn flat(saved: &ImportResult) -> String {
    saved
        .document
        .body()
        .blocks
        .iter()
        .map(|block| {
            block
                .as_paragraph()
                .unwrap()
                .plain_text(|_| "\u{FFFC}".into())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn plan(input: &TextInput<DocxSession>) -> Result<ExportPlan, ExportError> {
    input.backend().export_plan()
}

/// Exports the committed session, re-imports it and checks the flat text.
fn check(input: &TextInput<DocxSession>, expected: &str) -> ExportPlan {
    assert_eq!(input.view().index().text(), expected);
    let plan = plan(input).unwrap();
    assert_eq!(plan.version(), input.backend().session().version());
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(saved.editability.is_unrestricted());
    assert_eq!(flat(&saved), expected);
    assert!(!saved.document.validate().has_errors());
    plan
}

fn caret(input: &mut TextInput<DocxSession>, offset: usize) {
    input.select(offset..offset, false).unwrap();
}

fn utf16(text: &str) -> usize {
    text.encode_utf16().count()
}

#[test]
fn enter_backspace_delete_and_multiline_paste_round_trip() {
    let bytes = package(&format!("{}{}{}", p("甲乙丙"), p("丁戊"), p("己")));
    let mut input = open(&bytes);
    assert_eq!(plan(&input).unwrap().bytes(), bytes);
    // Enter in the middle, at the very start and twice at the very end.
    caret(&mut input, 1);
    input.replace(None, "\n").unwrap();
    check(&input, "甲\n乙丙\n丁戊\n己");
    caret(&mut input, 0);
    input.replace(None, "\n").unwrap();
    check(&input, "\n甲\n乙丙\n丁戊\n己");
    let end = utf16(input.view().index().text());
    caret(&mut input, end);
    input.replace(None, "\n").unwrap();
    input.replace(None, "\n").unwrap();
    input.replace(None, "新段🙂").unwrap();
    check(&input, "\n甲\n乙丙\n丁戊\n己\n\n新段🙂");
    // Backspace at a paragraph start merges with the previous paragraph.
    caret(&mut input, 3);
    input.delete(Direction::Backward).unwrap();
    check(&input, "\n甲乙丙\n丁戊\n己\n\n新段🙂");
    // Forward delete at a paragraph end merges the next one.
    caret(&mut input, 4);
    input.delete(Direction::Forward).unwrap();
    check(&input, "\n甲乙丙丁戊\n己\n\n新段🙂");
    // A selection across three paragraphs.
    input.select(3..9, false).unwrap();
    input.delete(Direction::Backward).unwrap();
    check(&input, "\n甲乙\n新段🙂");
    // Multi-line replacement of a selection.
    input.select(2..4, false).unwrap();
    input.replace(None, "X\nY\n\nZ").unwrap();
    check(&input, "\n甲X\nY\n\nZ新段🙂");
    assert_eq!(input.backend().backing().original_bytes(), bytes);
}

#[test]
fn undo_redo_and_repeated_exports_are_versioned_and_deterministic() {
    let bytes = package(&format!("{}{}", p("春夏"), p("秋冬")));
    let mut input = open(&bytes);
    caret(&mut input, 1);
    input.replace(None, "\n").unwrap();
    let split = check(&input, "春\n夏\n秋冬");
    let split_bytes = split.bytes().to_vec();
    input.select(3..4, false).unwrap();
    input.delete(Direction::Backward).unwrap();
    let merged = check(&input, "春\n夏秋冬");
    assert_ne!(split.version(), merged.version());
    assert_eq!(split.bytes(), split_bytes, "plans are immutable");
    assert_eq!(merged.bytes(), plan(&input).unwrap().bytes());
    input.undo().unwrap();
    let again = check(&input, "春\n夏\n秋冬");
    assert_ne!(again.version(), split.version());
    assert_eq!(again.bytes(), split_bytes);
    input.undo().unwrap();
    let original = plan(&input).unwrap();
    assert_eq!(original.mode(), ExportMode::OriginalPackage);
    assert_eq!(original.bytes(), bytes);
    input.redo().unwrap();
    input.redo().unwrap();
    assert_eq!(check(&input, "春\n夏秋冬").bytes(), merged.bytes());
}

#[test]
fn composition_previews_are_never_exported() {
    let bytes = package(&p("组合"));
    let mut input = open(&bytes);
    caret(&mut input, 1);
    input.mark(None, "预\n编辑", None).unwrap();
    assert!(input.is_composing());
    assert_eq!(input.view().index().text(), "组预\n编辑合");
    let preview = plan(&input).unwrap();
    assert_eq!(preview.mode(), ExportMode::OriginalPackage);
    assert_eq!(preview.bytes(), bytes);
    input.cancel_composition().unwrap();
    assert_eq!(plan(&input).unwrap().bytes(), bytes);
    input.mark(None, "确\n认", None).unwrap();
    input.unmark().unwrap();
    assert_eq!(check(&input, "组确\n认合").mode(), ExportMode::TextPatch);
}

#[test]
fn tabs_and_xml_specials_through_engine_commands() {
    let bytes = package(&p("表"));
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    let document = session.session().document();
    let at = Position {
        story: document.body_id(),
        paragraph: document.body().blocks.get(0).unwrap().id(),
        offset: ScalarOffset(1),
        bias: Bias::After,
    };
    session
        .apply(Transaction::new(
            session.session().version(),
            vec![
                Command::InsertText {
                    at,
                    text: "\t<&>\t".into(),
                },
                Command::SplitParagraph { at },
            ],
        ))
        .unwrap();
    let plan = session.export_plan().unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(flat(&saved), "表\n\t<&>\t");
}

#[test]
fn refused_structure_edits_leave_session_and_backing_untouched() {
    let section = format!(
        r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/></w:sectPr></w:pPr><w:r><w:t>节末</w:t></w:r></w:p>{}"#,
        p("下一节")
    );
    for (body, offset, expected) in [
        (
            r#"<w:p><w:r><w:rPr><w:highlight w:val="yellow"/></w:rPr><w:t>高亮</w:t></w:r></w:p>"#
                .to_owned(),
            1,
            "paragraph holds preserved markup",
        ),
        (section, 2, "paragraph ends a section"),
        (
            r#"<w:p><w:r><w:t>注</w:t></w:r><!--keep--><w:r><w:t>释</w:t></w:r></w:p>"#.to_owned(),
            1,
            "paragraph contains comments or processing instructions",
        ),
        (
            r#"<w:p x:keep="yes"><w:r><w:t>属性</w:t></w:r></w:p>"#.to_owned(),
            2,
            "paragraph has unknown attributes",
        ),
    ] {
        let bytes = package(&body);
        let mut input = open(&bytes);
        caret(&mut input, offset);
        input.replace(None, "\n").unwrap();
        let version = input.backend().session().version();
        let text = input.view().index().text().to_owned();
        match plan(&input) {
            Err(ExportError::Unsupported { reason, .. }) => assert_eq!(reason, expected),
            other => panic!("{body}: {other:?}"),
        }
        assert_eq!(input.backend().session().version(), version);
        assert_eq!(input.view().index().text(), text);
        assert_eq!(input.backend().backing().original_bytes(), bytes);
        assert!(input.can_undo());
        input.undo().unwrap();
        assert_eq!(plan(&input).unwrap().bytes(), bytes);
    }
}

#[test]
fn corpus_fixtures_split_only_where_supported() {
    // DOCX-MIN-001: one plain paragraph.
    let mut input = open(&fixture(1));
    caret(&mut input, 4);
    input.replace(None, "\n").unwrap();
    check(&input, "合作协议\n：甲方与乙方于 2026 年签署。");
    // DOCX-MIN-002: the heading's only property is a mapped pStyle, so W12
    // copies it onto the new paragraph. The following plain paragraph splits too.
    let mut input = open(&fixture(2));
    let heading = utf16("第一条 合作内容");
    caret(&mut input, heading + 3);
    input.replace(None, "\n").unwrap();
    check(&input, "第一条 合作内容\n甲方\n委托乙方完成设计服务。");
    caret(&mut input, 2);
    input.replace(None, "\n").unwrap();
    check(&input, "第一\n条 合作内容\n甲方\n委托乙方完成设计服务。");
    // DOCX-MIN-003/004/005 hold unknown, linked or protected content and
    // open read-only; nothing can be split.
    for id in [3, 4, 5] {
        let session = DocxSession::open(&fixture(id), &Budget::default()).unwrap();
        assert!(session.session().policy().read_only, "fixture {id}");
        match TextInput::new(session, InputLimits::default()) {
            Ok(mut input) => {
                caret(&mut input, 1);
                assert_eq!(input.replace(None, "\n"), Err(InputError::ReadOnly));
            }
            Err(error) => assert_eq!(error, InputError::UnsupportedBody, "fixture {id}"),
        }
    }
}

/// Grapheme-aligned UTF-16 boundaries of `text`.
fn boundaries(text: &str) -> Vec<usize> {
    let mut offsets: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(byte, _)| utf16(&text[..byte]))
        .collect();
    offsets.push(utf16(text));
    offsets
}

fn byte_offset(text: &str, utf16_offset: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == utf16_offset {
            return byte;
        }
        units += ch.len_utf16();
    }
    assert_eq!(units, utf16_offset);
    text.len()
}

#[test]
fn deterministic_input_sequences_match_an_independent_text_model() {
    let bytes = package(&format!("{}{}{}", p("开始"), p("middle"), p("结束")));
    let mut input = open(&bytes);
    let mut model = String::from("开始\nmiddle\n结束");
    let mut undo: Vec<String> = Vec::new();
    let mut redo: Vec<String> = Vec::new();
    let pool = [
        "\n",
        "\n",
        "中",
        "🙂",
        "e\u{301}",
        "<&>",
        " ",
        "👩‍👩‍👧‍👦",
        "甲\n乙",
        "\n\n",
    ];
    let mut seed = 0x0057_0005_2026u64;
    let mut next = |bound: usize| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % bound
    };
    for step in 0..400 {
        let marks = boundaries(&model);
        let operation = next(10);
        if operation == 0 && !undo.is_empty() {
            input.undo().unwrap();
            redo.push(std::mem::replace(&mut model, undo.pop().unwrap()));
        } else if operation == 1 && !redo.is_empty() {
            input.redo().unwrap();
            undo.push(std::mem::replace(&mut model, redo.pop().unwrap()));
        } else {
            let a = marks[next(marks.len())];
            let b = if operation < 5 {
                a
            } else {
                marks[next(marks.len())]
            };
            let (start, end) = (a.min(b), a.max(b));
            // Deletions (empty text) and insertions, including LF splits.
            let text = if operation >= 8 && start != end {
                ""
            } else {
                pool[next(pool.len())]
            };
            // Command replacements keep one undo step per edit (T209).
            let changed = input
                .replace_from(InputSource::Command, Some(start..end), text)
                .unwrap();
            let mut edited = model.clone();
            edited.replace_range(byte_offset(&model, start)..byte_offset(&model, end), text);
            // Replacing text with identical text may still be a transaction.
            assert!(changed || edited == model, "step {step}");
            if changed {
                undo.push(std::mem::replace(&mut model, edited));
                redo.clear();
            }
        }
        assert_eq!(input.view().index().text(), model, "step {step}");
        if step % 20 == 0 || step == 399 {
            check(&input, &model);
        }
    }
}
