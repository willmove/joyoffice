use std::path::Path;
use std::process::{Command, Output};

use joyoffice_opc::{Method, write_archive};
use serde_json::Value;

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

fn run(command: &str, path: &Path, args: &[&str]) -> Output {
    cli().arg(command).arg(path).args(args).output().unwrap()
}

fn package(body: &str) -> Vec<u8> {
    let document = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
    );
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    write_archive(&[
        ("[Content_Types].xml", types.as_bytes(), Method::Stored),
        ("_rels/.rels", rels.as_bytes(), Method::Stored),
        ("word/document.xml", document.as_bytes(), Method::Stored),
    ])
    .unwrap()
}

#[test]
fn five_original_cases_are_stable_and_text_agrees() {
    let dir = tempfile::tempdir().unwrap();
    for case in joyword_corpus::cases().into_iter().take(5) {
        let path = dir.path().join(case.file);
        let bytes = case.package();
        std::fs::write(&path, &bytes).unwrap();
        let first = run("inspect", &path, &[]);
        let second = run("inspect", &path, &[]);
        assert_eq!(first.status.code(), Some(0), "{}", case.id);
        assert_eq!(first.stdout, second.stdout, "{}", case.id);
        assert!(first.stderr.is_empty());
        let json: Value = serde_json::from_slice(&first.stdout).unwrap();
        let expected = match case.id {
            "DOCX-MIN-001" => "合作协议：甲方与乙方于 2026 年签署。".to_string(),
            "DOCX-MIN-002" => "第一条 合作内容\n甲方委托乙方完成设计服务。".to_string(),
            "DOCX-MIN-003" => "第一段：未知内容之前。\n第二段：未知内容之后。".to_string(),
            "DOCX-MIN-004" => "项目说明\n模板与链接都指向本包之外。".to_string(),
            "DOCX-SAMPLE-001" => include_str!("../../../tests/fixtures/text/cjk-contract.txt")
                .lines()
                .collect::<Vec<_>>()
                .join("\n"),
            _ => unreachable!(),
        };
        assert_eq!(json["text"], expected, "{}", case.id);
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["truncated"], false);
        assert_eq!(json["projection"]["hidden_text"], "included");
        let text = run("text", &path, &[]);
        assert_eq!(text.status.code(), Some(0));
        assert_eq!(
            std::str::from_utf8(&text.stdout).unwrap(),
            json["text"].as_str().unwrap()
        );
        let metadata: Value = serde_json::from_slice(&text.stderr).unwrap();
        assert_eq!(metadata["projection"], json["projection"]);
        assert!(!String::from_utf8_lossy(&text.stdout).contains(joyword_model::OBJECT_MARKER));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn projection_preserves_hidden_but_excludes_deleted_and_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("projection.docx");
    std::fs::write(&path, package(r#"<w:p><w:r><w:rPr><w:vanish/></w:rPr><w:t>隐A</w:t><w:br/><w:t>B</w:t></w:r><w:ins><w:r><w:t>INSERTED</w:t></w:r></w:ins><w:moveTo><w:r><w:t>MOVED</w:t></w:r></w:moveTo><w:moveFrom><w:r><w:t>OLD</w:t></w:r></w:moveFrom><w:del><w:r><w:delText>DELETED</w:delText></w:r></w:del><w:r><w:drawing/></w:r><w:r><w:br w:type="page"/><w:t>C</w:t></w:r></w:p>"#)).unwrap();
    let output = run("inspect", &path, &[]);
    assert_eq!(output.status.code(), Some(0));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["text"], "隐A\nBINSERTEDMOVED\u{000c}C");
    assert_eq!(json["partial"], true);
    assert_eq!(json["truncated"], false);
    assert_eq!(json["projection"]["deleted_revisions"], "excluded");
    assert_eq!(json["projection"]["inserted_revisions"], "included");
    assert_eq!(json["projection"]["moved_to_revisions"], "included");
    assert_eq!(json["projection"]["moved_from_revisions"], "excluded");
    assert_eq!(json["projection"]["comments"], "excluded");
    assert!(json["omitted_objects"].as_u64().unwrap() >= 2);
}

#[test]
fn truncation_is_a_utf8_prefix_with_explicit_status() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unicode.docx");
    std::fs::write(&path, package("<w:p><w:r><w:t>甲😀乙</w:t></w:r></w:p>")).unwrap();
    for (limit, expected, truncated) in [
        (0, "", true),
        (2, "", true),
        (3, "甲", true),
        (6, "甲", true),
        (7, "甲😀", true),
        (10, "甲😀乙", false),
    ] {
        let args = ["--max-text-bytes", &limit.to_string()];
        let output = run("inspect", &path, &args);
        assert_eq!(output.status.code(), Some(if truncated { 3 } else { 0 }));
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(json["text"], expected);
        assert_eq!(json["truncated"], truncated);
        let text = run("text", &path, &args);
        assert_eq!(text.status.code(), output.status.code());
        assert_eq!(text.stdout, expected.as_bytes());
        let metadata: Value = serde_json::from_slice(&text.stderr).unwrap();
        assert_eq!(metadata["truncated"], truncated);
    }
}

