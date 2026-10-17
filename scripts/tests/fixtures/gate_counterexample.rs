// Counterexample for scripts/check_gates.py. Not part of the build.
// name = "joyword-model"
// [dependencies]
// joyword-shell = { path = "../joyword-shell" }
fn bad() {
    let _ = std::fs::write("secret", "data");
    let _value = Some(1).unwrap();
}
