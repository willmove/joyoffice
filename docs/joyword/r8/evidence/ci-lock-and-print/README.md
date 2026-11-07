# Preference lock lifetime and deterministic print assertions

2026-11-07; existing PR #156, starting head 7c06470a. Main a187b040. No new PR, merge or close. #157 already targets main and contains the theme preservation implementation; it is not merged again here.

## Preference lock

macOS job 128402101985 failed repeated appearance saves with `Preferences: operation would block`, despite an isolated temporary directory. The existing save lock relied on closing its handle. A duplicate/inherited handle can keep the shared open-file-description lock alive after the save ends. CLOEXEC only closes an inherited descriptor when the child executes, leaving a fork/exec window with concurrent subprocess tests or host processes.

A deterministic Unix regression duplicates the locked file handle, verifies an active transaction still blocks a competing save, drops the original owner, and saves while the duplicate remains open. Before the fix it fails with WouldBlock. SaveLock now explicitly unlocks on Drop, including error paths after acquisition. It does not unlink the sidecar, wait/retry, or bypass stale-file/schema checks. Existing cross-process contention/crash-release tests remain enabled. The regression proves the lifetime defect; the macOS CI log alone does not identify which child inherited that particular descriptor.

## Print snapshot

ARM64/QEMU job 128402101768 compared two 9050-byte PDFs whose creation/modification timestamps differ by one second. Normalizing only those timestamps and PDF/XMP instance/document IDs makes every byte identical; rendered content is not implicated by this failure.

The test now opens a fixture with a fixed creation date, waits across a one-second boundary, and retains full-byte equality for formatting marks and the immutable snapshot after editing, plus inequality for the edited live document. Production export timestamps/IDs are unchanged; no PDF content bytes are ignored in the regression assertions. No printer queue or native device is used.

## Validation

Linux x64: all 105 Shell tests passed, including the new lock lifetime regression, real child-process contention/crash release, repeated appearance commands and the fixed-date print snapshot test. Strict all-target Shell Clippy, formatting, documentation and engineering gates passed. Updated only the adapted preference source fingerprint; original source fingerprint retained. macOS and ARM64/QEMU require latest-head remote CI; Linux results are not their acceptance.
