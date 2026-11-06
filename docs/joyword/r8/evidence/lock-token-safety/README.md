# JoyWord lock-token read safety

2026-11-06. Follow-up to PR #147; independent of the Deck footer PRs.

The original-save path may skip its own JoyWord lock when the complete file contents match the caller's token. Previously `metadata` and `read_to_string` followed a symbolic link, so a link to a matching token could be accepted as an owned regular lock. The read also had no byte cap after the metadata length check; a concurrent writer could grow it during verification.

The check now uses `symlink_metadata`, verifies the opened file is regular, reads at most token length plus one byte, and checks the path again. Unix opens additionally use `NOFOLLOW`, `NONBLOCK`, and `CLOEXEC`, and compare the existing storage identity representation before/opened/after. Errors, unknown state, links, and mismatches remain lock conflicts. Nothing deletes or expires a lock, and finding one's own token does not grant exclusive access to the document.

Tests cover normal complete tokens, empty/absent/wrong/truncated/extended tokens, a directory, a continuously growing input stream, and a Unix symbolic link to an exact matching token. The directory-level regression confirms that the symbolic link remains a conflict and its target bytes are untouched. All 76 joyword-shell tests passed, including existing original-save failure preservation tests. Strict all-target Clippy, formatting and repository/documentation gates passed. Source provenance retains the original hash and records the adapted hash.

Limits: this is not a cross-platform proof against every filesystem race. The no-follow/nonblocking open and identity comparison are Unix-specific; other platforms retain regular-file/length checks and bounded reads. Windows/macOS CI and real-device acceptance are separate; no real-device validation was performed.

## Footer assessment before switching priority

PR #148 distinguishes recognized explicit layout/master footer flags, but absent and unrecognized `p:hf` both remain `None`. `HeaderFooter.hide_on_title` has no complete PPTX read/write/render contract in the inspected code. Therefore this batch does not infer implicit enablement, change existing documents, or relax the conservative title-page rejection in PR #149. Those model semantics and real-window UI validation remain open.
