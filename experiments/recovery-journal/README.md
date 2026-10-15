# W08 experimental recovery journal

Independent workspace, Rust 1.98.1. This is a recovery kernel, not an application save promise.

Batch 1 format: `JWJRNL01`, schema 1, 44-byte little-endian record header. See src/journal.rs for offsets. Hard limits: 8 MiB payload, 32 MiB file, 4096 records. Callers may lower limits. CRC32 covers header and payload, detects accidental damage only. First kind is baseline, later kinds are events. Session identity and contiguous sequence must agree. A bad/torn record stops scanning at its start; unknown schema is refused. No resynchronization or source-log repair.

The append sink performs write_all, flush, sync_all. Partial write/flush/sync errors poison the writer and must be treated as uncertain durability, even if all bytes are readable. Fresh files use create_new. Callers must exclusively own their log; no interprocess locking is claimed. File sync does not prove directory entry durability or power-loss safety.

Sources: Rust std::io::Write (write_all/flush), std::fs::File::sync_all documentation; independently defined experimental binary format. No external editor implementation.

Run `cargo test --manifest-path experiments/recovery-journal/Cargo.toml --locked` and the corresponding `--release`, `cargo fmt ... --all -- --check`, `cargo clippy ... --all-targets --locked -- -D warnings`.

Batch 2 replay uses a restricted default-property plain-text body. Paragraph ordinals + Unicode scalar offsets resolve against the actual engine state, never serialized NodeIds. Each event binds source/text pre/post SHA-256, before/after monotonic engine versions, and actions. Mixed action transactions go through one engine transaction; undo/redo uses engine history. Overlay/preedit is absent from this committed-event API. Saved-version receipts are bookkeeping only, not disk saves. A closed session rejects late operations. Each commit rebuilds a candidate from the source and prior events, validates it, writes/syncs the event, then publishes. This bounded O(history) experiment does not claim interactive performance.

Replay limits additionally cap actions at 128/event, events at 4095, text at 4 MiB, paragraph text at 64 KiB and paragraphs at 4096. Import budgets are explicitly lower than production defaults. All arbitrary object/format bodies are refused; imported protection is never cleared. C07 structural exports may still fail while the recovery session correctly holds split paragraphs.
