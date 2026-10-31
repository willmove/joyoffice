fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../assets/joysheet.ico");
    println!("cargo:rerun-if-changed=joysheet.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("joysheet.rc", embed_resource::NONE).manifest_optional().map_err(|e| format!("Windows resources: {e:?}"))?;
    }
    Ok(())
}
