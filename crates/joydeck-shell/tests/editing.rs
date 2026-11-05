#![allow(clippy::unwrap_used, clippy::expect_used, reason = "Test assertions")]
use joydeck_engine::Command;
use joydeck_shell::{Cancellation, Document, editor::Editor, storage::Recovery};
use std::sync::Arc;
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
    path::PathBuf,
};
const PPTX: &[u8] = include_bytes!("../../../tests/fixtures/joydeck/independent.pptx");
fn work() -> PathBuf {
    joydeck_shell::platform::private_directory("r2-tests").unwrap()
}
fn editor(root: &std::path::Path) -> Editor {
    let source = root.join("原稿 test.pptx");
    std::fs::write(&source, PPTX).unwrap();
    Editor::open(&source, root.join("recovery")).unwrap()
}
fn parts(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().unwrap().into_owned();
        let mut b = vec![];
        f.read_to_end(&mut b).unwrap();
        parts.insert(name, b);
    }
    parts
}
fn zip(parts: BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(vec![]));
    for (n, b) in parts {
        z.start_file(n, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(&b).unwrap();
    }
    z.finish().unwrap().into_inner()
}
#[test]
fn prepared_open_uses_the_parsed_file_fingerprint_without_publishing_recent() {
    let root = work();
    let recovery = root.join("recovery");
    let path = root.join("document.pptx");
    std::fs::write(&path, PPTX).unwrap();
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(
        Editor::prepare_open(&path, recovery.clone(), &cancel)
            .err()
            .as_deref(),
        Some("operation cancelled")
    );
    let e = Editor::prepare_open(&path, recovery.clone(), &Cancellation::default()).unwrap();
    assert_eq!(
        e.fingerprint,
        Some(joydeck_shell::storage::fingerprint(&path).unwrap())
    );
    assert!(
        joydeck_shell::storage::recent_files(&recovery)
            .unwrap()
            .is_empty()
    );
    Editor::open(&path, recovery.clone()).unwrap();
    assert_eq!(
        joydeck_shell::storage::recent_files(&recovery).unwrap(),
        vec![path.canonicalize().unwrap()]
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn text_transform_save_reopen_and_untouched_parts_preserved() {
    let root = work();
    let mut e = editor(&root);
    let old = e.session.snapshot();
    e.session
        .select(0, vec![joydeck_model::ShapeId(2)])
        .unwrap();
    e.session
        .execute(Command::ReplaceText {
            text: "JoyDeck R2 中文 😀\n".into(),
        })
        .unwrap();
    e.session
        .execute(Command::Transform {
            dx: 12.,
            dy: 6.,
            dw: 0.,
            dh: 0.,
            rotation: 15.,
        })
        .unwrap();
    let path = root.join("已编辑.pptx");
    let saved = e.save_job(path.clone()).unwrap().run().unwrap();
    e.apply_saved(saved);
    assert!(!e.session.dirty());
    let read = Document::open(&path, &Cancellation::default()).unwrap();
    assert_eq!(
        read.presentation.slides[0].shapes[0]
            .text
            .as_ref()
            .unwrap()
            .text(),
        "JoyDeck R2 中文 😀\n"
    );
    assert_eq!(read.presentation.slides[0].shapes[0].xfrm.unwrap().rot, 15.);
    let before = parts(PPTX);
    let after = parts(&std::fs::read(path).unwrap());
    for (name, bytes) in before {
        if name != "ppt/slides/slide1.xml" {
            assert_eq!(after[&name], bytes, "untouched {name}");
        }
    }
    assert_eq!(std::fs::read(root.join("原稿 test.pptx")).unwrap(), PPTX);
    assert_eq!(
        old.model.slides[0].shapes[0].text.as_ref().unwrap().text(),
        "Independent PPTX / 独立样本"
    );
}
#[test]
fn failed_transactions_do_not_mutate_history_ids_or_model() {
    let root = work();
    let mut e = editor(&root);
    e.session
        .select(0, vec![joydeck_model::ShapeId(2)])
        .unwrap();
    let before = e.session.snapshot();
    for command in [
        Command::Transform {
            dx: f64::NAN,
            dy: 0.,
            dw: 0.,
            dh: 0.,
            rotation: 0.,
        },
        Command::Transform {
            dx: 0.,
            dy: 0.,
            dw: -9999.,
            dh: 0.,
            rotation: 0.,
        },
        Command::InsertText {
            x: 0.,
            y: 0.,
            text: "x".repeat(65537),
        },
    ] {
        assert!(e.session.execute(command).is_err());
        assert_eq!(e.session.model(), before.model.as_ref());
        assert!(!e.session.can_undo());
        assert!(!e.session.dirty());
        assert_eq!(e.session.revision(), 0);
    }
}
#[test]
fn undo_redo_branches_dirty_and_selection_restore() {
    let root = work();
    let mut e = editor(&root);
    let original = e.session.snapshot();
    e.session
        .execute(Command::InsertRect {
            x: 10.,
            y: 20.,
            width: 50.,
            height: 60.,
        })
        .unwrap();
    let inserted = e.session.selection().shapes[0];
    assert!(e.session.dirty());
    e.session.execute(Command::Undo).unwrap();
    assert_eq!(e.session.model(), original.model.as_ref());
    assert!(!e.session.dirty());
    e.session.execute(Command::Redo).unwrap();
    assert_eq!(e.session.selection().shapes, vec![inserted]);
    e.session.execute(Command::Undo).unwrap();
    e.session
        .execute(Command::InsertText {
            x: 0.,
            y: 0.,
            text: "branch".into(),
        })
        .unwrap();
    assert!(!e.session.can_redo());
}
#[test]
fn save_captures_revision_and_later_edits_stay_dirty() {
    let root = work();
    let mut e = editor(&root);
    e.session
        .execute(Command::InsertRect {
            x: 10.,
            y: 20.,
            width: 50.,
            height: 60.,
        })
        .unwrap();
    let job = e.save_job(root.join("snapshot.joydeck")).unwrap();
    e.session
        .execute(Command::Transform {
            dx: 24.,
            dy: 0.,
            dw: 0.,
            dh: 0.,
            rotation: 0.,
        })
        .unwrap();
    e.apply_saved(job.run().unwrap());
    assert!(e.session.dirty());
    let read = Document::open(&root.join("snapshot.joydeck"), &Cancellation::default()).unwrap();
    assert_eq!(
        read.presentation.slides[0]
            .shapes
            .last()
            .unwrap()
            .xfrm
            .unwrap()
            .x,
        10.
    );
    e.session.execute(Command::Undo).unwrap();
    assert!(!e.session.dirty());
    e.session.execute(Command::Redo).unwrap();
    assert!(e.session.dirty());
}
#[test]
fn external_changes_and_existing_save_as_targets_are_not_overwritten() {
    let root = work();
    let e = editor(&root);
    let path = e.source.clone().unwrap();
    let job = e.save_job(path.clone()).unwrap();
    std::fs::write(&path, b"external change").unwrap();
    assert!(job.run().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"external change");
    let dest = root.join("existing.pptx");
    std::fs::write(&dest, b"retain me").unwrap();
    assert!(e.save_job(dest.clone()).unwrap().run().is_err());
    assert_eq!(std::fs::read(dest).unwrap(), b"retain me");
}
#[test]
fn concurrent_saves_from_one_fingerprint_publish_only_one_result() {
    use joydeck_shell::storage::{fingerprint, save};
    use std::sync::Barrier;

    let root = work();
    let path = root.join("shared.joydeck");
    let original = vec![0; 2 * 1024 * 1024];
    std::fs::write(&path, &original).unwrap();
    let expected = fingerprint(&path).unwrap();
    let writes = [vec![1; original.len()], vec![2; original.len()]];
    let ready = Barrier::new(writes.len());
    let results = std::thread::scope(|scope| {
        let jobs = writes.each_ref().map(|bytes| {
            let expected = expected.clone();
            let path = &path;
            let ready = &ready;
            scope.spawn(move || {
                ready.wait();
                save(path, bytes, Some(expected))
            })
        });
        jobs.map(|job| job.join().unwrap())
    });
    let [success] = results
        .iter()
        .enumerate()
        .filter_map(|(index, result)| result.is_ok().then_some(index))
        .collect::<Vec<_>>()[..]
    else {
        panic!("expected exactly one published save, got {results:?}");
    };
    assert_eq!(std::fs::read(&path).unwrap(), writes[success]);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_destination_retains_original_and_session_dirty() {
    let root = work();
    let mut e = editor(&root);
    e.session
        .execute(Command::InsertRect {
            x: 10.,
            y: 10.,
            width: 50.,
            height: 50.,
        })
        .unwrap();
    let bad = root.join("not-a-directory");
    std::fs::write(&bad, b"file").unwrap();
    assert!(e.save_job(bad.join("x.pptx")).unwrap().run().is_err());
    assert!(e.session.dirty());
    assert_eq!(std::fs::read(e.source.as_ref().unwrap()).unwrap(), PPTX);
}
#[test]
fn native_v2_round_trip_restores_source_binding_and_rejects_legacy_reader() {
    let root = work();
    let mut e = editor(&root);
    e.session
        .execute(Command::InsertRect {
            x: 10.,
            y: 10.,
            width: 50.,
            height: 50.,
        })
        .unwrap();
    let path = root.join("project.joydeck");
    e.save_job(path.clone()).unwrap().run().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(joydeck_format::load(&bytes).is_err());
    let mut loaded = Editor::open(&path, root.join("other-recovery")).unwrap();
    assert_eq!(loaded.base.original.as_ref(), PPTX);
    loaded.session.execute(Command::Undo).unwrap();
    assert!(!loaded.session.can_undo());
    loaded
        .save_job(root.join("roundtrip.pptx"))
        .unwrap()
        .run()
        .unwrap();
    assert_eq!(
        Document::open(&root.join("roundtrip.pptx"), &Cancellation::default())
            .unwrap()
            .presentation
            .slides[0]
            .shapes
            .len(),
        5
    );
    let mut p = parts(&bytes);
    let mut manifest: serde_json::Value = serde_json::from_slice(&p["manifest.json"]).unwrap();
    manifest["required_features"]
        .as_array_mut()
        .unwrap()
        .push("future-editing-v99".into());
    p.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(joydeck_shell::native::load(&zip(p), &Cancellation::default()).is_err());
}
#[test]
fn simple_slide_structure_and_picture_copy_round_trip_in_public_pptx() {
    let root = work();
    let mut e = editor(&root);
    e.session.select(1, vec![]).unwrap();
    e.session.execute(Command::DuplicateSlide).unwrap();
    e.session.execute(Command::NewSlide).unwrap();
    e.session
        .execute(Command::InsertText {
            x: 80.,
            y: 80.,
            text: "new slide".into(),
        })
        .unwrap();
    e.session.select(0, vec![]).unwrap();
    e.session.execute(Command::DeleteSlide).unwrap();
    let path = root.join("slides.pptx");
    e.save_job(path.clone()).unwrap().run().unwrap();
    let read = Document::open(&path, &Cancellation::default()).unwrap();
    assert_eq!(read.presentation.slides.len(), 4);
    assert_eq!(read.presentation.media.len(), 1);
    assert_eq!(
        read.presentation.slides[2].shapes[0]
            .text
            .as_ref()
            .unwrap()
            .text(),
        "new slide"
    );
    assert_eq!(
        read.presentation.slides[1].notes.text(),
        "Original independent speaker notes / 备注 2"
    );
}
#[test]
fn recovery_leases_corruption_and_restore_without_overwriting_source() {
    let root = work();
    let recovery_root = root.join("recovery");
    let observer = Recovery::new(recovery_root.clone());
    {
        let mut e = editor(&root);
        e.session
            .select(0, vec![joydeck_model::ShapeId(2)])
            .unwrap();
        e.session
            .execute(Command::ReplaceText {
                text: "recover this".into(),
            })
            .unwrap();
        e.checkpoint_job().checkpoint().unwrap();
        assert!(observer.list().unwrap()[0].active);
    }
    let rows = observer.list().unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].active);
    assert!(rows[0].problem.is_none());
    let recovered = Editor::restore(recovery_root.clone(), &rows[0].record).unwrap();
    assert!(recovered.session.dirty());
    assert!(recovered.source.is_none());
    assert_eq!(
        recovered.session.model().slides[0].shapes[0]
            .text
            .as_ref()
            .unwrap()
            .text(),
        "recover this"
    );
    let bad = Recovery::new(recovery_root.clone());
    bad.write(5, &b"broken".to_vec(), None, None).unwrap();
    drop(bad);
    let rows = observer.list().unwrap();
    assert!(rows.iter().any(|r| r.problem.is_some()));
    assert!(rows.iter().any(|r| r.problem.is_none()));
    assert_eq!(std::fs::read(root.join("原稿 test.pptx")).unwrap(), PPTX);
}
#[test]
fn recovery_clean_blocks_stale_checkpoints() {
    let root = work();
    let mut e = editor(&root);
    e.session
        .execute(Command::InsertRect {
            x: 1.,
            y: 1.,
            width: 20.,
            height: 20.,
        })
        .unwrap();
    let old = e.checkpoint_job();
    e.recovery.clean(99).unwrap();
    old.checkpoint().unwrap();
    assert!(e.recovery.list().unwrap().is_empty());
}
#[test]
fn unknown_native_parts_survive_resave() {
    let root = work();
    let e = editor(&root);
    let bytes = e
        .save_job(root.join("x.joydeck"))
        .unwrap()
        .native_bytes()
        .unwrap();
    let mut p = parts(&bytes);
    let mut manifest: serde_json::Value = serde_json::from_slice(&p["manifest.json"]).unwrap();
    manifest["vendor_optional"] = serde_json::json!({"keep":true});
    p.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    p.insert(
        "vendor/unknown.bin".into(),
        b"keep original unknown payload".to_vec(),
    );
    let decorated = zip(p);
    let loaded = joydeck_shell::native::load(&decorated, &Cancellation::default()).unwrap();
    let state =
        joydeck_engine::Session::new(loaded.model, loaded.manifest.origins, true).snapshot();
    let encoded = joydeck_shell::native::encode(
        &state,
        &loaded.source,
        &loaded.manifest.source_format,
        Some(&decorated),
    )
    .unwrap();
    assert_eq!(
        parts(&encoded)["vendor/unknown.bin"],
        b"keep original unknown payload"
    );
    let entries = parts(&encoded);
    let manifest: serde_json::Value = serde_json::from_slice(&entries["manifest.json"]).unwrap();
    assert_eq!(manifest["vendor_optional"]["keep"], true);
    assert!(entries.contains_key("retained-original-model.deckcraft"));
}

#[test]
fn history_budget_keeps_only_latest_128_gestures() {
    let root = work();
    let mut e = editor(&root);
    e.session
        .select(0, vec![joydeck_model::ShapeId(2)])
        .unwrap();
    for _ in 0..200 {
        e.session
            .execute(Command::Transform {
                dx: 1.,
                dy: 0.,
                dw: 0.,
                dh: 0.,
                rotation: 0.,
            })
            .unwrap();
    }
    let mut undone = 0;
    while e.session.execute(Command::Undo).unwrap() {
        undone += 1;
    }
    assert_eq!(undone, 128);
    assert!(e.session.dirty());
}
#[test]
fn id_exhaustion_and_locked_objects_fail_atomically() {
    let p = joydeck_model::Presentation {
        next_id: u32::MAX,
        ..Default::default()
    };
    let bindings = joydeck_engine::original_bindings(&p);
    let mut session = joydeck_engine::Session::new(p, bindings, true);
    let old = session.snapshot();
    assert!(
        session
            .execute(Command::InsertRect {
                x: 1.,
                y: 1.,
                width: 40.,
                height: 40.
            })
            .is_err()
    );
    assert_eq!(session.model(), old.model.as_ref());
    assert!(!session.can_undo());
    let root = work();
    let mut e = editor(&root);
    let mut p = e.session.model().clone();
    Arc::make_mut(&mut p.slides[0]).shapes[0].locked = true;
    let bindings = joydeck_engine::original_bindings(&p);
    e.session = joydeck_engine::Session::new(p, bindings, true);
    e.session
        .select(0, vec![joydeck_model::ShapeId(2)])
        .unwrap();
    assert!(e.session.execute(Command::Delete).is_err());
    assert!(!e.session.dirty());
}
#[test]
fn published_save_is_reported_as_success_even_if_recovery_cleanup_fails() {
    let root = work();
    let mut e = editor(&root);
    let bad = root.join("not-a-recovery-directory");
    std::fs::write(&bad, b"keep").unwrap();
    e.recovery = Recovery::new(bad);
    let path = root.join("published.pptx");
    let saved = e.save_job(path.clone()).unwrap().run().unwrap();
    assert!(saved.warning.is_some());
    e.apply_saved(saved);
    assert!(!e.session.dirty());
    assert!(Document::open(&path, &Cancellation::default()).is_ok());
}

#[cfg(windows)]
#[test]
fn windows_atomic_replace_failure_preserves_existing_file_and_dirty_state() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = work();
    let mut e = editor(&root);
    let source = e.source.clone().unwrap();
    e.session
        .execute(Command::InsertRect {
            x: 10.,
            y: 10.,
            width: 40.,
            height: 40.,
        })
        .unwrap();
    let _held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&source)
        .unwrap();
    assert!(e.save_job(source.clone()).unwrap().run().is_err());
    assert_eq!(std::fs::read(source).unwrap(), PPTX);
    assert!(e.session.dirty());
}
