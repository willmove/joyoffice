#[test]
fn edit_save_reopen() {
    let dir = std::env::temp_dir().join(format!("joysheet-smoke-{}", std::process::id()));
    assert_eq!(super::run(&dir).unwrap()["passed"], true);
    let _ = std::fs::remove_file(dir.join("joysheet-smoke.xlsx"));
    let _ = std::fs::remove_dir(dir);
}
