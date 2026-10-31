use joydeck::{
    document,
    edit::{self, Command, Session},
    export::{self, ImageFormat, Options},
    interop, pptx_write,
    show::ShowSnapshot,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
static IDS: AtomicU64 = AtomicU64::new(1);
fn dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "joydeck-m7-{}-{}",
        std::process::id(),
        IDS.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).unwrap();
    p
}
fn sample() -> Session {
    let mut s = Session::new();
    for kind in ["rect", "ellipse", "roundRect", "arrow", "line", "text"] {
        let mut o = edit::object(kind);
        o.x += s.deck.slides[0].objects.len() as f32 * 20.;
        if kind == "text" {
            o.text = "JoyDeck 中文 🦀\nsecond line".into();
        }
        s.execute(s.id, Command::AddObject(0, o)).unwrap();
    }
    s.execute(s.id, Command::AddPage).unwrap();
    s.execute(s.id, Command::AddPage).unwrap();
    s.execute(s.id, Command::HidePages(vec![1])).unwrap();
    s
}
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/public-format-20.pptx")
}
#[test]
fn static_pptx_public_xml_relationships_and_private_snapshot_are_consistent() {
    let mut s = sample();
    s.execute(
        s.id,
        Command::Design(joydeck::design_commands::Action::Notes(
            0,
            "Notes compatibility regression".into(),
        )),
    )
    .unwrap();
    let parts = pptx_write::generate(&s.deck).unwrap();
    assert!(parts.contains_key("ppt/notesSlides/notesSlide1.xml"));
    assert!(!parts.contains_key("ppt/notesMasters/notesMaster1.xml"));
    interop::validate_graph(&parts).unwrap();
    assert!(parts.contains_key("ppt/slideMasters/slideMaster1.xml"));
    assert_eq!(pptx_write::read_snapshot(&parts).unwrap().unwrap(), *s.deck);
    for (name, data) in &parts {
        if name.ends_with(".xml") || name.ends_with(".rels") {
            interop::xml(std::str::from_utf8(data).unwrap()).unwrap();
        }
    }
    let mut changed = parts.clone();
    changed.get_mut("ppt/slides/slide1.xml").unwrap().push(b' ');
    assert!(pptx_write::read_snapshot(&changed).unwrap().is_none());
    let root = dir();
    let path = root.join("output.pptx");
    export::save_parts(&path, &parts, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        document::open(&path, &AtomicBool::new(false)).unwrap(),
        *s.deck
    );
    assert!(export::save_parts(&path, &parts, &AtomicBool::new(false)).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn preserve_unknown_relationship_closure_and_patch_single_run_text() {
    let root = dir();
    let mut parts = document::read_package(&fixture(), &AtomicBool::new(false)).unwrap();
    let xml=String::from_utf8(parts["_rels/.rels"].clone()).unwrap().replace("</Relationships>","<Relationship Id=\"unknown\" Type=\"urn:joydeck:test-unknown\" Target=\"unknown/owner.xml\"/></Relationships>");
    parts.insert("_rels/.rels".into(), xml.into_bytes());
    parts.insert("unknown/owner.xml".into(), b"<unknown/>".to_vec());
    parts.insert("unknown/_rels/owner.xml.rels".into(),b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"asset\" Type=\"urn:unknown\" Target=\"asset.bin\"/></Relationships>".to_vec());
    parts.insert("unknown/asset.bin".into(), vec![0, 255, 42, 9]);
    let path = root.join("source.pptx");
    export::save_parts(&path, &parts, &AtomicBool::new(false)).unwrap();
    let d = interop::import(&path).unwrap();
    let source = d.source.as_ref().unwrap();
    assert_eq!(source.preserve(&d).unwrap(), parts);
    assert!(!source.text.is_empty());
    let binding = source.text[0].clone();
    let mut s = Session::from_deck(d, false);
    let (p, i) = s
        .deck
        .slides
        .iter()
        .enumerate()
        .find_map(|(p, s)| {
            s.objects
                .iter()
                .position(|o| o.id == binding.object)
                .map(|i| (p, i))
        })
        .unwrap();
    let mut o = s.deck.slides[p].objects[i].clone();
    o.text = "Patched 中文 & <safe>".into();
    s.execute(s.id, Command::ReplaceObject(p, i, o)).unwrap();
    let patched = s.deck.source.as_ref().unwrap().preserve(&s.deck).unwrap();
    assert_eq!(patched["unknown/asset.bin"], parts["unknown/asset.bin"]);
    assert_eq!(
        patched["unknown/_rels/owner.xml.rels"],
        parts["unknown/_rels/owner.xml.rels"]
    );
    assert_ne!(patched[&binding.part], parts[&binding.part]);
    interop::validate_graph(&patched).unwrap();
    let native = root.join("source.joydeck");
    edit::save(&native, &s.deck).unwrap();
    let reopened = edit::open_editable(&native).unwrap();
    assert!(serde_json::to_value(&*s.deck).unwrap() == serde_json::to_value(&reopened).unwrap());
    assert_eq!(
        reopened
            .source
            .as_ref()
            .unwrap()
            .preserve(&reopened)
            .unwrap(),
        patched
    );
    s.execute(s.id, Command::AddPage).unwrap();
    assert!(s.deck.source.as_ref().unwrap().preserve(&s.deck).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn dangling_duplicate_and_escaping_relationships_are_rejected() {
    let mut parts = document::read_package(&fixture(), &AtomicBool::new(false)).unwrap();
    parts.remove("slideset/pages/p20.xml");
    assert!(interop::validate_graph(&parts).is_err());
    assert!(interop::target("a/file.xml", "../../outside").is_err());
    assert!(interop::target("a/file.xml", "file:///private").is_err());
    assert_eq!(
        interop::target("a/file.xml", "../image.png").unwrap(),
        "image.png"
    );
    let mut parts = std::collections::BTreeMap::new();
    parts.insert("_rels/.rels".into(),b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"x\" Type=\"urn:test\" TargetMode=\"External\" Target=\"https://a\"/><Relationship Id=\"x\" Type=\"urn:test\" TargetMode=\"External\" Target=\"https://b\"/></Relationships>".to_vec());
    assert!(interop::validate_graph(&parts).is_err());
}
#[test]
fn page_range_transparency_dpi_and_no_clobber_exports() {
    let s = sample();
    assert_eq!(export::pages(&s.deck, "", false).unwrap(), vec![0, 2]);
    assert_eq!(export::pages(&s.deck, "1-3", true).unwrap(), vec![0, 1, 2]);
    assert!(export::pages(&s.deck, "1,1", true).is_err());
    assert!(export::pages(&s.deck, "0", false).is_err());
    let root = dir();
    let cancel = AtomicBool::new(false);
    let options = Options {
        pages: vec![0],
        dpi: 72,
        transparent: true,
        notes: false,
    };
    let out = root.join("png");
    export::images(&out, &s.deck, &options, ImageFormat::Png, &cancel).unwrap();
    let image = image::open(out.join("slide-001.png")).unwrap().into_rgba8();
    assert_eq!(image.dimensions(), (960, 540));
    assert_eq!(image.get_pixel(0, 0)[3], 0);
    let file = std::fs::File::open(out.join("slide-001.png")).unwrap();
    let reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .unwrap();
    assert_eq!(reader.info().pixel_dims.unwrap().xppu, 2835);
    assert!(export::images(&out, &s.deck, &options, ImageFormat::Png, &cancel).is_err());
    assert!(
        export::images(
            &root.join("jpg"),
            &s.deck,
            &options,
            ImageFormat::Jpeg,
            &cancel
        )
        .is_err()
    );
    cancel.store(true, Ordering::Relaxed);
    assert!(
        export::images(
            &root.join("cancel"),
            &s.deck,
            &options,
            ImageFormat::Png,
            &cancel
        )
        .is_err()
    );
    assert!(!root.join("cancel").exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn snapshot_show_skips_hidden_and_source_edits_do_not_change_show() {
    let mut s = sample();
    let before = s.deck.clone();
    let mut show = ShowSnapshot::new(before.clone(), 0).unwrap();
    assert_eq!(show.page(), 0);
    show.next_page();
    assert_eq!(show.page(), 2);
    show.next_page();
    assert_eq!(show.page(), 2);
    show.previous();
    assert_eq!(show.page(), 0);
    show.last();
    assert_eq!(show.page(), 2);
    show.first();
    assert_eq!(show.page(), 0);
    s.execute(s.id, Command::AddPage).unwrap();
    assert!(Arc::ptr_eq(&show.deck, &before));
    assert_eq!(show.deck.slides.len(), 3);
    assert_eq!(s.deck.slides.len(), 4);
}
#[test]
fn pdf_snapshot_notes_and_export_failure_preserve_document_and_files() {
    let s = sample();
    let root = dir();
    let path = root.join("slides.pdf");
    let cancel = AtomicBool::new(false);
    let options = Options {
        pages: vec![0, 2],
        dpi: 72,
        transparent: false,
        notes: false,
    };
    export::pdf(&path, &s.deck, &options, &cancel).unwrap();
    let data = std::fs::read(&path).unwrap();
    assert!(data.starts_with(b"%PDF-1.4"));
    assert!(data.windows(11).any(|w| w == b"/ActualText"));
    assert!(data.windows(9).any(|w| w == b"/Outlines"));
    assert!(export::pdf(&path, &s.deck, &options, &cancel).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), data);
    assert_eq!(s.deck.slides.len(), 3);
    let fp = joydeck::storage::fingerprint(&path).unwrap();
    std::fs::write(&path, b"external").unwrap();
    assert!(export::atomic(&path, b"replacement", Some(fp), &cancel).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"external");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}
