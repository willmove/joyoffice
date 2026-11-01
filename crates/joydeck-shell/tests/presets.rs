use joydeck_model::{identity, preset};
use joydeck_shell::{
    document,
    edit::{Command, Session},
    interop, pptx, pptx_write,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
};

fn public_parts(deck: &document::Deck) -> BTreeMap<String, Vec<u8>> {
    let mut parts = pptx_write::generate(deck).unwrap();
    // Deliberately invalidate the private snapshot to exercise public DrawingML.
    parts.get_mut("ppt/slides/slide1.xml").unwrap().push(b' ');
    assert!(pptx_write::read_snapshot(&parts).unwrap().is_none());
    parts
}

#[test]
fn entire_new_catalog_roundtrips_through_public_pptx_and_native() {
    let mut deck = identity::blank();
    for name in preset::names().filter(|n| {
        ![
            "rect",
            "ellipse",
            "roundRect",
            "rightArrow",
            "line",
            "textBox",
        ]
        .contains(n)
    }) {
        let mut o = preset::object(name).unwrap();
        o.id = format!("preset-{name}");
        if let Some(values) = preset::defaults(name) {
            o.preset.as_mut().unwrap().adjustments = values.to_vec();
        }
        deck.slides[0].objects.push(o);
    }
    deck.validate().unwrap();
    let parts = public_parts(&deck);
    let imported = pptx::import(&parts, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        imported.slides[0].objects.len(),
        deck.slides[0].objects.len()
    );
    for (a, b) in imported.slides[0]
        .objects
        .iter()
        .zip(&deck.slides[0].objects)
    {
        assert_eq!(a.preset, b.preset);
    }
    let bytes = joydeck_formats::native::encode(&deck).unwrap();
    let parts =
        joydeck_formats::document::read_package_bytes(&bytes, &AtomicBool::new(false)).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&parts["manifest.json"]).unwrap();
    assert_eq!(manifest["minReader"], 12);
    assert_eq!(manifest["schemaMinor"], 11);
    let roundtrip =
        joydeck_formats::document::decode_parts(&parts, &AtomicBool::new(false)).unwrap();
    assert_eq!(deck, roundtrip);
    let mut lying_manifest = parts.clone();
    let mut manifest = manifest;
    manifest["minReader"] = serde_json::json!(11);
    lying_manifest.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(
        joydeck_formats::document::decode_parts(&lying_manifest, &AtomicBool::new(false)).is_err()
    );
    let old = joydeck_formats::native::encode(&identity::blank()).unwrap();
    let old = joydeck_formats::document::read_package_bytes(&old, &AtomicBool::new(false)).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&old["manifest.json"]).unwrap();
    assert_eq!(manifest["minReader"], 11);
}

#[test]
fn parameter_changes_are_one_undoable_transaction_and_validate_atomically() {
    let mut session = Session::new();
    session
        .execute(
            session.id,
            Command::AddObject(0, preset::object("donut").unwrap()),
        )
        .unwrap();
    let before = session.deck.clone();
    let adjusted = preset::adjust(&before.slides[0].objects[0], 0, 1.).unwrap();
    session
        .execute(session.id, Command::ReplaceMany(0, vec![adjusted]))
        .unwrap();
    let after = session.deck.clone();
    assert_ne!(before, after);
    session.undo();
    assert_eq!(before, session.deck);
    session.redo();
    assert_eq!(after, session.deck);
    let mut bad = after.slides[0].objects[0].clone();
    bad.preset.as_mut().unwrap().adjustments.push(1.);
    assert!(
        session
            .execute(session.id, Command::ReplaceMany(0, vec![bad]))
            .is_err()
    );
    assert!(Arc::ptr_eq(&after, &session.deck));
}

#[test]
fn unknown_geometry_and_nonliteral_adjusts_are_reported_and_source_is_preserved() {
    let mut deck = identity::blank();
    deck.slides[0]
        .objects
        .push(preset::object("triangle").unwrap());
    let mut parts = public_parts(&deck);
    let slide = String::from_utf8(parts["ppt/slides/slide1.xml"].clone()).unwrap();
    let slide = slide.replace(
        "<a:avLst/>",
        r#"<a:avLst><a:gd name="adj" fmla="*/ w 1 2"/></a:avLst>"#,
    );
    // The writer uses paired avLst tags for presets.
    let slide = slide.replace(
        "<a:avLst></a:avLst>",
        r#"<a:avLst><a:gd name="adj" fmla="*/ w 1 2"/></a:avLst>"#,
    );
    parts.insert("ppt/slides/slide1.xml".into(), slide.into_bytes());
    let d = pptx::import(&parts, &AtomicBool::new(false)).unwrap();
    assert!(d.warnings.iter().any(|w| w.code == "preset-unsupported"));
    assert_eq!(d.slides[0].objects[0].kind, "placeholder");
    let mut d = Session::from_deck(d, false).deck.as_ref().clone();
    let source = interop::SourcePackage {
        parts: parts.clone(),
        baseline: serde_json::to_vec(&d).unwrap(),
        text: vec![],
    };
    d.source = Some(Arc::new(source));
    assert_eq!(d.source.as_ref().unwrap().preserve(&d).unwrap(), parts);
    d.slides[0].objects[0].x += 1.;
    assert!(d.source.as_ref().unwrap().preserve(&d).is_err());
}

