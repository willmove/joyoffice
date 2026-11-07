# Offline Word print boundary checks — 2026-11-06

Based on main `0c92b4cf20a129dcf1b1e018a573ca34fc751fd0`. No native dialog, printer, virtual queue or real print job was invoked.

The Windows helper returns `outcome`, `queueSubmitted` and, for submissions, `pages`. Previously Word checked only the first two fields, allowing a success result without a valid page count. Parsing was inside the Windows-only function, leaving it outside Linux CI coverage.

The production dialog now calls a shared, platform-neutral decoder. A successful submission must contain a positive page count no greater than the selected PDF page count. Cancellation must report no submission or positive page count. Process failure takes precedence over stdout, including a success/cancellation claim. Typed decoding rejects duplicate fields, malformed JSON, contradictory values and unexpected fields. Successful replies are limited to 4096 bytes; failure diagnostics retain the existing 2000-character cap and now have a nonempty fallback. This bounds parsing, not subprocess output capture or execution time.

Four targeted printing tests passed on Linux:

- Valid submission and cancellation, process failure and an empty failure diagnostic.
- Missing/zero/negative/fractional/excessive page counts, contradictory flags, duplicate fields, validation-only replies, malformed/trailing JSON, oversized output and invalid UTF-8.
- Existing selected-page PDF export from unsaved content without changing the edit revision.
- PDF snapshot remains byte-identical after later edits; formatting marks do not alter the printed PDF or edit revision, while a newly captured job reflects new content.

Run: `cargo test --locked -p joyword-shell printing::tests`.

The unchanged native PowerShell script is shared with Sheet. No changes were made to its queue behavior. Windows-only rasterization/preview and its existing `-Validate` test were not run: this environment has only a Linux Rust target. No assertion is made about actual spooler acceptance, queue cancellation, device output or native UI. A `submitted` reply means helper-reported queue submission, not paper output. Blindly retrying after an ambiguous result risks duplicate print jobs and was not introduced. The subsequent helper lifecycle change is described below. JW-808 remains partially verified.


## Owned helper deadline and output capture

The Windows dialog path now executes the owned PowerShell helper with a 15-minute deadline, including time spent in its dialog. This runs on the existing background worker. It polls exit status, terminates and waits for the direct helper on timeout/output overflow, and reports cleanup failure rather than claiming success. A Drop fallback also attempts cleanup on unexpected unwinding. There is no automatic retry and timeout/overflow reports queue submission as **unknown**, not cancelled: stopping the helper cannot undo an already submitted spooler job.

Stdout/stderr go to private temporary files, avoiding pipe backpressure and reader threads that could hang on inherited pipes. Each stream has a 64-KiB read cap and its disk length is checked every 20 ms. This is not a strict disk quota: output may overshoot between checks. Stdin is disconnected. Successful replies still have the separate 4096-byte decoder cap. The helper's descendants are not managed; the current script executes its dialog in-process. This does not bound OS spawn/kill/wait latency or WinRT PDF rendering before the helper launches.

Eight printing tests passed on Linux, including isolated invocations of the test executable for successful/failed exit, a sleeping helper terminated at a 100-ms deadline, output overflow, zero-deadline refusal and launch failure. The sleeping helper would wait five seconds without termination; the test confirms return in under four seconds after explicit kill/wait cleanup. No PowerShell, printer, native window, queue or real print job was started. Windows execution and actual dialog/queue cancellation remain unverified.

Follow-up checks: shell **all-target** strict Clippy, fmt, docs, source fingerprints and engineering gates passed. The filesystem gate adds only the new helper module, whose filesystem access is limited to its owned anonymous stdout/stderr files.
