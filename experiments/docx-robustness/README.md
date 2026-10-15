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

The stdlib-only driver is mutation testing without coverage feedback:

```sh
python3 experiments/docx-robustness/driver.py smoke --binary experiments/docx-robustness/target/debug/joyword-docx-robustness --out experiments/docx-robustness/target/smoke
python3 experiments/docx-robustness/driver.py campaign --binary experiments/docx-robustness/target/debug/joyword-docx-robustness --seed 20261015 --iterations 600 --seconds 120 --out experiments/docx-robustness/target/campaign
python3 experiments/docx-robustness/driver.py replay --binary experiments/docx-robustness/target/debug/joyword-docx-robustness --target roundtrip --seed 0 --input experiments/docx-robustness/seeds/minimal.docx
python3 experiments/docx-robustness/driver.py minimize --binary experiments/docx-robustness/target/debug/joyword-docx-robustness --target roundtrip --seed 0 --input INPUT.docx --class invariant --max-runs 128 --out NEW-minimized.docx
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s experiments/docx-robustness/tests -p 'test_*.py' -v
```

Use a new output directory per campaign. The driver caps input, output retained in memory, per-child timeout, minimizer runs, total campaign iterations/time and saved failures (stop at eight). It records JSONL results and summary with campaign seed, mutation, input hash, exact production SHA, harness Git SHA and binary hash, budget, exit, OS and replay arguments. No panic is caught as success. stdin is binary and commands are argument lists.

Timeouts kill/reap the new process group on POSIX; Windows uses taskkill /T if available, otherwise only the direct child is controlled and that limitation is reported. An RLIMIT_AS allocation probe runs before enabling address-space limits; Windows has no Job Object backend. Capability must be read from summary, never inferred from platform name. The RSS measurement is the OS cumulative maximum of all child processes in the driver lifetime (including the probe), not per-case memory or a hard RSS limit. stdout/stderr are temporary files, retained excerpts are capped at 64 KiB each. This does not provide a hard scratch-disk quota for arbitrary third-party binaries; the intended harness emits one bounded JSON line.

Minimization starts only on a reproducing crash/timeout/invariant/protocol failure and retains that same category. Unit tests include an actual nonzero child minimized to `BUG`, binary stdin, Unicode/space paths, descendants terminated on timeout, malformed protocol and output limits. Synthetic failures are driver tests, not production bugs.

Primary sources: https://docs.python.org/3/library/subprocess.html and https://docs.python.org/3/library/resource.html . Mutation operators and delta reduction are independently implemented. External coverage-guided drivers can invoke the three target commands with stdin bytes or call the Rust `run` function; exit/JSON and limits must remain checked. Installing a coverage engine, dependencies or a nightly toolchain requires C08's explicit tool/version/license review; the fixed production toolchain is unchanged.

The generated text sequence contains up to four engine transactions; a separate Unicode character-vector reference is updated after every insertion/deletion. The harness undoes/redoes the complete committed sequence, not a private replacement undo stack. Validated text serialization may split an original run at tabs; the checker normalizes those token boundaries while continuing to compare properties and kept fragment bytes.
