//! Find-bar checks and headless rehearsals of the manual acceptance scenarios.
//!
//! The no-panic gate scans `main.rs` in full, including `#[cfg(test)]`.
//! A path ending in `tests.rs` is outside that scan.
//!
//! Rehearsals drive `Office` / `TextInput` directly. Each one is an automated
//! rehearsal and does not replace 人工验收.

use super::input_ui::edit_request_from_args;
use super::{
    ArgValue, CommandId, FIND_NEXT, FIND_PREVIOUS, FIND_REPLACE, FIND_REPLACE_ALL, FindBarFlags,
    supplement_find_flags,
};

fn mapped(args: &[(&'static str, ArgValue)]) -> std::collections::BTreeMap<&'static str, ArgValue> {
    args.iter().cloned().collect()
}

fn flag(args: &std::collections::BTreeMap<&'static str, ArgValue>, name: &str) -> bool {
    matches!(args.get(name), Some(ArgValue::Bool(true)))
}

fn request_width_regex(request: &joyword_shell::input::EditRequest) -> (bool, bool) {
    match request {
        joyword_shell::input::EditRequest::Find {
            fold_width, regex, ..
        }
        | joyword_shell::input::EditRequest::Replace {
            fold_width, regex, ..
        } => (*fold_width, *regex),
        _ => panic!("not a find request"),
    }
}

#[test]
fn find_bar_checkboxes_set_width_and_regex_on_every_find_command() {
    let off = FindBarFlags::default();
    assert_eq!(
        off,
        FindBarFlags {
            case_sensitive: false,
            whole_word: false,
            fold_width: false,
            regex: false,
        }
    );
    for replacement in [None, Some("x")] {
        let args = off.args("ABC", replacement);
        let map = mapped(&args);
        assert!(!flag(&map, "case"));
        assert!(!flag(&map, "word"));
        assert!(!flag(&map, "width"));
        assert!(!flag(&map, "regex"));
        assert_eq!(map.contains_key("replacement"), replacement.is_some());
    }

    let cases = [
        (
            FindBarFlags {
                fold_width: true,
                ..off
            },
            true,
            false,
        ),
        (FindBarFlags { regex: true, ..off }, false, true),
        (
            FindBarFlags {
                fold_width: true,
                regex: true,
                ..off
            },
            true,
            true,
        ),
    ];
    for (flags, width_on, regex_on) in cases {
        for (id, replacement) in [
            (FIND_NEXT, None),
            (FIND_PREVIOUS, None),
            (FIND_REPLACE, Some("x")),
            (FIND_REPLACE_ALL, Some("x")),
        ] {
            let map = mapped(&flags.args("ABC", replacement));
            assert_eq!(flag(&map, "width"), width_on, "{id}");
            assert_eq!(flag(&map, "regex"), regex_on, "{id}");
            let request = edit_request_from_args(id, &map).unwrap();
            assert_eq!(request_width_regex(&request), (width_on, regex_on), "{id}");
        }
    }

    // A caller that only sends the older keys (the ribbon binder) picks up
    // the checkboxes. An explicit false stays false.
    let older: Vec<_> = off
        .args("ABC", None)
        .into_iter()
        .filter(|(name, _)| *name != "width" && *name != "regex")
        .collect();
    let filled = supplement_find_flags(&older, true, true);
    let map = mapped(&filled);
    assert!(flag(&map, "width") && flag(&map, "regex"));
    let explicit = supplement_find_flags(&off.args("ABC", None), true, true);
    let map = mapped(&explicit);
    assert!(!flag(&map, "width") && !flag(&map, "regex"));
}

fn wide_editor() -> joyword_shell::input::TextInput<joyword_engine::Session> {
    use joyword_engine::{EditPolicy, Session};
    use joyword_model::{BlockSeq, Document, PageSetup, Paragraph};
    use std::sync::Arc;

    let mut document = Document::new(PageSetup::a4_zh_cn());
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text("ＡＢＣ 12 dog 34", None, Arc::default())
        .unwrap();
    let story = document.body_id();
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let mut session = Session::new(document);
    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: false,
    });
    joyword_shell::input::TextInput::new(session, joyword_shell::input::InputLimits::default())
        .unwrap()
}

fn body_text(input: &joyword_shell::input::TextInput<joyword_engine::Session>) -> String {
    input
        .backend()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .plain_text(|_| String::new())
}

fn invoke_find(
    input: &mut joyword_shell::input::TextInput<joyword_engine::Session>,
    id: CommandId,
    flags: FindBarFlags,
    query: &str,
    replacement: Option<&str>,
) -> Result<joyword_shell::input::EditReport, joyword_shell::input::InputError> {
    let map = mapped(&flags.args(query, replacement));
    let request = edit_request_from_args(id, &map).expect("find bar args");
    input.editing(request)
}

#[test]
fn find_bar_width_hits_fullwidth_and_regex_rejects_invalid_patterns() {
    let original = "ＡＢＣ 12 dog 34";
    let mut input = wide_editor();
    assert_eq!(body_text(&input), original);

    let miss = invoke_find(&mut input, FIND_NEXT, FindBarFlags::default(), "ABC", None).unwrap();
    assert_eq!(miss.matches, 0);
    assert_eq!(input.selected_text().unwrap(), "");
    assert_eq!(body_text(&input), original);

    let literal = invoke_find(
        &mut input,
        FIND_NEXT,
        FindBarFlags::default(),
        "ＡＢＣ",
        None,
    )
    .unwrap();
    assert_eq!(literal.matches, 1);
    assert_eq!(input.selected_text().unwrap(), "ＡＢＣ");

    let mut folded = wide_editor();
    let next = invoke_find(
        &mut folded,
        FIND_NEXT,
        FindBarFlags {
            fold_width: true,
            ..FindBarFlags::default()
        },
        "ABC",
        None,
    )
    .unwrap();
    assert_eq!(next.matches, 1);
    assert_eq!(folded.selected_text().unwrap(), "ＡＢＣ");
    let mut backward = wide_editor();
    let previous = invoke_find(
        &mut backward,
        FIND_PREVIOUS,
        FindBarFlags {
            fold_width: true,
            ..FindBarFlags::default()
        },
        "ABC",
        None,
    )
    .unwrap();
    assert_eq!(previous.matches, 1);
    assert_eq!(backward.selected_text().unwrap(), "ＡＢＣ");

    let replaced = invoke_find(
        &mut folded,
        FIND_REPLACE,
        FindBarFlags {
            fold_width: true,
            ..FindBarFlags::default()
        },
        "ABC",
        Some("Z"),
    )
    .unwrap();
    assert_eq!(replaced.replaced, 1);
    assert_eq!(body_text(&folded), "Z 12 dog 34");
    folded.undo().unwrap();
    assert_eq!(body_text(&folded), original);

    let literal_digits = invoke_find(
        &mut input,
        FIND_REPLACE_ALL,
        FindBarFlags::default(),
        r"\d+",
        Some("#"),
    )
    .unwrap();
    assert_eq!(literal_digits.replaced, 0);
    assert_eq!(body_text(&input), original);

    let once = invoke_find(
        &mut input,
        FIND_REPLACE,
        FindBarFlags {
            regex: true,
            ..FindBarFlags::default()
        },
        r"\d+",
        Some("#"),
    )
    .unwrap();
    assert_eq!(once.replaced, 1);
    assert_eq!(body_text(&input), "ＡＢＣ # dog 34");
    input.undo().unwrap();
    assert_eq!(body_text(&input), original);

    let all = invoke_find(
        &mut input,
        FIND_REPLACE_ALL,
        FindBarFlags {
            regex: true,
            ..FindBarFlags::default()
        },
        r"\d+",
        Some("#"),
    )
    .unwrap();
    assert_eq!(all.replaced, 2);
    assert_eq!(body_text(&input), "ＡＢＣ # dog #");
    input.undo().unwrap();
    assert_eq!(body_text(&input), original);

    let invalid = invoke_find(
        &mut input,
        FIND_NEXT,
        FindBarFlags {
            regex: true,
            ..FindBarFlags::default()
        },
        "(",
        None,
    )
    .unwrap_err();
    assert_eq!(
        invalid,
        joyword_shell::input::InputError::Rejected("query is not a valid pattern")
    );
    assert_eq!(body_text(&input), original);
}

// --- Headless rehearsals of T200, T201, T202, T209 and T300. ---
//
// These chains call the same registry and input objects the window uses.
// Drag, autocorrect, the IME candidate window, on-screen zoom and the Word
// or WPS side of an exchange are not rehearsable here.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use joyword_foundation::{Emu, ScreenScale};
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, ViewOptions, layout_document_viewed, layout_with_view, to_screen,
};
use joyword_model::{
    Block, BorderKind, HeaderFooterKind, Orientation, PageSetup, RevisionKind, RevisionView,
    StoryKind, TableAlign, TableWidth, VerticalMerge,
};
use joyword_shell::command::{
    EDIT_BOLD, FILE_OPEN, FILE_SAVE_COPY, FORMAT_COPY, FORMAT_PAINT, LIST_APPLY, LIST_LEVEL,
    PAGE_EDIT_BODY, PAGE_EDIT_HEADER, PAGE_MARGINS, PAGE_NUMBER, PAGE_ORIENTATION, REVIEW_ACCEPT,
    REVIEW_ACCEPT_ALL, REVIEW_FINAL, REVIEW_MARKUP, REVIEW_ORIGINAL, REVIEW_REJECT,
    REVIEW_REJECT_ALL, REVIEW_TRACK, STYLE_APPLY, STYLE_LIST, TABLE_COLUMN_INSERT, TABLE_INSERT,
    TABLE_MERGE_DOWN, TABLE_MERGE_RIGHT, TABLE_PROPERTIES, TABLE_ROW_DELETE, TABLE_ROW_INSERT,
    TABLE_SPLIT,
};
use joyword_shell::host::Office;
use joyword_shell::input::{Direction, InputSource};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const CT_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL_OFFICE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const CT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const CT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const CT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const CT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";

