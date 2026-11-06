use std::{fs, process::Command};

#[test]
fn run_refuses_unknown_namespace_sources_before_editing_or_creating_output()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("joyword-cli-read-only-{}", std::process::id()));
    fs::create_dir(&dir)?;
    for (name, bytes, read_only) in [
        (
            "unknown",
            include_bytes!("../../../experiments/docx-robustness/seeds/unknown-namespace.docx")
                .as_slice(),
            true,
        ),
        (
            "nested",
            include_bytes!("../../../experiments/docx-robustness/seeds/xml-depth.docx").as_slice(),
            true,
        ),
        (
            "ordinary",
            include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx").as_slice(),
            false,
        ),
    ] {
        let source = dir.join(format!("{name}.docx"));
        let output = dir.join(format!("{name}-saved.docx"));
        fs::write(&source, bytes)?;
        let result = Command::new(env!("CARGO_BIN_EXE_joyword-cli"))
            .arg("run")
            .arg(&source)
            .args([
                "--cmd",
                "caret.docEnd",
                "--cmd",
                r#"text.insert={"text":" retained edit"}"#,
            ])
            .arg("--save")
            .arg(&output)
            .output()?;
        assert_eq!(
            result.status.success(),
            !read_only,
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(output.exists(), !read_only);
        assert_eq!(fs::read(&source)?, bytes);
        let converted = dir.join(format!("{name}-converted.docx"));
        let conversion = Command::new(env!("CARGO_BIN_EXE_joyword-cli"))
            .arg("convert")
            .arg(&source)
            .arg(&converted)
            .output()?;
        assert_eq!(conversion.status.success(), !read_only);
        assert_eq!(converted.exists(), !read_only);

        if read_only {
            assert!(String::from_utf8_lossy(&result.stderr).contains("read-only"));
        } else {
            let report = Command::new(env!("CARGO_BIN_EXE_joyword-cli"))
                .arg("inspect")
                .arg(&output)
                .output()?;
            assert!(report.status.success());
            assert!(String::from_utf8_lossy(&report.stdout).contains("retained edit"));
        }
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}
