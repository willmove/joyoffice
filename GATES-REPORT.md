# Gate hardening report (T111 follow-up)

Branch `chore/gate-hardening-20261022`, base `main` at `dd5d0dd`. Scope: `scripts/` only.
No file under `crates/`, `apps/`, `docs/`, `.github/`, `tests/fixtures/` or `experiments/`
was touched. No `cargo` command was run (five sibling worktrees share `CARGO_TARGET_DIR`).

## 1. What changed

`scripts/check_gates.py`

| Blind spot | Fix |
| --- | --- |
| 1 — layer check parsed nothing | New `dependency_tiers()` understands all three Cargo spellings (`name.workspace = true`, `name = { ... }`, `[dependencies.name]`) plus `[target.'cfg(...)'.dependencies]` / `.dev-dependencies` / `[build-dependencies]` (build deps are held to the normal tier). `DEP_KEY_RE` replaces `DEP_RE`. Normal deps are checked strictly against `LAYERS`, no baseline. Dev deps are checked against `LAYERS ∪ DEV_LAYERS`, a new explicitly registered allowance dict with a per-entry comment naming the manifest line that needs it. `DEV_LAYERS` keys and values are validated against `LAYERS` so a typo cannot silently widen it. `manifest_layer_problems()` shares the new parser. |
| 2a — `"allow" not in line` bypass | Removed. `lint_suppressed()` only honours `#[allow(...)]` / `#[expect(...)]` on the same line or the immediately preceding line **and** only when the attribute names a panic-family lint (`unwrap_used`, `expect_used`, `panic`, `unreachable`, `todo`, `unimplemented`, `indexing_slicing`). This is deliberately narrower than the brief: `#[allow(dead_code)]` above an `unwrap` no longer hides it. |
| 2b — set comparison undercounted | `check_panic()` is now a `Counter` comparison per `(file, code text)` payload. The message names both counts and every current line number. Line-number drift stays tolerated. |
| 2c — dead code | `extra = sorted(set(current) - set(expected))` is gone. |
| 2d — `--write-baseline` vs `check_panic` scope | Both now call `gated_panic_sites()` / `panic_scope()`. `panic_scope` = "not a real test path" (`tests` path component or `tests.rs`), which is the stricter of the two old predicates, so regenerating cannot widen or narrow what is gated. New `--update-baseline` appends only the surplus (via `missing_baseline_entries()`), keeping existing lines byte-identical; `--write-baseline` keeps its full-rewrite meaning. |
| 2e — `todo!` / `unimplemented!` | Added to `PANIC_RE`. `assert!`/`assert_eq!`/`debug_assert!` were **not** added, per instruction. |
| 3a — `split("#[cfg(test)]")[0]` | Replaced by `test_module_ranges()`: locate `#[cfg(test)]` (also `cfg(all(test, ...))`) whose next non-attribute line is `mod NAME {`/`mod NAME;`, then brace-match the body on `mask_rust()` output (comments, string/char/raw literals blanked, offsets preserved) so braces inside literals cannot move the boundary. `production_lines()` scans everything else. `#[cfg(test)]` on a `use`/field/fn hides nothing. |
| 3b — `FS_RE` holes | Now covers `OpenOptions::new`, `File::open/create/open_options`, `read_dir`, `DirBuilder`, `remove_dir[_all]`, `hard_link`, `symlink`/`symlink_file`/`symlink_dir`/`symlink_metadata`, `canonicalize`, `read_link`, `std::process::Command`, `Command::new`, `env::set_var`/`remove_var`. Patterns are call/import forms, so prose in a doc comment (e.g. "dangling symlinks") does not trip the allowlist. **`FS_ALLOW` was not modified.** |
| 3c — `"tests/" in shown` | Replaced by `is_test_path()` (a `tests` *path component*, or a file named `tests.rs`) and `is_example_or_bin()` (an `examples` component, or `src/bin/`). |
| 4 — one counterexample shape | 4 new fixtures under `scripts/tests/fixtures/` and 38 tests in `scripts/tests/test_check_gates.py`. `.github/workflows/ci.yml:48` picks them up automatically; `ci.yml` was not edited and the `ci.yml:46` invocation of `gate_counterexample.rs` still works unchanged. |

