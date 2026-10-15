//! An export plan records the exact original package it was built from, so a
//! storage layer can refuse to write a plan over a different source. The
//! binding is byte-exact and private to the plan; versions never substitute.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use joyoffice_opc::Budget;
use joyword_docx::{ExportMode, ExportPlan, ImportResult, import, plan_export};
use joyword_model::{Block, ScalarOffset};

fn fixture(id: u8) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}

fn unchanged_plan(source: &ImportResult) -> ExportPlan {
    plan_export(
        &source.backing,
        &source.document.snapshot(),
        &Budget::default(),
    )
    .unwrap()
}

fn edited_plan(source: &ImportResult) -> ExportPlan {
    let mut model = source.document.clone();
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("first block is a paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(ScalarOffset(0), "origin ")
            .unwrap();
    });
    plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap()
}

#[test]
fn unchanged_and_edited_plans_originate_from_their_package() {
    let bytes = fixture(1);
    let source = import(&bytes, &Budget::default()).unwrap();
    let unchanged = unchanged_plan(&source);
    assert_eq!(unchanged.mode(), ExportMode::OriginalPackage);
    assert!(unchanged.originates_from(&bytes));
    let edited = edited_plan(&source);
    assert_eq!(edited.mode(), ExportMode::TextPatch);
    assert_ne!(edited.bytes(), bytes.as_slice());
    // The binding names the source package, not the plan's output.
    assert!(edited.originates_from(&bytes));
    assert!(!edited.originates_from(edited.bytes()));
}

#[test]
fn another_package_or_a_single_changed_byte_is_not_the_origin() {
    let first = fixture(1);
    let second = fixture(2);
    let a = unchanged_plan(&import(&first, &Budget::default()).unwrap());
    let b = unchanged_plan(&import(&second, &Budget::default()).unwrap());
    // Both are first-version plans of unedited documents.
    assert_eq!(a.version(), b.version());
    assert!(!a.originates_from(&second));
    assert!(!b.originates_from(&first));
    let mut flipped = first.clone();
    let last = flipped.len() - 1;
    flipped[last] ^= 1;
    assert!(!a.originates_from(&flipped));
    assert!(!a.originates_from(&first[..last]));
    assert!(!a.originates_from(&[]));
}

#[test]
fn clones_keep_the_binding() {
    let bytes = fixture(1);
    let plan = edited_plan(&import(&bytes, &Budget::default()).unwrap());
    let clone = plan.clone();
    assert!(clone.originates_from(&bytes));
}
