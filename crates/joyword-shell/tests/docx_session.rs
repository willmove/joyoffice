//! Exercise the real corpus/importer/engine boundary, including rejection,
//! preservation and the conservative whole-document lock fallback.

use std::path::PathBuf;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{ImportError, ReadOnlyReason};
use joyword_engine::{Command, Origin, Outcome, Transaction, TxError, UndoError};
use joyword_model::{Bias, Block, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;

fn fixture(id: u8) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx"));
    std::fs::read(path).unwrap()
}

fn insertion(session: &DocxSession) -> Transaction {
    let engine = session.session();
    let document = engine.document();
    let paragraph = document
        .body()
        .blocks
        .iter()
        .find_map(Block::as_paragraph)
        .unwrap();
    Transaction::new(
        engine.version(),
        vec![Command::InsertText {
            at: Position {
                story: document.body_id(),
                paragraph: paragraph.id(),
                offset: ScalarOffset(0),
                bias: Bias::After,
            },
            text: "新增文字🙂".into(),
        }],
    )
}

fn with_settings(settings: &str) -> Vec<u8> {
    let bytes = fixture(5);
    let package = Package::read(&bytes, &Budget::default()).unwrap();
    let xml = format!(
        r#"<w:settings xmlns:w="{}">{settings}</w:settings>"#,
        joyword_docx::W
    );
    let mut items = vec![(
        "[Content_Types].xml",
        package.content_types_xml(),
        Method::Deflated,
    )];
    for part in package.parts() {
        let data = if part.name.as_str() == "/word/settings.xml" {
            xml.as_bytes()
        } else {
            &part.data
        };
        items.push((part.name.item_name(), data, Method::Deflated));
    }
    write_archive(&items).unwrap()
}

#[test]
fn each_settings_restriction_independently_prevents_editing() {
    for (settings, reason) in [
        (
            r#"<w:documentProtection w:edit="readOnly" w:enforcement="1"/>"#,
            ReadOnlyReason::DocumentProtection {
                edit: Some("readOnly".into()),
            },
        ),
        ("<w:writeProtection/>", ReadOnlyReason::WriteProtection),
    ] {
        let bytes = with_settings(settings);
        let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
        assert_eq!(
            session
                .editability()
                .read_only_reasons()
                .collect::<Vec<_>>(),
            vec![&reason]
        );
        assert!(session.editability().locked().is_empty());
        let transaction = insertion(&session);
        assert_eq!(session.apply(transaction), Err(TxError::ReadOnly));
        assert_eq!(session.unchanged_package_bytes().unwrap(), bytes);
    }
}

#[test]
fn track_revisions_alone_stays_editable() {
    let bytes = with_settings("<w:trackRevisions/>");
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    assert!(session.editability().is_unrestricted());
    assert_eq!(
        session.session().document().settings().track_revisions,
        Some(true)
    );
    let transaction = insertion(&session);
    assert!(matches!(
        session.apply(transaction),
        Ok(Outcome::Applied(_))
    ));
}

#[test]
fn disabled_restrictions_allow_editing_without_enabling_agent_writes() {
    let bytes = with_settings(
        r#"<w:documentProtection w:edit="readOnly" w:enforcement="0"/><w:trackRevisions w:val="false"/>"#,
    );
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    assert!(session.editability().is_unrestricted());
    assert!(!session.session().policy().agent_writes);
    let transaction = insertion(&session);
    assert!(matches!(
        session.apply(transaction),
        Ok(Outcome::Applied(_))
    ));
    assert!(session.unchanged_package_bytes().is_err());
}

#[test]
fn untouched_corpus_keeps_every_original_byte_and_diagnostic() {
    for id in 1..=5 {
        let bytes = fixture(id);
        let session = DocxSession::open(&bytes, &Budget::default()).unwrap();
        assert_eq!(session.unchanged_package_bytes().unwrap(), bytes);
        assert!(
            session
                .session()
                .document()
                .semantic_difference(
                    &joyword_docx::import(&bytes, &Budget::default())
                        .unwrap()
                        .document
                )
                .is_none()
        );
        if matches!(id, 3..=5) {
            assert!(!session.diagnostics().is_empty());
        }
    }
}

