//! Open/save generation checks and recovery after a killed process.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use joyword_shell::command::{
    ArgValue, DOCUMENT_SWITCH, EDIT_INSERT, FILE_CLOSE, FILE_OPEN, FILE_OPEN_RECENT,
};
use joyword_shell::host::Office;
use joyword_shell::storage::package_is_encrypted;

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

#[test]
fn input_during_open_is_kept_and_a_stale_save_stays_dirty() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("open.docx");
    fs::write(&path, &bytes).unwrap();
    let mut office = Office::new();
    let generation = office.begin_open();
    assert!(office.is_opening());
    office.type_during_open("增补").unwrap();
    office.accept_open(generation, &path).unwrap();
    let text = office.active().unwrap().view().index().text();
    assert!(text.contains("增补"), "{text}");
    let id = office.active_id().unwrap();
    assert!(office.is_dirty(id));
    let destination = dir.path().join("during.docx");
    let ticket = office.start_save(&destination).unwrap();
    office
        .invoke(
            joyword_shell::command::EDIT_INSERT,
            &[("text", ArgValue::Text("又".into()))],
        )
        .unwrap();
    assert!(!office.finish_save(ticket).unwrap());
    assert!(office.is_dirty(id));
    assert!(!destination.exists());
    let saved = dir.path().join("saved.docx");
    let current = office.start_save(&saved).unwrap();
    assert!(office.finish_save(current).unwrap());
    assert!(!office.is_dirty(id));
    assert!(saved.is_file());
    assert_ne!(fs::read(&saved).unwrap(), bytes);
    let stale = office.begin_open();
    office.type_during_open("迟到").unwrap();
    let error = office.accept_open(stale + 1, &path).unwrap_err();
    assert!(error.to_string().contains("stale"));
    let kept = office.active().unwrap().view().index().text();
    assert!(kept.contains("迟到"), "{kept}");
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
}

#[test]
fn killed_child_restores_the_commit_and_encrypted_bytes_are_not_copied() {
    if std::env::var_os("JOYWORD_RECOVERY_CHILD").is_some() {
        let directory = PathBuf::from(std::env::var("JOYWORD_RECOVERY_DIR").unwrap());
        let source = directory.join("source.docx");
        fs::write(&source, fixture()).unwrap();
        let mut office = Office::new();
        office.set_recovery_directory(directory);
        office
            .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
            .unwrap();
        office
            .invoke(EDIT_INSERT, &[("text", ArgValue::Text("已提交".into()))])
            .unwrap();
        std::process::abort();
    }
    let dir = tempfile::tempdir().unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut child = if exe
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("qemu"))
    {
        // QEMU user mode without the binfmt fix-binary flag reports the
        // emulator as /proc/self/exe, and the guest test binary cannot exec
        // directly. Respawn through the runner cargo used; argv[0] still
        // carries the guest binary path.
        let runner_env = if cfg!(target_arch = "aarch64") {
            "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER"
        } else if cfg!(target_arch = "loongarch64") {
            "CARGO_TARGET_LOONGARCH64_UNKNOWN_LINUX_GNU_RUNNER"
        } else {
            panic!("unexpected QEMU runner on this architecture")
        };
        let runner = std::env::var(runner_env).expect("QEMU runner env var from CI");
        let mut words = runner.split_whitespace();
        let program = words.next().expect("runner program");
        let mut command = Command::new(program);
        command.args(words).arg(std::env::args().next().unwrap());
        command
    } else {
        Command::new(exe)
    };
    child
        .env("JOYWORD_RECOVERY_CHILD", "1")
        .env("JOYWORD_RECOVERY_DIR", dir.path())
        .arg("killed_child_restores_the_commit_and_encrypted_bytes_are_not_copied");
    let output = child.output().unwrap();
    assert!(
        !output.status.success(),
        "child unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let restored = Office::recover_from(dir.path()).unwrap_or_else(|error| {
        panic!(
            "recover failed: {error}; child status: {:?}; child stdout: {}; child stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let text = restored.active().unwrap().view().index().text();
    assert!(text.contains("已提交"), "{text}");
    let baseline = restored
        .active()
        .unwrap()
        .backend()
        .session()
        .backing()
        .original_bytes();
    assert_eq!(baseline, fixture());

    let mut encrypted = fixture();
    for signature in [b"PK\x03\x04".as_slice(), b"PK\x01\x02".as_slice()] {
        let mut start = 0;
        while let Some(found) = encrypted[start..]
            .windows(signature.len())
            .position(|window| window == signature)
        {
            let index = start + found;
            let flags = if signature == b"PK\x03\x04" {
                index + 6
            } else {
                index + 8
            };
            if flags < encrypted.len() {
                encrypted[flags] |= 0x01;
            }
            start = index + 4;
            if start >= encrypted.len() {
                break;
            }
        }
    }
    assert!(package_is_encrypted(&encrypted));
    let secret = dir.path().join("encrypted");
    fs::create_dir_all(&secret).unwrap();
    let mut office = Office::new();
    office.set_recovery_directory(secret.clone());
    let error = office.journal_bytes(&encrypted, "明文").unwrap_err();
    assert!(error.to_string().contains("encrypted"));
    assert!(!secret.join("recovery.jwr").exists());
    assert!(fs::read_dir(&secret).unwrap().next().is_none());
}

#[test]
fn recent_open_and_document_switch_use_the_command_registry() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.docx");
    let second = dir.path().join("second.docx");
    fs::write(&first, fixture()).unwrap();
    fs::write(&second, fixture()).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(first.clone()))])
        .unwrap();
    let first_id = office.active_id().unwrap();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(second.clone()))])
        .unwrap();
    let second_id = office.active_id().unwrap();
    assert_ne!(first_id, second_id);
    office
        .invoke(DOCUMENT_SWITCH, &[("id", ArgValue::Id(first_id.0))])
        .unwrap();
    assert_eq!(office.active_id(), Some(first_id));
    office
        .invoke(
            FILE_OPEN_RECENT,
            &[("path", ArgValue::Path(second.clone()))],
        )
        .unwrap();
    assert_eq!(office.active_id(), Some(second_id));
    let missing = dir.path().join("missing.docx");
    let error = office
        .invoke(FILE_OPEN_RECENT, &[("path", ArgValue::Path(missing))])
        .unwrap_err();
    assert!(format!("{error:?}").contains("recent"), "{error:?}");
}
