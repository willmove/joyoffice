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

The unchanged native PowerShell script is shared with Sheet. No changes were made to its queue behavior. Windows-only rasterization/preview and its existing `-Validate` test were not run: this environment has only a Linux Rust target. No assertion is made about actual spooler acceptance, queue cancellation, device output or native UI. A `submitted` reply means helper-reported queue submission, not paper output. The subprocess still has no timeout; blindly retrying after an ambiguous result risks duplicate print jobs and was not introduced. JW-808 remains partially verified.
