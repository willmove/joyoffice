# Preserve fixed style colors during theme changes

The regression `theme_colors_preserve_fixed_style_text_and_borders` failed against
production code at `5caab598`: a fixed red Heading1 (`FF0000`) became `8C2F39`
after `design.themeColors` selected Studio. The old `set_accent` loop inferred
intent from style IDs, overwriting unbound heading/intense text colors and top /
bottom paragraph border colors. It also ran after `design.styleSet`, overwriting
colors that the selected style set had just explicitly chosen.

`set_accent` now updates the existing palette entry and refreshes coherent color
bindings only. It does not infer theme intent from a style ID or matching RGB.
Fixed text colors and all fixed paragraph borders remain unchanged. The full
`design.theme` command continues to update its six palette accents and refresh
bindings; this change does not broaden the existing accent-one-only behavior of
`design.themeColors` or change tint/shade calculations.

New tests cover both theme commands, fixed red text/green borders, save/reopen and
undo; a bound heading alongside a same-value fixed style; and all nine Word files,
checking every style's fixed color/borders while bound colors follow the updated
palette. Basic, shaded and minimalist style sets retain their explicit dark,
white and gray heading colors instead of having them overwritten by accent1.

## Deliberate boundaries

Own built-in style colors currently use explicit RGB values. They now stay fixed
under theme changes unless they already have a coherent theme binding. This is
intentional: their dark heading blue is not the same RGB as default accent1, and
this repair does not invent an approximate tint/shade mapping or silently change
the initial appearance. Converting those defaults to explicitly designed theme
colors is a separate product/model change. Border/shading theme references are
still not modeled by this repair.

The earlier combined tint/shade precedence uncertainty, full EA/CS rendering,
unknown script-font save refusals, real Word/WPS/native desktop validation and
all-target test-lint backlog remain as documented in the preceding phase records.
This is a targeted removal of destructive style-name heuristics, not full theme
or visual-rendering acceptance.

## Validation

- Before fix: the new fixed-color regression failed as quoted above against the
  unmodified `5caab598` production implementation.
- After fix: all four new regressions passed within a 2,018-test combined run:
  DOCX 136 unit + 404 integration, engine 1,168 unit + 43 integration, model 267.
  One existing recovery-size measurement remained ignored.
- Strict model/DOCX/engine library Clippy, workspace formatting, docs, 291 source
  fingerprints, 1,464-file architecture gates and whitespace checks passed.
- Desktop/CLI compilation was verified in the preceding script-font phase; this
  command-only follow-up reran the related libraries/tests, not native GUI checks.
- Main remained `0c92b4cf`, prerequisite #156 remained `7bac2e60`, and #157 still
  targeted #156's branch. Existing four checks were queued before this update.
