// Counterexample for the no-panic gate in scripts/check_gates.py. Not part of the build.
// Every site below must be reported unless a genuine panic-family lint attribute
// covers it.
fn commented_allow_does_not_hide_the_site(value: Option<u8>) -> u8 {
    value.unwrap() // allow
}

fn the_word_allow_anywhere_in_a_comment_does_not_hide_the_site(value: Option<u8>) -> u8 {
    // The reviewer allowed this once; the gate must still see it.
    value.expect("allowlisted by a comment only")
}

fn identical_sites_are_counted_not_deduplicated() {
    panic!("duplicated counterexample payload");
    panic!("duplicated counterexample payload");
    panic!("duplicated counterexample payload");
}

fn abort_on_purpose_macros_are_sites() {
    todo!("not written yet");
    unimplemented!("not written yet");
}

fn genuine_lint_allowances_still_suppress(value: Option<u8>) -> u8 {
    #[allow(clippy::unwrap_used)]
    let inline = value.unwrap();
    #[expect(clippy::unwrap_used, reason = "reviewed counterexample")]
    let preceding = value.unwrap();
    inline + preceding
}

#[allow(clippy::unwrap_used)]
fn an_allowance_above_a_fn_does_not_cover_its_body(value: Option<u8>) -> u8 {
    value.unwrap()
}

#[allow(dead_code)]
fn an_unrelated_allowance_suppresses_nothing(value: Option<u8>) -> u8 {
    value.unwrap()
}
