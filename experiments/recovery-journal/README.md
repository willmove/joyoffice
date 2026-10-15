# W08 experimental recovery journal

Independent workspace, Rust 1.98.1. This is a recovery kernel, not an application save promise.

Batch 1 format: `JWJRNL01`, schema 1, 44-byte little-endian record header. See src/journal.rs for offsets. Hard limits: 8 MiB payload, 32 MiB file, 4096 records. Callers may lower limits. CRC32 covers header and payload, detects accidental damage only. First kind is baseline, later kinds are events. Session identity and contiguous sequence must agree. A bad/torn record stops scanning at its start; unknown schema is refused. No resynchronization or source-log repair.

The append sink performs write_all, flush, sync_all. Partial write/flush/sync errors poison the writer and must be treated as uncertain durability, even if all bytes are readable. Fresh files use create_new. Callers must exclusively own their log; no interprocess locking is claimed. File sync does not prove directory entry durability or power-loss safety.

Sources: Rust std::io::Write (write_all/flush), std::fs::File::sync_all documentation; independently defined experimental binary format. No external editor implementation.

Run `cargo test --manifest-path experiments/recovery-journal/Cargo.toml --locked` and the corresponding `--release`, `cargo fmt ... --all -- --check`, `cargo clippy ... --all-targets --locked -- -D warnings`.

Batch 2 replay uses a restricted default-property plain-text body. Paragraph ordinals + Unicode scalar offsets resolve against the actual engine state, never serialized NodeIds. Each event binds source/text pre/post SHA-256, before/after monotonic engine versions, and actions. Mixed action transactions go through one engine transaction; undo/redo uses engine history. Overlay/preedit is absent from this committed-event API. Saved-version receipts are bookkeeping only, not disk saves. A closed session rejects late operations. Each commit rebuilds a candidate from the source and prior events, validates it, writes/syncs the event, then publishes. This bounded O(history) experiment does not claim interactive performance.

Replay limits additionally cap actions at 128/event, events at 4095, text at 4 MiB, paragraph text at 64 KiB and paragraphs at 4096. Import budgets are explicitly lower than production defaults. All arbitrary object/format bodies are refused; imported protection is never cleared. C07 structural exports may still fail while the recovery session correctly holds split paragraphs.

Batch 3 CLI (run from repository root):

```sh
cargo build --manifest-path experiments/recovery-journal/Cargo.toml --locked
experiments/recovery-journal/target/debug/joyword-recovery-journal inspect path/to/log.jwj
experiments/recovery-journal/target/debug/joyword-recovery-journal recover path/to/log.jwj tests/fixtures/docx/DOCX-MIN-001.docx path/to/NEW-recovery.jwj
experiments/recovery-journal/target/debug/joyword-recovery-journal checkpoint path/to/log.jwj path/to/checkpoint-directory
```

`recover` requires the original source fingerprint and creates a new recovery artifact with create_new, not a DOCX. It restores an independent DocxSession; C07 may still refuse DOCX export of structural edits. `inspect` prints record count, valid-prefix bytes and tail reason. `crash-worker SOURCE NEW_LOG` is a test-only helper: it syncs two Unicode edits, prints READY, then parks until its parent terminates it.

Checkpoints copy the verified baseline + events (no history compression). Two slots rotate through a create_new pending file, flush/sync, readback + real replay, close handle, then rename into the older slot. The source log and other complete generation are retained. Divergent/stale histories are refused; a leftover pending artifact is preserved and requires an explicit new directory/review rather than automatic deletion. The bound is source log + two 32 MiB generations + one 32 MiB pending file. Periodicity is caller controlled; no scheduler/background monitor exists.

After publication, directory-sync/hook errors carry `Error::Published { path, cause }`. Unix directory sync is attempted and reported; Windows returns directory_synced=false and native rename/open-handle behavior remains unverified. IO fault injection and child kill are not actual power-loss tests. Exclusive ownership of the source log and checkpoint directory is required; no interprocess writer synchronization or hostile-directory race guarantee is claimed.

References: https://doc.rust-lang.org/std/fs/struct.File.html and https://doc.rust-lang.org/std/io/trait.Write.html . The format and state machine are original.
