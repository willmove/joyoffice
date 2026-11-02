use serde_json::Value;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_joyword-cli"))
}

#[test]
fn plans_and_unaccepted_losses_write_nothing_and_publication_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    // Explicit --from controls import; same-path output has a matching suffix
    // so its refusal exercises no-clobber publication, not argument validation.
    let source = dir.path().join("源文件.html");
    let target = dir.path().join("转换.html");
    let content = "中文😀 <script>&literal";
    std::fs::write(&source, content).unwrap();
    let run = |args: &[&str]| {
        cli()
            .arg("exchange")
            .arg(&source)
            .args(["--from", "txt", "--to", "html"])
            .args(args)
            .output()
            .unwrap()
    };
    let preview = run(&["--plan"]);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let report: Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(report["writes"], 0);
    assert!(!report["losses"].as_array().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let output = target.to_str().unwrap();
    let rejected = run(&["--output", output]);
    assert_eq!(rejected.status.code(), Some(2));
    assert!(!target.exists());
    let exported = run(&["--output", output, "--accept-losses"]);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let bytes = std::fs::read(&target).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("&lt;script&gt;&amp;literal"));
    assert!(
        !run(&["--output", output, "--accept-losses"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&target).unwrap(), bytes);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), content);
    let self_write = cli()
        .arg("exchange")
        .arg(&source)
        .args([
            "--from",
            "txt",
            "--to",
            "html",
            "--accept-losses",
            "--output",
        ])
        .arg(&source)
        .output()
        .unwrap();
    assert!(!self_write.status.success());
    assert_eq!(std::fs::read_to_string(&source).unwrap(), content);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn concurrent_publishers_leave_one_complete_file_and_no_temporary_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.txt");
    let output = dir.path().join("output.html");
    std::fs::write(&input, "中文😀 & literal").unwrap();
    let mut jobs = Vec::new();
    for _ in 0..8 {
        let input = input.clone();
        let output = output.clone();
        jobs.push(std::thread::spawn(move || {
            cli()
                .arg("exchange")
                .arg(input)
                .args(["--from", "txt", "--to", "html", "--output"])
                .arg(output)
                .arg("--accept-losses")
                .output()
                .unwrap()
                .status
                .success()
        }));
    }
    assert_eq!(
        jobs.into_iter()
            .filter_map(|j| j.join().ok())
            .filter(|s| *s)
            .count(),
        1
    );
    assert!(
        std::fs::read_to_string(output)
            .unwrap()
            .contains("中文😀 &amp; literal")
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn malformed_input_and_duplicate_options_leave_no_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bad.txt");
    std::fs::write(&input, [0xff, 0xfe, 0x41]).unwrap();
    let output = dir.path().join("out.md");
    let result = cli()
        .arg("exchange")
        .arg(&input)
        .args([
            "--from",
            "txt",
            "--to",
            "markdown",
            "--accept-losses",
            "--output",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!output.exists());
    for args in [
        vec!["--plan", "--plan"],
        vec!["--plan", "--accept-losses"],
        vec!["--from", "html", "--plan"],
    ] {
        let result = cli()
            .arg("exchange")
            .arg(&input)
            .args(["--from", "txt", "--to", "markdown"])
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success());
    }
}
