//! Complete checkpoint recovery: preserve rich state and the formal baseline.
use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_shell::command::{
    ArgValue, EDIT_BOLD, EDIT_INSERT, EDIT_SELECT_ALL, FILE_OPEN, FILE_SAVE_COPY, PAGE_EDIT_HEADER,
    REVIEW_TRACK, TABLE_INSERT,
};
use joyword_shell::{host::Office, storage};
use std::fs;

fn baseline(header: bool) -> Vec<u8> {
    let original = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
    let package = Package::read(original, &Budget::default()).unwrap();
    let mut types = String::from_utf8(package.content_types_xml().to_vec()).unwrap();
    types = types.replace(
        "</Types>",
        "<Default Extension=\"bin\" ContentType=\"application/octet-stream\"/></Types>",
    );
    let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:r><w:t>Body contract</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:body></w:document>"#;
    let header_xml = r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>Header baseline</w:t></w:r></w:p></w:hdr>"#;
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#;
    if header {
        types = types.replace("</Types>", "<Override PartName=\"/word/header1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml\"/></Types>");
    }
    let mut parts = vec![("[Content_Types].xml", types.as_bytes(), Method::Deflated)];
    for part in package.parts() {
        if part.name.as_str() == "/word/_rels/document.xml.rels" && header {
            continue;
        }
        let bytes = if part.name.as_str() == "/word/document.xml" && header {
            document.as_bytes()
        } else {
            &part.data
        };
        parts.push((part.name.item_name(), bytes, part.method));
    }
    if header {
        parts.push(("word/header1.xml", header_xml.as_bytes(), Method::Deflated));
        parts.push((
            "word/_rels/document.xml.rels",
            rels.as_bytes(),
            Method::Deflated,
        ));
    }
    parts.push((
        "custom/audit.bin",
        b"unknown bytes must remain unchanged\x00\xff",
        Method::Stored,
    ));
    write_archive(&parts).unwrap()
}

fn open(source: &std::path::Path, recovery: &std::path::Path) -> Office {
    let mut office = Office::new();
    office.set_recovery_directory(recovery.to_owned());
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source.to_owned()))])
        .unwrap();
    office
}

#[test]
fn rich_recovery_keeps_formatting_tables_unknown_parts_and_original_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("formal.docx");
    let bytes = baseline(false);
    fs::write(&source, &bytes).unwrap();
    let journal = directory.path().join("journal");
    let mut office = open(&source, &journal);
    office
        .invoke(
            EDIT_INSERT,
            &[("text", ArgValue::Text("Committed ".into()))],
        )
        .unwrap();
    office.invoke(EDIT_SELECT_ALL, &[]).unwrap();
    office.invoke(EDIT_BOLD, &[]).unwrap();
    office.invoke(TABLE_INSERT, &[]).unwrap();
    let checkpoint = storage::recover_document_checkpoint(&journal).unwrap();
    assert_eq!(checkpoint.checkpoint.recovered.baseline, bytes);
    assert!(!checkpoint.degraded);
    assert!(
        fs::read(journal.join("recovery-a.jwr"))
            .unwrap()
            .starts_with(b"JWREC003")
    );
    let restored = Office::recover_from(&journal).unwrap();
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
        None
    );
    assert!(recovered.has_unsaved_changes());
    assert!(
        !recovered.can_undo(),
        "undo history is session-local, not invented by replay"
    );
    let plan = recovered.backend().session().export_plan().unwrap();
    assert_eq!(Some(plan.bytes()), checkpoint.snapshot.as_deref());
    let saved = Package::read(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(
        &*saved.part("/custom/audit.bin").unwrap().data,
        b"unknown bytes must remain unchanged\x00\xff"
    );
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn a_header_edit_checkpoints_body_text_and_restores_every_story() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("header.docx");
    let bytes = baseline(true);
    fs::write(&source, &bytes).unwrap();
    let journal = directory.path().join("journal");
    let mut office = open(&source, &journal);
    office.invoke(PAGE_EDIT_HEADER, &[]).unwrap();
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("Edited ".into()))])
        .unwrap();
    assert!(
        office
            .active()
            .unwrap()
            .view()
            .index()
            .text()
            .starts_with("Edited Header")
    );
    let checkpoint = storage::recover_document_checkpoint(&journal).unwrap();
    assert_eq!(checkpoint.checkpoint.recovered.text, "Body contract");
    let restored = Office::recover_from(&journal).unwrap();
    assert_eq!(
        office
            .active()
            .unwrap()
            .backend()
            .session()
            .session()
            .document()
            .semantic_difference(
                restored
                    .active()
                    .unwrap()
                    .backend()
                    .session()
                    .session()
                    .document()
            ),
        None
    );
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn unsupported_export_cannot_publish_a_misleading_text_only_checkpoint() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("no-settings.docx");
    let bytes = baseline(false);
    fs::write(&source, &bytes).unwrap();
    let journal = directory.path().join("journal");
    let mut office = open(&source, &journal);
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("safe ".into()))])
        .unwrap();
    let valid = storage::recover_document_checkpoint(&journal).unwrap();
    assert!(
        office.invoke(REVIEW_TRACK, &[]).is_err(),
        "missing settings part is a T103 export restriction"
    );
    assert_eq!(
        storage::recover_document_checkpoint(&journal).unwrap(),
        valid
    );
    assert!(office.active().unwrap().has_unsaved_changes());
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn mismatch_is_rejected_before_creating_any_recovered_document() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = baseline(false);
    storage::commit_document_recovery_versioned(
        directory.path(),
        1,
        &bytes,
        "unrelated body",
        &bytes,
    )
    .unwrap();
    assert!(Office::recover_from(directory.path()).is_err());
    assert!(fs::read_dir(directory.path()).unwrap().all(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_none_or(|extension| extension != "docx")
    }));
}

