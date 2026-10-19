//! Paste policy and file insertion for still images (CLIP-001).

use std::sync::Arc;

use joyword_engine::Session;
use joyword_model::{BlockSeq, Document, InlineKind, Paragraph, sample_png_1x1};
use joyword_shell::input::{ClipboardOffer, InputLimits, PasteChoice, TextInput, choose_paste};

fn input(text: &str) -> TextInput {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    for line in text.split('\n') {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text(line, None, Arc::default()).unwrap();
        blocks.push(paragraph.into());
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    TextInput::new(Session::new(document), InputLimits::default()).unwrap()
}

#[test]
fn choose_paste_prefers_an_admitted_image_over_text() {
    let png = sample_png_1x1();
    let choice = choose_paste(&[
        ClipboardOffer::Text("keep"),
        ClipboardOffer::Image { bytes: &png },
    ]);
    assert_eq!(choice, PasteChoice::Image(png.as_slice()));
}

#[test]
fn a_refused_image_paste_does_not_change_the_document() {
    let choice = choose_paste(&[
        ClipboardOffer::Image {
            bytes: b"<svg></svg>",
        },
        ClipboardOffer::Text("fallback"),
    ]);
    assert!(matches!(choice, PasteChoice::Refused(_)));
    let mut editor = input("ab");
    let version = editor.backend().version();
    let error = editor.insert_image_bytes(b"<svg></svg>").unwrap_err();
    assert!(matches!(error, joyword_shell::input::InputError::Image(_)));
    assert_eq!(editor.backend().version(), version);
    assert_eq!(editor.view().index().text(), "ab");
}

#[test]
fn clipboard_file_paths_are_not_opened() {
    assert_eq!(
        choose_paste(&[ClipboardOffer::Paths]),
        PasteChoice::Refused("clipboard file paths are not opened")
    );
    assert_eq!(
        choose_paste(&[ClipboardOffer::Text("")]),
        PasteChoice::Empty
    );
}

#[test]
fn inserting_an_image_file_is_one_undo_and_remote_paths_are_refused() {
    let mut editor = input("ab");
    let path = std::env::temp_dir().join(format!("joyword-image-{}.png", std::process::id()));
    std::fs::write(&path, sample_png_1x1()).unwrap();
    editor.insert_image_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let paragraph = editor
        .backend()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert!(
        paragraph
            .inlines()
            .iter()
            .any(|inline| matches!(inline.kind, InlineKind::Image { .. }))
    );
    assert!(!editor.selected_text().unwrap().contains('\u{FDD0}'));
    editor.select_all().unwrap();
    assert!(!editor.selected_text().unwrap().contains('\u{FDD0}'));
    assert!(editor.selected_text().unwrap().contains('a'));
    editor.undo().unwrap();
    assert_eq!(editor.view().index().text(), "ab");
    let remote = editor.insert_image_file(std::path::Path::new("http://example.invalid/a.png"));
    assert_eq!(
        remote.unwrap_err(),
        joyword_shell::input::InputError::Image("remote image")
    );
    let unc = editor.insert_image_file(std::path::Path::new(r"\\server\share\a.png"));
    assert_eq!(
        unc.unwrap_err(),
        joyword_shell::input::InputError::Image("remote image")
    );
    // A drive letter is a local path. The file is absent, so the read fails
    // without the remote refusal that rejected `C:\...` temp files on Windows.
    let drive = editor.insert_image_file(std::path::Path::new(r"C:\joyword-missing.png"));
    assert_eq!(
        drive.unwrap_err(),
        joyword_shell::input::InputError::Image("image file could not be read")
    );
    assert_eq!(editor.view().index().text(), "ab");
}

#[test]
fn a_selected_image_exposes_its_bytes_for_the_clipboard() {
    let mut editor = input("");
    let png = sample_png_1x1();
    editor.insert_image_bytes(&png).unwrap();
    editor.select(0..1, false).unwrap();
    let copied = editor.selected_image_bytes().expect("image bytes");
    assert_eq!(copied.as_ref(), png.as_slice());
    editor.select(0..0, false).unwrap();
    assert!(editor.selected_image_bytes().is_none());
}

#[test]
fn dropped_image_paths_insert_in_order_and_skip_refused_files() {
    let mut editor = input("ab");
    let dir = std::env::temp_dir().join(format!("joyword-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = sample_png_1x1();
    let first = dir.join("first.png");
    let second = dir.join("second.png");
    let svg = dir.join("active.svg");
    std::fs::write(&first, &png).unwrap();
    std::fs::write(&second, &png).unwrap();
    std::fs::write(&svg, b"<svg></svg>").unwrap();
    let subdir = dir.join("folder");
    std::fs::create_dir_all(&subdir).unwrap();

    // The dropped set mixes admitted files with an active-content file and
    // a directory; the refused entries are skipped, not fatal.
    let inserted = editor
        .insert_image_paths(&[first.clone(), svg.clone(), second.clone(), subdir.clone()])
        .unwrap();
    assert_eq!(inserted, 2);
    let text = editor.view().index().text();
    assert_eq!(text.matches('\u{FDD0}').count(), 2, "{text}");

    // Each admitted image is its own undo step, in drop order.
    editor.undo().unwrap();
    assert_eq!(editor.view().index().text().matches('\u{FDD0}').count(), 1);
    editor.undo().unwrap();
    assert_eq!(editor.view().index().text(), "ab");

    // An all-refused drop reports the first refusal.
    let error = editor
        .insert_image_paths(&[svg.clone(), subdir.clone()])
        .unwrap_err();
    assert!(matches!(error, joyword_shell::input::InputError::Image(_)));
    let remote = editor
        .insert_image_paths(&[std::path::PathBuf::from("https://example.invalid/a.png")])
        .unwrap_err();
    assert_eq!(
        remote,
        joyword_shell::input::InputError::Image("remote image")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