/// ISO-HDLC / ITU-T V.42 CRC-32. The ZIP reader checks this value.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Stored ZIP (PKWARE APPNOTE). Method 0, UTF-8 names, no encryption.
fn store_zip(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut local = Vec::new();
    let mut central = Vec::new();
    for (name, data) in parts {
        let offset = u32::try_from(local.len()).expect("zip offset");
        let crc = crc32(data);
        let name_bytes = name.as_bytes();
        let name_len = u16::try_from(name_bytes.len()).expect("zip name");
        let size = u32::try_from(data.len()).expect("zip size");
        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&0x0800u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0x0021u16.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&name_len.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name_bytes);
        local.extend_from_slice(data);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0x0021u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let central_offset = u32::try_from(local.len()).expect("central offset");
    let central_size = u32::try_from(central.len()).expect("central size");
    let count = u16::try_from(parts.len()).expect("zip count");
    let mut out = local;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&central_size.to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn content_types(overrides: &[(&str, &str)]) -> String {
    let mut body = String::new();
    for (part, kind) in overrides {
        body.push_str(&format!(
            r#"<Override PartName="{part}" ContentType="{kind}"/>"#
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{body}</Types>"#
    )
}

fn relationships(items: &[(&str, &str, &str)]) -> String {
    let mut body = String::new();
    for (id, kind, target) in items {
        body.push_str(&format!(
            r#"<Relationship Id="{id}" Type="{kind}" Target="{target}"/>"#
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL_NS}">{body}</Relationships>"#
    )
}

fn document_xml(inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="{W_NS}" xmlns:r="{R_NS}"><w:body>{inner}</w:body></w:document>"#
    )
}

fn a4_sect(extra: &str) -> String {
    format!(
        r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838" w:orient="portrait"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/>{extra}</w:sectPr>"#
    )
}

fn scratch(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("joyword-s5-{label}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cmd(office: &mut Office, id: CommandId, args: &[(&'static str, ArgValue)]) {
    office
        .invoke(id, args)
        .unwrap_or_else(|error| panic!("{} failed: {error:?}", id.0));
}

fn open_package(bytes: &[u8], label: &str) -> (PathBuf, Office) {
    let dir = scratch(label);
    let source = dir.join("source.docx");
    std::fs::write(&source, bytes).unwrap();
    let mut office = Office::new();
    cmd(&mut office, FILE_OPEN, &[("path", ArgValue::Path(source))]);
    (dir, office)
}

fn save_and_reopen(office: &mut Office, dir: &Path, name: &str) {
    let destination = dir.join(name);
    cmd(
        office,
        FILE_SAVE_COPY,
        &[("destination", ArgValue::Path(destination.clone()))],
    );
    cmd(office, FILE_OPEN, &[("path", ArgValue::Path(destination))]);
}

fn active_document(office: &Office) -> &joyword_model::Document {
    office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
}

fn story_text(office: &Office) -> String {
    office.active().unwrap().view().index().text().to_owned()
}

fn select_paragraph(office: &mut Office, index: usize) {
    let text = story_text(office);
    let mut start = 0usize;
    for (at, part) in text.split('\n').enumerate() {
        let len = part.encode_utf16().count();
        if at == index {
            office
                .active_mut()
                .unwrap()
                .select(start..start + len, false)
                .unwrap();
            return;
        }
        start += len + 1;
    }
    panic!("paragraph {index} missing from {text:?}");
}

fn caret_at(office: &mut Office, offset: usize) {
    office
        .active_mut()
        .unwrap()
        .select(offset..offset, false)
        .unwrap();
}

fn undo(office: &mut Office) {
    office.active_mut().unwrap().undo().unwrap();
}

fn redo(office: &mut Office) {
    office.active_mut().unwrap().redo().unwrap();
}

fn undo_all(office: &mut Office) {
    let mut steps = 0usize;
    while office.active().unwrap().can_undo() {
        undo(office);
        steps += 1;
        assert!(steps < 80, "undo did not drain");
    }
}

fn redo_all(office: &mut Office) {
    let mut steps = 0usize;
    while office.active().unwrap().can_redo() {
        redo(office);
        steps += 1;
        assert!(steps < 80, "redo did not drain");
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParaMark {
    text: String,
    style: Option<String>,
    numbering: Option<(u32, u8)>,
    bold: bool,
}

fn paragraph_marks(office: &Office) -> Vec<ParaMark> {
    active_document(office)
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| ParaMark {
            text: paragraph.plain_text(|_| String::new()),
            style: paragraph.style.as_ref().map(|id| id.as_str().to_owned()),
            numbering: paragraph
                .props
                .numbering
                .as_ref()
                .map(|item| (item.num_id, item.level)),
            bold: paragraph
                .runs()
                .iter()
                .any(|run| run.props.bold == Some(true)),
        })
        .collect()
}

fn test_font() -> LoadedFont {
    LoadedFont::from_bytes(
        std::sync::Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .expect("joyoffice-test-serif")
}

fn rehearsal_profile() -> SingleFontProfile {
    SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    }
}

fn lay(office: &Office, view: ViewOptions) -> LayoutSnapshot {
    let font = test_font();
    let snapshot = active_document(office).snapshot();
    layout_with_view(
        &snapshot,
        FaceSet::uniform(&font),
        rehearsal_profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        view,
    )
    .unwrap_or_else(|error| panic!("layout failed: {error}"))
}

fn lay_review(office: &Office, view: RevisionView) -> LayoutSnapshot {
    let font = test_font();
    let snapshot = active_document(office).snapshot();
    layout_document_viewed(
        &snapshot,
        FaceSet::uniform(&font),
        rehearsal_profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        view,
    )
    .unwrap_or_else(|error| panic!("review layout failed: {error}"))
}

fn scene_text(snapshot: &LayoutSnapshot) -> String {
    snapshot
        .pages()
        .iter()
        .flat_map(|page| page.runs.iter().map(|run| run.text.as_str()))
        .collect()
}

fn body_page_text(snapshot: &LayoutSnapshot, page: usize) -> String {
    snapshot
        .lines()
        .iter()
        .filter(|line| line.page == page)
        .fold(String::new(), |mut text, line| {
            text.push_str(&snapshot.pages()[line.page].runs[line.run].text);
            text
        })
}

fn header_page_numbers(snapshot: &LayoutSnapshot) -> Vec<u32> {
    snapshot
        .pages()
        .iter()
        .map(|page| {
            let joined: String = page.runs.iter().map(|run| run.text.as_str()).collect();
            let start = joined
                .find('V')
                .unwrap_or_else(|| panic!("header run missing from {joined}"));
            let digits: String = joined[start + 'V'.len_utf8()..]
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect();
            digits
                .parse()
                .unwrap_or_else(|_| panic!("page number missing from {joined}"))
        })
        .collect()
}

fn styles_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="{W_NS}"><w:style w:type="paragraph" w:styleId="Title1"><w:name w:val="heading 1"/><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Title2"><w:name w:val="heading 2"/><w:basedOn w:val="Title1"/><w:rPr><w:i/></w:rPr></w:style></w:styles>"#
    )
}

fn numbering_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="{W_NS}"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1.%2."/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
    )
}

fn plain_package(paragraphs: &str) -> Vec<u8> {
    let types = content_types(&[("/word/document.xml", CT_DOCUMENT)]);
    let package_rels = relationships(&[("rId1", REL_OFFICE, "word/document.xml")]);
    let document = document_xml(&format!("{paragraphs}{}", a4_sect("")));
    let owned = [types, package_rels, document];
    let parts = [
        ("[Content_Types].xml", owned[0].as_bytes()),
        ("_rels/.rels", owned[1].as_bytes()),
        ("word/document.xml", owned[2].as_bytes()),
    ];
    store_zip(&parts)
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T200: headings, two list levels, the format painter, width-folded regex
/// replacement with `$1` and `$$`, then a full undo back to the opening text.
#[test]
fn t200_headings_lists_painter_and_regex_undo_round_trip() {
    let paragraphs = [
        "River brief",
        "Brook note",
        "First item",
        "Nested item",
        "ＡＡbbb fee",
    ]
    .into_iter()
    .map(|text| format!(r#"<w:p><w:r><w:t>{text}</w:t></w:r></w:p>"#))
    .collect::<String>();
    let types = content_types(&[
        ("/word/document.xml", CT_DOCUMENT),
        ("/word/styles.xml", CT_STYLES),
        ("/word/numbering.xml", CT_NUMBERING),
    ]);
    let package_rels = relationships(&[("rId1", REL_OFFICE, "word/document.xml")]);
    let document_rels = relationships(&[
        ("rId1", REL_STYLES, "styles.xml"),
        ("rId2", REL_NUMBERING, "numbering.xml"),
    ]);
    let document = document_xml(&format!("{paragraphs}{}", a4_sect("")));
    let styles = styles_xml();
    let numbering = numbering_xml();
    let owned = [
        types,
        package_rels,
        document_rels,
        document,
        styles,
        numbering,
    ];
    let bytes = store_zip(&[
        ("[Content_Types].xml", owned[0].as_bytes()),
        ("_rels/.rels", owned[1].as_bytes()),
        ("word/_rels/document.xml.rels", owned[2].as_bytes()),
        ("word/document.xml", owned[3].as_bytes()),
        ("word/styles.xml", owned[4].as_bytes()),
        ("word/numbering.xml", owned[5].as_bytes()),
    ]);
    let (dir, mut office) = open_package(&bytes, "t200");
    let opening = paragraph_marks(&office);
    assert_eq!(
        opening
            .iter()
            .map(|mark| mark.text.as_str())
            .collect::<Vec<_>>(),
        vec![
            "River brief",
            "Brook note",
            "First item",
            "Nested item",
            "ＡＡbbb fee"
        ]
    );
    assert!(
        opening
            .iter()
            .all(|mark| { mark.style.is_none() && mark.numbering.is_none() && !mark.bold })
    );

    cmd(&mut office, STYLE_LIST, &[]);
    let listed = office.listed_styles().join("\n");
    assert!(listed.contains("heading 1"), "{listed}");
    assert!(listed.contains("heading 2"), "{listed}");

    select_paragraph(&mut office, 0);
    cmd(&mut office, EDIT_BOLD, &[]);
    cmd(&mut office, FORMAT_COPY, &[]);
    assert!(office.active().unwrap().format_armed());
    select_paragraph(&mut office, 1);
    cmd(&mut office, FORMAT_PAINT, &[]);
    assert!(!office.active().unwrap().format_armed());
    assert!(paragraph_marks(&office)[0].bold);
    assert!(paragraph_marks(&office)[1].bold);

    select_paragraph(&mut office, 0);
    cmd(
        &mut office,
        STYLE_APPLY,
        &[("style", ArgValue::Text("Title1".into()))],
    );
    select_paragraph(&mut office, 1);
    cmd(
        &mut office,
        STYLE_APPLY,
        &[("style", ArgValue::Text("Title2".into()))],
    );
    assert_eq!(paragraph_marks(&office)[0].style.as_deref(), Some("Title1"));
    assert_eq!(paragraph_marks(&office)[1].style.as_deref(), Some("Title2"));

    select_paragraph(&mut office, 2);
    cmd(
        &mut office,
        LIST_APPLY,
        &[("num", ArgValue::Text("1".into()))],
    );
    select_paragraph(&mut office, 3);
    cmd(
        &mut office,
        LIST_APPLY,
        &[("num", ArgValue::Text("1".into()))],
    );
    cmd(
        &mut office,
        LIST_LEVEL,
        &[("level", ArgValue::Text("1".into()))],
    );
    assert_eq!(paragraph_marks(&office)[2].numbering, Some((1, 0)));
    assert_eq!(paragraph_marks(&office)[3].numbering, Some((1, 1)));

    // Half-width `A` does not hit `Ａ` until width folding is on.
    cmd(
        &mut office,
        FIND_REPLACE_ALL,
        &[
            ("query", ArgValue::Text("(A+)(b+)".into())),
            ("replacement", ArgValue::Text("$2-$$-$1".into())),
            ("regex", ArgValue::Bool(true)),
        ],
    );
    assert_eq!(paragraph_marks(&office)[4].text, "ＡＡbbb fee");
    cmd(
        &mut office,
        FIND_REPLACE_ALL,
        &[
            ("query", ArgValue::Text("(A+)(b+)".into())),
            ("replacement", ArgValue::Text("$2-$$-$1".into())),
            ("regex", ArgValue::Bool(true)),
            ("width", ArgValue::Bool(true)),
        ],
    );
    assert_eq!(paragraph_marks(&office)[4].text, "bbb-$-ＡＡ fee");
    let edited = paragraph_marks(&office);

    undo_all(&mut office);
    assert_eq!(paragraph_marks(&office), opening);
    redo_all(&mut office);
    assert_eq!(paragraph_marks(&office), edited);

    save_and_reopen(&mut office, &dir, "t200-copy.docx");
    assert_eq!(paragraph_marks(&office), edited);
    let _ = std::fs::remove_dir_all(&dir);
}

fn body_table(office: &Office) -> &joyword_model::Table {
    active_document(office)
        .body()
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table.as_ref()),
            _ => None,
        })
        .expect("contract table")
}

fn grid_fingerprint(office: &Office) -> String {
    let table = body_table(office);
    let mut out = format!(
        "cols={} width={:?} align={:?}\n",
        table.grid.len(),
        table.width,
        table.alignment
    );
    for row in &table.rows {
        for cell in &row.cells {
            let border = cell
                .borders
                .top
                .map(|edge| format!("{:?}", edge.kind))
                .unwrap_or_else(|| "-".into());
            out.push_str(&format!(
                " {}:{:?}:{border}",
                cell.grid_span, cell.vertical_merge
            ));
        }
        out.push('\n');
    }
    out
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T201: insert a contract table, add and remove a row, add a column, set
/// properties, merge down, split, merge right, undoing each step, then
/// export and re-import the same grid.
#[test]
fn t201_contract_table_edits_undo_and_reimport() {
    let (dir, mut office) = open_package(
        &plain_package(r#"<w:p><w:r><w:t>Northwind supply note</w:t></w:r></w:p>"#),
        "t201",
    );
    assert_eq!(story_text(&office), "Northwind supply note");

    cmd(
        &mut office,
        TABLE_INSERT,
        &[
            ("rows", ArgValue::Text("2".into())),
            ("columns", ArgValue::Text("2".into())),
        ],
    );
    assert_eq!(body_table(&office).rows.len(), 2);
    assert_eq!(body_table(&office).grid.len(), 2);
    undo(&mut office);
    assert!(
        active_document(&office)
            .body()
            .blocks
            .iter()
            .all(|block| { !matches!(block, Block::Table(_)) })
    );
    redo(&mut office);
    assert_eq!(body_table(&office).rows.len(), 2);
    // Undo restores the pre-insert caret, which sits in the paragraph after
    // the table. Later table commands need a cell, so place it in the first.
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_ROW_INSERT, &[]);
    assert_eq!(body_table(&office).rows.len(), 3);
    undo(&mut office);
    assert_eq!(body_table(&office).rows.len(), 2);
    redo(&mut office);
    assert_eq!(body_table(&office).rows.len(), 3);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_COLUMN_INSERT, &[]);
    assert_eq!(body_table(&office).grid.len(), 3);
    assert!(
        body_table(&office)
            .rows
            .iter()
            .all(|row| row.cells.len() == 3)
    );
    undo(&mut office);
    assert_eq!(body_table(&office).grid.len(), 2);
    redo(&mut office);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_ROW_DELETE, &[]);
    assert_eq!(body_table(&office).rows.len(), 2);
    undo(&mut office);
    assert_eq!(body_table(&office).rows.len(), 3);
    caret_at(&mut office, 0);

    cmd(
        &mut office,
        TABLE_PROPERTIES,
        &[
            ("commit", ArgValue::Text("apply".into())),
            ("width", ArgValue::Text("dxa:4800".into())),
            ("align", ArgValue::Text("center".into())),
            ("border", ArgValue::Text("dashed".into())),
            ("border_size", ArgValue::Text("8".into())),
        ],
    );
    assert_eq!(body_table(&office).width, Some(TableWidth::Dxa(4800)));
    assert_eq!(body_table(&office).alignment, Some(TableAlign::Center));
    assert_eq!(
        body_table(&office).rows[0].cells[0]
            .borders
            .top
            .unwrap()
            .kind,
        BorderKind::Dashed
    );
    undo(&mut office);
    assert!(body_table(&office).width.is_none());
    redo(&mut office);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_MERGE_DOWN, &[]);
    assert_eq!(
        body_table(&office).rows[0].cells[0].vertical_merge,
        VerticalMerge::Restart
    );
    assert_eq!(
        body_table(&office).rows[1].cells[0].vertical_merge,
        VerticalMerge::Continue
    );
    undo(&mut office);
    assert_eq!(
        body_table(&office).rows[0].cells[0].vertical_merge,
        VerticalMerge::None
    );
    redo(&mut office);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_SPLIT, &[]);
    assert_eq!(
        body_table(&office).rows[0].cells[0].vertical_merge,
        VerticalMerge::None
    );
    assert_eq!(
        body_table(&office).rows[1].cells[0].vertical_merge,
        VerticalMerge::None
    );
    undo(&mut office);
    assert_eq!(
        body_table(&office).rows[0].cells[0].vertical_merge,
        VerticalMerge::Restart
    );
    redo(&mut office);
    caret_at(&mut office, 0);

    cmd(&mut office, TABLE_MERGE_RIGHT, &[]);
    assert_eq!(body_table(&office).rows[0].cells[0].grid_span, 2);
    assert_eq!(body_table(&office).rows[0].cells.len(), 2);
    undo(&mut office);
    assert_eq!(body_table(&office).rows[0].cells.len(), 3);
    redo(&mut office);
    let edited = grid_fingerprint(&office);
    assert!(edited.contains("cols=3"), "{edited}");
    assert!(edited.contains("Dxa(4800)"), "{edited}");
    assert!(edited.contains("Center"), "{edited}");
    assert!(story_text(&office).contains("Northwind supply note"));

    save_and_reopen(&mut office, &dir, "t201-copy.docx");
    assert_eq!(grid_fingerprint(&office), edited);
    let _ = std::fs::remove_dir_all(&dir);
}

