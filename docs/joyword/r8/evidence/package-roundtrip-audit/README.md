# DOCX package and revision audit — 2026-11-06

Follow-up to the 104-file old/new CLI differential: inspect all 21 successful
regenerated saves among the 24 remaining report differences. Three other inputs
were refused as read-only. The saved files were produced by the preceding fixed
CLI (namespace protection plus Word safety changes); this audit does not claim
that a direct-main build includes the other pending PRs.

## Findings

- No original ZIP member was removed in these 21 outputs. No non-XML binary part
  changed. All output internal relationship targets resolve to package members.
  This is not proof of semantic preservation of the parts themselves.
- Nine Word-authored inputs each lose all 94 `a:font` script mappings from their
  theme. The writer reconstructs a minimal theme from major/minor Latin fonts
  and colors; the model does not retain the full imported theme. This is confirmed
  loss of formatting data, not a measured visual regression. Preserving theme
  data alongside subsequent theme edits needs a separate model/writer change.
- Styles also change: theme-based fonts/colors are materialized, some properties
  and attributes disappear, and default style references are normalized. These
  require effective-formatting comparisons; raw XML deltas alone cannot identify
  all semantic regressions. No formatting acceptance is claimed.
- Only `experiments/docx-robustness/seeds/revision.docx` contains revision elements
  in this subset. Ordered subtree comparison, ignoring the added `xml:space`
  attribute on text, shows only addition of missing `w:author="Unknown"` to the
  insertion. Revision ID, nesting, order and text remain unchanged. This is not
  coverage of move revisions, deleted text, or property-change revisions.
- `tests/fixtures/reference/word2024/breaks/word_keepnext.docx` loses
  `cp:lastPrinted` (`2026-10-17T21:42:00Z`) on a no-edit save. The regression test
  failed on main before the fix. The reader/model/writer now retain this value;
  saves do not invent or refresh a print timestamp. Older JSON snapshots default
  to an empty value, which remains absent when serialized.

## Evidence and limits

`package-checks.json` records the pre-fix package relationship, binary and theme
checks for every successful output. Full raw token deltas and ordered revision
subtrees are retained outside the repository at
`/workspace/joyword-compat-validation/complex/remaining-save-audit/all-24/` as
`package-audit.json` and `revision-structure-audit.json`. Original corpus files and
all outputs remain available there and under the adjacent `corpus` directory.

The repair's real-Word fixture regression checks the exact print timestamp after
both first and second saves. Model tests check older JSON and snapshot round trips.
No desktop, Word/WPS rendering, printed output or physical-device acceptance was
performed. Existing queued PR workflows were not restarted or cancelled.

## Local validation

- `cargo test --locked -p joyword-docx -p joyword-model -p joyword-doc`: 908 passed.
- `cargo test --locked -p joyword-model --test core_metadata`: 1 passed.
- Strict library Clippy for those three crates passed. The previously documented
  DOCX all-target test-lint backlog is not covered by this production check.
- Formatting, documentation checks, imported-source checks (291 records), and
  architectural gates (1453 Rust files) passed.
