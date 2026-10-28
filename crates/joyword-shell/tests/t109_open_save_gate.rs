//! T109: a slow open stays read-only until it is accepted, and close/quit
//! while a save is in flight still asks about edits made after that save started.
//! The pause is a channel gate, not a sleep.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use joyoffice_opc::Budget;
use joyword_shell::command::{
    ArgValue, Availability, CommandHost, CommandId, EDIT_BOLD, EDIT_INSERT, FILE_CLOSE, FILE_OPEN,
    FILE_OPEN_CANCEL, InvokeError, PAGE_BREAK,
};
use joyword_shell::docx::{BoundDocx, DocxSession};
use joyword_shell::host::{ClosePrompt, Office, close_prompt};
use joyword_shell::pause::StepGate;
use joyword_shell::storage::OriginalChoice;

const ENTERED: Duration = Duration::from_secs(30);

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

fn plain(bytes: &[u8]) -> String {
    DocxSession::open(bytes, &Budget::default())
        .unwrap()
        .session()
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn body(office: &Office) -> String {
    office.active().unwrap().view().index().text().to_owned()
}

fn version_of(office: &Office) -> u64 {
    office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .version()
        .get()
}

fn saved_of(office: &Office) -> u64 {
    office.active().unwrap().saved_version().get()
}

fn disabled(office: &Office, id: CommandId) -> bool {
    matches!(
        CommandHost::availability(office, id),
        Availability::Disabled("input.read_only")
    )
}

#[test]
fn slow_open_stays_read_only_until_accept_and_keeps_keystrokes() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.docx");
    let second = dir.path().join("second.docx");
    fs::write(&first, &bytes).unwrap();
    fs::write(&second, &bytes).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(first.clone()))])
        .unwrap();
    let first_id = office.active_id().unwrap();
    let first_text = body(&office);
    let generation = office.begin_open();
    let (controller, gate) = StepGate::pair();
    let opening = second.clone();
    let worker = std::thread::spawn(move || BoundDocx::open_default_paused(&opening, Some(&gate)));
    assert!(controller.wait_entered(ENTERED));
    assert!(office.is_opening());
    assert!(disabled(&office, EDIT_BOLD));
    assert!(disabled(&office, PAGE_BREAK));
    assert!(matches!(
        CommandHost::availability(&office, EDIT_INSERT),
        Availability::Enabled
    ));
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("键".into()))])
        .unwrap();
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("入".into()))])
        .unwrap();
    let blocked = office.invoke(EDIT_BOLD, &[]).unwrap_err();
    assert_eq!(blocked, InvokeError::Disabled("input.read_only"));
    assert_eq!(body(&office), first_text);
    assert_eq!(office.buffered_open_text(), Some("键入"));
    assert_eq!(fs::read(&first).unwrap(), bytes);
    assert_eq!(fs::read(&second).unwrap(), bytes);
    drop(controller);
    let bound = worker.join().unwrap().unwrap();
    office.accept_bound(generation, bound).unwrap();
    assert!(!office.is_opening());
    assert_ne!(office.active_id(), Some(first_id));
    let opened = body(&office);
    assert!(opened.contains("键入"), "{opened}");
    office
        .invoke(
            joyword_shell::command::DOCUMENT_SWITCH,
            &[("id", ArgValue::Id(first_id.0))],
        )
        .unwrap();
    assert_eq!(body(&office), first_text);
    assert!(matches!(
        CommandHost::availability(&office, EDIT_BOLD),
        Availability::Enabled
    ));
}

#[test]
fn a_stale_or_failed_open_keeps_keystrokes_on_the_current_document() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.docx");
    let second = dir.path().join("second.docx");
    fs::write(&first, fixture()).unwrap();
    fs::write(&second, fixture()).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(first))])
        .unwrap();
    let generation = office.begin_open();
    office.type_during_open("迟到").unwrap();
    let bound = BoundDocx::open_default(&second).unwrap();
    let error = office.accept_bound(generation + 1, bound).unwrap_err();
    assert!(error.to_string().contains("stale"));
    let kept = body(&office);
    assert!(kept.contains("迟到"), "{kept}");
    assert!(office.buffered_open_text().is_none());

    let again = office.begin_open();
    office.type_during_open("失败也留").unwrap();
    let buffered = office.finish_open(again).unwrap();
    assert_eq!(buffered, "失败也留");
    office.keep_unopened_keystrokes(&buffered).unwrap();
    let kept = body(&office);
    assert!(kept.contains("失败也留"), "{kept}");
    assert!(kept.contains("迟到"), "{kept}");
}

