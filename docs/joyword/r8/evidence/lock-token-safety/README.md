# JoyWord lock-token read safety

2026-11-06. Follow-up to PR #147; independent of the Deck footer PRs.

The original-save path may skip its own JoyWord lock when the complete file contents match the caller's token. Previously `metadata` and `read_to_string` followed a symbolic link, so a link to a matching token could be accepted as an owned regular lock. The read also had no byte cap after the metadata length check; a concurrent writer could grow it during verification.

The check now uses `symlink_metadata`, verifies the opened file is regular, reads at most token length plus one byte, and checks the path again. Unix opens additionally use `NOFOLLOW`, `NONBLOCK`, and `CLOEXEC`, and compare the existing storage identity representation before/opened/after. Errors, unknown state, links, and mismatches remain lock conflicts. Nothing deletes or expires a lock, and finding one's own token does not grant exclusive access to the document.

Tests cover normal complete tokens, empty/absent/wrong/truncated/extended tokens, a directory, a continuously growing input stream, and a Unix symbolic link to an exact matching token. The directory-level regression confirms that the symbolic link remains a conflict and its target bytes are untouched. All 76 joyword-shell tests passed, including existing original-save failure preservation tests. Strict all-target Clippy, formatting and repository/documentation gates passed. Source provenance retains the original hash and records the adapted hash.

Limits: this is not a cross-platform proof against every filesystem race. The no-follow/nonblocking open and identity comparison are Unix-specific; other platforms retain regular-file/length checks and bounded reads. Windows/macOS CI and real-device acceptance are separate; no real-device validation was performed.

## Footer assessment before switching priority

PR #148 distinguishes recognized explicit layout/master footer flags, but absent and unrecognized `p:hf` both remain `None`. `HeaderFooter.hide_on_title` has no complete PPTX read/write/render contract in the inspected code. Therefore this batch does not infer implicit enablement, change existing documents, or relax the conservative title-page rejection in PR #149. Those model semantics and real-window UI validation remain open.


## Preference special-file follow-up

The current and legacy preference loaders opened paths before checking their file type. A FIFO with no writer can therefore block startup before the 256 KiB read limit is reached. Both entry points now use a regular-handle check; Unix opens are nonblocking so FIFO opens cannot wait for a writer. Regular-file symbolic links retain their existing read compatibility (unlike owned-lock paths, where a symlink is not proof of ownership).

Unix regressions create real FIFOs without writers. The current-file case returns unreadable, read-only defaults and refuses saving; the legacy case falls back to the missing current configuration without changing or deleting the FIFO. Both leave their directories unchanged. A regular-file symlink regression confirms its existing contents can still be loaded. All 79 joyword-shell tests and strict all-target Clippy passed; fmt, source fingerprint recording, documentation and repository gates passed. Non-Unix opens retain their platform implementation; this does not claim Windows named-pipe race protection or real-device acceptance.

The legacy recovery journal's separate check-then-open boundary was initially left open; the following slice addresses its Unix replacement cases.


## Legacy recovery checked-open follow-up

The old recovery reader called `symlink_metadata` and then `File::open` by pathname. The gap also applied to `recovery.lock`. Deterministic regression tests retain the first metadata observation, replace the path, and call the checked-open helper: the previous read primitive follows a replacement symlink or reads a replacement regular inode. A replaced FIFO could block before the journal byte budget or cancellation check.

Unix opens now use `NOFOLLOW | NONBLOCK | CLOEXEC`, require a regular opened handle and regular final path, and compare device/inode/size/mtime/ctime across the pre-open observation, handle and post-open path. The returned handle supplies the existing bounded journal read; the journal codec, generation selection, writer lock and record budgets are unchanged. Initial absence still returns `None`; initial non-regular entries remain errors. Unstable opens fail conservatively and never delete, rename or rewrite journals.

Two new tests cover normal/missing/directory behavior and deterministic replacement by symlink, FIFO, or same-length regular file, checking original/target bytes and directory entries remain intact. All 81 shell tests passed, including existing V1/V2/V3 recovery, fields, images/tables, and original-save preservation regressions. Strict all-target Clippy, formatting and repository/documentation checks passed.

