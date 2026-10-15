use joyword_docx_robustness::{MAX_INPUT, ResultKind, run};
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
#[test]
fn targets_reject_bad_data_and_oversized_stdin_without_panicking() {
    for target in ["opc", "import", "roundtrip"] {
        assert_eq!(run(target, b"not a zip", 0).kind, ResultKind::Rejected);
        assert_eq!(
            run(target, &vec![0; MAX_INPUT + 1], 0).kind,
            ResultKind::Rejected
        );
    }
}
#[test]
fn registered_baseline_supports_deterministic_real_engine_roundtrips() {
    for seed in 0..12 {
        for target in ["opc", "import", "roundtrip"] {
            let result = run(target, SOURCE, seed);
            assert_eq!(
                result.kind,
                ResultKind::Ok,
                "{target}, seed {seed}: {}",
                result.reason
            );
        }
    }
}
#[test]
fn protected_and_unknown_fixtures_keep_original_copy_and_refusal_contract() {
    for source in [
        include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-003.docx").as_slice(),
        include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-004.docx"),
        include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-005.docx"),
    ] {
        let result = run("roundtrip", source, 0);
        assert_eq!(result.kind, ResultKind::Ok, "{}", result.reason);
    }
}