Also: `rust_files()` is now sorted (deterministic error order); filesystem errors carry
`file:line: code` instead of just the file; `panic_sites()` splits on `"\n"` rather than
`str.splitlines()` (verified no file in `crates/`+`apps/` contains `\r`, `\v`, `\f`,
`\x1c`–`\x1e`, `\x85`, `\u2028` or `\u2029`, so numbering is unchanged).

`scripts/panic_baseline.txt`: −12 / +19 lines (details in §3).

## 2. Verification (verbatim)

### 2.1 `python scripts/check_gates.py`

Before (on `dd5d0dd`, unmodified tree):

```
gates ok (216 rust files)
```

After:

```
filesystem access outside the allowlist: apps/joyword/src/linux_files.rs:11: use std::process::Command;
filesystem access outside the allowlist: apps/joyword/src/linux_files.rs:198: let mut command = Command::new(&argv[0]);
```

Exit status 1. **This is the single reported offender — see §4. It needs a human
`FS_ALLOW` decision; I did not add it.** Every other gate (layers incl. dev layers,
AI isolation, no-panic multiplicity) is green.

### 2.2 `python scripts/check_gates.py --counterexample scripts/tests/fixtures/gate_counterexample.rs`

Unchanged fixture, still rejected by all three checks:

```
joyword-model depends on joyword-shell, which its layer does not allow
filesystem access outside the allowlist: crates/joyword-model/src/lib.rs:6: let _ = std::fs::write("secret", "data");
new panic site: crates/joyword-model/src/lib.rs:let _value = Some(1).unwrap(); occurs 1x but the baseline lists it 0x (lines 7); append the missing entries to scripts/panic_baseline.txt
```

Exit status 0.

### 2.3 Per-blind-spot counterexample runs

`python -m unittest scripts.tests.test_check_gates` → `Ran 38 tests ... OK`

Run against the **pre-hardening** script (`git show HEAD:scripts/check_gates.py`, i.e.
`dd5d0dd`): `Ran 38 tests ... FAILED (failures=14, errors=14)` — **28 of 38 fail
before, 0 fail after.** Per-test before/after:

