# Remaining work and Linux CLI rollback evidence

Snapshot: 2026-11-06 UTC. Main is `0c92b4cf20a129dcf1b1e018a573ca34fc751fd0` (includes #147). Open PRs are proposed work, not main acceptance. No PR was merged or CI rerun for this exercise.

## Original priorities

| Task | Available evidence / pending PRs | Remaining acceptance |
| --- | --- | --- |
| JW-806 data safety | Recovery and compatibility baseline on main; [#151](https://github.com/willmove/joyoffice/pull/151) recovery handles, locks and preferences; [#154](https://github.com/willmove/joyoffice/pull/154) unknown namespace guard | Complex OLE, SmartArt, chart, equation and footnote corpus remains incomplete; actual crash/device coverage still required |
| JW-808 compatibility | Existing 19-fixture old/new CLI reports; [#155](https://github.com/willmove/joyoffice/pull/155) print timestamps; [#156](https://github.com/willmove/joyoffice/pull/156) theme graph; [#157](https://github.com/willmove/joyoffice/pull/157) theme bindings | Native printing and export acceptance; EA/CS script rendering and unconfirmed tint/shade semantics; live Agent transport and system credential store are not implemented |
| JW-812 rollback | Existing Windows portable exercise; Linux CLI-only exercise below | Formal installer, uninstall, associations, release-package rollback and real desktop verification across platforms |
| Deck R3 | [#152](https://github.com/willmove/joyoffice/pull/152) hidden clipboard parse failure guard; [#153](https://github.com/willmove/joyoffice/pull/153) footer model, commands and UI | JD-305 native footer UI acceptance; JD-307 native clipboard/Office object exchange; JD-308 focus/shortcut and JD-309 IME/window checks on remaining platforms. Existing macOS evidence does not establish other platforms |
| Release | Archive validation and provenance engineering fixes on main | Strict license/SBOM missing source evidence; signed packages, installer/notarization and actual device qualification |

PRs #151–156 target main. #157 targets `fix/joyword-theme-preservation` (#156); it must be reconciled with main after its base lands. All seven were open at this snapshot. Avoid replaying merged #147 changes. See the product [R8 status](../../../joyword/r8/STATUS.md) and [Deck task list](../../../joydeck/TASKS.md) for the broader milestone definitions.

The last recorded license audit still had Linux gaps for `mac@0.1.1`, `pathfinder_geometry@0.5.1`, and `seahash@4.1.0`; the macOS set also included `block@0.1.6` and `malloc_buf@0.0.6`. Windows had two gaps. This exercise did not obtain new authoritative license texts or rerun strict packaging; it does not establish legal compliance.

## Completed slice: Linux CLI-only rollback

New entry point: `scripts/joyword/verify-r8-headless-rollback.py`. It reuses the existing rollback context manager, uses a fresh isolated directory, and accepts trusted executable binaries rather than archives. Tests execute stand-in subprocesses and cover failed promotion, failed candidate validation, successful replacement, rollback, mutated-original rejection, output reuse rejection and platform/input checks. Existing Windows rollback tests remain included.

Actual executable evidence is in [verification.json](verification.json). Previous executable was built from `00fb06872d38bbbfc26f4dbc3404d6810b72aabb`; its archived provenance is `/workspace/joyword-compat-validation/verification.json`. Candidate was freshly built from main `0c92b4cf20a129dcf1b1e018a573ca34fc751fd0` with `cargo build --locked -p joyword-cli`. Binary SHA-256 values are recorded in the report; binaries are not committed.

The real run used `/workspace/joyword-compat-validation/fixtures` and a new Unicode/space-containing directory `/workspace/joyword-compat-validation/Linux 换版回退`. It verified restoration after a missing stage and a failing candidate process, successful replacement and rollback, byte-identical V3 snapshot restoration, and old-CLI readback of source and recovered documents. Five isolated originals and all 26 input fixture files remained byte-identical.

Reproduce with trusted runnable builds and the recovery fixture layout (`source.docx`, `snapshot.docx`, `rich/`):

```sh
python3 scripts/joyword/verify-r8-headless-rollback.py OLD_CLI NEW_CLI FIXTURES NEW_OUTPUT
python3 -m unittest discover -s scripts/tests -p 'test_joyword*rollback.py' -v
```

This is explicitly **CLI-only**: no GUI, installer, file association, signature, license or release archive acceptance. It does not mark JW-812 complete.

## CI investigation

The latest #157 head `938dec991a3488a9962699a5b8865dbc743eceae` initially had four queued workflows; Word run 42275146834 had no assigned runners, empty steps, and no pending deployments. An older JoySheet run [42255260720](https://github.com/willmove/joyoffice/actions/runs/42255260720) subsequently completed all six jobs successfully, including joint feature regression. Thus the queue is not evidence that every runner is unavailable. The API does not establish the cause of remaining scheduling delay; there is no demonstrated approval gate or configuration error to repair. No rerun or cancellation was requested.

## Local checks

The complete Python script suite passed: 182 tests, 2 skipped (the rollback subset passed all 8 tests). Current-main CLI build, documentation check, 291 imported-source fingerprints, engineering gates and whitespace check passed. Unit-test package fixtures are synthetic and do not certify release packages.
