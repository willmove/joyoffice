use std::process::Command;

#[test]
fn cached_executable_uses_the_callers_checkout_even_from_a_nested_directory() {
    let directory = tempfile::tempdir().unwrap();
    let scripts = directory.path().join("tools/xtask");
    let nested = directory.path().join("apps/another-checkout/src");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(directory.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    std::fs::write(
        scripts.join("product.py"),
        "import sys\nprint('caller-checkout:' + sys.argv[1])\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_joyoffice-xtask"))
        .current_dir(&nested)
        .arg("probe")
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "caller-checkout:probe"
    );
}

#[test]
fn invocation_outside_a_workspace_fails_without_running_the_compiled_in_checkout() {
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_joyoffice-xtask"))
        .current_dir(directory.path())
        .arg("--help")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Cannot locate JoyOffice workspace")
    );
}
