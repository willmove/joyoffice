use super::{read_nested_package, read_package_bytes};
use std::{
    io::{Cursor, Write},
    sync::atomic::AtomicBool,
};

fn stored_parts(count: usize, contents: &[u8]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..count {
        writer
            .start_file(
                format!("part-{index:03}"),
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(contents).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn full_document_does_not_inherit_embedded_workbook_limits() {
    let cancel = AtomicBool::new(false);
    let many = stored_parts(129, b"x");
    assert_eq!(read_package_bytes(&many, &cancel).unwrap().len(), 129);
    assert!(read_nested_package(&many, &cancel).is_err());
    let large = stored_parts(2, &vec![7; 5 * 1024 * 1024]);
    assert_eq!(read_package_bytes(&large, &cancel).unwrap().len(), 2);
    assert!(read_nested_package(&large, &cancel).is_err());
}

#[test]
fn raw_duplicate_names_are_rejected_before_name_indexing() {
    let mut bytes = stored_parts(2, b"x");
    for offset in 0..bytes.len().saturating_sub(7) {
        if &bytes[offset..offset + 8] == b"part-001" {
            bytes[offset..offset + 8].copy_from_slice(b"part-000");
        }
    }
    assert!(
        read_package_bytes(&bytes, &AtomicBool::new(false))
            .unwrap_err()
            .contains("重复")
    );
}

#[test]
fn symbolic_link_member_is_rejected_without_extraction() {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_symlink(
            "link",
            "../../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    assert!(read_package_bytes(&bytes, &AtomicBool::new(false)).is_err());
}
