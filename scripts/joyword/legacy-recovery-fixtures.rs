// Run as a joyword-shell example in a detached worktree at 00fb0687.
use joyword_shell::{
    command::{ArgValue, EDIT_BOLD, EDIT_INSERT, EDIT_SELECT_ALL, FILE_OPEN, TABLE_INSERT},
    host::Office,
    storage,
};
use std::{fs, path::PathBuf};

fn main() {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let out = args.next().expect("fixture output directory");
    fs::create_dir(&out).unwrap();
    let source = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
    let source_path = out.join("source.docx");
    fs::write(&source_path, source).unwrap();

    let rich = out.join("rich");
    let mut office = Office::new();
    office.set_recovery_directory(rich.clone());
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source_path.clone()))])
        .unwrap();
    office
        .invoke(
            EDIT_INSERT,
            &[("text", ArgValue::Text("旧版恢复😀 ".into()))],
        )
        .unwrap();
    office.invoke(EDIT_SELECT_ALL, &[]).unwrap();
    office.invoke(EDIT_BOLD, &[]).unwrap();
    office.invoke(TABLE_INSERT, &[]).unwrap();
    let checkpoint = storage::recover_document_checkpoint(&rich).unwrap();
    fs::write(out.join("snapshot.docx"), checkpoint.snapshot.unwrap()).unwrap();

    let text = "合作协议：甲方与乙方于 2026 年签署。";
    let dual = out.join("dual");
    storage::commit_recovery(&dual, source, &format!("{text} 初稿")).unwrap();
    storage::commit_recovery(&dual, source, &format!("{text} 定稿😀")).unwrap();

    let v1 = out.join("single");
    fs::create_dir(&v1).unwrap();
    // The baseline reader's JWREC001 fixture encoder (the production writer emits V2/V3).
    let mut bytes = b"JWREC001".to_vec();
    for value in [
        source.as_slice(),
        "合作协议：甲方与乙方于 2026 年签署。 单槽😀".as_bytes(),
    ] {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);
    }
    bytes.extend_from_slice(&crc32fast::hash(&bytes).to_le_bytes());
    fs::write(v1.join("recovery.jwr"), bytes).unwrap();
    assert!(
        storage::recover_checkpoint(&v1)
            .unwrap()
            .recovered
            .text
            .ends_with("单槽😀")
    );

    for name in ["fallback", "broken", "discarded", "conflict"] {
        let dir = out.join(name);
        fs::create_dir(&dir).unwrap();
        for entry in fs::read_dir(&dual).unwrap() {
            let entry = entry.unwrap();
            fs::copy(entry.path(), dir.join(entry.file_name())).unwrap();
        }
    }
    let fallback = out.join("fallback/recovery-b.jwr");
    let mut damaged = fs::read(&fallback).unwrap();
    damaged[25] ^= 1;
    fs::write(&fallback, damaged).unwrap();
    fs::write(out.join("broken/recovery-a.jwr"), b"JWREC002").unwrap();
    fs::copy(fallback, out.join("broken/recovery-b.jwr")).unwrap();
    fs::copy(v1.join("recovery.jwr"), out.join("broken/recovery.jwr")).unwrap();
    storage::clear_recovery(&out.join("discarded")).unwrap();
    let conflict = out.join("conflict/recovery-b.jwr");
    let mut record = fs::read(&conflict).unwrap();
    record[8..16].copy_from_slice(&1u64.to_le_bytes());
    record.truncate(record.len() - 4);
    record.extend_from_slice(&crc32fast::hash(&record).to_le_bytes());
    fs::write(conflict, record).unwrap();

    let missing = out.join("source-missing");
    fs::create_dir(&missing).unwrap();
    let missing_source = missing.join("deleted.docx");
    fs::copy(&source_path, &missing_source).unwrap();
    storage::commit_recovery(
        &missing,
        &fs::read(&missing_source).unwrap(),
        &format!("{text} 定稿😀"),
    )
    .unwrap();
    fs::remove_file(missing_source).unwrap();

    let mismatch = out.join("mismatch");
    storage::commit_document_recovery_versioned(&mismatch, 1, source, "mismatch", source).unwrap();
    assert_eq!(fs::read(source_path).unwrap(), source);
    assert!(storage::recover_document_checkpoint(&out.join("broken")).is_err());
    assert!(storage::recover_document_checkpoint(&out.join("discarded")).is_err());
    assert!(storage::recover_document_checkpoint(&out.join("conflict")).is_err());
}
