# AccessKit Windows 0.34.0 lifetime fix

The exact `accesskit_windows` 0.34.0 registry package is retained under
`third-party/accesskit_windows`. Upstream commit:
[`c88605b96d04431f9c3c792464a0f2f253480e94`](https://github.com/AccessKit/accesskit/tree/c88605b96d04431f9c3c792464a0f2f253480e94/platforms/windows).
Original and adapted file hashes, registry checksum and the two upstream license
texts are recorded in `source.json` and checked by `scripts/joydeck/check_sources.py`.

Real Windows GUI verification found a process abort after closing a second
JoyDeck window while UI Automation retained its provider. The original
`client_top_left` unwrapped a failed `ClientToScreen` call across a COM boundary.
The patch returns `UIA_E_ELEMENTNOTAVAILABLE` and propagates the error through
bounding rectangle, point hit testing and text range coordinate queries.
[Microsoft documents this error for destroyed elements](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-error-codes).
Live-window coordinates and accessibility remain enabled. No registry version
or other dependency edge changes; the root lock changes only this package's source.

This is a third-party platform adapter under its original MIT OR Apache-2.0
license, retaining its existing FFI. JoyOffice application crates continue to
forbid unsafe code. The package is excluded from workspace membership. Its
standalone lock supports the original upstream tests, including their winit
development dependency; it does not alter the product dependency graph.

Local verification command:

```text
cargo test --manifest-path third-party/accesskit_windows/Cargo.toml --target-dir target --lib closed_window_tests --locked
```

The regression checks an invalid HWND returns an unavailable-element error.
Product regression and the live multiwindow close/reopen scenario are recorded
in `docs/joydeck/evidence/R9-UI2/README.md`. Remove this patch when a reviewed,
explicitly authorized dependency update contains an equivalent lifetime fix.
