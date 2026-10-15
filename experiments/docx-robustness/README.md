# W09 bounded DOCX robustness laboratory

Independent Rust 1.98.1 workspace, MIT, unsafe forbidden. Targets: `opc`, `import`, `roundtrip`. The production target is fixed at C07 a88217136a570a8f224d92b808bc0b8840ab26ca; build.rs refuses a differing crates tree. No W05/W07 moving-branch integration is assumed. Inputs are capped before parsing. Each target reports schema-1 JSON: case, seed, input hash, exact production SHA, budget, outcome and invariant count. Exit 0 means checks passed, 2 means input was rejected under the declared subset/budget, 3 means invariant failure; unexpected panics crash the child process.

`roundtrip` checks unchanged package identity, real engine text transactions, failed-edit atomicity/protection, verified export and determinism, reimported text/format groups (ignoring nonsemantic run tokenization and resolving preserved handles to bytes), unknown parts/relationships, undo/redo. No external relationships are fetched. Structural text edits are not enabled in the C07 exporter.

Build and invoke:

```sh
cargo build --manifest-path experiments/docx-robustness/Cargo.toml --locked
experiments/docx-robustness/target/debug/joyword-docx-robustness roundtrip W09-smoke 0 tests/fixtures/docx/DOCX-MIN-001.docx
python3 experiments/docx-robustness/matrix.py experiments/docx-robustness/seeds
```

matrix.py creates 27 original deterministic ZIP/XML cases: Unicode, unknown binary data, external HTTP/file/UNC relations, cycle, protection, tracked changes, revision, signature, unknown namespace, AlternateContent/MustUnderstand, DTD/entity, depth/node budgets, duplicate/case-equivalent/traversal names, compressed expansion ratio, CRC/size/encryption/method/overlap/truncation. The manifest records every source and SHA-256. Generator uses Python stdlib zipfile, not an Office implementation. Existing DOCX-MIN test fixtures are read-only and are attributed by path.

Exact dependencies are limited to the task-card approved crates; all external versions in the independent lock also exist in the production lock. No dependency upgrades, root-toolchain changes or production fixes.
