# Joysheet working instructions

- Use PowerShell 7 (`pwsh.exe`) on Windows.
- This workspace is independent of the GridCraft source directory. Do not change that directory.
- Keep imported core behavior stable. `docs/upstream-source.json` records original hashes.
- UI code lives in `joysheet-ui-gpui`; core crates must not depend on GPUI or platform dialogs.
- All workbook mutations go through engine commands. The application worker owns `Session`.
- No unsafe code, panics, unwrap, expect or placeholder implementations in production code.
- Every implementation task updates `docs/STATUS.md` and the migration matrix, with evidence.
- Run appropriate tests and `cargo xtask ci` before claiming a milestone complete.
- Distinguish native runtime checks from compilation and synthetic input tests.
- Preserve upstream copyright notices. Attribute additional assets; use original or redistributable assets.

