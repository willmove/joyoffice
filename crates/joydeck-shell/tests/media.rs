extern crate joydeck_shell as joydeck;
use joydeck::{
    edit::{self, Command, Session},
    media::{Resource, Transfer},
    scene::SceneCache,
};
use std::{
    collections::BTreeSet,
    io::Cursor,
    sync::{Arc, atomic::AtomicBool},
};
fn resource() -> Resource {
    let image = image::RgbaImage::from_fn(40, 20, |x, y| {
        image::Rgba([
            if x < 20 { 255 } else { 0 },
            if x >= 20 { 255 } else { 0 },
            y as u8 * 10,
            if y < 10 { 255 } else { 128 },
        ])
    });
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    Resource::import(out.get_ref()).unwrap()
}
#[test]
fn import_crop_dedup_undo_transfer_and_resource_collection() {
    let mut s = Session::new();
    let r = resource();
    for _ in 0..2 {
        s.execute(s.id, Command::ImportImage(0, None, r.clone()))
            .unwrap();
    }
    assert_eq!(s.deck.resources.len(), 1);
    let mut o = s.deck.slides[0].objects[0].clone();
    o.image.as_mut().unwrap().crop = [0.5, 0., 0.5, 1.];
    s.execute(s.id, Command::ReplaceObject(0, 0, o.clone()))
        .unwrap();
    let before = s.deck.clone();
    o.image.as_mut().unwrap().crop = [0.9, 0., 0.5, 1.];
    assert!(s.execute(s.id, Command::ReplaceObject(0, 0, o)).is_err());
    assert!(Arc::ptr_eq(&before, &s.deck));
    let ids = s.deck.slides[0]
        .objects
        .iter()
        .map(|o| o.id.clone())
        .collect::<BTreeSet<_>>();
    let transfer = Transfer::capture(&s.deck, 0, &ids).unwrap();
    s.execute(
        s.id,
        Command::Batch(vec![
            Command::DeleteObject(0, 1),
            Command::DeleteObject(0, 0),
        ]),
    )
    .unwrap();
    assert!(s.deck.resources.is_empty());
    s.undo();
    assert_eq!(s.deck.resources.len(), 1);
    s.redo();
    drop(s);
    let mut dest = Session::new();
    dest.execute(dest.id, Command::Paste(0, transfer)).unwrap();
    assert_eq!(dest.deck.resources.len(), 1);
    assert_eq!(dest.deck.slides[0].objects.len(), 2);
    assert_ne!(
        dest.deck.slides[0].objects[0].id,
        dest.deck.slides[0].objects[1].id
    );
    dest.undo();
    assert!(dest.deck.resources.is_empty());
    dest.redo();
    let mut invalid = Transfer::capture(
        &dest.deck,
        0,
        &dest.deck.slides[0]
            .objects
            .iter()
            .map(|o| o.id.clone())
            .collect(),
    )
    .unwrap();
    invalid.version = 2;
    assert!(dest.execute(dest.id, Command::Paste(0, invalid)).is_err());
}
#[test]
fn native_resource_roundtrip_and_cpu_scene_pixels() {
    let mut s = Session::new();
    s.execute(s.id, Command::ImportImage(0, None, resource()))
        .unwrap();
    let mut o = s.deck.slides[0].objects[0].clone();
    o.image.as_mut().unwrap().crop = [0.5, 0., 0.5, 1.];
    o.width = 100.;
    o.height = 100.;
    s.execute(s.id, Command::ReplaceObject(0, 0, o)).unwrap();
    let path = std::env::temp_dir().join(format!("joydeck-m5-{}.joydeck", std::process::id()));
    edit::save(&path, &s.deck).unwrap();
    let reopened = edit::open_editable(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(*s.deck, reopened);
    let cancel = AtomicBool::new(false);
    let cpu = joydeck::preview::render(&reopened, 0, 960, &cancel).unwrap();
    let scene = SceneCache::default()
        .build(&reopened, 0, 1, 1, &cancel)
        .unwrap()
        .raster(960, &cancel)
        .unwrap();
    let sample = (100 * 960 + 100) * 4;
    assert!(
        cpu.bgra[sample + 1] > 200 && cpu.bgra[sample + 2] < 30,
        "crop should show green, not red: {:?}",
        &cpu.bgra[sample..sample + 4]
    );
    let error = cpu
        .bgra
        .iter()
        .zip(&scene.bgra)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum::<u64>() as f64
        / cpu.bgra.len() as f64;
    assert!(error < 1., "CPU/Scene mean error {error}");
}
#[test]
fn malformed_oversize_missing_hash_and_crop_rejected() {
    assert!(Resource::import(b"not an image").is_err());
    assert!(Resource::import(&vec![0; 8 * 1024 * 1024 + 1]).is_err());
    let mut s = Session::new();
    s.execute(s.id, Command::ImportImage(0, None, resource()))
        .unwrap();
    for crop in [
        [f32::NAN, 0., 1., 1.],
        [0., 0., 0., 1.],
        [-0.1, 0., 1., 1.],
        [0., 0., 2., 1.],
    ] {
        let mut o = s.deck.slides[0].objects[0].clone();
        o.image.as_mut().unwrap().crop = crop;
        assert!(s.execute(s.id, Command::ReplaceObject(0, 0, o)).is_err());
    }
    let mut d = s.deck.as_ref().clone();
    d.resources.clear();
    assert!(d.validate().is_err());
    let mut d = s.deck.as_ref().clone();
    let r = d.resources.pop_first().unwrap().1;
    d.resources.insert("wrong-hash".into(), r);
    assert!(d.validate().is_err());
}

#[test]
fn arrange_uses_rotated_bounds_and_single_undo() {
    use joydeck::geometry::{self, Arrange};
    let mut s = Session::new();
    for (x, angle) in [(20., 30.), (140., 0.), (400., -45.)] {
        let mut o = edit::object("rect");
        o.x = x;
        o.rotation = angle;
        s.execute(s.id, Command::AddObject(0, o)).unwrap();
    }
    let before = s.deck.clone();
    let aligned = geometry::arrange(&s.deck.slides[0].objects, Arrange::Left).unwrap();
    let x = geometry::bounds(&aligned[0]).x;
    assert!(
        aligned
            .iter()
            .all(|o| (geometry::bounds(o).x - x).abs() < 0.001)
    );
    s.execute(s.id, Command::ReplaceMany(0, aligned)).unwrap();
    s.undo();
    assert_eq!(s.deck, before);
    let distributed = geometry::arrange(&s.deck.slides[0].objects, Arrange::Horizontal).unwrap();
    let a = geometry::bounds(&distributed[0]);
    let b = geometry::bounds(&distributed[1]);
    let c = geometry::bounds(&distributed[2]);
    assert!(((b.x - a.x - a.w) - (c.x - b.x - b.w)).abs() < 0.001);
    assert!(geometry::arrange(&distributed[..2], Arrange::Horizontal).is_err());
}

#[test]
fn jpeg_exif_orientation_and_resource_recovery() {
    let image = image::RgbImage::from_pixel(30, 10, image::Rgb([60, 120, 200]));
    let mut jpeg = Cursor::new(Vec::new());
    image.write_to(&mut jpeg, image::ImageFormat::Jpeg).unwrap();
    // APP1 Exif / big-endian TIFF: orientation = 6 (90° clockwise).
    let exif = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0";
    let mut bytes = vec![0xff, 0xd8, 0xff, 0xe1];
    bytes.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    bytes.extend_from_slice(exif);
    bytes.extend_from_slice(&jpeg.get_ref()[2..]);
    let r = Resource::import(&bytes).unwrap();
    assert_eq!((r.width, r.height), (10, 30));
    let mut invalid = r.clone();
    invalid.width = 11;
    assert!(invalid.validate().is_err());
    let mut s = Session::new();
    s.execute(s.id, Command::ImportImage(0, None, r)).unwrap();
    let root = std::env::temp_dir().join(format!("joydeck-m5-recovery-{}", std::process::id()));
    let recovery = joydeck::storage::Recovery::new(root.clone());
    recovery.write(s.revision, &s.deck, None, None).unwrap();
    let entry = recovery.list().unwrap().remove(0);
    assert_eq!(recovery.restore(&entry.record).unwrap(), *s.deck);
    recovery.discard().unwrap();
    drop(recovery);
    std::fs::remove_dir_all(root).unwrap();
}
