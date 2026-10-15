# W09 reproducible evidence

Production target: C07 `a88217136a570a8f224d92b808bc0b8840ab26ca` (crates tree unchanged).
Harness source: `db719a671a07b882ec50925a33c837fc49b50c00`, clean experiment tree at execution.
The two summaries bind the release binary SHA-256, OS/Python, exact production and harness revisions, seed, time/count limits, address-space capability and RSS measurement scope.

- smoke-summary.json + matrix-results.json: 27 original seeds × 3 targets, 81 executions.
- campaign-summary.json + campaign-cases.jsonl: 600 deterministic mutations, seed 20261015, capped at 120 seconds/3 seconds per input. All 600 input byte streams were independently regenerated from the seed and matched to their recorded SHA-256 after execution.
- replay.json: actual one-click stdin replay of the minimal valid DOCX through the real engine/export/reimport chain.
- No production anomaly was found. No minimal production failure is invented; the synthetic BUG minimizer belongs only to driver tests.

From the repository root, reproduce with a fresh output directory:

```sh
cargo build --manifest-path experiments/docx-robustness/Cargo.toml --release --locked
python3 experiments/docx-robustness/driver.py smoke --binary experiments/docx-robustness/target/release/joyword-docx-robustness --out experiments/docx-robustness/target/reproduce-smoke
python3 experiments/docx-robustness/driver.py campaign --binary experiments/docx-robustness/target/release/joyword-docx-robustness --seed 20261015 --iterations 600 --seconds 120 --out experiments/docx-robustness/target/reproduce-campaign
python3 experiments/docx-robustness/driver.py replay --binary experiments/docx-robustness/target/release/joyword-docx-robustness --target roundtrip --seed 0 --input experiments/docx-robustness/seeds/minimal.docx
```

Elapsed times, per-process scheduling and cumulative child RSS are observational, not deterministic assertions. This is mutation testing with no coverage feedback and no active OS memory cap on this host (probe exit 3). Native Windows, GUI and Office round-trips remain unverified. The campaign ended normally, with no monitor or background service.
