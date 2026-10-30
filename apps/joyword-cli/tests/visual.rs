use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn cli() -> Command {
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
fn archival_export_requires_explicit_year_is_repeatable_and_never_overwrites() {
    let Some(font) = font_spec() else { return };
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mail-merge/template.docx");
    let source = std::fs::read(&root).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("archive.pdf");
    for year in [None, Some("0"), Some("10000"), Some("bad")] {
        let mut command = cli();
        command
            .arg("convert")
            .arg(&root)
            .args(["--to", "pdfa-2b", "--font", &font, "--output"])
            .arg(&output);
        if let Some(year) = year {
            command.args(["--archive-year", year]);
        }
        assert!(!command.output().unwrap().status.success());
        assert!(!output.exists());
    }
    let run = |path: &Path| {
        cli()
            .arg("convert")
            .arg(&root)
            .args([
                "--to",
                "pdfa-2b",
                "--archive-year",
                "2026",
                "--font",
                &font,
                "--output",
            ])
            .arg(path)
            .output()
            .unwrap()
    };
    let result = run(&output);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["target"], "pdfa-2b");
    assert_eq!(report["independent_conformance_verified"], false);
    let second = dir.path().join("second.pdf");
    assert!(run(&second).status.success());
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(bytes, std::fs::read(second).unwrap());
    assert!(!run(&output).status.success());
    assert_eq!(bytes, std::fs::read(&output).unwrap());
    assert_eq!(source, std::fs::read(root).unwrap());
}

fn font_spec() -> Option<String> {
    if let Ok(spec) = std::env::var("JOYWORD_TEST_FONT") {
        return Some(spec);
    }
    for (path, face) in [
        ("/System/Library/Fonts/Supplemental/Songti.ttc", 6),
        ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 2),
        ("C:\\Windows\\Fonts\\simsun.ttc", 0),
    ] {
        if Path::new(path).is_file() {
            return Some(format!("{path}:{face}"));
        }
    }
    assert!(
        std::env::var_os("JOYWORD_REQUIRE_TEST_FONT").is_none(),
        "a real CJK font is required"
    );
    eprintln!("SKIPPED: no CJK font for visual CLI tests");
    None
}

fn run(args: &[&str]) -> Output {
    cli().args(args).output().unwrap()
}

fn input(dir: &Path) -> (std::path::PathBuf, Vec<u8>) {
    let path = dir.join("input.docx");
    let bytes = include_bytes!("../../../tests/fixtures/docx/DOCX-SAMPLE-001.docx").to_vec();
    std::fs::write(&path, &bytes).unwrap();
    (path, bytes)
}

fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    (
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    )
}

#[test]
fn render_is_repeatable_and_refuses_overwrite() {
    let Some(font) = font_spec() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (input, original) = input(dir.path());
    let input = input.to_str().unwrap();
    let first_dir = dir.path().join("first");
    let second_dir = dir.path().join("second");
    for output in [&first_dir, &second_dir] {
        let result = run(&[
            "render",
            input,
            "--font",
            &font,
            "--output",
            output.to_str().unwrap(),
            "--dpi",
            "96",
            "--page",
            "1",
        ]);
        assert_eq!(result.status.code(), Some(0), "{:?}", result.stderr);
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["operation"], "render");
        assert_eq!(report["pages"], 1);
        assert_eq!(report["dpi"], 96);
        assert_eq!(report["files"][0]["name"], "page-1.png");
        assert_eq!(report["schema_version"], 1);
        assert_eq!(report["omitted_blocks"], 0);
    }
    let first = std::fs::read(first_dir.join("page-1.png")).unwrap();
    let second = std::fs::read(second_dir.join("page-1.png")).unwrap();
    assert_eq!(first, second);
    let (width, height) = png_size(&first);
    assert!(width > 500 && height > 500);
    assert_eq!(std::fs::read(input).unwrap(), original);

    let repeated = run(&[
        "render",
        input,
        "--font",
        &font,
        "--output",
        first_dir.to_str().unwrap(),
        "--page",
        "1",
    ]);
    assert_eq!(repeated.status.code(), Some(2));
    assert!(repeated.stdout.is_empty());
    let outside = run(&[
        "render",
        input,
        "--font",
        &font,
        "--output",
        dir.path().join("outside").to_str().unwrap(),
        "--page",
        "99",
    ]);
    assert_eq!(outside.status.code(), Some(2));
    assert!(outside.stdout.is_empty());
}

