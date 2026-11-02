# JoyDeck working rules

- User instructions take precedence. This workspace implements the approved DeckCraft to GPUI refactoring plan in `docs/joydeck/PLAN.md`.
- Use PowerShell 7 (`pwsh.exe`) on Windows.
- Product/package/binary names are `joydeck` and `joydeck-*`. Use the JoyOffice directory layout and dependency baseline; do not add absolute machine paths as build dependencies.
- Keep Rust 1.98.1, edition 2024, resolver 3 and exact dependency versions. Application GPUI APIs come through gpui-kit 0.8.1 / gpui-pre 0.3.10. Do not enable gpui-fast or introduce egui.
- Preserve source attribution and licenses when importing DeckCraft or JoyOffice code. Imported code is not described as clean-room work. Never copy proprietary product assets or redistribute system fonts.
- Models, commands and CLI remain independent of GPUI. IO belongs to platform/shell. Preserve input budgets, unknown content and original files; errors must be recoverable.
- Keep every milestone buildable. Run relevant tests, fmt and strict Clippy. Build and inspect the actual desktop binary for UI work; record unverified platforms accurately.
- Update TASKS.md and milestone evidence with actual results. Do not mark future functionality implemented because a control or diagnostic exists.
- Do not modify the source JoyOffice or DeckCraft checkout while implementing in this workspace. Integration is rehearsed in a separate temporary checkout before replacement.