| Test (file `scripts/tests/test_check_gates.py`) | Fixture | Before | After |
| --- | --- | --- | --- |
| `LayerGate.test_dependency_tiers_reads_every_cargo_spelling` | `gate_layers_counterexample.toml` | ERROR | ok |
| `LayerGate.test_dependency_tiers_reads_the_inline_table_form` | (inline manifest) | ERROR | ok |
| `LayerGate.test_dotted_normal_dependency_is_rejected` | `gate_layers_counterexample.toml` | FAIL | ok |
| `LayerGate.test_target_specific_dependency_is_rejected` | `gate_layers_counterexample.toml` | FAIL | ok |
| `LayerGate.test_unregistered_dev_dependency_is_rejected` | `gate_layers_counterexample.toml` | FAIL | ok |
| `LayerGate.test_dev_layers_entries_name_registered_crates` | real tree | ERROR | ok |
| `LayerGate.test_bare_inline_table_dependency_is_still_rejected` | `gate_layers_counterexample.toml` | ok | ok |
| `LayerGate.test_registered_dev_allowance_is_accepted` | (inline manifest) | ok | ok |
| `LayerGate.test_unregistered_crate_still_errors` | (inline manifest) | ok | ok |
| `PanicGate.test_a_comment_mentioning_allow_does_not_hide_a_site` | `gate_panic_allow_bypass.rs` | FAIL | ok |
| `PanicGate.test_the_word_allow_inside_a_message_does_not_hide_a_site` | `gate_panic_allow_bypass.rs` | FAIL | ok |
| `PanicGate.test_todo_and_unimplemented_are_sites` | `gate_panic_allow_bypass.rs` | FAIL | ok |
| `PanicGate.test_a_genuine_lint_allowance_still_suppresses` | `gate_panic_allow_bypass.rs` | FAIL | ok |
| `PanicGate.test_duplicate_sites_need_one_baseline_entry_each` | `gate_panic_allow_bypass.rs` | FAIL | ok |
| `PanicGate.test_write_baseline_and_check_panic_share_one_scope` | (temp tree) | ERROR | ok |
| `PanicGate.test_missing_baseline_entries_is_exact` | (temp tree) | ERROR | ok |
| `PanicGate.test_repo_baseline_has_no_entry_for_code_that_is_gone` | real tree | ERROR | ok |
| `PanicGate.test_an_unrelated_allowance_suppresses_nothing` | `gate_panic_allow_bypass.rs` | ok | ok |
| `PanicGate.test_a_fully_inventoried_duplicate_passes` | `gate_panic_allow_bypass.rs` | ok | ok |
| `PanicGate.test_repo_baseline_covers_every_gated_site` | real tree | ok | ok |
| `TestModuleRanges.*` (6 tests) | (inline sources) | ERROR ×6 | ok |
| `FilesystemGate.test_production_code_below_a_test_module_is_scanned` | `gate_filesystem_hidden.rs` | FAIL | ok |
| `FilesystemGate.test_only_the_test_module_body_is_skipped` | `gate_filesystem_hidden.rs` | FAIL | ok |
| `FilesystemGate.test_env_set_var_is_an_entry_point` | (inline source) | FAIL | ok |
| `FilesystemGate.test_a_lookalike_tests_path_is_not_skipped` | `gate_filesystem_lookalike.rs` | FAIL | ok |
| `FilesystemGate.test_a_tests_rs_module_is_still_skipped` | `gate_filesystem_lookalike.rs` | FAIL | ok |
| `FilesystemGate.test_repo_files_below_a_test_module_are_really_scanned` | real tree | ERROR | ok |
| `FilesystemGate.test_a_real_test_directory_is_still_skipped` | `gate_filesystem_lookalike.rs` | ok | ok |
| `FilesystemGate.test_examples_are_still_skipped` | `gate_filesystem_lookalike.rs` | ok | ok |
| `FilesystemGate.test_repo_offenders_are_only_the_reported_one` | real tree | ok | ok |
| `CounterexampleCommand.test_layer_fixture_is_rejected_through_the_shared_tree` | `gate_layers_counterexample.toml` | ERROR | ok |
| `CounterexampleCommand.test_manifest_layer_problems_understands_the_dotted_form` | `gate_layers_counterexample.toml` | FAIL | ok |
| `CounterexampleCommand.test_legacy_counterexample_still_trips_all_three_checks` | `gate_counterexample.rs` | ok | ok |

### 2.4 `python -m unittest discover -s scripts/tests`

```
.............................................................F.............................................
======================================================================
FAIL: test_committed_bytes_and_manifest_hashes_match (test_make_feature_fixtures.FeatureFixtureGeneratorTests.test_committed_bytes_and_manifest_hashes_match)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "D:\Coding\EditorProjects\joyoffice-wt\gates\scripts\tests\test_make_feature_fixtures.py", line 42, in test_committed_bytes_and_manifest_hashes_match
    self.assertEqual(problems, [])
AssertionError: Lists differ: ['DOCX-FEAT-T200-NUM: committed bytes diff[988 chars]20)'] != []
...
Ran 107 tests in 22.189s

FAILED (failures=1)
```

