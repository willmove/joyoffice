# JD-305: conservative selected-page footer insertion

2026-11-06. Follow-up to PR #148's layout/master visibility metadata.

`add-missing-footer-text` adds one footer to the currently selected page. It does not edit existing footers, other pages, layout/master objects, the document-wide footer summary, or the selection. It uses the regular session transaction and a single undo/redo entry. It is an engine command, not a new UI. The separate all-missing command is described below.

The command accepts only a unique layout belonging to an unambiguous master, with both explicitly declaring `footer: true`. Missing, unrecognized, omitted, or disabled flags are rejected; no inheritance defaults are guessed. Hidden slides, suppressed master shapes, title/section-header layouts, and retained unknown extensions on the slide/layout/master are rejected. The layout must have exactly one top-level editable plain-text footer, an unambiguous placeholder index, and an explicit finite, positive, unrotated box. The master must also have one editable footer. Grouped, hidden, locked, field/mixed-run, tagged, linked, diagram, or extension-bearing candidate shapes are rejected.

The inserted shape contains fresh plain text, a newly allocated ID, and only the layout placeholder identity and explicit box. Formatting follows the existing placeholder chain; template-owned relationships and raw XML are not copied into the slide. Inputs retain the existing single-line/64 KiB footer budget. ID exhaustion and the session's whole-document budgets fail without changing the live document or history.

Tests cover selected-page scope in a mixed-layout deck, retained page differences and selection, no mutation during validation, refusal to overwrite existing footers, one-step undo/redo, and 16 ambiguous/unsupported cases. A shell regression exports and reimports a PPTX, adds the footer, saves through the source-preserving path, and reopens it; the unselected slide and original layout/master XML remain byte-identical. Native-format persistence is also covered.

This is intentionally narrower than a complete Header & Footer dialog. Default documents with absent explicit flags, title pages, implicit inheritance, and complex templates remain unsupported. No real-window, PowerPoint/WPS, or physical-device acceptance is claimed. License-source gaps are unrelated and remain open.

Validation: 97 engine/shell tests passed; the imported-PPTX insertion regression was rerun after replacing test unwraps with propagated errors. Strict all-target Clippy for engine, shell, and the JoyDeck application passed. Formatting, documentation, source provenance (291 files), and repository gates (1452 Rust files) passed. These are automated checks, not desktop acceptance.


## All missing pages follow-up

`add-all-missing-footer-text` scans all pages for existing footer placeholders, including nested/grouped placeholders. It leaves those pages untouched, preserving their text and formatting differences. Each remaining page must pass the same conservative selected-page checks. A failed page reports its one-based page number and aborts the entire operation; no silent skipping or partial save occurs. No targets is a no-op. The command stages a separate snapshot, enforces 256 targets / 1 MiB inserted text and the whole-model budgets, then publishes one model change. Selection and document summary remain unchanged. This is separate from `set-existing-footer-text`; it does not mean replace every footer with the supplied text.

Regression coverage includes multiple missing pages, locked existing footers, idempotence, one undo/redo, a later unsafe layout, unchanged direct snapshot on preflight failure, target/text/output budgets, and native/PPTX round trips. Existing selected-page mixed-layout and untouched-XML tests remain in place.

UI work remains blocked by the local validation prerequisites in [UI-PARITY.md](../../UI-PARITY.md): “后续 UI 改动须先观察真实产品，再实现、构建、通过真实窗口操作并对照截图”. This environment has no real desktop/reference-product session. No new UI was implemented or accepted. A future dialog must distinguish updating existing text from filling missing footers, preserve mixed state until explicit input, validate against the current session, and show a page-specific failure without partially applying changes.

All-pages follow-up validation: 100 engine/shell tests passed; the footer-specific suite reran with 11 passing tests after adding grouped-existing-footer coverage. Strict all-target engine/shell/application Clippy, fmt, documentation, source provenance and repository gates passed. The environment has neither DISPLAY nor WAYLAND_DISPLAY configured and no Xvfb; no GUI acceptance was attempted.

## Direct-main consolidation

2026-11-06. PR #147 was squash-merged into main at `0c92b4cf20a129dcf1b1e018a573ca34fc751fd0`. PR #148 merged into the old integration branch and #149 into the visibility branch; neither merge alone placed these footer changes on main. The `fix/joydeck-footer-main` branch applies only the four footer commits from #148/#149 to that main. Before this evidence update its complete file tree was identical to the previously verified `cee223ad38894b849489dacd6e16ceeb26cc05f0`; no already-landed safety, clipboard or release changes are repeated in the diff. Original branches remain intact.

Combined Linux regression on the direct-main branch: `cargo test --locked -p joydeck-model -p joydeck-engine -p joydeck-format -p joydeck-pptx -p joydeck-shell` passed all 312 tests (model 98, engine 38, format 48, PPTX 66, shell 62), with zero failures or ignored tests. This includes 11 footer-engine cases, imported/native/PPTX preservation, atomic all-missing updates and undo/redo. Documentation, 291 imported-source fingerprints and repository gates passed. No UI or real-device acceptance is added; the conservative eligibility and unsupported cases above remain unchanged.

Strict all-target Clippy for the same five crates plus the `joydeck` application passed with `RUST_FONTCONFIG_DLOPEN=1`; `cargo fmt --all --check` also passed. These are Linux compilation checks, not native desktop execution.