Platform limits: non-Unix builds add regular-handle/path checks and size/mtime comparison, but retain their existing open primitive; this does not prove protection against Windows reparse-point or named-pipe races. Unix `NOFOLLOW` applies to the final path component. A handle pins the opened object; this does not freeze its contents against an uncooperative concurrent writer. Existing shared-lock, bounded-read, checksum and snapshot validation remain necessary. No real-device acceptance is claimed.

## Fail closed on uncertain recovery slots

A regression was first run against the prior implementation and failed: with a valid older record in slot A and a directory in slot B, `select_locked` returned the older record. Non-regular paths and detected opening races used `InvalidData`, the same category as checksum/format damage, so the selector treated filesystem uncertainty as permission to fall back.

These path errors now use a non-data error category and abort selection. Deterministic tests cover directory, FIFO and symbolic-link slots; the prior replacement tests also assert their errors cannot enter the damaged-record fallback branch. A separate checksum-corruption test confirms that supported fallback still returns the older record with the damaged-record flag and leaves both files unchanged. The initial failing test and subsequent passing suite demonstrate the behavioral difference. All 83 shell tests and strict all-target Clippy passed, along with formatting and repository/documentation checks.


## Save-time future-schema guard

A failing-before regression confirmed that a process holding a writable configuration could overwrite a newer-schema file published after its load. Saving now uses the same bounded regular-file reader to inspect the current target before creating its temporary file and again before publication. Any recognized `schema` declaration above this editor's version rejects the save, even when other future syntax is not parseable or the file has conflicting schema declarations. A stale writable flag is no longer sufficient to downgrade such a file. Read failures and special/oversized targets also refuse publication.

The regression checks the newer file remains byte-identical and no extra files are left. Additional coverage tests unknown future syntax and conflicting declarations. Existing corruption backup/migration and normal repeated-save behavior remain covered by the shell suite. This is a best-effort schema guard, not an atomic compare-and-swap or same-schema conflict merge: a writer can still race the final check and rename, and concurrent same-schema updates are not resolved by this slice. Existing intentionally backed-up corrupt configurations keep their repair behavior.

Future-schema slice validation: 85 joyword-shell tests passed; strict all-target Clippy, formatting, repository and documentation gates passed. The pre-fix stale-schema test failed as expected, then passed after the guard was added. No real-device validation was performed.


## Same-schema stale-session conflicts

A failing-before test confirmed data loss: two sessions load the same schema-1 file; the second saves its new author, then the first overwrites it with stale preferences. Each `Loaded` now privately retains its absolute store path and the exact bytes observed, or verified initial absence. Save compares the current bounded read with that observation before preparing the temporary file and again before publishing. A mismatch or deleted target returns an error without changing the file. Cross-store saves are rejected. Successful publication advances only that `Loaded` observation so normal repeated saves work; cloning copies the observation instead of sharing updates with stale sessions. An in-memory mutex protects observation access without changing the existing `save(&Loaded)` call pattern.

Tests cover two independently loaded sessions, a stale clone, repeated successful saves, same-length external changes, external deletion, and cross-store misuse. Existing missing-file creation, legacy migration and corrupt-file repair remain covered. All 88 shell tests passed.

This is platform-neutral stale-state detection tested on Linux, **not a complete cross-process transaction**. There is no new interprocess lock or atomic compare-and-swap: overlapping writers can still both pass the final check before either rename. The in-memory mutex only serializes use of one `Loaded` observation. A future complete solution needs a verified cooperative locking/publication protocol, including lock lifetime, crash recovery and platform-specific replacement behavior. Windows/macOS CI and real-device validation remain separate.

Same-schema slice checks: strict all-target Clippy for both `joyword-shell` and the desktop `joyword` application passed on Linux (`RUST_FONTCONFIG_DLOPEN=1`), as did fmt, repository and documentation checks. The unchanged `save(&Loaded)` callers compile; no GUI acceptance is inferred from this.
