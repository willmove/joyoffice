use joydeck::document::*;
use std::{
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
};
use std::{path::PathBuf, sync::atomic::AtomicBool};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}
#[test]
fn native_pages_text_and_geometry() {
    let d = open(&fixture("native-20.joydeck"), &AtomicBool::new(false)).unwrap();
    assert_eq!(d.slides.len(), 20);
    assert_eq!((d.width, d.height), (960., 540.));
    assert!(
        d.slides[19]
            .objects
            .iter()
            .any(|o| o.text.contains("第 20 页"))
    );
}
#[test]
fn truncated_zip_and_pre_cancelled_load_are_errors() {
    assert!(open(&fixture("broken.pptx"), &AtomicBool::new(false)).is_err());
    assert!(open(&fixture("native-20.joydeck"), &AtomicBool::new(true)).is_err());
}
#[test]
fn pptx_resolves_main_part_and_uses_presentation_order() {
    let d = open(&fixture("public-format-20.pptx"), &AtomicBool::new(false)).unwrap();
    assert_eq!(d.slides.len(), 20);
    assert!(d.slides[0].title.contains("20"));
    assert!(d.slides[19].title.contains("01"));
    assert!(
        d.warnings
            .iter()
            .any(|w| w.page == Some(1) && w.code == "unsupported-object")
    );
    assert_eq!((d.width, d.height), (960., 540.));
}
#[test]
fn background_preview_is_bounded_and_bgra() {
    let d = open(&fixture("native-20.joydeck"), &AtomicBool::new(false)).unwrap();
    let r = joydeck::preview::render(&d, 0, 320, &AtomicBool::new(false)).unwrap();
    assert_eq!((r.width, r.height), (320, 180));
    assert_eq!(&r.bgra[..4], &[0xa7, 0xb6, 0x27, 0xff]);
    assert!(joydeck::preview::render(&d, 20, 320, &AtomicBool::new(false)).is_err());
}

fn read_generated(entries: Vec<(String, Vec<u8>)>) -> Result<Deck> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "joydeck-reader-{}-{}.zip",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let file = std::fs::File::create(&path).unwrap();
    let mut z = zip::ZipWriter::new(file);
    for (name, bytes) in entries {
        z.start_file(
            name,
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        z.write_all(&bytes).unwrap();
    }
    z.finish().unwrap();
    let result = open(&path, &AtomicBool::new(false));
    std::fs::remove_file(path).unwrap();
    result
}
#[test]
fn rejects_ambiguous_paths_duplicate_members_and_expansion_limit() {
    for name in [
        "../escape",
        "/absolute",
        "C:/absolute",
        "a\\b",
        "a/%2e%2e/b",
        "a/./b",
    ] {
        assert!(
            read_generated(vec![(name.into(), vec![0])]).is_err(),
            "{name}"
        );
    }
    assert!(read_generated(vec![("same".into(), vec![0]), ("same".into(), vec![1])]).is_err());
    assert!(
        read_generated(vec![("large".into(), vec![0; MAX_PART + 1])])
            .unwrap_err()
            .contains("8 MiB")
    );
}
#[test]
fn rejects_unknown_required_feature_major_duplicate_ids_and_bad_geometry() {
    let base = read_package(&fixture("native-20.joydeck"), &AtomicBool::new(false)).unwrap();
    for (member, field, value) in [
        ("manifest.json", "schemaMajor", serde_json::json!(2)),
        (
            "manifest.json",
            "requiredFeatures",
            serde_json::json!(["unknown"]),
        ),
        ("document.json", "width", serde_json::json!(1e30)),
    ] {
        let mut parts = base.clone();
        let mut data: serde_json::Value = serde_json::from_slice(&parts[member]).unwrap();
        data[field] = value;
        parts.insert(member.into(), serde_json::to_vec(&data).unwrap());
        assert!(read_generated(parts.into_iter().collect()).is_err());
    }
    let mut parts = base;
    let mut data: serde_json::Value = serde_json::from_slice(&parts["document.json"]).unwrap();
    data["slides"][1]["id"] = data["slides"][0]["id"].clone();
    parts.insert("document.json".into(), serde_json::to_vec(&data).unwrap());
    assert!(
        read_generated(parts.into_iter().collect())
            .unwrap_err()
            .contains("ID")
    );
}
#[test]
fn missing_slide_external_main_and_xml_dtd_fail_safely() {
    let base = read_package(&fixture("public-format-20.pptx"), &AtomicBool::new(false)).unwrap();
    let mut missing = base.clone();
    missing.remove("slideset/pages/p20.xml");
    assert!(
        read_generated(missing.into_iter().collect())
            .unwrap_err()
            .contains("缺少部件")
    );
    let mut external = base.clone();
    let root = String::from_utf8(external["_rels/.rels"].clone())
        .unwrap()
        .replace("Target=", "TargetMode=\"External\" Target=");
    external.insert("_rels/.rels".into(), root.into_bytes());
    assert!(read_generated(external.into_iter().collect()).is_err());
    let mut dtd = base;
    dtd.insert(
        "slideset/main.xml".into(),
        b"<!DOCTYPE a [<!ENTITY e 'bad'>]><a>&e;</a>".to_vec(),
    );
    assert!(
        read_generated(dtd.into_iter().collect())
            .unwrap_err()
            .contains("XML")
    );
}
