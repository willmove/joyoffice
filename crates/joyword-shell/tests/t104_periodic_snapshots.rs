//! T104: the production journal is a per-commit periodic snapshot. Every
//! mutating command publishes a complete generation — the original package
//! baseline (`PackageBacking` original bytes, not just the model) plus the
//! full committed text — and the previous generation stays on disk in the
//! other slot. Non-mutating commands publish nothing.

use std::fs;
use std::path::PathBuf;

use joyword_shell::command::{
    ArgValue, CommandId, EDIT_ALIGN_CENTER, EDIT_BOLD, EDIT_INSERT, EDIT_SELECT_ALL, FILE_OPEN,
    FIND_REPLACE_ALL, PAGE_BREAK, REVIEW_REJECT_ALL, REVIEW_TRACK, TABLE_INSERT,
};
use joyword_shell::host::Office;
use joyword_shell::storage;

fn fixture() -> Vec<u8> {
    use joyoffice_opc::{Budget, Method, Package, write_archive};
    let original = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap();
    let package = Package::read(&original, &Budget::default()).unwrap();
    // trackRevisions currently requires an existing settings part (T103's
    // separate creation limitation). Use a writable review-capable package.
    let content_types = String::from_utf8(package.content_types_xml().to_vec()).unwrap()
        .replace("</Types>", "<Override PartName=\"/word/settings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml\"/></Types>");
    let relationships = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdSettings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#;
    let settings = r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:settings>"#;
    let mut parts = vec![(
        "[Content_Types].xml",
        content_types.as_bytes(),
        Method::Deflated,
    )];
    parts.extend(
        package
            .parts()
            .filter(|part| part.name.as_str() != "/word/_rels/document.xml.rels")
            .map(|part| (part.name.item_name(), part.data.as_ref(), part.method)),
    );
    parts.push((
        "word/_rels/document.xml.rels",
        relationships.as_bytes(),
        Method::Deflated,
    ));
    parts.push(("word/settings.xml", settings.as_bytes(), Method::Deflated));
    write_archive(&parts).unwrap()
}

#[test]
fn every_committed_edit_publishes_a_complete_periodic_snapshot() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.docx");
    fs::write(&source, &bytes).unwrap();
    let recovery = dir.path().join("recovery");
    fs::create_dir(&recovery).unwrap();
    let mut office = Office::new();
    office.set_recovery_directory(recovery.clone());
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
        .unwrap();

    // The snapshot after each step: the generation advanced by exactly one,
    // the text is the whole committed document text, and the baseline is the
    // untouched original package — PackageBacking, not just the model.
    let step = |office: &mut Office,
                command: CommandId,
                args: &[(&'static str, ArgValue)],
                expected_generation: u64| {
        office
            .invoke(command, args)
            .unwrap_or_else(|error| panic!("checkpoint after {}: {error:?}", command.0));
        let checkpoint = storage::recover_checkpoint(&recovery).unwrap();
        assert_eq!(
            checkpoint.generation, expected_generation,
            "after {}",
            command.0
        );
        let text = office.active().unwrap().view().index().text();
        assert_eq!(
            checkpoint.recovered.text, text,
            "snapshot text after {}",
            command.0
        );
        assert_eq!(
            checkpoint.recovered.baseline, bytes,
            "snapshot baseline after {}",
            command.0
        );
        assert!(
            checkpoint.version.is_some(),
            "versioned snapshot after {}",
            command.0
        );
        let document = storage::recover_document_checkpoint(&recovery).unwrap();
        assert!(
            document.snapshot.is_some(),
            "complete package after {}",
            command.0
        );
        let restored = Office::recover_from(&recovery).unwrap();
        let live = office
            .active()
            .unwrap()
            .backend()
            .session()
            .session()
            .document();
        let recovered = restored.active().unwrap();
        assert_eq!(
            live.semantic_difference(recovered.backend().session().session().document()),
            None,
            "rich recovery after {}",
            command.0
        );
        assert!(
            recovered.has_unsaved_changes(),
            "recovered document retains save/discard choice"
        );
    };

    step(
        &mut office,
        EDIT_INSERT,
        &[("text", ArgValue::Text("快照甲".into()))],
        1,
    );
    // A non-mutating command publishes no new generation.
    office.invoke(EDIT_SELECT_ALL, &[]).unwrap();
    let checkpoint = storage::recover_checkpoint(&recovery).unwrap();
    assert_eq!(checkpoint.generation, 1, "select-all must not journal");

    step(&mut office, EDIT_BOLD, &[], 2);
    step(&mut office, EDIT_ALIGN_CENTER, &[], 3);
    step(&mut office, PAGE_BREAK, &[], 4);
    step(
        &mut office,
        FIND_REPLACE_ALL,
        &[
            ("query", ArgValue::Text("快照甲".into())),
            ("replacement", ArgValue::Text("快照乙".into())),
        ],
        5,
    );
    step(&mut office, TABLE_INSERT, &[], 6);
    step(&mut office, REVIEW_TRACK, &[], 7);
    step(
        &mut office,
        EDIT_INSERT,
        &[("text", ArgValue::Text("修订字".into()))],
        8,
    );
    step(&mut office, REVIEW_REJECT_ALL, &[], 9);

    // Two slots alternate: after several generations both files exist, so
    // the previous valid snapshot is always retained on disk.
    assert!(recovery.join("recovery-a.jwr").is_file());
    assert!(recovery.join("recovery-b.jwr").is_file());
}