107 tests = the 69 pre-existing + my 38. The single failure is the **known
environmental zlib-ng failure** described in the task: this machine runs
`Python 3.14.7` with `zlib.ZLIB_VERSION = zlib.ZLIB_RUNTIME_VERSION = 1.3.1.zlib-ng`
(verified with `python -c "import zlib; print(zlib.ZLIB_VERSION, zlib.ZLIB_RUNTIME_VERSION)"`),
whose level-6 deflate bytes differ from CI's stock zlib, so 5 DOCX-FEAT fixture
hashes differ. Not counted against this change; `scripts/make_feature_fixtures.py`,
`scripts/tests/test_make_feature_fixtures.py` and `tests/fixtures/feature/*.docx` were
not touched. Excluding it: **106 pass, 0 fail.**

### 2.5 `python scripts/check_docs.py` (unchanged, run for safety)

```
OK: 84 requirements; 66 tasks; acyclic dependencies; stage-consistent; M0–M7 required by T704; 37 Markdown files; 15 registered cases; locale parity; 12 workflow actions pinned by SHA; 53 behavior registry entries; feature catalog floor 59.
```

### 2.6 Baseline tooling consistency

```
python scripts/check_gates.py --write-baseline   -> wrote 561 sites   (full rewrite, not committed)
python scripts/check_gates.py --update-baseline  -> appended 0 sites  (after the rewrite)
python scripts/check_gates.py --update-baseline  -> appended 19 sites (from the committed 554-line baseline)
```

The curated baseline (554 − 12 + 19 = 561 entries) has exactly the same
`(file, code)` multiset as a full rewrite, but its diff is 31 lines instead of 554.

## 3. Baseline diff (−12 / +19)

**Appended, 19 entries.** All 19 are inside `#[cfg(test)]` module bodies, i.e. T111
category **1 — test/example code** (verified programmatically against
`test_module_ranges`):

| File | Lines | Enclosing `#[cfg(test)]` module | Category |
| --- | --- | --- | --- |
| `apps/joyword/src/input_ui.rs` | 4395, 4409, 4411, 4441, 4456, 4507 | 4324–4933 | 1 test code |
| `apps/joyword/src/linux_files.rs` | 327 | 240–348 | 1 test code |
| `crates/joyword-shell/src/storage/metadata.rs` | 404, 406, 411 | 300–456 | 1 test code |
| `crates/joyword-shell/src/storage/original.rs` | 1232, 1546, 1566, 1568, 1591, 1808 | 1075–1827 | 1 test code |
| `crates/joyword-shell/src/storage/source.rs` | 919, 935, 955 | 780–980 | 1 test code |

No production site was added. These are surplus *duplicates* of payloads the baseline
already listed once or twice (`let dir = tempfile::tempdir().unwrap();`,
`std::fs::write(&path, &bytes).unwrap();`, bare `.unwrap()` continuation lines, …),
which the old set comparison silently absorbed. Categories 2–4 ("check before use"
invariants, resource-exhaustion guards, static input / startup assembly, test
injection hooks) gained nothing.

**Removed, 12 entries** whose code text no longer exists anywhere in the gated scope
(verified by grep; they are refactor leftovers, not drift):

```
crates/joyword-shell/src/input.rs:232:let content = p.plain_text(|_| unreachable!("objects rejected"));
crates/joyword-shell/src/input.rs:287:.expect("indexed paragraph");
crates/joyword-shell/src/input.rs:317:.expect("indexed paragraph");
crates/joyword-shell/src/input.rs:333:.expect("indexed paragraph");
crates/joyword-shell/src/storage/original.rs:1304:assert_eq!(std::fs::read(error.retained_backup().unwrap()).unwrap(), bytes);
crates/joyword-model/src/block.rs:282:offset => self.run_at(offset - 1).expect("offset within runs").0,
crates/joyword-layout/src/document.rs:315:let paragraph = block.as_paragraph().expect("preflight paragraphs");
crates/joyword-layout/src/document.rs:316:let text = paragraph.plain_text(|_| unreachable!("preflight objects"));
crates/joyword-layout/src/document.rs:568:let paragraph = block.as_paragraph().expect("preflight checked all blocks");
crates/joyword-layout/src/document.rs:597:let text = paragraph.plain_text(|_| unreachable!("objects rejected"));
crates/joyword-layout/src/document.rs:753:if needs_page && !result.pages.last().expect("one page").runs.is_empty() {
crates/joyword-layout/src/document.rs:801:let page = result.pages.last_mut().expect("one page");
```

