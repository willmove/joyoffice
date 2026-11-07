# JW-806/JW-808: unknown namespaces and regenerated saves

2026-11-06, Linux x86_64, Rust 1.98.1. Baseline binary: `00fb06872d38bbbfc26f4dbc3404d6810b72aabb`; pre-fix current binary: `dc82d671d4b25d9929c9f4e82abca0b1c941d53d`. All 104 tracked DOCX inputs and their hashes were retained in `/workspace/joyword-compat-validation/complex/corpus`; complete before/after reports and reproduction scripts are alongside it. Source files were not changed.

## Confirmed defect and scope

The old importer marks a non-ignorable unknown element namespace read-only. The compatibility projection only checked `mc:MustUnderstand` and document protection, losing that safety signal on `unknown-namespace.docx` and `xml-depth.docx`. The new unit regression failed before the fix. Both files also reproduced an actual loss path: `joyword-cli run INPUT --save NEW.docx` succeeded but omitted their unknown XML. This was a regenerated output, not an overwrite of the original.

The bounded projection now checks namespace URIs before discarding prefix declarations. `mc:Ignorable` is inherited by URI and obeys element scope, including aliases and rebinding. The existing recognized namespace vocabulary is reused. AlternateContent remains opaque to this projection. Tests cover both actual seeds, scoped/aliased/rebound ignorable namespaces, sibling scope exit and UTF-16. No external resource is loaded.

The CLI `run` entry and source-bound DOCX save/export/merge preparation now refuse a read-only source before regeneration. The check uses the captured source bytes, not a second filesystem read. The CLI integration regression verifies both seeds fail before output creation and a normal DOCX still edits, saves and reopens. `inspect` remains available. This is not a universal lossless export guarantee: unsupported content in other formats, unbound in-memory documents, desktop workflow acceptance and every conversion path remain outside this slice.

Recovery was checked separately with the pre-fix binary: both V2 edited text checkpoints already refused with no output; both V3 matching full snapshots saved byte-identical packages. Original source and journal bytes remained unchanged. The incorrect report therefore did not demonstrate data loss in those recovery paths; the directly reproduced loss was CLI regeneration.

## Classification of the wider differences

Before fixing: 68 exact reports, 11 matching refusals, 25 differing reports. After fixing: 69 exact reports, 11 matching refusals, 24 differing reports. Both read-only flags now match. The deep-XML file still differs in paragraph count. All 93 readable files retain matching text and omitted-object counts.

- **Confirmed safety defect:** the two read-only flags and regenerated output refusal, fixed here.
- **Empty-body normalization:** the deep-XML and AlternateContent cases report one paragraph instead of zero because the adapter explicitly clamps to one. This is a model normalization difference, not proof that input text was lost; schema-1 exactness remains unresolved.
- **Missing warning translation:** 9 native-workspace samples plus synthetic-compat, must-understand and revision lack the old missing-section warning. The old diagnostic helper identified `SectionDefaults { missing: "w:sectPr" }`; the new adapter only validates a section when one is present. This is an unresolved report-contract gap, not a demonstrated content regression.
- **Capability-related warnings:** three Word-resaved break fixtures lose ten old `InvalidValue(w:themeColor="text1")` messages each. The current reader explicitly handles the `text1` theme slot. Removing this obsolete warning is not itself a defect; full report-count compatibility and visual equivalence are distinct and remain unaccepted.
- **Preserved-fragment counts:** ten Word-produced files differ (66→60 or 7→6). Old diagnostics identify style/settings preservation; the lightweight adapter counts properties differently and does not count every opaque style subtree. These are unresolved accounting/preservation-contract differences, not evidence by themselves that saved XML was removed.
- **Style approximation warnings:** six Word-produced cases now flag approximation. This is a more conservative warning, not a proven rendering regression; property-level attribution and visual checks remain open.

No blanket acceptance or suppression of the remaining differences was added. OLE, charts/SmartArt, notes, content controls and equations still lack sufficient real-corpus coverage in this audit.

## Validation and delivery limits

The DOCX/Shell/CLI library and integration suites passed 622 tests (138 DOCX unit, 386 DOCX integration, 97 Shell, 1 real CLI regression). After extending the shared export/save guard, the 97 Shell tests and CLI regression passed again. The 104-file binary differential rerun preserved every source hash.

Strict all-target DOCX Clippy remains blocked by 181 pre-existing test lint errors, also reproduced in the clean footer worktree whose DOCX sources predate this fix. No blanket lint allowance was added. Production DOCX and all-target Shell/CLI Clippy passed separately. Formatting, documentation, 291 imported-source fingerprints and repository gates also passed. No real-window/device acceptance, CI rerun, push or merge is part of this local validation slice.

## Direct-main PR and remaining save audit

The fix was cherry-picked alone onto main `0c92b4cf20a129dcf1b1e018a573ca34fc751fd0`; it does not depend on PR #151's preference/recovery changes or alter #151–153 heads. The direct-main build passed 598 tests: 138 DOCX unit, 386 DOCX integration, 73 Shell and 1 CLI integration test. The earlier 622 count refers to the combined Word safety branch, not this narrower branch.

A further save audit retained all 24 remaining input files and generated separate outputs using the same fixed regeneration code. Three sources were refused (MustUnderstand, unknown deep XML, and protected DOCX-MIN-005). Twenty-one regenerated successfully with unchanged concatenated main-body text, deleted text and field instructions. `revision.docx` gained `w:author="Unknown"` where the source insertion had only `w:id="1"`; neither existing author data nor the revision ID/text was removed. This is normalization of incomplete metadata, not evidence of lost authorship. The AlternateContent fixture retained its fallback text in regenerated output even though the legacy inspection projection intentionally omits that opaque block.

Detailed before/after fields and output packages are preserved at `/workspace/joyword-compat-validation/complex/remaining-save-audit/all-24/`. These checks do not prove equivalent formatting, all package parts, revision semantics, rendering, or visual fidelity. The 24 schema-report differences remain classified rather than suppressed; no additional confirmed text loss was found in this audit.
