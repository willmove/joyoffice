use joydeck::{
    design::{self, Layer},
    design_commands::{Action, PageTransfer},
    edit::{self, Command, Session},
    workspace::SharedSession,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, atomic::AtomicBool},
};
fn action(s: &mut Session, a: Action) {
    s.execute(s.id, Command::Design(a)).unwrap();
}
fn sample() -> Session {
    let mut s = Session::new();
    action(&mut s, Action::SetDesign(design::starter()));
    action(&mut s, Action::ApplyLayout(0, "layout-1".into(), false));
    let mut objects = design::resolve_slide(&s.deck, 0).unwrap().objects;
    for o in &mut objects {
        o.runs.clear();
        o.text = if o.binding.as_ref().unwrap().kind == "title" {
            "中文 JoyDeck"
        } else {
            "Hello 世界 Hello"
        }
        .into();
    }
    s.execute(s.id, Command::ReplaceMany(0, objects)).unwrap();
    s
}
#[test]
fn cascade_direct_override_reset_and_reapply_preserve_content() {
    let mut s = sample();
    let resolved = design::resolve_slide(&s.deck, 0).unwrap();
    assert_eq!(resolved.objects[0].font_size, 40.);
    assert_eq!(resolved.objects[0].fill, "#087f8c");
    let mut title = resolved.objects[0].clone();
    title.runs.clear();
    title.fill = "#ff0000".into();
    title.x = 100.;
    s.execute(s.id, Command::ReplaceObject(0, 0, title))
        .unwrap();
    let mut d = s.deck.design.clone();
    d.themes
        .get_mut("theme-1")
        .unwrap()
        .colors
        .insert("accent".into(), "#0088ff".into());
    d.masters.get_mut("master-1").unwrap().placeholders[0]
        .style
        .size = Some(50.);
    action(&mut s, Action::SetDesign(d));
    let r = design::resolve_slide(&s.deck, 0).unwrap();
    assert_eq!(r.objects[0].fill, "#ff0000");
    assert_eq!(r.objects[0].x, 100.);
    assert_eq!(r.objects[0].font_size, 50.);
    let content = r.objects[0].text.clone();
    let id = r.objects[0].id.clone();
    action(&mut s, Action::ApplyLayout(0, "layout-1".into(), true));
    let r = design::resolve_slide(&s.deck, 0).unwrap();
    assert_eq!(r.objects[0].fill, "#0088ff");
    assert_eq!(r.objects[0].x, 60.);
    assert_eq!(r.objects[0].text, content);
    assert_eq!(r.objects[0].id, id);
    s.undo();
    assert_eq!(
        design::resolve_slide(&s.deck, 0).unwrap().objects[0].fill,
        "#ff0000"
    );
}
#[test]
fn referenced_design_delete_is_atomic_and_replacement_retains_text() {
    let mut s = sample();
    let before = s.deck.clone();
    assert!(
        s.execute(
            s.id,
            Command::Design(Action::DeleteLayout("layout-1".into(), None))
        )
        .is_err()
    );
    assert!(Arc::ptr_eq(&before, &s.deck));
    let mut d = s.deck.design.clone();
    d.masters.clear();
    assert!(
        s.execute(s.id, Command::Design(Action::SetDesign(d)))
            .is_err()
    );
    assert!(Arc::ptr_eq(&before, &s.deck));
    let mut d = s.deck.design.clone();
    let mut l = d.layouts["layout-1"].clone();
    l.placeholders.clear();
    l.master = "master-1".into();
    d.layouts.insert("layout-2".into(), l);
    action(&mut s, Action::SetDesign(d));
    let previous = design::resolve_slide(&s.deck, 0).unwrap();
    action(
        &mut s,
        Action::DeleteLayout("layout-1".into(), Some("layout-2".into())),
    );
    let r = design::resolve_slide(&s.deck, 0).unwrap();
    assert_eq!(r.objects.len(), 2);
    assert_eq!(r.objects[1].text, previous.objects[1].text);
    assert_eq!(r.objects[1].y, previous.objects[1].y);
    assert!(r.objects[1].binding.is_none());
}
#[test]
fn notes_sections_search_replace_are_single_undo_and_utf8_safe() {
    let mut s = sample();
    action(&mut s, Action::Notes(0, "演讲者备注\n下一步".into()));
    action(
        &mut s,
        Action::Section(vec![0], "section-1".into(), "开场".into()),
    );
    let before = s.deck.clone();
    let hits = joydeck::design_commands::find(&s.deck, "Hello").unwrap();
    assert_eq!(hits.len(), 2);
    action(&mut s, Action::ReplaceAll("Hello".into(), "你好🦀".into()));
    assert_eq!(s.deck.slides[0].objects[1].text, "你好🦀 世界 你好🦀");
    assert!(s.deck.slides[0].objects[1].runs.is_empty());
    s.undo();
    assert_eq!(s.deck, before);
    action(&mut s, Action::DeleteSection("section-1".into()));
    assert_eq!(s.deck.slides.len(), 1);
    assert!(s.deck.slides[0].section.is_none());
    assert_eq!(s.deck.slides[0].notes, "演讲者备注\n下一步");
    s.undo();
    assert!(s.deck.slides[0].section.is_some());
    assert!(joydeck::design_commands::find(&s.deck, "").is_err());
}
#[test]
fn cross_document_page_transfer_remaps_designs_resources_ids_and_theme() {
    let source = sample();
    let mut t = PageTransfer::capture(&source.deck, &[0]).unwrap();
    let original = t.deck.slides[0].objects[0].id.clone();
    t.deck.slides[0].notes = "source notes".into();
    drop(source);
    let mut dest = sample();
    let before = dest.deck.clone();
    action(&mut dest, Action::PastePages(1, t.clone(), None));
    assert_eq!(dest.deck.slides.len(), 2);
    assert_eq!(dest.deck.slides[1].notes, "source notes");
    assert_ne!(dest.deck.slides[1].objects[0].id, original);
    assert_ne!(dest.deck.slides[1].layout, dest.deck.slides[0].layout);
    assert_ne!(dest.deck.slides[1].id, dest.deck.slides[0].id);
    dest.undo();
    assert_eq!(dest.deck, before);
    let mut d = dest.deck.design.clone();
    d.themes
        .get_mut("theme-1")
        .unwrap()
        .colors
        .insert("accent".into(), "#ff8800".into());
    action(&mut dest, Action::SetDesign(d));
    action(&mut dest, Action::PastePages(1, t, Some("theme-1".into())));
    assert_eq!(
        design::resolve_slide(&dest.deck, 1).unwrap().objects[0].fill,
        "#ff8800"
    );
    dest.deck.validate().unwrap();
}
#[test]
fn design_roundtrip_fields_scene_and_clipboard_flattening() {
    let mut s = sample();
    let mut d = s.deck.design.clone();
    d.layouts
        .get_mut("layout-1")
        .unwrap()
        .placeholders
        .push(design::Placeholder {
            kind: "number".into(),
            index: 0,
            style: Layer {
                frame: Some([800., 480., 80., 30.]),
                ..Default::default()
            },
        });
    action(&mut s, Action::SetDesign(d));
    action(&mut s, Action::ApplyLayout(0, "layout-1".into(), false));
    let path =
        std::env::temp_dir().join(format!("joydeck-m6-design-{}.joydeck", std::process::id()));
    edit::save(&path, &s.deck).unwrap();
    let reopened = edit::open_editable(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(*s.deck, reopened);
    let r = design::resolve_slide(&reopened, 0).unwrap();
    assert_eq!(r.objects[2].text, "1");
    let ids = r
        .objects
        .iter()
        .map(|o| o.id.clone())
        .collect::<BTreeSet<_>>();
    let transfer = joydeck::media::Transfer::capture(&reopened, 0, &ids).unwrap();
    assert!(
        transfer
            .objects
            .iter()
            .all(|o| o.binding.is_none() && o.field.is_none())
    );
    transfer.validate().unwrap();
    let cancel = AtomicBool::new(false);
    let cpu = joydeck::preview::render(&reopened, 0, 960, &cancel).unwrap();
    let scene = joydeck::scene::SceneCache::default()
        .build(&reopened, 0, 10, 1, &cancel)
        .unwrap()
        .raster(960, &cancel)
        .unwrap();
    let error = cpu
        .bgra
        .iter()
        .zip(&scene.bgra)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum::<u64>() as f64
        / cpu.bgra.len() as f64;
    assert!(error < 1., "{error}");
}
#[test]
fn shared_document_stale_write_history_and_save_snapshot_guard() {
    let mut a = SharedSession::new();
    let mut b = a.clone();
    a.execute(a.id, Command::AddPage).unwrap();
    assert!(b.execute(b.id, Command::AddPage).is_err());
    assert!(b.sync());
    assert_eq!(b.deck.slides.len(), 2);
    let saved = b.deck.clone();
    let lease = a.try_save().unwrap();
    assert!(b.try_save().is_none());
    b.execute(b.id, Command::AddPage).unwrap();
    a.mark_saved(saved);
    assert!(a.dirty());
    b.sync();
    assert!(b.dirty());
    drop(lease);
    assert!(b.try_save().is_some());
    a.undo();
    assert_eq!(a.deck.slides.len(), 2);
    assert!(!a.dirty());
    b.sync();
    assert_eq!(a.deck, b.deck);
    b.redo();
    a.sync();
    assert_eq!(a.deck.slides.len(), 3);
    let mut other = SharedSession::new();
    assert!(other.execute(a.id, Command::AddPage).is_err());
    assert_eq!(other.deck.slides.len(), 1);
}
#[test]
fn invalid_design_and_paste_never_partially_mutate() {
    let mut s = sample();
    for kind in 0..4 {
        let mut d = s.deck.design.clone();
        match kind {
            0 => d.layouts.get_mut("layout-1").unwrap().master = "missing".into(),
            1 => {
                d.masters.get_mut("master-1").unwrap().placeholders[0]
                    .style
                    .fill = Some("@missing".into())
            }
            2 => {
                d.layouts.get_mut("layout-1").unwrap().placeholders[0]
                    .style
                    .frame = Some([0., 0., f32::NAN, 10.])
            }
            _ => d
                .themes
                .get_mut("theme-1")
                .unwrap()
                .colors
                .insert("accent".into(), "bad".into())
                .map(|_| ())
                .unwrap(),
        };
        let before = s.deck.clone();
        assert!(
            s.execute(s.id, Command::Design(Action::SetDesign(d)))
                .is_err()
        );
        assert!(Arc::ptr_eq(&before, &s.deck));
    }
    let mut transfer = PageTransfer::capture(&s.deck, &[0]).unwrap();
    transfer.version = 99;
    let before = s.deck.clone();
    assert!(
        s.execute(s.id, Command::Design(Action::PastePages(0, transfer, None)))
            .is_err()
    );
    assert!(Arc::ptr_eq(&before, &s.deck));
}

#[test]
fn page_transfer_scales_media_and_preserves_references_after_source_closes() {
    let mut source = sample();
    let image = image::RgbaImage::from_pixel(8, 4, image::Rgba([50, 100, 200, 128]));
    let mut png = std::io::Cursor::new(vec![]);
    image.write_to(&mut png, image::ImageFormat::Png).unwrap();
    let resource = joydeck::media::Resource::import(png.get_ref()).unwrap();
    source
        .execute(source.id, Command::ImportImage(0, None, resource))
        .unwrap();
    let transfer = PageTransfer::capture(&source.deck, &[0]).unwrap();
    drop(source);
    let mut dest = Session::new();
    dest.execute(dest.id, Command::PageSize(480., 270.))
        .unwrap();
    action(&mut dest, Action::PastePages(1, transfer, None));
    assert_eq!(dest.deck.resources.len(), 1);
    let s = design::resolve_slide(&dest.deck, 1).unwrap();
    assert_eq!(s.objects[0].x, 30.);
    assert_eq!(s.objects[0].font_size, 20.);
    assert_eq!(s.objects[2].width, 4.);
    assert_eq!(s.objects[2].height, 2.);
    dest.deck.validate().unwrap();
    dest.undo();
    assert!(dest.deck.resources.is_empty());
    dest.redo();
    assert_eq!(dest.deck.resources.len(), 1);
}
#[test]
fn replace_fonts_updates_theme_and_direct_runs_and_is_undoable() {
    let mut s = sample();
    let before = s.deck.clone();
    action(
        &mut s,
        Action::ReplaceFont("sans-serif".into(), "JoyDeck Missing Font".into()),
    );
    assert_eq!(
        s.deck.design.themes["theme-1"].fonts["body"],
        "JoyDeck Missing Font"
    );
    assert!(
        s.deck.slides[0]
            .objects
            .iter()
            .flat_map(|o| &o.runs)
            .all(|r| r.style.family == "JoyDeck Missing Font")
    );
    s.undo();
    assert_eq!(s.deck, before);
}

#[test]
fn new_page_has_no_copied_notes_and_unbound_fields_are_rejected() {
    let mut s = sample();
    action(&mut s, Action::Notes(0, "private note".into()));
    action(
        &mut s,
        Action::Section(vec![0], "intro".into(), "Intro".into()),
    );
    s.execute(s.id, Command::AddPage).unwrap();
    assert!(s.deck.slides[1].notes.is_empty());
    assert!(s.deck.slides[1].section.is_none());
    assert_eq!(s.deck.slides[0].notes, "private note");
    let mut o = edit::object("text");
    o.field = Some("number".into());
    let before = s.deck.clone();
    assert!(s.execute(s.id, Command::AddObject(1, o)).is_err());
    assert!(Arc::ptr_eq(&before, &s.deck));
}

#[test]
fn transfer_avoids_source_identity_and_animation_target_cascades() {
    let source = sample();
    let mut source_deck = (*source.deck).clone();
    source_deck.slides[0].objects[0].id = "object-6".into();
    source_deck.slides[0].objects[1].id = "object-7".into();
    let page = source_deck.slides[0].id.clone();
    source_deck.presentation.slides.insert(
        page,
        joydeck::motion::Timing {
            effects: vec![
                joydeck::motion::Effect {
                    id: "first-effect".into(),
                    target: "object-6".into(),
                    ..Default::default()
                },
                joydeck::motion::Effect {
                    id: "second-effect".into(),
                    target: "object-7".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
    );
    let transfer = PageTransfer::capture(&source_deck, &[0]).unwrap();
    let mut destination = (*source.deck).clone();
    destination.slides[0].objects[0].id = "destination-first".into();
    destination.slides[0].objects[1].id = "destination-second".into();
    let mut target = Session::from_deck(destination, true);
    action(&mut target, Action::PastePages(1, transfer, None));
    let copied = &target.deck.slides[1];
    let ids = copied
        .objects
        .iter()
        .map(|o| o.id.clone())
        .collect::<Vec<_>>();
    assert!(ids.iter().all(|id| id != "object-6" && id != "object-7"));
    assert_ne!(ids[0], ids[1]);
    let timing = &target.deck.presentation.slides[&copied.id];
    assert_eq!(timing.effects[0].target, ids[0]);
    assert_eq!(timing.effects[1].target, ids[1]);
}