Rationale for removing rather than leaving them: a stale entry is a bypass — anyone
re-introducing that exact line in that exact file would sail through. With
multiplicity now meaningful, keeping the file accurate matters more.
`PanicGate.test_repo_baseline_has_no_entry_for_code_that_is_gone` pins this so the
baseline cannot rot silently again. The remaining 542 lines are byte-identical,
including their drifted line numbers.

## 4. Newly surfaced violations on main and their disposition

| # | Gate | Finding on `main` (`dd5d0dd`) | Disposition |
| --- | --- | --- | --- |
| 1 | layers, normal deps | none. Every `[dependencies]` and `[target.'cfg(...)'.dependencies]` edge in all 13 manifests complies with `LAYERS` | nothing to do (confirms the diagnosis) |
| 2 | layers, dev deps | `joyword-docx` → `joyword-corpus`, `crates/joyword-docx/Cargo.toml:17` | `DEV_LAYERS` entry |
| 3 | layers, dev deps | `joyword-layout` → `joyword-engine`, `crates/joyword-layout/Cargo.toml:20` | `DEV_LAYERS` entry |
| 4 | layers, dev deps | `joyword-layout` → `joyword-render`, `crates/joyword-layout/Cargo.toml:21` | `DEV_LAYERS` entry |
| 5 | layers, dev deps | `joyword-pdf` → `joyword-engine`, `crates/joyword-pdf/Cargo.toml:21` | `DEV_LAYERS` entry |
| 6 | layers, dev deps | `joyword-render` → `joyword-pdf`, `crates/joyword-render/Cargo.toml:22` — **not in the brief's list of four** | `DEV_LAYERS` entry, needs your review |
| 7 | layers, dev deps | `joyword-cli` → `joyword-corpus`, `apps/joyword-cli/Cargo.toml:22` — **not in the brief's list of four** | `DEV_LAYERS` entry, needs your review |
| 8 | panic, multiplicity | 19 surplus duplicate sites across 5 files (§3) | appended to `scripts/panic_baseline.txt`, all category 1 |
| 9 | panic, stale baseline | 12 entries for code that no longer exists (§3) | removed |
| 10 | panic, `"allow"` bypass | **0** sites newly visible. No line inside the gated scope contains `allow` together with a panic pattern; the only two in the tree (`crates/joyword-agent/tests/contract.rs:107`, `crates/joyword-pdf/tests/export_pdf.rs:188`) are excluded by the tests-dir rule. Also 0 `#[allow(...)]`/`#[expect(...)]` attributes anywhere in `crates/`+`apps/` sit on, or immediately above, a panic line (all 36 are `clippy::too_many_arguments` / `dead_code`) | bypass closed, nothing to inventory |
| 11 | panic, `todo!`/`unimplemented!` | **0** occurrences in `crates/` and `apps/` | regex extended, nothing to inventory |
| 12 | filesystem | `apps/joyword/src/linux_files.rs:11` (`use std::process::Command;`) and `:198` (`Command::new(&argv[0])`) | **REPORTED, LEFT RED — see below** |
| 13 | filesystem, `#[cfg(test)]` truncation | 44 files were being truncated. The newly visible production code contains no filesystem/process entry point outside `FS_ALLOW` other than #12 | nothing to do |
| 14 | filesystem, `tests/` substring | no path in the tree was actually mis-skipped by the substring rule: `apps/joyword/src/tests.rs` is the only `tests` path outside a `tests/` directory, and it is the `#[cfg(test)] #[path = "tests.rs"] mod find_bar_tests;` body declared at `apps/joyword/src/main.rs:2467-2469`. The filesystem gate had no `tests.rs` rule, so that file was being scanned as production code and escaped only because its own doc comment mentions `#[cfg(test)]` at line 3 (see §6.5) | predicate tightened and shared with the panic gate, fixture covers it |

