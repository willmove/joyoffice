# Complex object corpus slice — 2026-11-06

Main remains `0c92b4cf20a129dcf1b1e018a573ca34fc751fd0`. The earlier 104-file differential inventory contained no footnote/endnote, equation or OLE cases. This adds those categories rather than repeating the existing theme/printing audit.

## New reproducible fixtures

`scripts/joyword/check-complex-corpus.py` generates five deterministic DOCX packages from synthetic XML. Notes and OLE shapes follow the repository's existing `crates/joyword-docx/tests/it/note_comments.rs` and `fixtures.rs` fixtures; the simple OMML fraction contains numerator `x+1` and denominator `y`. The OLE payload is an inert marker, not a functioning compound document or executable. It must never be used to claim embedded-application support.

The archived baseline CLI is from `00fb06872d38bbbfc26f4dbc3404d6810b72aabb`; the candidate CLI was freshly built earlier from current main `0c92b4cf20a129dcf1b1e018a573ca34fc751fd0`. [Results and SHA-256 fingerprints](results.json) identify both binaries and each generated source. Each candidate case survived two DOCX saves; the old CLI inspected each saved result successfully. The old CLI does not implement the new positional DOCX convert command, so no old-version save is claimed.

Checks preserve note reference targets and text, fraction structure and math text, OLE relationship/program identity and byte-identical payload/preview image. Source files and executable fingerprints remain unchanged. All five old/new input reports match except diagnostic counts: notes 5→4, equation 2→1, OLE 5→2. The script records complete reports and rejects differences outside diagnostics; it does not declare diagnostic changes correct or prove visual fidelity. Missing style/default metadata in these minimal packages remains reflected in compatibility flags.

```sh
python3 scripts/joyword/check-complex-corpus.py OLD_CLI NEW_CLI NEW_OUTPUT
python3 -m unittest discover -s scripts/tests -p test_complex_corpus.py
```

The fixture regression verifies deterministic generation and catches intentional note/formula text changes and OLE payload corruption. Real Office-authored complex documents, richer chart/SmartArt models, nested/mixed objects, layout and rendering comparisons remain open. This slice does not close JW-806/808.

## Remaining priorities / open PR inventory

At this snapshot #151–161 are all open. All target main except #157, which targets #156's theme-preservation branch. Pending changes are not main acceptance:

- #151 recovery/preferences safety; #154 namespace preservation; #155 print timestamp; #156–157 theme graphs/bindings.
- #152 hidden clipboard fallback; #153 footer model/UI. Deck JD-305 still needs native UI acceptance, JD-307 platform clipboard/full Office exchange, JD-308 keyboard/focus and JD-309 remaining platform input regression. Existing macOS evidence is retained, not repeated.
- #158 Linux CLI-only rollback; installer/association/real desktop rollback remains open.
- #159 Agent cancellation/task owner; real transport, credentials and desktop integration remain open. #160 offline redaction utility.
- #161 native-print reply/deadline work; real Windows queue/window/device and WinRT rendering remain open.
- Release still needs authoritative missing license texts, strict packaging qualification, signing/installers and actual devices. No new license evidence was obtained here.

## CI scheduling evidence

At 2026-11-06 19:34:08 UTC, #151 head run [42257194221](https://github.com/willmove/joyoffice/actions/runs/42257194221), created 16:57:42 UTC, remained queued for 2h36m26s. All nine jobs reported runner_id=0. #161 head run [42280239877](https://github.com/willmove/joyoffice/actions/runs/42280239877), created 19:30:38 UTC, remained queued for 3m30s. The repository queued-run listing contained 78 runs, including older/superseded submissions; this is not 78 distinct current PR failures.

No execution failure log or concrete workflow error was available for these queued jobs. An authorized repository/account operator can inspect Actions capacity/account limits, but the API evidence here does not establish a billing, capacity or configuration cause. No rerun/cancellation was performed. Main has not absorbed any of these pending PRs.


## Chart and SmartArt extension

Added a synthetic bar chart with an opaque workbook dependency and a four-part SmartArt graph (data, layout, quick style and colors). Chart XML follows the repository's `chart_round_trips_with_its_parts` fixture; the workbook is deliberately inert, not a valid spreadsheet. SmartArt has no saved visual drawing and is not a usable/qualified Office layout fixture.

The invariant traverses relationships from the actual document object references, checks every reachable part's byte hash and content type, and compares edge types independent of relocated part paths. Missing targets, cycles or external edges fail the check. Both new cases passed two candidate saves and old-CLI readback, with original sources and binaries unchanged. No embedded content was executed. Mutation tests confirm detection of removed graph parts, changed payload bytes and changed content types; both corpus test methods passed.

At 19:38:41 UTC, the original #162 head `23614f84` still had all three workflows queued (Word run 42281025269). No rerun or cancellation was performed. The graph extension is a new commit on the same PR, not a retry of those checks.
