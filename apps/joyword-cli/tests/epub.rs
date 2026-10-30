use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
fn source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/epub/book.docx")
}
fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_joyword-cli"));
    command.arg("epub").arg(source());
    command
}
#[test]
fn epub_plan_then_explicit_loss_acceptance_export_is_isolated_and_repeatable() {
    let dir = tempfile::tempdir().unwrap();
    let input = std::fs::read(source()).unwrap();
    let plan = cli().arg("--plan").output().unwrap();
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let plan: Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(plan["writes"], 0);
    assert_eq!(plan["images"], 1);
    assert_eq!(plan["paragraphs"], 8);
    assert!(!plan["losses"].as_array().unwrap().is_empty());
    let first = dir.path().join("first.epub");
    let refused = cli().arg("--output").arg(&first).output().unwrap();
    assert_eq!(refused.status.code(), Some(2));
    assert!(!first.exists());
    for name in ["first.epub", "second.epub"] {
        let result = cli()
            .arg("--output")
            .arg(dir.path().join(name))
            .args([
                "--accept-losses",
                "--language",
                "zh-CN",
                "--modified",
                "2026-10-30T00:00:00Z",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(result["writes"], 1);
        assert_eq!(result["source_overwritten"], false);
    }
    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(dir.path().join("second.epub")).unwrap()
    );
    assert_eq!(std::fs::read(source()).unwrap(), input);
    let existing = std::fs::read(&first).unwrap();
    let result = cli()
        .arg("--output")
        .arg(&first)
        .arg("--accept-losses")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(std::fs::read(first).unwrap(), existing);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}
#[test]
fn malformed_epub_options_and_dates_produce_no_output() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["--plan", "--plan"],
        vec!["--plan", "--accept-losses"],
        vec!["--plan", "--modified", "2026-02-30T00:00:00Z"],
        vec!["--plan", "--language", "1"],
        vec!["--output"],
    ] {
        assert_eq!(cli().args(args).output().unwrap().status.code(), Some(2));
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