#[test]
fn donut_scene_and_export_keep_the_hole_and_use_the_same_path() {
    let mut deck = identity::blank();
    let mut o = preset::object("donut").unwrap();
    o.x = 20.;
    o.y = 20.;
    o.width = 100.;
    o.height = 100.;
    deck.slides[0].objects.push(o);
    let cancel = AtomicBool::new(false);
    let image = joydeck_export::export::raster(&deck, 0, 72, false, &cancel).unwrap();
    assert_eq!(image.get_pixel(70, 70).0, [255, 255, 255, 255]);
    assert_eq!(image.get_pixel(30, 70).0, [39, 182, 167, 255]);
    let svg = joydeck_scene::preview::svg(&deck, 0, &cancel).unwrap();
    assert!(svg.contains("fill-rule=\"evenodd\""));
    assert!(preset::hit(&deck.slides[0].objects[0], 10., 50.));
    assert!(!preset::hit(&deck.slides[0].objects[0], 50., 50.));
    let mut cache = joydeck_scene::scene::SceneCache::default();
    let scene = cache.build(&deck, 0, 1, 0, &cancel).unwrap();
    assert!(!scene.nodes.is_empty());
}

#[test]
fn unfilled_rotated_preset_keeps_stroke_and_does_not_hit_its_empty_interior() {
    let mut deck = identity::blank();
    let mut object = preset::object("triangle").unwrap();
    object.rotation = 37.;
    object.stroke_width = 3.;
    object.stroke = "#3311aa".into();
    object.preset.as_mut().unwrap().no_fill = true;
    assert!(!preset::hit(&object, object.width / 2., object.height / 2.));
    deck.slides[0].objects.push(object.clone());
    let parts = public_parts(&deck);
    let imported = pptx::import(&parts, &AtomicBool::new(false)).unwrap();
    let actual = &imported.slides[0].objects[0];
    assert_eq!(actual.preset, object.preset);
    assert_eq!(actual.rotation, object.rotation);
    assert_eq!(actual.stroke_width, object.stroke_width);
    assert_eq!(actual.stroke, object.stroke);
}

#[test]
fn preview_path_budget_rejects_expensive_page_without_mutating_document() {
    let mut deck = identity::blank();
    let object = preset::object("star32").unwrap();
    deck.slides[0].objects = vec![object; 2000];
    let before = serde_json::to_vec(&deck).unwrap();
    let error = joydeck_scene::preview::svg(&deck, 0, &AtomicBool::new(false)).unwrap_err();
    assert!(error.contains("100000"));
    assert_eq!(serde_json::to_vec(&deck).unwrap(), before);
}

#[test]
fn public_textbox_alias_is_written_as_valid_rect_geometry() {
    let mut deck = identity::blank();
    deck.slides[0]
        .objects
        .push(preset::object("textBox").unwrap());
    let parts = public_parts(&deck);
    let xml = std::str::from_utf8(&parts["ppt/slides/slide1.xml"]).unwrap();
    assert!(xml.contains("prst=\"rect\""));
    assert!(!xml.contains("prst=\"textBox\""));
}

#[test]
fn retained_source_baseline_keeps_feature_floor_after_last_visible_preset_is_deleted() {
    let mut deck = identity::blank();
    deck.slides[0]
        .objects
        .push(preset::object("triangle").unwrap());
    let source = interop::SourcePackage {
        parts: public_parts(&deck),
        baseline: serde_json::to_vec(&deck).unwrap(),
        text: vec![],
    };
    deck.source = Some(Arc::new(source));
    deck.slides[0].objects.clear();
    let bytes = joydeck_formats::native::encode(&deck).unwrap();
    let mut parts =
        joydeck_formats::document::read_package_bytes(&bytes, &AtomicBool::new(false)).unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&parts["manifest.json"]).unwrap();
    assert_eq!(manifest["minReader"], 12);
    joydeck_formats::document::decode_parts(&parts, &AtomicBool::new(false)).unwrap();
    manifest["minReader"] = serde_json::json!(11);
    parts.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(joydeck_formats::document::decode_parts(&parts, &AtomicBool::new(false)).is_err());
}
