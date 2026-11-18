# GPUI components and assets

GPUI Kit, Base, Component and GPUI snapshots retain their Apache 2.0 notices. Lucide icons retain LICENSE-LUCIDE. Exact versions and checksums are in Cargo.lock.

Source: https://github.com/longbridge/gpui-kit and https://github.com/zed-industries/zed/tree/d2a4fb6ded7c470be25d1541281da3882495b65b/crates/gpui

Current desktop baseline: GPUI Kit/Base/Component/Macros/Assets 0.9.1 and GPUI Pre snapshots 0.3.12. The separately versioned gpui-pre-reqwest remains 0.12.15. Source review: docs/joyoffice/GPUI-KIT-0.9.1-UPGRADE.md.

The Linux X11 backend uses a local Apache-2.0 patch for gpui-pre-linux 0.3.12.
The original registry archive has SHA-256
`54bab418d0b7c027af9495f278619b46887362b3bd6446e246c87c5b0fa167df`.
Only `src/linux/x11/client.rs` changes. Focus, caret positioning and reset wait
for XIM_CREATE_IC to finish. The build sources and original license are in
`third-party/gpui-pre-linux`. The provenance check compares the build manifest
and every file with the pinned archive, verifies the reviewed patch digest and
then compares the archive with the pinned upstream revision.
