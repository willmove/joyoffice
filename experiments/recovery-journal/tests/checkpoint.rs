use joyword_recovery_journal::{
    Error,
    checkpoint::{self, Phase},
    journal::*,
    replay::*,
};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
fn setup(dir: &std::path::Path) -> (Recovery, AppendLog<std::fs::File>, std::path::PathBuf) {
    let path = dir.join("中文 source log.jwj");
    let r = Recovery::open(SOURCE, [2; 16]).unwrap();
    let mut log = AppendLog::create(&path, [2; 16], Limits::default()).unwrap();
    log.append(&r.baseline_payload()).unwrap();
    (r, log, path)
}
fn edit(r: &mut Recovery, l: &mut AppendLog<std::fs::File>) {
    r.commit(
        l,
        r.version(),
        vec![Action::Insert {
            at: Point {
                paragraph: 0,
                scalar: 0,
            },
            text: "甲".into(),
        }],
        &AtomicBool::new(false),
    )
    .unwrap();
}
#[test]
fn two_generation_rotation_readback_and_prefix_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let (mut r, mut log, path) = setup(dir.path());
    let copies = dir.path().join("checkpoints");
    for _ in 0..5 {
        edit(&mut r, &mut log);
        let receipt = checkpoint::rotate(&path, &copies, [2; 16], source_hash(SOURCE)).unwrap();
        let (_, restored) = checkpoint::load(&receipt.path, [2; 16], source_hash(SOURCE)).unwrap();
        assert_eq!(restored.texts(), r.texts());
        assert_eq!(restored.version(), r.version());
    }
    assert_eq!(std::fs::read_dir(&copies).unwrap().count(), 2);
    let before = std::fs::read(&path).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"torn tail")
        .unwrap();
    let recovered = dir.path().join("recovered.jwj");
    checkpoint::copy_prefix(&path, &recovered, [2; 16], source_hash(SOURCE)).unwrap();
    assert_eq!(std::fs::read(&recovered).unwrap(), before);
    assert!(checkpoint::copy_prefix(&path, &recovered, [2; 16], source_hash(SOURCE)).is_err());
}
#[test]
fn checkpoint_interruption_never_destroys_prior_chain() {
    for phase in [Phase::Written, Phase::Verified, Phase::Published] {
        let dir = tempfile::tempdir().unwrap();
        let (mut r, mut log, path) = setup(dir.path());
        edit(&mut r, &mut log);
        let copies = dir.path().join("copies");
        let old = checkpoint::rotate(&path, &copies, [2; 16], source_hash(SOURCE)).unwrap();
        let bytes = std::fs::read(&old.path).unwrap();
        edit(&mut r, &mut log);
        let result = checkpoint::rotate_with(&path, &copies, [2; 16], source_hash(SOURCE), |p| {
            if p == phase {
                Err(Error::Invalid("injected interruption"))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&old.path).unwrap(), bytes);
        let (_, restored) = checkpoint::load(&path, [2; 16], source_hash(SOURCE)).unwrap();
        assert_eq!(restored.texts(), r.texts());
        if phase == Phase::Published {
            assert!(matches!(result, Err(Error::Published { .. })));
        }
    }
}
#[test]
fn terminated_child_recovers_last_complete_record_and_preserves_docx() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("中文 original.docx");
    std::fs::write(&source, SOURCE).unwrap();
    let log = dir.path().join("child log.jwj");
    let mut child = Command::new(env!("CARGO_BIN_EXE_joyword-recovery-journal"))
        .arg("crash-worker")
        .arg(&source)
        .arg(&log)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Bounded wait on a separate reader; never leave a stuck worker behind.
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(line);
    });
    let ready = rx.recv_timeout(std::time::Duration::from_secs(10));
    child.kill().unwrap();
    let status = child.wait().unwrap();
    reader.join().unwrap();
    assert!(!status.success());
    assert!(ready.unwrap().starts_with("READY version=2"));
    let bytes = read(&log, Limits::default()).unwrap();
    let scanned = scan(&bytes, Limits::default()).unwrap();
    assert_eq!(scanned.valid_bytes, bytes.len());
    assert_eq!(scanned.records.len(), 3);
    let sid: [u8; 16] = source_hash(SOURCE)[..16].try_into().unwrap();
    let restored = recover(
        &scanned.records,
        sid,
        source_hash(SOURCE),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(restored.texts()[0].starts_with("👩‍💻e\u{301}恢复甲"));
    assert_eq!(restored.version(), 2);
    assert_eq!(std::fs::read(&source).unwrap(), SOURCE);
    let output = Command::new(env!("CARGO_BIN_EXE_joyword-recovery-journal"))
        .arg("recover")
        .arg(&log)
        .arg(&source)
        .arg(dir.path().join("独立恢复.jwj"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("version=2")
    );
}