### #12 in detail — the one red item, needs a human decision

`apps/joyword/src/linux_files.rs` is unconditional production code in the GUI app
(`mod linux_files;` at `apps/joyword/src/main.rs:9`). `run_program()` (line 190) builds
an argv for the first installed helper among zenity/qarma/kdialog, resolves it against
`PATH` (`run_first_dialog()`, line 167) and spawns it with `std::process::Command`
(line 198), then reads the child's stdout/stderr. That is exactly the class of
capability `FS_ALLOW` exists to enumerate, and it is not listed.

Why T111's review missed it: `FS_RE` did not cover `process::Command`, and the file's
`std::fs::remove_dir_all` calls are all inside its `#[cfg(test)]` module (lines
240–348). Note the `#[cfg(test)]` truncation was *not* the reason — the marker sits at
line 240, after the `use` at line 11 — so this finding comes purely from the extended
regex.

Precedent for allowing it: `apps/joyword/src/input_ui.rs:1633` already spawns an
external program with `std::process::Command::new(program)` to open a URL, and
`input_ui.rs` **is** in `FS_ALLOW`. `linux_files.rs` is the same capability in the same
crate.

Per instruction I did **not** add it to `FS_ALLOW`. To turn the gate green, a human
reviewer must add exactly this line to the `FS_ALLOW` set in
`scripts/check_gates.py` (after `"apps/joyword/src/input_ui.rs",`):

```python
    "apps/joyword/src/linux_files.rs",
```

Nothing else is needed; `FilesystemGate.test_repo_offenders_are_only_the_reported_one`
is written as a ratchet (`assertLessEqual` semantics) and keeps passing either way.

## 5. Deliberately not changed

- **`FS_ALLOW`** — untouched. Human-reviewed security boundary; the one offender is
  reported in §4 instead.
- **`assert!` / `assert_eq!` / `debug_assert!`** — not added to `PANIC_RE`, per
  instruction. They are legitimate "check then use" guards throughout the tree and
  would flood the baseline.
- **`docs/`** — untouched. Consequence: `docs/joyword/ARCHITECTURE.md:22` still says the
  layer table is "由自动检查固化" without mentioning that `[dev-dependencies]` edges are
  governed by a separate reviewed allowance list. `DEV_LAYERS` is documented in code
  (a comment block above the dict) and in §1/§4 here. Someone with docs scope should
  add one sentence to ARCHITECTURE §1 and a line to the T111 record in
  `docs/joyword/IMPLEMENTATION.md`. `docs/joyword/TASKS.md:267` also still quotes "基线
  513 条"; the file has been 554 lines since before this change and is 561 now.
- **`.github/workflows/ci.yml`** — untouched, as required. Line 46 still passes the
  unchanged `gate_counterexample.rs`; line 48 discovers the new tests automatically.
- **`examples/` and `src/bin/`** remain outside the filesystem gate (pre-existing
  behaviour, preserved). For the record, `crates/joyword-corpus/src/bin/generate_corpus.rs`
  writes files via `std::fs::create_dir_all` / `std::fs::write` and
  `crates/joyword-model/examples/text_storage.rs` spawns `ps` and `current_exe()`;
  both are excluded by that rule, not by `FS_ALLOW`.
- **`panic_sites()` still scans `#[cfg(test)]` module bodies.** T111 inventoried those
  sites (roughly half the baseline), and `apps/joyword/src/main.rs:2465-2466` documents
  the intent in a code comment. Excluding them would halve the baseline and weaken the
  gate; that is a separate policy decision.
- **`check_panic()` stays one-directional** (a baseline may list more than the tree
  has) — the drift tolerance the baseline exists for. The 12 entries that were stale
  *today* were pruned, and a test now pins "no stale entries".
