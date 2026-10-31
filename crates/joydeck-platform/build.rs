fn run() -> Result<(), String> {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return Ok(());
    }
    let directory = std::env::var_os("OUT_DIR").ok_or("Cargo OUT_DIR is missing")?;
    for name in ["media", "print", "video"] {
        let source = format!("native/{name}-macos.swift");
        println!("cargo:rerun-if-changed={source}");
        let result = std::process::Command::new("swiftc")
            .args(["-O", &source, "-o"])
            .arg(std::path::PathBuf::from(&directory).join(format!("joydeck-{name}-macos")))
            .output()
            .map_err(|e| format!("Swift adapter requires Xcode Command Line Tools: {e}"))?;
        if !result.status.success() {
            return Err(format!(
                "Swift {name} adapter failed: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1)
    }
}
