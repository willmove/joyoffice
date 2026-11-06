# Latin theme-font binding repair (first phase)

This phase fixes the Latin font portion of the confirmed theme-response loss. It
does **not** resolve the text-color findings or claim complete OOXML formatting
fidelity.

## Design

`CharProps.theme_font` keeps supported ASCII/high-ANSI theme references alongside
the resolved font used for layout. Its imported/resolved value guards against
writing a stale reference after a low-level value mutation. JSON snapshots retain
the metadata; older snapshots without it continue to default to no binding.

An explicit font edit uses `set_fixed_font`, even when the chosen value equals the
currently resolved font. Overlay and subtraction consider the binding as well as
the font value, so a fixed-value override cannot disappear as a redundant property.
Font commands, list and conditional table style editing, paragraph-style resets,
content-control fonts and inserted symbols use the explicit setter. Undo snapshots
retain the previous binding. Low-level API callers must likewise use the setter
for explicit edits; assigning the public `font` field to the same string cannot
express that intent by itself.

Theme-font commands keep coherent bindings and refresh their layout cache in live
stories, style regions, numbering, note separators and pending typing properties. Unrelated paragraph blocks
remain shared. Original glossary XML is not regenerated merely to refresh a cache.
Importing another document with source formatting and exporting a fragment to its
own document freeze the source's font, avoiding accidental binding to another
document's theme.

The DOCX writer retains only coherent, recognized Latin references. An hAnsi-only
reference does not acquire a fixed ASCII override. Arbitrary XML is never replayed
by this repair. The separate theme-preservation code continues to own opaque theme
XML and dependency graphs; its unknown-content tests remain in the DOCX suite.

## Evidence

Re-running the nine real-Word fixtures checks 106 paragraph/character styles:

- Current effective formatting differences: **0**.
- Theme-response Latin font differences: **103 before → 0 after**.
- Theme-response text-color differences: **48 before → 48 after**, still open.

`font-binding-results.json` contains source hashes and per-file results. Full
packages and values are at `/workspace/joyword-compat-validation/style-font-bindings`.
The same reproducible probe in `README.md` generated these results.

Targeted tests cover explicit same-value overrides, overlay/minus, JSON persistence
and old snapshots, source-format import, ASCII/hAnsi and hAnsi-only references,
two saves plus a theme change, real formatting/theme commands and undo. General
DOCX, model, engine and shell tests validate surrounding behavior.

## Remaining scope

Text colors require equivalent binding metadata plus tint/shade semantics and
explicit color-command handling. EA/CS theme slots, mixed fixed-ASCII plus themed
hAnsi precedence, font hints, borders/shading and arbitrary unknown style XML are
not solved by this phase. Glossary insertion after a theme change needs its own
cache-resolution audit. Conditional table and numbering font caches are updated,
but their full effective-formatting matrix has not been independently accepted.

Linux headless execution only: Word/WPS display, native Windows/macOS fonts,
desktop controls and physical printing remain unverified. No queued PR is reset
by this local phase; the audit and repair can be consolidated after review.

The existing theme-font command still applies its historical fallback heuristic to
style fonts with no theme binding. This phase tests fixed direct-text overrides;
it does not claim that every unbound style-level font choice is immune to that
document-wide command. Distinguishing legacy built-in style defaults from explicit
unbound style choices needs a further origin-model decision and regression matrix.

## Validation

The full run passed 136 DOCX unit, 394 DOCX integration, 1158 engine unit, 43 engine
integration and 262 model tests (1993 total). One existing large-picture measurement
remains ignored. Shell separately passed 73 tests. The final focused rerun includes
the added cross-document source-font test, Together with the duplicate-font-declaration regression, unique covered tests total 2068.
Strict library Clippy for model/DOCX/engine, formatting, docs, source records and
architectural gates are recorded separately; no all-target lint cleanup is claimed.

The desktop and CLI hosts also passed `cargo check --locked -p joyword -p joyword-cli`
on Linux (fontconfig dynamic loading enabled). This is compilation, not a window
or device acceptance. A later fixed font declaration in malformed duplicate
`w:rFonts` input now clears an earlier binding rather than reusing stale metadata.
