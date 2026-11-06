# Explicit style fonts and script-font audit

The old `design.themeFonts` implementation guessed theme intent from style names
and current font strings. It overwrote all unbound style fonts, including an
explicit font set to the same name as the old default; it also overwrote an
explicit document-default font. This defeated the preceding binding-detachment
fix for style-level formatting.

Theme font changes now refresh only coherent bindings. New built-in style sheets
attach major/minor bindings while constructing their own known defaults, before
any user edit or DOCX import. Heading 1/2 and Title use the heading role; built-in
styles whose declared font is BODY_FONT retain the body role. There is no
post-import guessing based on IDs, matching font names or builtin flags.
Explicit fixed fonts (including document defaults) survive theme changes and
undo. New-document bindings survive DOCX save/reopen and still follow themes.
`design.styleSet` remains an explicit replacement of selected style definitions.
Older snapshots without bindings stay fixed; intent is not recoverable by name.

Tests include fixed same-value heading fonts, fixed document defaults, new
built-in heading/body roles after roundtrip, undo, and the nine real Word files.
For every imported style/default without a Latin binding, theme-font changes
preserve its font; explicit EA/CS values are also checked unchanged.

## EA/CS audit (not a repair)

`audit-script-fonts.py` counts attributes in `word/styles.xml` and
`word/document.xml` from the same nine source/saved package pairs as `probe.py`.
It reports lexical reference preservation, not visible glyph differences:

| Attribute | Original | Saved |
| --- | ---: | ---: |
| eastAsiaTheme | 44 | 0 |
| cstheme | 74 | 0 |
| eastAsia | 43 | 43 |
| cs | 1 | 1 |
| hint | 58 | 0 |

The imported complete theme and script-font table are preserved by #156, but
EA/CS run-property references and hints are still lost. The reader has a bidi
script resolver used by its general theme-slot resolution; the `w:rFonts` EA/CS
branches currently only capture fixed `eastAsia`/`cs` attributes. Therefore it is
incorrect to infer full script-font support from theme-table preservation alone.
A safe fix still needs script/language selection, hint handling, explicit override
precedence, metadata and cross-document semantics, and an independent rendering
oracle. No new normative precedence is guessed, and the previously denied
specification download was not retried or bypassed.

This phase resolves the unbound **Latin font** command heuristic only. The old
unbound color/border style heuristic and combined tint/shade interpretation remain
as documented in COLOR-BINDING-PHASE.md. Physical/native Word/WPS rendering and
EA/CS visual behavior remain unverified.

## Validation

- 2,007 related tests passed: DOCX 136 unit + 399 integration, engine 1,163 unit +
  43 integration, model 266; one pre-existing measurement ignored.
- Strict model/DOCX/engine library Clippy, workspace fmt check, docs, 291 source
  fingerprints, 1,463-file architecture gates and whitespace check passed.
- Nine-file probe rerun under `/workspace/joyword-compat-validation/style-fixed-fonts`:
  106 styles, zero font/color theme-response differences, original hashes unchanged.
  EA/CS audit results match the preceding phase exactly.
- Main remains `0c92b4cf`; #151–156 still target main, #157 still depends on #156.
  All observed checks were queued, with no reruns/cancellations requested.
- Desktop/CLI compilation was verified in the preceding phase; this narrow
  follow-up reran model/DOCX/engine checks, not native desktop acceptance.