#[test]
fn protected_and_tracked_documents_deny_all_write_origins_and_history() {
    let bytes = fixture(5);
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    assert!(
        session
            .editability()
            .read_only_reasons()
            .any(|reason| matches!(reason, ReadOnlyReason::DocumentProtection { .. }))
    );
    assert_eq!(
        session.session().document().settings().track_revisions,
        Some(true)
    );
    let before = session.session().document().clone();
    let version = session.session().version();
    for origin in [Origin::User, Origin::Agent, Origin::System] {
        let mut transaction = insertion(&session);
        transaction.origin = origin;
        assert_eq!(session.apply(transaction), Err(TxError::ReadOnly));
    }
    assert_eq!(session.undo(), Err(UndoError::ReadOnly));
    assert_eq!(session.redo(), Err(UndoError::ReadOnly));
    assert_eq!(session.session().version(), version);
    assert!(
        session
            .session()
            .document()
            .semantic_difference(&before)
            .is_none()
    );
    assert!(!session.session().can_undo());
    assert!(!session.session().can_redo());
    assert_eq!(session.unchanged_package_bytes().unwrap(), bytes);
}

#[test]
fn any_preserved_node_locks_even_an_otherwise_editable_paragraph() {
    let bytes = fixture(3);
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    assert!(!session.editability().is_read_only());
    assert!(!session.editability().locked().is_empty());
    assert!(session.session().policy().read_only);
    let transaction = insertion(&session);
    let Command::InsertText { at, .. } = transaction.commands[0] else {
        unreachable!()
    };
    assert!(session.editability().can_edit(at.paragraph));
    assert_eq!(session.apply(transaction), Err(TxError::ReadOnly));
    assert!(session.backing().fragments().next().is_some());
    assert_eq!(session.unchanged_package_bytes().unwrap(), bytes);
}

#[test]
fn editable_import_uses_engine_and_never_exports_stale_original_after_undo() {
    let bytes = fixture(1);
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    assert!(session.editability().is_unrestricted());
    assert!(!session.session().policy().read_only);
    let before = session.session().document().clone();
    let transaction = insertion(&session);
    assert!(matches!(
        session.apply(transaction),
        Ok(Outcome::Applied(_))
    ));
    let error = session.unchanged_package_bytes().unwrap_err();
    assert_eq!(error.current, session.session().version());
    assert!(error.current > error.imported);
    session.undo().unwrap();
    assert!(
        session
            .session()
            .document()
            .semantic_difference(&before)
            .is_none()
    );
    assert!(session.unchanged_package_bytes().is_err());
    session.redo().unwrap();
    assert!(session.unchanged_package_bytes().is_err());
    assert_eq!(session.backing().original_bytes(), bytes);
}

#[test]
fn failed_and_noop_transactions_keep_original_copy_available() {
    let bytes = fixture(1);
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    let mut transaction = insertion(&session);
    transaction.origin = Origin::Agent;
    assert_eq!(
        session.apply(transaction),
        Err(TxError::AgentWritesDisabled)
    );
    let transaction = Transaction::new(session.session().version(), vec![]);
    assert!(matches!(session.apply(transaction), Ok(Outcome::NoOp(_))));
    assert_eq!(session.unchanged_package_bytes().unwrap(), bytes);
    assert!(!session.session().can_undo());
}

#[test]
fn refused_and_over_budget_imports_do_not_create_sessions() {
    assert!(matches!(
        DocxSession::open(b"invalid ZIP", &Budget::default()),
        Err(ImportError::Package(_))
    ));
    let bytes = fixture(1);
    let budget = Budget {
        max_entries: 0,
        ..Budget::default()
    };
    assert!(matches!(
        DocxSession::open(&bytes, &budget),
        Err(ImportError::Package(_))
    ));
}