fn section_page(office: &Office) -> PageSetup {
    active_document(office).sections()[0].page.clone()
}

fn header_story_present(office: &Office) -> bool {
    let section = &active_document(office).sections()[0];
    section.headers.iter().any(|(kind, story)| {
        *kind == HeaderFooterKind::Default
            && active_document(office)
                .story(*story)
                .is_some_and(|item| item.kind == StoryKind::Header)
    })
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T202: margins, orientation and a page number, with the header story and
/// the laid-out PAGE values. Zoom is a screen projection. Formatting marks
/// are a view option that does not change the page count. Inserting
/// `page.number` into an existing body paragraph is outside the export
/// subset, so that step is undone before the package is saved; the header
/// PAGE field is what the re-import lays out.
#[test]
fn t202_margins_orientation_page_numbers_and_zoom() {
    let line = "A A A A A A A A A A";
    let mut paragraphs = String::new();
    for _ in 0..60 {
        paragraphs.push_str(&format!(r#"<w:p><w:r><w:t>{line}</w:t></w:r></w:p>"#));
    }
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W_NS}"><w:p><w:r><w:t>V</w:t></w:r><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p></w:hdr>"#
    );
    let types = content_types(&[
        ("/word/document.xml", CT_DOCUMENT),
        ("/word/header1.xml", CT_HEADER),
    ]);
    let package_rels = relationships(&[("rId1", REL_OFFICE, "word/document.xml")]);
    let document_rels = relationships(&[("rId3", REL_HEADER, "header1.xml")]);
    let document = document_xml(&format!(
        "{paragraphs}{}",
        a4_sect(r#"<w:headerReference w:type="default" r:id="rId3"/>"#)
    ));
    let owned = [types, package_rels, document_rels, document, header];
    let bytes = store_zip(&[
        ("[Content_Types].xml", owned[0].as_bytes()),
        ("_rels/.rels", owned[1].as_bytes()),
        ("word/_rels/document.xml.rels", owned[2].as_bytes()),
        ("word/document.xml", owned[3].as_bytes()),
        ("word/header1.xml", owned[4].as_bytes()),
    ]);
    let (dir, mut office) = open_package(&bytes, "t202");
    assert!(header_story_present(&office));
    let opening = section_page(&office);
    let opening_pages = lay(&office, ViewOptions::default());
    let opening_count = opening_pages.pages().len();
    assert!(opening_count >= 2, "opening pages {opening_count}");
    let opening_numbers = header_page_numbers(&opening_pages);
    assert_eq!(
        opening_numbers,
        (1..=opening_count as u32).collect::<Vec<_>>()
    );

    cmd(
        &mut office,
        PAGE_MARGINS,
        &[
            ("top", ArgValue::Text("4000".into())),
            ("right", ArgValue::Text("1800".into())),
            ("bottom", ArgValue::Text("4000".into())),
            ("left", ArgValue::Text("1800".into())),
        ],
    );
    assert_eq!(
        section_page(&office).top,
        joyword_model::Measure::Twips(4000)
    );
    let narrow_count = lay(&office, ViewOptions::default()).pages().len();
    assert!(
        narrow_count > opening_count,
        "margins {opening_count} -> {narrow_count}"
    );
    undo(&mut office);
    assert_eq!(section_page(&office).top, opening.top);
    assert_eq!(
        lay(&office, ViewOptions::default()).pages().len(),
        opening_count
    );
    redo(&mut office);

    cmd(
        &mut office,
        PAGE_ORIENTATION,
        &[("orientation", ArgValue::Text("landscape".into()))],
    );
    let landscape = section_page(&office);
    assert_eq!(landscape.orientation, Orientation::Landscape);
    assert_eq!(landscape.width, opening.height);
    assert_eq!(landscape.height, opening.width);
    let landscape_count = lay(&office, ViewOptions::default()).pages().len();
    assert!(
        landscape_count > narrow_count,
        "landscape {narrow_count} -> {landscape_count}"
    );
    undo(&mut office);
    assert_eq!(section_page(&office).orientation, Orientation::Portrait);
    redo(&mut office);

    caret_at(&mut office, 0);
    cmd(&mut office, PAGE_NUMBER, &[]);
    let laid = lay(&office, ViewOptions::default());
    assert!(
        body_page_text(&laid, 0).contains('1'),
        "body {}",
        body_page_text(&laid, 0)
    );
    let numbers = header_page_numbers(&laid);
    assert_eq!(numbers, (1..=laid.pages().len() as u32).collect::<Vec<_>>());
    assert_eq!(laid.pages().len(), landscape_count);
    undo(&mut office);
    assert!(!body_page_text(&lay(&office, ViewOptions::default()), 0).contains('1'));
    redo(&mut office);
    let refused = office
        .active()
        .unwrap()
        .backend()
        .session()
        .export_plan()
        .expect_err("a body PAGE field is outside the export subset");
    let message = refused.to_string();
    assert!(
        message.contains("paragraph formatting, source or inline objects changed"),
        "{message}"
    );
    undo(&mut office);
    assert!(!body_page_text(&lay(&office, ViewOptions::default()), 0).contains('1'));

    let again = lay(&office, ViewOptions::default());
    let marks = lay(
        &office,
        ViewOptions {
            formatting_marks: true,
            ..ViewOptions::default()
        },
    );
    assert_eq!(again.pages().len(), marks.pages().len());
    assert_eq!(
        again
            .lines()
            .iter()
            .map(|line| line.page)
            .collect::<Vec<_>>(),
        marks
            .lines()
            .iter()
            .map(|line| line.page)
            .collect::<Vec<_>>()
    );
    assert_ne!(again.key().view, marks.key().view);
    let width = again.pages()[0].width;
    let half = to_screen(width, ScreenScale::new(0.5).unwrap());
    let full = to_screen(width, ScreenScale::new(1.0).unwrap());
    let twice = to_screen(width, ScreenScale::new(2.0).unwrap());
    assert!((full - half * 2.0).abs() < 0.01);
    assert!((twice - full * 2.0).abs() < 0.01);
    assert_eq!(again.pages().len(), landscape_count);

    save_and_reopen(&mut office, &dir, "t202-copy.docx");
    assert!(header_story_present(&office));
    assert_eq!(section_page(&office).orientation, Orientation::Landscape);
    assert_eq!(
        section_page(&office).top,
        joyword_model::Measure::Twips(4000)
    );
    let reopened = lay(&office, ViewOptions::default());
    assert_eq!(reopened.pages().len(), landscape_count);
    assert_eq!(
        header_page_numbers(&reopened),
        (1..=landscape_count as u32).collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T209: keyboard typing coalesces, and a deletion run, a paste, a command
/// insert and two marked-text commits are each their own undo step.
/// Drag and autocorrect are not implemented, so they are not rehearsable.
#[test]
fn t209_keyboard_paste_command_and_ime_undo_one_step_at_a_time() {
    let (dir, mut office) = open_package(
        &plain_package(r#"<w:p><w:r><w:t>Note</w:t></w:r></w:p>"#),
        "t209",
    );
    let end = office.active().unwrap().view().index().len_utf16();
    caret_at(&mut office, end);
    for ch in ['a', 'b'] {
        office
            .active_mut()
            .unwrap()
            .replace(None, &ch.to_string())
            .unwrap();
    }
    assert_eq!(story_text(&office), "Noteab");
    assert_eq!(
        office
            .active()
            .unwrap()
            .backend()
            .session()
            .session()
            .undo_depth(),
        1
    );
    office
        .active_mut()
        .unwrap()
        .delete(Direction::Backward)
        .unwrap();
    office
        .active_mut()
        .unwrap()
        .delete(Direction::Backward)
        .unwrap();
    assert_eq!(story_text(&office), "Note");
    office
        .active_mut()
        .unwrap()
        .replace_from(InputSource::Paste, None, "P")
        .unwrap();
    office
        .active_mut()
        .unwrap()
        .replace_from(InputSource::Command, None, "C")
        .unwrap();
    office.active_mut().unwrap().mark(None, "fa", None).unwrap();
    office.active_mut().unwrap().unmark().unwrap();
    office.active_mut().unwrap().mark(None, "ii", None).unwrap();
    office.active_mut().unwrap().replace(None, "i").unwrap();
    assert_eq!(story_text(&office), "NotePCfai");
    assert_eq!(
        office
            .active()
            .unwrap()
            .backend()
            .session()
            .session()
            .undo_depth(),
        6
    );

    let mut seen = Vec::new();
    while office.active().unwrap().can_undo() {
        undo(&mut office);
        seen.push(story_text(&office));
    }
    assert_eq!(
        seen,
        vec![
            "NotePCfa".to_owned(),
            "NotePC".to_owned(),
            "NoteP".to_owned(),
            "Note".to_owned(),
            "Noteab".to_owned(),
            "Note".to_owned(),
        ]
    );
    redo_all(&mut office);
    assert_eq!(story_text(&office), "NotePCfai");
    save_and_reopen(&mut office, &dir, "t209-copy.docx");
    assert_eq!(story_text(&office), "NotePCfai");
    let _ = std::fs::remove_dir_all(&dir);
}

fn run_kinds(office: &Office, header: bool) -> Vec<RevisionKind> {
    let document = active_document(office);
    let paragraphs: Vec<_> = if header {
        let story = document.sections()[0].headers[0].1;
        document.story(story).unwrap().blocks.iter().collect()
    } else {
        document.body().blocks.iter().collect()
    };
    paragraphs
        .into_iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|paragraph| paragraph.runs().to_vec())
        .filter_map(|run| run.revision.map(|mark| mark.kind))
        .collect()
}

/// Automated rehearsal; does not replace 人工验收.
///
/// T300, JoyWord side only: track, type and delete in the body and the
/// header, accept and reject one change and the whole set, project the three
/// review views, undo, then keep `w:ins` / `w:del` / move pairs across export.
#[test]
fn t300_tracked_edits_views_and_revision_round_trip() {
    let body = r#"<w:p><w:moveFromRangeStart w:id="1" w:author="Ada" w:name="shift" w:date="2026-04-01T00:00:00Z"/><w:moveFrom w:id="2" w:author="Ada" w:date="2026-04-01T00:00:00Z"><w:r><w:delText>A</w:delText></w:r></w:moveFrom><w:moveFromRangeEnd w:id="3" w:author="Ada" w:name="shift" w:date="2026-04-01T00:00:00Z"/><w:moveToRangeStart w:id="4" w:author="Ada" w:name="shift" w:date="2026-04-01T00:00:00Z"/><w:moveTo w:id="5" w:author="Ada" w:date="2026-04-01T00:00:00Z"><w:r><w:t>V</w:t></w:r></w:moveTo><w:moveToRangeEnd w:id="6" w:author="Ada" w:name="shift" w:date="2026-04-01T00:00:00Z"/><w:r><w:t>i</w:t></w:r></w:p>"#;
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W_NS}"><w:p><w:r><w:t>i</w:t></w:r></w:p></w:hdr>"#
    );
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W_NS}"></w:settings>"#
    );
    let types = content_types(&[
        ("/word/document.xml", CT_DOCUMENT),
        ("/word/header1.xml", CT_HEADER),
        ("/word/settings.xml", CT_SETTINGS),
    ]);
    let package_rels = relationships(&[("rId1", REL_OFFICE, "word/document.xml")]);
    let document_rels = relationships(&[
        ("rId2", REL_SETTINGS, "settings.xml"),
        ("rId3", REL_HEADER, "header1.xml"),
    ]);
    let document = document_xml(&format!(
        "{body}{}",
        a4_sect(r#"<w:headerReference w:type="default" r:id="rId3"/>"#)
    ));
    let owned = [
        types,
        package_rels,
        document_rels,
        document,
        header,
        settings,
    ];
    let bytes = store_zip(&[
        ("[Content_Types].xml", owned[0].as_bytes()),
        ("_rels/.rels", owned[1].as_bytes()),
        ("word/_rels/document.xml.rels", owned[2].as_bytes()),
        ("word/document.xml", owned[3].as_bytes()),
        ("word/header1.xml", owned[4].as_bytes()),
        ("word/settings.xml", owned[5].as_bytes()),
    ]);
    let (dir, mut office) = open_package(&bytes, "t300");
    assert_eq!(story_text(&office), "AVi");
    assert_eq!(active_document(&office).move_markers().len(), 4);
    assert!(header_story_present(&office));

    cmd(
        &mut office,
        REVIEW_TRACK,
        &[("enabled", ArgValue::Bool(true))],
    );
    assert!(office.active().unwrap().tracking_enabled());
    let end = office.active().unwrap().view().index().len_utf16();
    caret_at(&mut office, end);
    office.active_mut().unwrap().replace(None, "f").unwrap();
    caret_at(&mut office, 2);
    office
        .active_mut()
        .unwrap()
        .delete(Direction::Forward)
        .unwrap();
    assert_eq!(story_text(&office), "AVif");

    cmd(&mut office, PAGE_EDIT_HEADER, &[]);
    assert!(!office.active().unwrap().is_editing_body());
    let header_end = office.active().unwrap().view().index().len_utf16();
    caret_at(&mut office, header_end);
    office.active_mut().unwrap().replace(None, "f").unwrap();
    caret_at(&mut office, 0);
    office
        .active_mut()
        .unwrap()
        .delete(Direction::Forward)
        .unwrap();
    cmd(&mut office, PAGE_EDIT_BODY, &[]);
    assert!(office.active().unwrap().is_editing_body());

    let body_kinds = run_kinds(&office, false);
    let header_kinds = run_kinds(&office, true);
    assert!(body_kinds.contains(&RevisionKind::Insert), "{body_kinds:?}");
    assert!(body_kinds.contains(&RevisionKind::Delete), "{body_kinds:?}");
    assert!(
        body_kinds.contains(&RevisionKind::MoveFrom),
        "{body_kinds:?}"
    );
    assert!(body_kinds.contains(&RevisionKind::MoveTo), "{body_kinds:?}");
    assert!(
        header_kinds.contains(&RevisionKind::Insert),
        "{header_kinds:?}"
    );
    assert!(
        header_kinds.contains(&RevisionKind::Delete),
        "{header_kinds:?}"
    );

    for (id, view) in [
        (REVIEW_MARKUP, RevisionView::FinalWithMarkup),
        (REVIEW_FINAL, RevisionView::Final),
        (REVIEW_ORIGINAL, RevisionView::Original),
    ] {
        cmd(&mut office, id, &[]);
        assert_eq!(office.review_view(), view);
    }
    let markup = scene_text(&lay_review(&office, RevisionView::FinalWithMarkup));
    let final_text = scene_text(&lay_review(&office, RevisionView::Final));
    let original = scene_text(&lay_review(&office, RevisionView::Original));
    for letter in ['A', 'V', 'i', 'f'] {
        assert!(markup.contains(letter), "{letter} missing from {markup}");
    }
    assert!(
        final_text.contains('V') && final_text.contains('f'),
        "{final_text}"
    );
    assert!(!final_text.contains('A'), "{final_text}");
    assert!(
        original.contains('A') && original.contains('i'),
        "{original}"
    );
    assert!(
        !original.contains('V') && !original.contains('f'),
        "{original}"
    );
    cmd(&mut office, REVIEW_MARKUP, &[]);

    let end = office.active().unwrap().view().index().len_utf16();
    caret_at(&mut office, end);
    cmd(&mut office, REVIEW_ACCEPT, &[]);
    assert!(!run_kinds(&office, false).contains(&RevisionKind::Insert));
    undo(&mut office);
    assert!(run_kinds(&office, false).contains(&RevisionKind::Insert));

    caret_at(&mut office, 3);
    cmd(&mut office, REVIEW_REJECT, &[]);
    assert!(!run_kinds(&office, false).contains(&RevisionKind::Delete));
    undo(&mut office);
    assert!(run_kinds(&office, false).contains(&RevisionKind::Delete));

    cmd(&mut office, REVIEW_ACCEPT_ALL, &[]);
    assert!(run_kinds(&office, false).is_empty());
    assert!(active_document(&office).move_markers().is_empty());
    undo(&mut office);
    assert_eq!(active_document(&office).move_markers().len(), 4);
    assert!(run_kinds(&office, false).contains(&RevisionKind::MoveFrom));

    cmd(&mut office, REVIEW_REJECT_ALL, &[]);
    assert!(run_kinds(&office, false).is_empty());
    undo(&mut office);
    assert_eq!(run_kinds(&office, false), body_kinds);
    assert_eq!(run_kinds(&office, true), header_kinds);

    let plan = office
        .active()
        .unwrap()
        .backend()
        .session()
        .export_plan()
        .unwrap();
    let xml = String::from_utf8_lossy(plan.bytes());
    assert!(xml.contains("w:ins"), "export dropped insertions");
    assert!(xml.contains("w:del"), "export dropped deletions");
    assert!(xml.contains("w:moveFrom"), "export dropped moveFrom");
    assert!(xml.contains("w:moveTo"), "export dropped moveTo");

    save_and_reopen(&mut office, &dir, "t300-copy.docx");
    assert_eq!(run_kinds(&office, false), body_kinds);
    assert_eq!(run_kinds(&office, true), header_kinds);
    assert_eq!(active_document(&office).move_markers().len(), 4);
    assert_eq!(story_text(&office), "AVif");
    let _ = std::fs::remove_dir_all(&dir);
}
