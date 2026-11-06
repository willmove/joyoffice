# JD-305: conservative selected-page footer insertion

2026-11-06. Follow-up to PR #148's layout/master visibility metadata.

`add-missing-footer-text` adds one footer to the currently selected page. It does not edit existing footers, other pages, layout/master objects, the document-wide footer summary, or the selection. It uses the regular session transaction and a single undo/redo entry. It is an engine command, not a new UI or an apply-to-all operation.

The command accepts only a unique layout belonging to an unambiguous master, with both explicitly declaring `footer: true`. Missing, unrecognized, omitted, or disabled flags are rejected; no inheritance defaults are guessed. Hidden slides, suppressed master shapes, title/section-header layouts, and retained unknown extensions on the slide/layout/master are rejected. The layout must have exactly one top-level editable plain-text footer, an unambiguous placeholder index, and an explicit finite, positive, unrotated box. The master must also have one editable footer. Grouped, hidden, locked, field/mixed-run, tagged, linked, diagram, or extension-bearing candidate shapes are rejected.

The inserted shape contains fresh plain text, a newly allocated ID, and only the layout placeholder identity and explicit box. Formatting follows the existing placeholder chain; template-owned relationships and raw XML are not copied into the slide. Inputs retain the existing single-line/64 KiB footer budget. ID exhaustion and the session's whole-document budgets fail without changing the live document or history.

Tests cover selected-page scope in a mixed-layout deck, retained page differences and selection, no mutation during validation, refusal to overwrite existing footers, one-step undo/redo, and 16 ambiguous/unsupported cases. A shell regression exports and reimports a PPTX, adds the footer, saves through the source-preserving path, and reopens it; the unselected slide and original layout/master XML remain byte-identical. Native-format persistence is also covered.

This is intentionally narrower than a complete Header & Footer dialog. Default documents with absent explicit flags, title pages, implicit inheritance, all-pages creation, and complex templates remain unsupported. No real-window, PowerPoint/WPS, or physical-device acceptance is claimed. License-source gaps are unrelated and remain open.

Validation: 97 engine/shell tests passed; the imported-PPTX insertion regression was rerun after replacing test unwraps with propagated errors. Strict all-target Clippy for engine, shell, and the JoyDeck application passed. Formatting, documentation, source provenance (291 files), and repository gates (1452 Rust files) passed. These are automated checks, not desktop acceptance.
