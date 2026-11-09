fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../assets/brand/joydeck.ico");
    println!("cargo:rerun-if-changed=joydeck.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("joydeck.rc", embed_resource::NONE)
            .manifest_required()
            .map_err(|error| format!("Windows resources: {error:?}"))?;
    }
    Ok(())
}
