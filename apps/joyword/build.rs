fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../assets/brand/joyword.ico");
    println!("cargo:rerun-if-changed=joyword.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Native ribbon rendering and document review share the GUI thread.
        // Reserve the same stack space as the desktop layout workers.
        let stack = match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
            Ok("msvc") => Some("/STACK:8388608"),
            Ok("gnu") => Some("-Wl,--stack,8388608"),
            _ => None,
        };
        if let Some(stack) = stack {
            println!("cargo:rustc-link-arg-bin=joyword={stack}");
        }
        embed_resource::compile("joyword.rc", embed_resource::NONE)
            .manifest_required()
            .map_err(|error| format!("Windows resources: {error:?}"))?;
    }
    Ok(())
}
