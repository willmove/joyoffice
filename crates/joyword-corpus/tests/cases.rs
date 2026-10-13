//! The committed minimal cases are exactly what the generator produces, and
//! each is a valid OPC package with the structure its case relies on.

use std::path::Path;

use joyoffice_opc::{Budget, Diagnostic, ExternalKind, Package, PartName, Target};

fn committed(file: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/docx")
        .join(file);
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn generation_is_reproducible_byte_for_byte() {
    for case in joyword_corpus::cases() {
        assert_eq!(
            case.package(),
            committed(case.file),
            "{} differs from the committed file; regenerate with `cargo run -p joyword-corpus --bin generate_corpus` and review",
            case.id
        );
        assert_eq!(
            case.package(),
            case.package(),
            "{} is not deterministic",
            case.id
        );
    }
}

#[test]
fn every_case_is_a_clean_package() {
    for case in joyword_corpus::cases() {
        let package = Package::read(&case.package(), &Budget::default())
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert!(
            package.diagnostics().is_empty(),
            "{}: {:?}",
            case.id,
            package.diagnostics()
        );
        let main = package
            .relationships(None)
            .iter()
            .find(|relationship| relationship.kind.ends_with("/officeDocument"))
            .expect("main document relationship");
        assert_eq!(
            main.target,
            Target::Internal(PartName::new("/word/document.xml").unwrap())
        );
    }
}

#[test]
fn external_case_targets_are_recorded_not_resolved() {
    let case = joyword_corpus::cases()
        .into_iter()
        .find(|case| case.id == "DOCX-MIN-004")
        .unwrap();
    let package = Package::read(&case.package(), &Budget::default()).unwrap();
    let settings = PartName::new("/word/settings.xml").unwrap();
    let document = PartName::new("/word/document.xml").unwrap();
    let kinds: Vec<ExternalKind> = package
        .relationships(Some(&settings))
        .iter()
        .chain(package.relationships(Some(&document)))
        .filter_map(|relationship| match &relationship.target {
            Target::External { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect();
    assert!(kinds.contains(&ExternalKind::NetworkShare));
    assert!(kinds.contains(&ExternalKind::Web));
    assert!(
        !package
            .diagnostics()
            .iter()
            .any(|d| matches!(d, Diagnostic::DanglingTarget { .. }))
    );
}
