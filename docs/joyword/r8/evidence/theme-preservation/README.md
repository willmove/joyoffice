# Imported theme preservation — 2026-11-06

The package audit identified nine Word-authored DOCX files whose regenerated
`word/theme/theme1.xml` lost all 94 per-script font mappings. The old reader used
these mappings transiently but the writer rebuilt the theme from Latin major/minor
fonts, colors and a name. It also replaced format schemes and unknown extensions.

The reader now retains the complete imported theme plus its relationship graph in
existing document passthrough storage, and a baseline of the modeled fields. No
new dependency or model schema is required. Themes already identical to the
writer's generated theme and without relationships need no retained copy.

An unchanged theme is emitted byte-for-byte, even after body edits. Editing the
modeled theme name, Latin fonts or simple colors patches only the corresponding
attributes. Per-script fonts, EA/CS fonts, unknown attributes/elements, extension
payloads, comments, and the rest of the XML remain untouched. Relationship IDs are
kept; dependent parts are copied by the existing bounded package graph writer,
with targets adjusted if a part name must change. No external resource is fetched.

Missing dependencies (including in a restored model), missing baseline data,
ambiguous duplicate edited fields, absent edited fields, and edited non-empty
color elements fail saving explicitly. In particular, a color with transforms
cannot safely be rewritten from the model's flat RGB value. Keeping or editing
other fields of such a theme remains possible. This is a conservative limitation,
not complete theme editing support. Old snapshots that never retained a theme
cannot recover previously discarded data.

## Regression coverage

`crates/joyword-docx/tests/it/theme_preservation.rs` checks:

- All nine original Word fixtures, each with 94 mappings: exact theme bytes after
  a body edit, first save, reopen and second save.
- A minimal theme with an alternate DrawingML prefix, script/EA/CS mappings,
  unknown namespace payloads, comments and a related binary resource. Theme
  name, both Latin fonts and both system/RGB colors change without losing that
  data. Reopening observes the edited values, including an ampersand in a font.
- UTF-8 and BOM-marked UTF-16 LE original themes, including edited-theme encoding
  preservation and a second save.
- Missing original and subsequently removed retained resources, ambiguous Latin
  font declarations, and unsupported color transforms: saves refuse explicitly.

Only Linux headless tests were executed. Word/WPS rendering, platform-native
font selection, desktop theme controls, Windows/macOS execution, and physical
print output remain unverified. The package-level guarantee does not fix the
separate style-font/color materialization findings in the preceding audit.

## Local validation result

- DOCX: 136 unit tests and 391 integration tests passed.
- Shell: 73 tests passed; combined total 600.
- Strict DOCX library Clippy passed. The existing all-target DOCX test lint backlog
  is not claimed resolved by this library check.
- Formatting, documentation checks, imported-source validation (291 records),
  and architectural gates (1454 Rust files) passed.
- Flat OPC/DOCX equivalence regression passed: declaration-free Flat OPC copies of
  the writer's own exact theme do not acquire unnecessary passthrough metadata.
- PRs #151–155 were still queued when checked; no reruns or cancellations issued.
