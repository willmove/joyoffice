# Word style fidelity audit — 2026-11-06

This audit runs on the theme-preservation change in PR #156 (`7bac2e60`), which
keeps the theme itself. It identifies a separate loss in the styles that refer to
that theme. It is a local follow-up, not a completed production fix.

## Results

For nine existing real-Word fixtures, 106 paragraph/character styles have identical
JoyWord-resolved character and paragraph properties before and after a no-edit
DOCX save. `effective_styles.rs` records this regression. The initial exploratory
count was 124 style records; the final contextual comparison excludes 18 table
and numbering styles, whose conditional regions need separate evaluation.

A controlled change sets the major/minor Latin theme fonts to `Audit Heading` and
`Audit Body`, and accent1 to red, independently in the original and saved packages.
Reimporting those packages exposes **103 of 106 styles** with different responses:
all 103 have different effective Latin fonts; a subset also differs in color
(see `results.json`). Three styles in DOCX-MIN-002 are unaffected. Every original
fixture remains byte-identical.

Example: in `word_keepnext.docx`, Normal responds with `Audit Body` in the original
but remains `等线` in the saved copy. Heading style `1` responds with `Audit Heading`
and shaded red `(191, 0, 0)` in the original, but retains `等线 Light` and
`(16, 72, 97)` in the saved copy. This proves lost theme-response semantics; it is
not a measured visual regression with an unchanged theme.

## Root cause and repair boundary

`read/props.rs` resolves theme references into concrete `CharProps.font` and
`CharProps.color`; `write/props.rs` writes those concrete values. `CharProps` does
not retain whether a value came from a theme reference. EA/CS theme references,
font hints, border/shading theme colors, and conditional table regions require
additional checks beyond the Latin font and text color probe.

A safe repair needs explicit theme-binding metadata in the model and coordinated
handling in property inheritance/overlay/minus, formatting commands, theme changes,
snapshots and DOCX serialization. Merely restoring source XML when a concrete value
matches an import baseline cannot distinguish an unchanged theme reference from a
user explicitly choosing the same fixed font/color. It could undo that user's
formatting choice. No such shortcut or production behavior change is included.

## Reproduce

From the workspace root:

```sh
cargo build --locked -p joyword-docx --example style-audit
python3 experiments/docx-style-audit/probe.py --binary target/debug/examples/style-audit --output /tmp/new-style-audit
cargo test --locked -p joyword-docx effective_styles
```

Use a new output directory. The script retains both variants, saved DOCX files,
per-style resolved values, source hashes and full differences. The reviewed run is
at `/workspace/joyword-compat-validation/style-audit-reviewed`. `results.json` is
its compact summary. The probe only mutates new package copies and does not invoke
Word/WPS or access external resources.

Linux headless execution does not validate native fonts, Word/WPS rendering,
physical printing, or platform UI. This batch creates no new PR and does not
restart queued CI. Pending production work is the theme-binding model repair.

## Validation and delivery state

The new integration test passed (106 contextual style comparisons). The example's
strict Clippy, formatting, documentation, source records (291) and architectural
gates (1456 Rust files) passed. The reproducible probe completed successfully and
reported 103 font differences and 48 color differences. No production code was
changed in this follow-up. Its commit remains local on `fix/joyword-style-audit`,
based on PR #156, to avoid adding another PR or resetting queued checks. PR #156's
four workflows were still queued at the final read-only status check.