#[test]
fn close_and_quit_while_a_copy_save_is_in_flight_prompt_for_later_edits() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, &bytes).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(path.clone()))])
        .unwrap();
    let id = office.active_id().unwrap();
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("早".into()))])
        .unwrap();
    let saved = saved_of(&office);
    let job = office.active().unwrap().backend().save_copy_job();
    let saving = job.version().get();
    assert!(saving > saved);
    assert_eq!(version_of(&office), saving);
    // The in-flight save covers the current version, so close/quit still ask,
    // but not about edits that do not exist yet.
    assert_eq!(
        close_prompt(saved, saving, Some(saving)),
        ClosePrompt::Unsaved { version: saving }
    );
    let destination = dir.path().join("copy.docx");
    let (controller, gate) = StepGate::pair();
    let writing = destination.clone();
    let worker = std::thread::spawn(move || job.run_paused(&writing, Some(&gate)));
    assert!(controller.wait_entered(ENTERED));
    assert!(!destination.exists());
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("晚".into()))])
        .unwrap();
    let current = version_of(&office);
    assert!(current > saving);
    let prompt = close_prompt(saved_of(&office), current, Some(saving));
    assert_eq!(prompt, ClosePrompt::DirtyAfterSaveStart { saving, current });
    // Close and quit share `close_prompt`. Neither may proceed on this result.
    assert_ne!(prompt, ClosePrompt::Allow);
    let pending = office
        .invoke(FILE_CLOSE, &[("id", ArgValue::Id(id.0))])
        .unwrap_err();
    assert_eq!(
        pending,
        InvokeError::PendingInput {
            missing: vec!["choice"]
        }
    );
    office
        .invoke(
            FILE_CLOSE,
            &[
                ("id", ArgValue::Id(id.0)),
                ("choice", ArgValue::Text("cancel".into())),
            ],
        )
        .unwrap();
    assert_eq!(office.active_id(), Some(id));
    assert!(body(&office).contains("晚"));
    assert!(!destination.exists());
    drop(controller);
    let receipt = worker.join().unwrap().unwrap();
    let written = plain(&fs::read(receipt.path()).unwrap());
    assert!(written.contains("早"), "{written}");
    assert!(!written.contains("晚"), "{written}");
    assert!(office.is_dirty(id));
    assert_eq!(
        close_prompt(saved_of(&office), version_of(&office), None),
        ClosePrompt::Unsaved {
            version: version_of(&office)
        }
    );
    let still = office
        .invoke(FILE_CLOSE, &[("id", ArgValue::Id(id.0))])
        .unwrap_err();
    assert!(matches!(still, InvokeError::PendingInput { .. }));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn an_original_save_in_flight_prompts_for_later_edits_and_routes_to_its_document() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let first_path = dir.path().join("first.docx");
    let second_path = dir.path().join("second.docx");
    fs::write(&first_path, &bytes).unwrap();
    fs::write(&second_path, &bytes).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(first_path.clone()))])
        .unwrap();
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("原件".into()))])
        .unwrap();
    let first_id = office.active_id().unwrap();
    let saved = saved_of(&office);
    let job = office.active().unwrap().save_original_job();
    let saving = job.version().get();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(second_path.clone()))])
        .unwrap();
    let second_id = office.active_id().unwrap();
    assert_ne!(first_id, second_id);
    let (controller, gate) = StepGate::pair();
    let worker = std::thread::spawn(move || job.run_paused(OriginalChoice::Confirm, Some(&gate)));
    assert!(controller.wait_entered(ENTERED));
    office
        .invoke(
            joyword_shell::command::DOCUMENT_SWITCH,
            &[("id", ArgValue::Id(first_id.0))],
        )
        .unwrap();
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("随后".into()))])
        .unwrap();
    let current = version_of(&office);
    assert_eq!(
        close_prompt(saved, current, Some(saving)),
        ClosePrompt::DirtyAfterSaveStart { saving, current }
    );
    let pending = office
        .invoke(FILE_CLOSE, &[("id", ArgValue::Id(first_id.0))])
        .unwrap_err();
    assert!(matches!(pending, InvokeError::PendingInput { .. }));
    office
        .invoke(
            FILE_CLOSE,
            &[
                ("id", ArgValue::Id(first_id.0)),
                ("choice", ArgValue::Text("cancel".into())),
            ],
        )
        .unwrap();
    assert_eq!(office.active_id(), Some(first_id));
    drop(controller);
    let output = worker.join().unwrap();
    let receipt = office
        .apply_original_save(first_id, output)
        .expect("first document still exists")
        .unwrap();
    assert_eq!(receipt.version().get(), saving);
    assert_eq!(fs::read(&second_path).unwrap(), bytes);
    let on_first = plain(&fs::read(&first_path).unwrap());
    assert!(on_first.contains("原件"), "{on_first}");
    assert!(!on_first.contains("随后"), "{on_first}");
    assert!(office.is_dirty(first_id));
    assert!(!office.is_dirty(second_id));
    office
        .invoke(
            joyword_shell::command::DOCUMENT_SWITCH,
            &[("id", ArgValue::Id(second_id.0))],
        )
        .unwrap();
    assert_eq!(fs::read(&second_path).unwrap(), bytes);
    assert!(!body(&office).contains("原件"));
    assert!(!body(&office).contains("随后"));
}

