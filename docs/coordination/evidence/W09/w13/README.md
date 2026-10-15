# W13 reproducible evidence

Production target: main `01604919f89e84e9bf5e1039ac5a4290fbcbef05` (PR #15 merge; crates tree unchanged from that commit).
Harness source: `0e22a294767b3665c08b905997c0477aae75007f`, clean experiment tree at execution (`harness_worktree_dirty=false`).
Release binary SHA-256: `d36ad50f42e6eaa4ec5b23dd7e086cee05bb8fff5dfcc999f6eba1186ea8d649`.
Host: Linux 6.12.94+ x86_64, Python 3.12.3. The address-space probe exited 0, so the driver enabled a 512 MiB `RLIMIT_AS` cap. RSS figures are the OS cumulative maximum across child processes, including the probe, not a per-case peak.

Historical files in the parent directory stay bound to the original W09 run. They are not evidence for this `TARGET_SHA`.

- smoke-summary.json + matrix-results.json: 27 original seeds × 3 targets, 81 executions. 43 ok, 38 rejected, 0 crash/timeout/invariant/protocol.
- campaign-summary.json + campaign-cases.jsonl: 600 deterministic mutations, seed 20261015, capped at 120 seconds and 3 seconds per input. 91 ok, 509 rejected, 0 anomalies. All 600 input byte streams were regenerated from the seed and matched the recorded SHA-256.
- replay.json: stdin replay of `seeds/minimal.docx` through roundtrip. class=ok, 9 checks.

From the repository root, after the harness commit above:

```sh
cargo build --manifest-path experiments/docx-robustness/Cargo.toml --release --locked
PYTHONDONTWRITEBYTECODE=1 python3 experiments/docx-robustness/driver.py smoke --binary experiments/docx-robustness/target/release/joyword-docx-robustness --out experiments/docx-robustness/target/w13-smoke-final
PYTHONDONTWRITEBYTECODE=1 python3 experiments/docx-robustness/driver.py campaign --binary experiments/docx-robustness/target/release/joyword-docx-robustness --seed 20261015 --iterations 600 --seconds 120 --out experiments/docx-robustness/target/w13-campaign-final
PYTHONDONTWRITEBYTECODE=1 python3 experiments/docx-robustness/driver.py replay --binary experiments/docx-robustness/target/release/joyword-docx-robustness --target roundtrip --seed 0 --input experiments/docx-robustness/seeds/minimal.docx
```

Elapsed times and cumulative child RSS are observational. This is fixed-seed mutation testing with no coverage feedback. Native Windows, GUI and Office round-trips remain unverified.
