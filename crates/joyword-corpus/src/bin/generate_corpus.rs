//! Writes every minimal format case to `tests/fixtures/docx` and prints its
//! SHA-256 for the registry. Run after changing a case, then update
//! `tests/fixtures/manifest.json`; never edit the registry hash by hand to
//! hide a regression.

use std::path::Path;

fn main() -> std::io::Result<()> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/docx");
    std::fs::create_dir_all(&dir)?;
    for case in joyword_corpus::cases() {
        let bytes = case.package();
        std::fs::write(dir.join(case.file), &bytes)?;
        println!("{} docx/{} {} bytes", case.id, case.file, bytes.len());
    }
    Ok(())
}