#[test]
fn v2_migration_v3_fallback_and_discard_preserve_published_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = baseline(false);
    storage::commit_recovery(directory.path(), &bytes, "legacy text").unwrap();
    assert!(
        storage::recover_document_checkpoint(directory.path())
            .unwrap()
            .snapshot
            .is_none()
    );
    let second =
        storage::commit_document_recovery_versioned(directory.path(), 2, &bytes, "body", &bytes)
            .unwrap();
    let second_bytes = fs::read(&second.path).unwrap();
    let third =
        storage::commit_document_recovery_versioned(directory.path(), 3, &bytes, "body", &bytes)
            .unwrap();
    fs::write(&third.path, b"truncated generation three").unwrap();
    let recovered = storage::recover_document_checkpoint(directory.path()).unwrap();
    assert!(recovered.degraded);
    assert_eq!(recovered.checkpoint.generation, 2);
    assert_eq!(recovered.snapshot.as_deref(), Some(bytes.as_slice()));
    assert!(
        storage::commit_document_recovery_versioned(directory.path(), 4, &bytes, "body", &bytes)
            .is_err()
    );
    assert_eq!(fs::read(second.path).unwrap(), second_bytes);
    assert_eq!(fs::read(third.path).unwrap(), b"truncated generation three");
    // A healthy journal's tombstone prevents rich checkpoints from resurfacing.
    let healthy = directory.path().join("healthy");
    storage::commit_document_recovery_versioned(&healthy, 1, &bytes, "body", &bytes).unwrap();
    storage::clear_recovery(&healthy).unwrap();
    assert!(storage::recover_document_checkpoint(&healthy).is_err());
}

#[test]
fn encrypted_envelopes_or_snapshots_never_create_plaintext_files() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = baseline(false);
    let envelope = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1encrypted envelope";
    for (name, original, snapshot) in [
        ("baseline", envelope.as_slice(), bytes.as_slice()),
        ("snapshot", bytes.as_slice(), envelope.as_slice()),
    ] {
        let target = directory.path().join(name);
        assert!(
            storage::commit_document_recovery_versioned(&target, 1, original, "secret", snapshot)
                .is_err()
        );
        assert!(!target.exists());
    }
}

#[test]
fn saving_a_recovered_copy_does_not_clear_its_original_choice() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = baseline(false);
    // Recovery never consumes slots or treats Save As as an original save.
    let source = directory.path().join("formal.docx");
    fs::write(&source, &bytes).unwrap();
    let journal = directory.path().join("actual");
    let mut original = open(&source, &journal);
    original
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("unsaved ".into()))])
        .unwrap();
    let mut restored = Office::recover_from(&journal).unwrap();
    restored
        .invoke(
            FILE_SAVE_COPY,
            &[(
                "destination",
                ArgValue::Path(directory.path().join("copy.docx")),
            )],
        )
        .unwrap();
    assert!(restored.active().unwrap().has_unsaved_changes());
    assert!(storage::recover_document_checkpoint(&journal).is_ok());
    assert_eq!(fs::read(source).unwrap(), bytes);
}
