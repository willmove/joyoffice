# Run color theme references

This follow-up retains `w:color/@w:themeColor`, `w:themeTint` and `w:themeShade`
separately from the resolved layout RGB. The original logical name and imported
physical palette slot are retained so custom `clrSchemeMapping` still works.
The existing settings merger keeps that mapping on save. Modifier attribute
spellings, including unsupported values, are retained rather than normalized.
Explicit run color edits (including the same RGB or auto), clearing formatting,
list colors and table-style color patches detach the binding. Overlay/minus
compare the binding as well as RGB, preventing a fixed same-value override from
being dropped. Native snapshots serialize the metadata; older snapshots default
to no binding and cannot recover previously discarded references.

Theme commands refresh bound colors in live stories, styles, conditional table
regions, list levels and tracked formatting, including pending insertion props.
Cross-document source-format adoption freezes source font/color values. Raw
preserved glossary XML is not rewritten merely to refresh caches. Existing
imported-theme XML/relationship preservation and conservative save refusals are
unchanged; its unknown content regressions remain part of the combined suite.

The schema names are documented in Microsoft's [Color reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.color?view=openxml-3.0.1).
The [ECMA-376 specification](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
is the normative reference. Its official Part 1 ZIP download returned HTTP 403
in this environment. This change deliberately retains the existing import HSL
interpretation (tint followed by shade when both are present), rather than making
an unverified compatibility change. Tests prove preservation and consistent
refresh for both attributes, **not conformance of that combined-modifier case**.
Independent normative/Word/WPS confirmation and any precedence correction remain
open. Individual tint/shade endpoints and a shaded red value have numeric tests.

## Scope limits

- Run text colors only: underline, borders, shading and DrawingML bindings are
  not newly modeled here.
- Legacy theme commands still heuristically recolor some unbound built-in styles
  and replace unbound style fonts. This does not promise that all explicitly
  colored/fonted style definitions resist those pre-existing commands.
- Arbitrary unknown run/style XML is not newly preserved. Unknown theme XML is
  covered by the prerequisite theme-preservation change.
- Changing clrSchemeMapping after import through external direct model mutation,
  full EA/CS font bindings, and mixed fixed ASCII/themed hAnsi remain outside scope.
- Linux headless verification does not establish Word/WPS appearance, physical
  device or native desktop acceptance.

## Real corpus result

The reproducible `probe.py` run covers nine original Word DOCX files and 106
paragraph/character styles. After independent theme replacement in the original
and saved packages, font differences are 0 and color differences are 0 (previous
phases: 103/48, then 0/48). Effective original-theme styles also match. All source
SHA-256 values remained unchanged. See `color-binding-results.json`; detailed
local packages/logs are under `/workspace/joyword-compat-validation/style-color-bindings`.
This is model/import/save evidence, not an independent visual rendering oracle.

## Combined validation

- 2,004 tests passed: DOCX 136 unit + 399 integration; engine 1,160 unit +
  43 integration; model 266. One pre-existing recovery-size measurement ignored.
- Strict model/DOCX/engine library Clippy passed. Existing all-target DOCX test
  lint backlog is not claimed resolved.
- Desktop `joyword` and `joyword-cli` cargo check passed with dynamic fontconfig.
- Workspace formatting, docs, 291 imported-source records and 1,463-file
  architectural gates passed; whitespace check passed with CRLF provenance data.
- Prerequisite #156 remained open at `7bac2e60`, with four queued workflows.
  No old workflow was restarted or canceled.

Follow-up: [fixed style fonts and EA/CS audit](FIXED-STYLE-PHASE.md) removes the
legacy unbound Latin-font replacement heuristic; the other limitations above remain.

Follow-up: [fixed style colors](FIXED-COLOR-PHASE.md) removes the old unbound
text/border recoloring heuristic; literal built-in colors remain fixed.