#[test]
fn invalid_commands_packages_and_budgets_fail_without_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.docx");
    std::fs::write(&path, b"not a zip").unwrap();
    for (command, args) in [
        ("inspect", vec![]),
        ("render", vec![]),
        ("convert", vec![]),
        ("text", vec!["--max-text-bytes", "1048577"]),
        ("text", vec!["--max-text-bytes", "-1"]),
        ("text", vec!["--hidden"]),
    ] {
        let output = run(command, &path, &args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.len() < 256);
    }
    assert_eq!(run("inspect", dir.path(), &[]).status.code(), Some(2));
    let file = std::fs::File::create(&path).unwrap();
    file.set_len((32 << 20) + 1).unwrap();
    assert_eq!(run("inspect", &path, &[]).status.code(), Some(2));
    assert_eq!(
        run("text", &dir.path().join("missing"), &[]).status.code(),
        Some(2)
    );
}

#[test]
fn xml_and_expanded_part_budgets_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("budget.docx");
    let nested = format!(
        "{}{}",
        "<w:unknown>".repeat(129),
        "</w:unknown>".repeat(129)
    );
    std::fs::write(&path, package(&nested)).unwrap();
    let output = run("inspect", &path, &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let huge = format!("<w:p><w:r><w:t>{}</w:t></w:r></w:p>", "a".repeat(16 << 20));
    std::fs::write(&path, package(&huge)).unwrap();
    let output = run("inspect", &path, &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.len() < 256);
}

#[test]
fn help_options_and_literal_paths_are_unambiguous() {
    let dir = tempfile::tempdir().unwrap();
    let name = "--max-text-bytes";
    let path = dir.path().join(name);
    let bytes = package("<w:p><w:r><w:t>甲乙</w:t></w:r></w:p>");
    std::fs::write(&path, &bytes).unwrap();
    for command in ["inspect", "text"] {
        let help = cli().args([command, "--help"]).output().unwrap();
        assert_eq!(help.status.code(), Some(0));
        assert!(help.stderr.is_empty());
        assert!(
            String::from_utf8(help.stdout)
                .unwrap()
                .contains("--max-text-bytes")
        );
        let before = cli()
            .args([command, "--max-text-bytes", "3"])
            .arg(&path)
            .output()
            .unwrap();
        let after = run(command, &path, &["--max-text-bytes", "3"]);
        assert_eq!(before.status.code(), Some(3));
        assert_eq!(before.stdout, after.stdout);
        assert_eq!(before.stderr, after.stderr);
        let literal = cli()
            .current_dir(dir.path())
            .args([command, "--max-text-bytes", "3", "--", name])
            .output()
            .unwrap();
        assert_eq!(literal.status.code(), Some(3));
        assert_eq!(literal.stdout, before.stdout);
        assert_eq!(literal.stderr, before.stderr);
    }
    for args in [
        vec!["inspect"],
        vec!["text", "--"],
        vec!["inspect", "--unknown"],
        vec!["inspect", "--max-text-bytes"],
        vec!["inspect", "--help", "extra"],
        vec!["inspect", "--max-text-bytes", "3", "--max-text-bytes", "3"],
    ] {
        let output = cli().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn empty_body_and_paragraph_separators_obey_exact_text_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boundaries.docx");
    for (body, limit, expected, truncated, paragraphs) in [
        ("", 0, "", false, 1),
        ("<w:p/>", 0, "", false, 1),
        ("<w:p/><w:p/>", 0, "", true, 2),
        ("<w:p/><w:p/>", 1, "\n", false, 2),
        ("<w:p><w:r><w:t>A</w:t></w:r></w:p><w:p/>", 1, "A", true, 2),
        (
            "<w:p><w:r><w:t>A</w:t></w:r></w:p><w:p/>",
            2,
            "A\n",
            false,
            2,
        ),
    ] {
        let bytes = package(body);
        std::fs::write(&path, &bytes).unwrap();
        let args = ["--max-text-bytes", &limit.to_string()];
        let inspect = run("inspect", &path, &args);
        assert_eq!(inspect.status.code(), Some(if truncated { 3 } else { 0 }));
        let mut metadata: Value = serde_json::from_slice(&inspect.stdout).unwrap();
        assert_eq!(metadata["text"], expected);
        assert_eq!(metadata["text_bytes"], expected.len());
        assert_eq!(metadata["paragraphs"], paragraphs);
        assert_eq!(metadata["truncated"], truncated);
        metadata.as_object_mut().unwrap().remove("text");
        let text = run("text", &path, &args);
        assert_eq!(text.status.code(), inspect.status.code());
        assert_eq!(text.stdout, expected.as_bytes());
        assert_eq!(
            serde_json::from_slice::<Value>(&text.stderr).unwrap(),
            metadata
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[cfg(unix)]
#[test]
fn broken_output_streams_never_report_success() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;

    fn broken_stream() -> Stdio {
        let (writer, reader) = UnixStream::pair().unwrap();
        drop(reader);
        Stdio::from(OwnedFd::from(writer))
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.docx");
    let bytes = package("<w:p><w:r><w:t>正文</w:t></w:r></w:p>");
    std::fs::write(&path, &bytes).unwrap();
    for command in ["inspect", "text"] {
        let output = cli()
            .arg(command)
            .arg(&path)
            .stdout(broken_stream())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
    let output = cli()
        .arg("text")
        .arg(&path)
        .stderr(broken_stream())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
