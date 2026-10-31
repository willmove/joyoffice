fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../assets/joysheet/joysheet.ico");
    println!("cargo:rerun-if-changed=joysheet.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let major = env!("CARGO_PKG_VERSION_MAJOR").parse::<u16>()?;
        let minor = env!("CARGO_PKG_VERSION_MINOR").parse::<u16>()?;
        let patch = env!("CARGO_PKG_VERSION_PATCH").parse::<u16>()?;
        let macros =
            [format!("JOYSHEET_VERSION_NUM={major},{minor},{patch},0"), format!("JOYSHEET_VERSION_STR=\"{}\\0\"", env!("CARGO_PKG_VERSION"))];
        embed_resource::compile("joysheet.rc", macros.as_slice()).manifest_optional().map_err(|e| format!("Windows resources: {e:?}"))?;
    }
    Ok(())
}
