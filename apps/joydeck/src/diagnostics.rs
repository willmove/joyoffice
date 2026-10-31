//! Shareable diagnostics contain no document text, paths, environment values or credentials.
pub fn report() -> serde_json::Value {
    serde_json::json!({"application":"JoyDeck","version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"desktopFeature":cfg!(feature="desktop"),"nativeReaderMax":11,"writerSchema":"1.10","preferencesSchema":2,"automationProtocol":crate::automation::VERSION,"videoExport":"static-page MP4 requires macOS encoder helper","diagnosticScope":"build capabilities; not device or signature acceptance"})
}
