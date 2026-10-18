#![cfg(windows)]

use std::fs;
use std::path::{Path, PathBuf};

use joyoffice_opc::Budget;
use joyword_docx::ExportPlan;
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::storage::{OriginalChoice, open_source, save_original_with_choice};

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

fn edited_plan(bytes: &[u8], text: &str) -> ExportPlan {
    let mut session = DocxSession::open(bytes, &Budget::default()).unwrap();
    let document = session.session().document();
    let at = Position {
        story: document.body_id(),
        paragraph: document.body().blocks.get(0).unwrap().id(),
        offset: ScalarOffset(0),
        bias: Bias::After,
    };
    session
        .apply(Transaction::new(
            session.session().version(),
            vec![Command::InsertText {
                at,
                text: text.into(),
            }],
        ))
        .unwrap();
    session.export_plan().unwrap()
}

fn zone(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}:Zone.Identifier", path.display()))
}

#[test]
fn confirmed_replacement_preserves_metadata_and_supports_a_second_save() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, &bytes).unwrap();
    let zone = zone(&path);
    let zone_bytes = b"[ZoneTransfer]\r\nZoneId=3\r\n";
    fs::write(&zone, zone_bytes).unwrap();
    let created = fs::metadata(&path).unwrap().created().unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let plan = edited_plan(&bytes, "第一次");

    let receipt = save_original_with_choice(&plan, &stamp, None, OriginalChoice::Confirm).unwrap();
    assert_eq!(fs::read(&path).unwrap(), plan.bytes());
    assert_eq!(fs::read(&zone).unwrap(), zone_bytes);
    assert_eq!(fs::metadata(&path).unwrap().created().unwrap(), created);
    assert_eq!(receipt.bytes(), plan.bytes().len() as u64);
    joyword_shell::storage::check_source(receipt.stamp()).unwrap();
    assert!(joyword_shell::storage::check_source(&stamp).is_err());
    let reopened = DocxSession::open(&fs::read(&path).unwrap(), &Budget::default()).unwrap();
    assert!(
        reopened
            .session()
            .document()
            .body()
            .blocks
            .iter()
            .any(|block| {
                block.as_paragraph().is_some_and(|paragraph| {
                    paragraph.plain_text(|_| String::new()).contains("第一次")
                })
            })
    );

    let second = edited_plan(plan.bytes(), "第二次");
    let second_receipt =
        save_original_with_choice(&second, receipt.stamp(), None, OriginalChoice::Confirm).unwrap();
    assert_eq!(fs::read(&path).unwrap(), second.bytes());
    assert_eq!(fs::read(&zone).unwrap(), zone_bytes);
    assert_eq!(fs::metadata(&path).unwrap().created().unwrap(), created);
    joyword_shell::storage::check_source(second_receipt.stamp()).unwrap();
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}