#[test]
fn convert_to_pdf_is_repeatable_and_other_targets_are_refused() {
    let Some(font) = font_spec() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (input, original) = input(dir.path());
    let input = input.to_str().unwrap();
    let first = dir.path().join("first.pdf");
    let second = dir.path().join("second.pdf");
    for output in [&first, &second] {
        let result = run(&[
            "convert",
            input,
            "--to",
            "pdf",
            "--font",
            &font,
            "--output",
            output.to_str().unwrap(),
        ]);
        assert_eq!(result.status.code(), Some(0), "{:?}", result.stderr);
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["operation"], "convert");
        assert_eq!(report["target"], "pdf");
        assert!(report["pages"].as_u64().unwrap() >= 1);
        assert_eq!(report["schema_version"], 1);
        assert_eq!(report["omitted_blocks"], 0);
    }
    let first_bytes = std::fs::read(&first).unwrap();
    assert_eq!(first_bytes, std::fs::read(&second).unwrap());
    assert!(first_bytes.starts_with(b"%PDF-"));
    assert!(first_bytes.len() > 1000);
    assert_eq!(std::fs::read(input).unwrap(), original);

    let extracted = dir.path().join("extracted.txt");
    match Command::new("pdftotext")
        .args(["-enc", "UTF-8"])
        .arg(&first)
        .arg(&extracted)
        .output()
    {
        Ok(output) => {
            assert!(output.status.success(), "{:?}", output.stderr);
            let text = std::fs::read_to_string(extracted).unwrap();
            assert!(text.contains("合作协议（原创测试文本）"), "{text}");
            assert!(text.contains("青禾工作室"), "{text}");
            assert!(text.contains("English summary"), "{text}");
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("SKIPPED: pdftotext unavailable for independent UTF-8 read-back");
        }
        Err(error) => panic!("cannot run pdftotext: {error}"),
    }

    let repeated = run(&[
        "convert",
        input,
        "--to",
        "pdf",
        "--font",
        &font,
        "--output",
        first.to_str().unwrap(),
    ]);
    assert_eq!(repeated.status.code(), Some(2));
    assert!(repeated.stdout.is_empty());
    let unsupported = run(&[
        "convert",
        input,
        "--to",
        "txt",
        "--font",
        &font,
        "--output",
        dir.path().join("out.txt").to_str().unwrap(),
    ]);
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(unsupported.stdout.is_empty());
}