#[test]
fn cancelling_a_slow_open_discards_it_and_keeps_keystrokes_on_the_current_document() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.docx");
    let second = dir.path().join("second.docx");
    fs::write(&first, &bytes).unwrap();
    fs::write(&second, &bytes).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(first.clone()))])
        .unwrap();
    let first_id = office.active_id().unwrap();
    let first_text = body(&office);
    let generation = office.begin_open();
    let (controller, gate) = StepGate::pair();
    let opening = second.clone();
    let worker = std::thread::spawn(move || BoundDocx::open_default_paused(&opening, Some(&gate)));
    assert!(controller.wait_entered(ENTERED));
    // The cancel entry exists only while the open is in flight.
    assert!(matches!(
        CommandHost::availability(&office, FILE_OPEN_CANCEL),
        Availability::Enabled
    ));
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("取消留".into()))])
        .unwrap();
    assert_eq!(office.buffered_open_text(), Some("取消留"));

    office.invoke(FILE_OPEN_CANCEL, &[]).unwrap();
    assert!(!office.is_opening());
    assert_eq!(office.active_id(), Some(first_id));
    assert!(office.buffered_open_text().is_none());
    let kept = body(&office);
    assert!(kept.contains("取消留"), "{kept}");
    assert!(kept.contains(&first_text), "{kept}");
    assert_eq!(fs::read(&first).unwrap(), bytes);
    assert_eq!(fs::read(&second).unwrap(), bytes);

    // The document that was not ready yet cannot land anymore.
    drop(controller);
    let bound = worker.join().unwrap().unwrap();
    let error = office.accept_bound(generation, bound).unwrap_err();
    assert!(error.to_string().contains("cancelled"), "{error}");
    assert_eq!(office.active_id(), Some(first_id));
    assert_eq!(body(&office), kept);

    // Opening again still works and keeps its own keystrokes.
    let again = office.begin_open();
    office.type_during_open("再开").unwrap();
    let bound = BoundDocx::open_default(&second).unwrap();
    office.accept_bound(again, bound).unwrap();
    assert!(!office.is_opening());
    assert_ne!(office.active_id(), Some(first_id));
    let opened = body(&office);
    assert!(opened.contains("再开"), "{opened}");

    // With nothing in flight the cancel entry reports why it is unavailable.
    assert!(matches!(
        CommandHost::availability(&office, FILE_OPEN_CANCEL),
        Availability::Disabled("file.open.not_opening")
    ));
    // Availability gates the invoke before the host branch is reached.
    assert!(matches!(
        office.invoke(FILE_OPEN_CANCEL, &[]),
        Err(InvokeError::Disabled("file.open.not_opening"))
    ));
}

#[test]
fn cancelling_with_no_current_document_reports_instead_of_dropping_keystrokes() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.docx");
    fs::write(&source, fixture()).unwrap();
    let mut office = Office::new();
    assert!(office.active_id().is_none());
    let generation = office.begin_open();
    office.type_during_open("无处安放").unwrap();
    // Cancelling reports the missing destination instead of silently
    // discarding the buffered keystrokes.
    assert!(matches!(
        office.invoke(FILE_OPEN_CANCEL, &[]),
        Err(InvokeError::Failed(message)) if message.contains("no document")
    ));
    assert!(!office.is_opening());
    let _ = generation;
}