- **`--write-baseline` keeps its full-rewrite semantics.** `--update-baseline` was added
  beside it rather than changing it, so the two modes stay distinguishable and the
  reviewable-diff path is the default for top-ups.
- **No `cargo` / `git worktree` / `cargo clean` command was run.**

## 6. Corrections to the diagnosis in the task brief

1. **`DEV_LAYERS` needs six pairs, not four.** `joyword-render` dev-depends on
   `joyword-pdf` (`crates/joyword-render/Cargo.toml:22`) and `joyword-cli` dev-depends
   on `joyword-corpus` (`apps/joyword-cli/Cargo.toml:22`). Seeding only the four listed
   pairs would have left the gate red on two pre-existing, legitimate edges. All six are
   registered with per-entry comments; #6 and #7 in §4 need your explicit review.
   (`joyword-pdf` dev-dep `joyword-model` at `:20` and `joyword-render` dev-deps
   `joyword-foundation`/`joyword-model` at `:20-21` are already allowed by `LAYERS`, so
   they need no `DEV_LAYERS` entry.)
2. **The baseline is 554 lines on `main`, not 513** (the T111 number in
   `docs/joyword/TASKS.md:267`), and it had drifted in both directions: 12 entries for
   code that no longer exists, and 19 un-inventoried surplus duplicates. So blind spot
   2b was not merely theoretical — it was actively concealing 19 sites on the day this
   was written.
3. **Blind spot 2a currently fires on nothing.** The `"allow" not in line` bypass
   matched zero lines inside the gated scope, so fixing it inventories nothing. It is a
   closed bypass, not a closed violation.
4. **Blind spot 2e also fires on nothing.** `todo!` and `unimplemented!` appear nowhere
   in `crates/` or `apps/`.
5. **The `"tests/"` substring examples in the brief would not have been skipped.**
   `"tests/" in shown` is false for `src/tests_helper.rs` (no slash after `tests`) and
   false for `latest/` (`latest/` contains `test` but not `tests/`). The realistic
   mis-skip is a directory whose name *ends* in `tests` — `src/contests/`, `src/greatest/`
   — which is what `gate_filesystem_lookalike.rs` pins. No such path exists in the tree
   today, so this is a closed hole rather than a live violation. Conversely the brief did
   not mention a real asymmetry: the filesystem gate had **no** `tests.rs` rule at all
   (only the panic gate did), so `apps/joyword/src/tests.rs` was gated as production
   code. It escaped only by accident, and the accident is worth recording: the file's
   first `#[cfg(test)]` occurrence is *inside its own doc comment* at line 3
   (`//! The no-panic gate scans \`main.rs\` in full, including \`#[cfg(test)]\`.`), so
   `text.split("#[cfg(test)]", 1)[0]` truncated all 1821 lines at line 3. It is the only
   file in the tree where the first marker is in a comment. The sentence in that doc
   comment describing the gate is what disabled the gate for that file.
6. **The `#[cfg(test)]` truncation is worse than "production code below a test
   module".** `#[cfg(test)]` is used throughout this repo on `use` statements, struct
   fields and individual fns, not just on test modules. `apps/joyword/src/document_canvas.rs:6`
   is a `#[cfg(test)] use joyword_engine::{...};`, so the old split discarded **1939 of
   1945 lines** of that file. 44 files were truncated. Because all of the badly truncated
   ones happen to be in `FS_ALLOW` or contain no FS calls, the only thing this surfaced
   is #12 — but the hole was much larger than described.
7. **The blind-spot-3a fix is not what caught #12.** `linux_files.rs`'s `#[cfg(test)]`
   marker is at line 240, after both offending lines, so the extended `FS_RE` alone
   catches it. Worth knowing because it means the two sub-fixes are independent.
8. Minor: `check_panic()`'s dead `extra` was at line 201 as stated — confirmed and
   removed.
