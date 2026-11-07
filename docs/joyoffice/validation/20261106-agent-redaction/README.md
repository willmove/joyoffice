# Agent credential redaction — 2026-11-06

Based directly on main `0c92b4cf20a129dcf1b1e018a573ca34fc751fd0`; independent of pending #159. This is an offline credential utility correction, not system keychain or live logging acceptance.

Before the fix, sequential replacement of synthetic secrets `token` and `token-private` left `-private` visible. Overlapping Unicode secrets similarly left a suffix, while a secret equal to `redacted` rewrote the generated marker. Three regression tests reproduced these failures.

The implementation now scans the original text, tracks coverage by overlapping exact matches, and never scans generated markers. Secret ordering no longer changes which original characters are hidden. Empty secrets are ignored and unrelated Unicode text is preserved. No real credential, socket or external service is used. The helper currently has no production caller in this crate; these tests do not establish that a real credential leak occurred or that future logs are safe.

Validation: all 66 main-based Agent tests (including doc test) passed, strict library Clippy and crate fmt passed. Documentation, 291 imported-source fingerprints, engineering gates and whitespace checks passed.

## Task owner integration limitation

Review of #159 head `291163c3397052a6d76d04c0e581981e840565d0` confirms that its explicit close path retains an active/timed-out task, but direct Drop can still detach a non-cooperative thread. No safe synchronous destructor can guarantee both bounded UI latency and termination of an arbitrary blocked Provider.

A desktop integration must keep the task owner in an application-level service, retain it when a view closes, suppress document callbacks and continue polling until the worker is reaped. Application exit needs either a responsive wait or a separately designed owned-process termination contract. Do not use a global leaked owner, an untracked reaper or a blocking UI Drop as a purported fix. Desktop integration and forced termination are not claimed. #159's three workflows were still queued at this review; none was rerun.
