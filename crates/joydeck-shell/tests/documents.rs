#![allow(clippy::unwrap_used, clippy::expect_used, reason = "Test assertions")]
use joydeck_shell::{Cancellation, Document};
use std::io::{Cursor, Write};
const NATIVE: &[u8] = include_bytes!("../../../tests/fixtures/joydeck/r1-preview.deckcraft");
const PPTX: &[u8] = include_bytes!("../../../tests/fixtures/joydeck/independent.pptx");
fn package(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(
                *name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn load(bytes: &[u8]) -> Result<Document, String> {
    Document::from_bytes(
        "misleading.bin".into(),
        bytes.to_vec(),
        &Cancellation::default(),
    )
}
fn pptx_with_part(base: &[u8], name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(base)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default().compression_method(entry.compression()),
            )
            .unwrap();
        std::io::copy(&mut entry, &mut output).unwrap();
    }
    output
        .start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
    output.write_all(bytes).unwrap();
    output.finish().unwrap().into_inner()
}
#[test]
fn pptx_larger_than_the_old_file_part_and_total_limits_opens_in_the_editor() {
    let large = pptx_with_part(PPTX, "ppt/media/large.bin", &vec![0x5a; (128 << 20) + 1]);
    assert!(large.len() as u64 > 64 << 20);
    assert!(large.len() as u64 <= joydeck_shell::document::MAX_INPUT);
    let root = joydeck_shell::platform::private_directory("large-pptx").unwrap();
    let path = root.join("large.pptx");
    std::fs::write(&path, large).unwrap();
    let doc = Document::open(&path, &Cancellation::default()).unwrap();
    assert_eq!(doc.info.format, "pptx");
    assert_eq!(doc.info.slides.len(), 3);
    drop(doc);
    let editor = joydeck_shell::editor::Editor::prepare_open(
        &path,
        root.join("recovery"),
        &Cancellation::default(),
    )
    .unwrap();
    assert_eq!(
        editor.base.info.source_bytes,
        std::fs::metadata(&path).unwrap().len() as usize
    );
    drop(editor);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn input_limit_is_300_mib_and_larger_files_are_rejected_before_reading() {
    assert_eq!(joydeck_shell::document::MAX_INPUT, 300 << 20);
    let root = joydeck_shell::platform::private_directory("oversized-pptx").unwrap();
    let path = root.join("oversized.pptx");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(joydeck_shell::document::MAX_INPUT + 1)
        .unwrap();
    assert_eq!(
        Document::open(&path, &Cancellation::default()).unwrap_err_text(),
        "input exceeds 300 MiB"
    );
    drop(file);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn public_formats_are_sniffed_from_bytes_and_original_is_preserved() {
    for (bytes, format) in [(NATIVE, "deckcraft"), (PPTX, "pptx")] {
        let doc = load(bytes).unwrap();
        assert_eq!(doc.info.format, format);
        assert_eq!(doc.info.slides.len(), 3);
        assert_eq!(&*doc.original, bytes);
        assert!(doc.info.read_only);
        assert!(doc.info.slides[0].notes.contains('1'));
    }
}
#[test]
fn independent_pptx_renders_geometry_picture_table_and_same_thumbnail_content() {
    let doc = load(PPTX).unwrap();
    assert_eq!(doc.info.media, 1);
    let cancel = Cancellation::default();
    let page = doc.render(1, 1., &cancel).unwrap();
    let thumb = doc.render(1, 0.25, &cancel).unwrap();
    assert_eq!((page.width, page.height), (960, 540));
    assert_eq!((thumb.width, thumb.height), (240, 135));
    // Independent producer places a green shape at 0.7in,3in and a yellow raster at 4.5in,3in.
    assert_eq!(page.pixel(80, 240), [34, 111, 76, 255]);
    assert_eq!(page.pixel(330, 220), [232, 193, 112, 255]);
    assert_eq!(thumb.pixel(20, 60), page.pixel(80, 240));
    assert!(page.to_png().starts_with(b"\x89PNG"));
    assert!(doc.info.slides[1].shapes >= 6);
}
#[test]
fn native_and_standard_pptx_do_not_depend_on_private_sidecars() {
    let a = load(NATIVE).unwrap();
    let b = load(include_bytes!(
        "../../../tests/fixtures/joydeck/r1-preview.pptx"
    ))
    .unwrap();
    assert_eq!(a.info.width_pt, b.info.width_pt);
    let cancel = Cancellation::default();
    assert_eq!(
        a.render(0, 0.25, &cancel).unwrap().pixel(1, 60),
        b.render(0, 0.25, &cancel).unwrap().pixel(1, 60)
    );
}
#[test]
fn malformed_packages_and_future_native_versions_fail_recoverably() {
    for bytes in [
        b"not a zip".to_vec(),
        NATIVE[..NATIVE.len() / 2].to_vec(),
        package(&[("../presentation.json", b"{}")]),
        package(&[("ppt/presentation.xml", b"<!DOCTYPE a><a/>")]),
        package(&[("ppt/presentation.xml", b"<a>")]),
        package(&[(
            "presentation.json",
            br#"{"format":"deckcraft","version":999}"#,
        )]),
        package(&[("PRESENTATION.JSON", b"{}"), ("presentation.json", b"{}")]),
    ] {
        assert!(load(&bytes).is_err());
    }
}
#[test]
fn budgets_crc_and_cancellation_are_enforced() {
    let huge = vec![0; (8 << 20) + 1];
    assert!(
        load(&package(&[("oversized", &huge)]))
            .unwrap_err_text()
            .contains("PartSize")
    );
    let deep = format!("{}{}", "<a>".repeat(129), "</a>".repeat(129));
    assert!(
        load(&package(&[("ppt/presentation.xml", deep.as_bytes())]))
            .unwrap_err_text()
            .contains("depth")
    );
    let mut broken = package(&[("mimetype", b"foobar")]);
    let pos = broken.windows(6).position(|p| p == b"foobar").unwrap();
    broken[pos] ^= 1;
    assert!(load(&broken).unwrap_err_text().contains("Corrupt"));
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert!(Document::from_bytes("x".into(), NATIVE.to_vec(), &cancelled).is_err());
    let doc = load(NATIVE).unwrap();
    for (page, scale) in [(999, 1.), (0, f64::NAN), (0, 8.)] {
        assert!(doc.render(page, scale, &Cancellation::default()).is_err());
    }
    assert!(doc.render(0, 1., &cancelled).is_err());
}
trait ErrorText {
    fn unwrap_err_text(self) -> String;
}
impl ErrorText for Result<Document, String> {
    fn unwrap_err_text(self) -> String {
        match self {
            Err(e) => e,
            Ok(_) => String::new(),
        }
    }
}
