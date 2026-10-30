use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn source(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mail-merge")
        .join(name)
}
fn cli() -> Command {
    // Cargo's runner wraps the test harness, not processes spawned by it.
    // Mirror the existing CI QEMU runner for the real CLI process tests.
    let key = format!(
        "CARGO_TARGET_{}_UNKNOWN_LINUX_GNU_RUNNER",
        std::env::consts::ARCH.to_uppercase()
    );
    let runner = std::env::var(key).ok();
    let mut parts = runner.as_deref().unwrap_or("").split_whitespace();
    if let Some(program) = parts.next() {
        let mut process = Command::new(program);
        process.args(parts).arg(env!("CARGO_BIN_EXE_joyword-cli"));
        process
    } else {
        Command::new(env!("CARGO_BIN_EXE_joyword-cli"))
    }
}

#[test]
fn local_preview_and_batch_preserve_template_and_generate_three_verified_letters() {
    let output = tempfile::tempdir().unwrap();
    let template = source("template.docx");
    let data = source("recipients.csv");
    let before_template = std::fs::read(&template).unwrap();
    let before_data = std::fs::read(&data).unwrap();
    let preview = cli()
        .arg("merge")
        .arg("--template")
        .arg(&template)
        .arg("--data")
        .arg(&data)
        .args(["--preview-row", "3"])
        .output()
        .unwrap();
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let preview: Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["writes"], 0);
    assert!(preview["text"].as_str().unwrap().contains("南山\n研究组"));
    assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 0);
    let result = cli()
        .arg("merge")
        .arg("--template")
        .arg(&template)
        .arg("--data")
        .arg(&data)
        .arg("--output-parent")
        .arg(output.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(result["records"], 3);
    for (i, file) in result["files"].as_array().unwrap().iter().enumerate() {
        let readback = cli()
            .arg("inspect")
            .arg(file.as_str().unwrap())
            .output()
            .unwrap();
        assert!(readback.status.success());
        let readback: Value = serde_json::from_slice(&readback.stdout).unwrap();
        assert!(
            readback["text"]
                .as_str()
                .unwrap()
                .contains(["张三", "李四", "王五"][i])
        );
        assert!(!readback["text"].as_str().unwrap().contains("{{"));
    }
    assert_eq!(std::fs::read(template).unwrap(), before_template);
    assert_eq!(std::fs::read(data).unwrap(), before_data);
}

#[test]
fn invalid_options_encoding_columns_and_paths_create_no_output() {
    let output = tempfile::tempdir().unwrap();
    for extra in [
        vec!["--preview-row", "0"],
        vec!["--preview-row", "1", "--output-parent", "."],
        vec!["--map", "bad", "--preview-row", "1"],
        vec![
            "--map",
            "姓名=不存在",
            "--output-parent",
            output.path().to_str().unwrap(),
        ],
    ] {
        let result = cli()
            .arg("merge")
            .arg("--template")
            .arg(source("template.docx"))
            .arg("--data")
            .arg(source("recipients.csv"))
            .args(extra)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
    }
    assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 0);
    assert!(cli().args(["merge", "--help"]).status().unwrap().success());
}
