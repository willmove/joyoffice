fn main() {
    // Match the usual 8 MiB Unix main-thread reserve. GPUI's composed debug view tree
    // outgrows MSVC's 1 MiB default; the UI must stay on the native main thread.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg-bin=joydeck=/STACK:8388608");
    }
}
