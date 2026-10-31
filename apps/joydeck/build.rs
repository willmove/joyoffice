// Compile only the independent platform helpers for desktop macOS builds.
fn main() {
    for name in ["media", "print", "video"] {
        let source = format!("native/{name}-macos.swift");
        println!("cargo:rerun-if-changed={source}");
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
            && std::env::var_os("CARGO_FEATURE_DESKTOP").is_some()
        {
            let destination = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
                .join(format!("joydeck-{name}-macos"));
            let result = std::process::Command::new("swiftc")
                .args(["-O", &source, "-o"])
                .arg(destination)
                .output()
                .expect(
                    "macOS desktop media/print adapters require Xcode Command Line Tools swiftc",
                );
            assert!(
                result.status.success(),
                "Swift {name} adapter failed: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}
