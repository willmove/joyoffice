//! Shareable diagnostics contain no document text, paths, environment values or credentials.
pub fn report_for(version: &str, desktop: bool) -> serde_json::Value {
    serde_json::json!({"application":"JoyDeck","version":version,"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"desktopFeature":desktop,"nativeReaderMax":11,"writerSchema":"1.10","preferencesSchema":2,"automationProtocol":crate::automation::VERSION,"videoExport":"static-page MP4 requires macOS encoder helper","diagnosticScope":"build capabilities; not device or signature acceptance"})
}

/// Generic service metadata; applications inject their own release version.
pub fn report() -> serde_json::Value {
    report_for("unversioned-service", false)
}
