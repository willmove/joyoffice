# JD-305: explicit layout/master header-footer visibility

2026-11-06. Follow-up to the footer preflight in PR #147. This slice preserves metadata; footer creation and the dialog remain open.

Previously, otherwise identical PPTX files containing `p:hf ftr="0"` and `p:hf ftr="1"` imported into equal models. The source-preserving save retained the original layout XML, but a native-format round trip or fresh PPTX export lost the flags. Layout and master models now retain recognized `dt`, `ftr`, `hdr`, and `sldNum` attributes as optional booleans. Empty elements remain distinct from absent/unrecognized elements; omitted attributes are not eagerly filled. Old native documents without the new optional field remain readable through the existing serde defaults.

The attribute semantics come from Microsoft's [Open XML HeaderFooter reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.presentation.headerfooter?view=openxml-3.0.1). This implementation does not resolve inheritance, change rendering, or authorize missing-footer creation. It recognizes the conventional `p:hf` element with the Transitional PresentationML namespace. Alternate prefixes/namespaces, duplicate elements, unknown attributes/children, and invalid values remain unclassified. Unknown XML is retained by the existing source-preserving save path; this change does not add arbitrary unknown-XML support to fresh export.

Regression coverage verifies opposite flags produce different models, all four master flags and layout overrides survive native serialization and fresh PPTX export/import, and unrelated slide edits preserve both recognized and unrecognized layout XML byte-for-byte. No test creates a missing footer.

Local validation:

- `cargo test --locked -p joydeck-model -p joydeck-pptx`: 164 passed. After strengthening unknown-XML preservation coverage, PPTX tests rerun: 66 passed.
- `cargo test --locked -p joydeck-engine -p joydeck-shell -p joydeck-format`: 142 passed.
- Strict all-target Clippy for model, PPTX, engine, shell, and the JoyDeck application passed (Linux with `RUST_FONTCONFIG_DLOPEN=1`).
- Formatting, imported-source records (291 files), repository gates (1452 Rust files), and documentation checks passed.

No real-window/device acceptance was performed. Safe footer creation still requires an explicit inheritance/eligibility rule and command/undo tests before UI work. Release license-source gaps are unaffected.
