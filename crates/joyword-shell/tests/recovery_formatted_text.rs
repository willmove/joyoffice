//! Text-only checkpoints must not delete formatting on unchanged text.

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{W, import};
use joyword_model::Justification;
use joyword_shell::{host::Office, storage};
use std::{fs, path::PathBuf};

fn baseline() -> Vec<u8> {
    let source = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap();
    let package = Package::read(&source, &Budget::default()).unwrap();
    let xml = format!(
        r#"<w:document xmlns:w="{W}"><w:body><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>甲🙂乙</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>丙𐐀丁</w:t></w:r></w:p></w:body></w:document>"#
    );
    let mut members = vec![(
        "[Content_Types].xml",
        package.content_types_xml(),
        Method::Stored,
    )];
    for part in package.parts() {
        let data = if part.name.as_str() == "/word/document.xml" {
            xml.as_bytes()
        } else {
            &part.data
        };
        members.push((part.name.item_name(), data, part.method));
    }
    write_archive(&members).unwrap()
}

#[test]
fn text_recovery_preserves_unchanged_runs_and_exports_after_unicode_edits() {
    let original = baseline();
    for text in [
        "甲🙂乙丙𐐀丁 [RECOVERY-M7]",
        "前缀甲🙂乙丙𐐀丁",
        "甲🙂增乙丙𐐀丁",
        "甲🙂乙丙丁",
        "甲🙂乙换𐐀丁",
        "甲🙂乙丙𐐀丁", // No edit: preserve the exact original package.
    ] {
        let directory = tempfile::tempdir().unwrap();
        storage::commit_recovery(directory.path(), &original, text).unwrap();
        let office = Office::recover_from(directory.path()).unwrap();
        let input = office.active().unwrap();
        assert_eq!(input.view().index().text(), text);
        assert_eq!(
            input.backend().session().backing().original_bytes(),
            original
        );
        let plan = input.backend().session().export_plan().unwrap();
        let saved = import(plan.bytes(), &Budget::default()).unwrap();
        let paragraph = saved
            .document
            .body()
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap();
        assert_eq!(paragraph.plain_text(|_| String::new()), text);
        assert_eq!(paragraph.props.justification, Some(Justification::Center));
        assert_eq!(paragraph.runs().first().unwrap().props.bold, Some(true));
        assert_eq!(paragraph.runs().last().unwrap().props.italic, Some(true));
        if text == "甲🙂乙丙𐐀丁" {
            assert_eq!(plan.bytes(), original);
        }
    }
}

#[test]
fn repeated_recovery_never_overwrites_an_existing_or_locked_recovery_copy() {
    let original = baseline();
    let directory = tempfile::tempdir().unwrap();
    let previous = directory.path().join("recovered.docx");
    let previous_lock = directory.path().join(".recovered.docx.joyword-lock");
    fs::write(&previous, b"previous recovered document must survive").unwrap();
    fs::write(&previous_lock, b"previous owner").unwrap();
    storage::commit_recovery(directory.path(), &original, "甲🙂乙丙𐐀丁追加").unwrap();
    let checkpoint = storage::recover_checkpoint(directory.path()).unwrap();
    let first = Office::recover_from(directory.path()).unwrap();
    let first_path = first.active().unwrap().backend().source_path().to_owned();
    let first_bytes = fs::read(&first_path).unwrap();
    // Keep the first session (and its lock) alive during the second recovery.
    let second = Office::recover_from(directory.path()).unwrap();
    let second_path = second.active().unwrap().backend().source_path();
    assert_ne!(first_path, second_path);
    assert_ne!(previous, second_path);
    assert_eq!(fs::read(first_path).unwrap(), first_bytes);
    assert_eq!(
        fs::read(previous).unwrap(),
        b"previous recovered document must survive"
    );
    assert_eq!(fs::read(previous_lock).unwrap(), b"previous owner");
    assert_eq!(
        storage::recover_checkpoint(directory.path()).unwrap(),
        checkpoint
    );
    for office in [&first, &second] {
        let input = office.active().unwrap();
        assert_eq!(input.view().index().text(), "甲🙂乙丙𐐀丁追加");
        assert!(input.backend().session().export_plan().is_ok());
    }
}
