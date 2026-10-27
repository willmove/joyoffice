//! T204 error path: `convert` refuses a font whose OS/2 embedding bits forbid
//! embedding, reports failure and leaves no output file.

use std::path::Path;
use std::process::{Command, Output};

use joyoffice_opc::{Method, write_archive};

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

fn convert(docx: &Path, font: &Path, output: &Path) -> Output {
    cli()
        .arg("convert")
        .arg(docx)
        .args([
            "--font",
            &format!("{}:0", font.display()),
            "--to",
            "pdf",
            "--output",
            &output.display().to_string(),
        ])
        .output()
        .unwrap()
}

fn package() -> Vec<u8> {
    let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>A</w:t></w:r></w:p></w:body></w:document>"#;
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    write_archive(&[
        ("[Content_Types].xml", types.as_bytes(), Method::Stored),
        ("_rels/.rels", rels.as_bytes(), Method::Stored),
        ("word/document.xml", document.as_bytes(), Method::Stored),
    ])
    .unwrap()
}

/// The committed fixture with its OS/2 `fsType` patched to the given value.
fn font_bytes_with_fs_type(fs_type: u16) -> Vec<u8> {
    let mut bytes =
        include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").to_vec();
    let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let record = (0..count)
        .map(|index| 12 + 16 * index)
        .find(|&at| bytes[at..at + 4] == *b"OS/2")
        .expect("fixture carries an OS/2 table");
    let table = u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
    bytes[table + 8..table + 10].copy_from_slice(&fs_type.to_be_bytes());
    bytes
}

#[test]
fn convert_refuses_a_restricted_license_font_and_writes_no_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let docx = dir.path().join("doc.docx");
    std::fs::write(&docx, package()).unwrap();
    // fsType & 0xF == 2 is Restricted License embedding (OpenType OS/2).
    let font = dir.path().join("restricted.ttf");
    std::fs::write(&font, font_bytes_with_fs_type(2)).unwrap();
    let output = dir.path().join("out.pdf");
    let result = convert(&docx, &font, &output);
    assert_eq!(result.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("font cannot be embedded"),
        "unexpected stderr: {stderr}"
    );
    assert!(result.stdout.is_empty());
    assert!(!output.exists(), "a refused export leaves no file");
}

#[test]
fn convert_still_exports_with_an_embeddable_font() {
    let dir = tempfile::tempdir().unwrap();
    let docx = dir.path().join("doc.docx");
    std::fs::write(&docx, package()).unwrap();
    // Editable-embedding bits (fsType 8) do not refuse export; the patched
    // bytes otherwise exercise the same code path as the refusal case.
    let font = dir.path().join("embeddable.ttf");
    std::fs::write(&font, font_bytes_with_fs_type(8)).unwrap();
    let output = dir.path().join("out.pdf");
    let result = convert(&docx, &font, &output);
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());
    let bytes = std::fs::read(&output).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}
