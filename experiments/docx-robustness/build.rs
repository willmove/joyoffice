use std::process::Command;
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("git")
        .args([
            "diff",
            "--quiet",
            "655cbccdd1737827da29ea182c45ac6923e74828",
            "--",
            "crates",
        ])
        .current_dir(root)
        .status()
        .expect("verify production baseline");
    assert!(
        result.success(),
        "Production crates differ from the fixed W09 target; obtain an approved integration SHA before retargeting"
    );
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../crates");
}