#[test]
fn original_corpus_render_and_convert_are_byte_identical() {
    let Some(font) = font_spec() else { return };
    let fixtures = [
        (
            "DOCX-MIN-001",
            include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx").as_slice(),
            0,
        ),
        (
            "DOCX-MIN-002",
            include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-002.docx").as_slice(),
            0,
        ),
        (
            "DOCX-MIN-003",
            include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-003.docx").as_slice(),
            1,
        ),
        (
            "DOCX-MIN-004",
            include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-004.docx").as_slice(),
            0,
        ),
        (
            "DOCX-MIN-005",
            include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-005.docx").as_slice(),
            0,
        ),
    ];
    for (id, bytes, omitted) in fixtures {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join(format!("{id}.docx"));
        std::fs::write(&input, bytes).unwrap();
        let input_arg = input.to_str().unwrap();
        let mut render_reports = Vec::new();
        let mut pngs = Vec::new();
        for pass in ["first", "second"] {
            let output = dir.path().join(format!("render-{pass}"));
            let result = run(&[
                "render",
                input_arg,
                "--font",
                &font,
                "--output",
                output.to_str().unwrap(),
                "--dpi",
                "96",
            ]);
            assert_eq!(
                result.status.code(),
                Some(0),
                "{id} render {pass}: {:?}",
                result.stderr
            );
            render_reports.push(result.stdout);
            let report: Value = serde_json::from_slice(render_reports.last().unwrap()).unwrap();
            assert_eq!(report["schema_version"], 1, "{id}");
            assert_eq!(report["operation"], "render", "{id}");
            assert_eq!(report["omitted_blocks"], omitted, "{id}");
            assert!(report["pages"].as_u64().unwrap() >= 1, "{id}");
            let page = std::fs::read(output.join("page-1.png")).unwrap();
            assert!(page.starts_with(b"\x89PNG\r\n\x1a\n"), "{id}");
            pngs.push(page);
        }
        assert_eq!(render_reports[0], render_reports[1], "{id} render report");
        assert_eq!(pngs[0], pngs[1], "{id} png");
        let repeated = run(&[
            "render",
            input_arg,
            "--font",
            &font,
            "--output",
            dir.path().join("render-first").to_str().unwrap(),
            "--dpi",
            "96",
        ]);
        assert_eq!(repeated.status.code(), Some(2), "{id} overwrite");
        assert!(repeated.stdout.is_empty(), "{id}");

        let mut convert_reports = Vec::new();
        let mut pdfs = Vec::new();
        for pass in ["first", "second"] {
            let output = dir.path().join(format!("{pass}.pdf"));
            let result = run(&[
                "convert",
                input_arg,
                "--to",
                "pdf",
                "--font",
                &font,
                "--output",
                output.to_str().unwrap(),
            ]);
            assert_eq!(
                result.status.code(),
                Some(0),
                "{id} convert {pass}: {:?}",
                result.stderr
            );
            convert_reports.push(result.stdout);
            let report: Value = serde_json::from_slice(convert_reports.last().unwrap()).unwrap();
            assert_eq!(report["schema_version"], 1, "{id}");
            assert_eq!(report["operation"], "convert", "{id}");
            assert_eq!(report["target"], "pdf", "{id}");
            assert_eq!(report["omitted_blocks"], omitted, "{id}");
            let pdf = std::fs::read(&output).unwrap();
            assert!(pdf.starts_with(b"%PDF-"), "{id}");
            pdfs.push(pdf);
        }
        assert_eq!(
            convert_reports[0], convert_reports[1],
            "{id} convert report"
        );
        assert_eq!(pdfs[0], pdfs[1], "{id} pdf");
        let repeated_pdf = run(&[
            "convert",
            input_arg,
            "--to",
            "pdf",
            "--font",
            &font,
            "--output",
            dir.path().join("first.pdf").to_str().unwrap(),
        ]);
        assert_eq!(repeated_pdf.status.code(), Some(2), "{id} pdf overwrite");
        assert!(repeated_pdf.stdout.is_empty(), "{id}");
        let refused = run(&[
            "convert",
            input_arg,
            "--to",
            "html",
            "--font",
            &font,
            "--output",
            dir.path().join("out.html").to_str().unwrap(),
        ]);
        assert_eq!(refused.status.code(), Some(2), "{id} unsupported target");
        assert!(refused.stdout.is_empty(), "{id}");
        assert!(!dir.path().join("out.html").exists(), "{id}");
        assert_eq!(std::fs::read(&input).unwrap(), bytes, "{id} input");
    }
}

#[test]
fn visual_help_and_literal_paths_are_unambiguous() {
    for command in ["render", "convert"] {
        let help = run(&[command, "--help"]);
        assert_eq!(help.status.code(), Some(0));
        assert!(help.stderr.is_empty());
        let text = String::from_utf8(help.stdout).unwrap();
        assert!(text.contains(command));
        assert!(text.contains("omitted_blocks"));
    }
}
