# Opaque EA/CS theme references and font hints

`eastAsiaTheme`, `cstheme` and `hint` are now retained in character properties and
written back to `w:rFonts`. The first two carry the original fixed fallback as a
coherence guard; they are **not** a newly resolved font. Unknown attribute values
(including future theme-slot/hint names) are escaped and emitted unchanged.
Layout's existing EA/CS fallback behavior is unchanged. This phase makes no new
claim about script/language selection, hint semantics or glyph accuracy.

Explicit EA/CS font setters clear only their corresponding theme reference,
including same-value edits. The existing font command uses these setters; clearing
formatting clears both references and hint. Overlay/minus preserve the distinction
between a fixed fallback and an opaque theme reference. Combining property layers
keeps the existing inherited layout fallback while retaining the theme reference.
Later `rFonts` declarations replace earlier references/hint. Native property
snapshots serialize the metadata; older snapshots default to absent references.

Unknown `rFonts` attributes or children cannot be faithfully re-emitted by the
existing XML tree (unknown namespaces are discarded by its serializer). Rather
than pretend otherwise, import sets a persistent unsupported-font marker and DOCX
saving explicitly refuses. The original input file is untouched; the model does
not claim to retain that unknown XML payload. The marker is intentionally sticky,
including after unrelated edits; it is not automatically cleared by a font edit.

Source-format adoption/export of a fragment with unresolved script references
cannot safely freeze those fonts under a different theme. It therefore propagates
the same refusal marker instead of dropping references or claiming the original
font was resolved. This is conservative and can reject an adoption whose unused
source style contains a script reference. The model operation can return a
fragment, but subsequent DOCX save refuses. No cross-document UI acceptance is
claimed. The guard applies to the DOCX `write_parts` family; it is not a general
new guarantee for every unrelated format exporter.

Tests cover opaque/unknown values, two saves, explicit same-value edits, isolated
EA/CS font commands and undo, overlay/minus and older/native property snapshots,
duplicate declarations, unknown attributes/namespaces/children, and conservative
cross-document save refusal. Existing theme-XML unknown-content preservation
regressions remain in the full suite. No new normative script-selection rule was
inferred and no inaccessible source was retried.

## Real corpus

Nine original Word files were passed through the reproducible `probe.py` and
`audit-script-fonts.py` runs. Both the first and second saves retain all audited
attribute values/counts: eastAsiaTheme 44→44, cstheme 74→74, hint 58→58, fixed
East Asian fonts 43→43 and fixed CS fonts 1→1. Source hashes remain unchanged.
The 106-style effective-format/theme-response probe still reports zero font/color
differences. Results are in `script-font-preserved-results.json`; original and
both saved generations/logs are under
`/workspace/joyword-compat-validation/style-script-fonts`. Attribute counters are
not a glyph-rendering oracle; individual binding ownership is covered by focused
single-run, duplicate-declaration and explicit-edit tests.

## Validation

- 2,014 tests passed: DOCX 136 unit + 404 integration, engine 1,164 unit +
  43 integration, model 267. One pre-existing measurement ignored.
- Strict model/DOCX/engine library Clippy, desktop and CLI cargo check, formatting,
  docs, 291 source fingerprints and 1,464-file architecture gates passed.
- Main and dependency heads remained unchanged before submission. This extends
  #157 rather than creating another dependent PR; no merge or manual rerun.
- Linux headless only; native Word/WPS, full EA/CS rendering, clipboard UI and
  physical-device acceptance remain unverified.

A supplementary model `--all-targets` Clippy attempt reported test-code lints
(including the existing byte-store, property and model test suites). It is not a
passing gate. The new script-property test uses fallible returns rather than
adding unwrap-based assertions. The strict production-library Clippy result above
must not be read as all-target test-lint clearance.
