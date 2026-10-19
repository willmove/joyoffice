use std::process::Command;
fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("git")
        .args([
            "diff",
            "--quiet",
            "aa2f97faacd447c43aa4987a095ed94e1c494e83",
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
