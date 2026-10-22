"""Counterexamples for scripts/check_gates.py.

Every test documents whether it failed against the pre-hardening script
(``main`` at dd5d0dd). "fails before" means the blind spot was real: the check
returned no finding for a fixture that must be rejected. Tests without that
marker are regression guards for behaviour that already worked, or integration
checks over the real tree.
"""
import importlib.util
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
FIXTURES = HERE / "fixtures"

SPEC = importlib.util.spec_from_file_location("check_gates", ROOT / "scripts" / "check_gates.py")
check_gates = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check_gates)

# The one filesystem offender the hardened scan surfaces on main. It is reported
# to the reviewer instead of being added to FS_ALLOW, which is a human-reviewed
# security boundary; see GATES-REPORT.md. The assertion below is a ratchet, so
# the test keeps passing once that file is allowlisted by a human decision.
REPORTED_FS_OFFENDER = "apps/joyword/src/linux_files.rs"


def fixture(name):
    return (FIXTURES / name).read_text(encoding="utf-8")


class TempTree(unittest.TestCase):
    """A throwaway workspace so the checks can be run against fixtures."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.crates = self.root / "crates"
        self.apps = self.root / "apps"
        self.apps.mkdir(parents=True)

    def write(self, relative, text):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    def layers(self):
        return check_gates.check_layers(self.crates, self.apps)

    def filesystem(self):
        return check_gates.check_filesystem(self.crates, self.apps, self.root)

    def panic(self, baseline_text=""):
        baseline = self.write("panic_baseline.txt", baseline_text)
        return check_gates.check_panic(baseline, self.crates, self.apps, self.root)

    def sites(self, relative, text):
        path = self.write(relative, text)
        return check_gates.panic_sites(path, self.root)


class LayerGate(TempTree):
    LAYERS_FIXTURE = "gate_layers_counterexample.toml"

    def test_dependency_tiers_reads_every_cargo_spelling(self):
        """Fails before: `dependency_tiers` did not exist and the dotted
        `name.workspace = true` form used by every internal dependency in this
        repository was invisible to `DEP_RE`."""
        manifest = self.write("crates/joyword-model/Cargo.toml", fixture(self.LAYERS_FIXTURE))
        tiers = check_gates.dependency_tiers(manifest)
        self.assertEqual(
            tiers["normal"],
            {"joyword-foundation", "joyword-shell", "joyword-agent", "sha2", "joyword-render"},
        )
        self.assertEqual(tiers["dev"], {"joyword-corpus"})

    def test_dependency_tiers_reads_the_inline_table_form(self):
        """Fails before: `[dependencies.name]` was parsed as a section change
        and the package name was dropped."""
        manifest = self.write(
            "crates/joyword-model/Cargo.toml",
            '[package]\nname = "joyword-model"\n\n'
            "[dependencies.joyword-shell]\npath = \"../joyword-shell\"\n\n"
            "[dev-dependencies.joyword-corpus]\npath = \"../joyword-corpus\"\n",
        )
        tiers = check_gates.dependency_tiers(manifest)
        self.assertEqual(tiers["normal"], {"joyword-shell"})
        self.assertEqual(tiers["dev"], {"joyword-corpus"})

    def test_dotted_normal_dependency_is_rejected(self):
        """Fails before: the layer gate reported nothing for the dotted form."""
        self.write("crates/joyword-model/Cargo.toml", fixture(self.LAYERS_FIXTURE))
        errors = self.layers()
        self.assertIn("joyword-model depends on joyword-shell, which its layer does not allow", errors)

    def test_bare_inline_table_dependency_is_still_rejected(self):
        """Regression guard: the spelling the old gate did understand."""
        self.write("crates/joyword-model/Cargo.toml", fixture(self.LAYERS_FIXTURE))
        self.assertIn("joyword-model depends on joyword-agent, which its layer does not allow", self.layers())

    def test_target_specific_dependency_is_rejected(self):
        """Fails before: `[target.'cfg(unix)'.dependencies]` was not a
        dependency section, so a platform-only edge was invisible."""
        self.write("crates/joyword-model/Cargo.toml", fixture(self.LAYERS_FIXTURE))
        self.assertIn("joyword-model depends on joyword-render, which its layer does not allow", self.layers())

    def test_unregistered_dev_dependency_is_rejected(self):
        """Fails before: dev-dependencies were never parsed at all."""
        self.write("crates/joyword-model/Cargo.toml", fixture(self.LAYERS_FIXTURE))
        errors = self.layers()
        self.assertTrue(
            any("dev-depends on joyword-corpus" in error for error in errors),
            errors,
        )

    def test_registered_dev_allowance_is_accepted(self):
        """Guard: a pair registered in DEV_LAYERS is not an error, and a
        dev-dependency already allowed by LAYERS needs no second entry."""
        self.write(
            "crates/joyword-docx/Cargo.toml",
            '[package]\nname = "joyword-docx"\n\n[dependencies]\n'
            "joyoffice-opc.workspace = true\n\n[dev-dependencies]\n"
            "joyword-corpus.workspace = true\njoyword-model.workspace = true\n",
        )
        self.assertEqual(self.layers(), [])

    def test_unregistered_crate_still_errors(self):
        """Guard: a new crate with no LAYERS entry is rejected."""
        self.write(
            "crates/joyword-brand-new/Cargo.toml",
            '[package]\nname = "joyword-brand-new"\n\n[dependencies]\n',
        )
        self.assertTrue(any("unregistered crate joyword-brand-new" in e for e in self.layers()))

    def test_dev_layers_entries_name_registered_crates(self):
        """Fails before: DEV_LAYERS did not exist. A typo in the allowance
        table must not silently widen it."""
        for crate, deps in check_gates.DEV_LAYERS.items():
            self.assertIn(crate, check_gates.LAYERS)
            for dep in deps:
                self.assertIn(dep, check_gates.LAYERS)
        self.assertEqual(check_gates.check_layers(), [], "the real tree must satisfy LAYERS + DEV_LAYERS")


class PanicGate(TempTree):
    FIXTURE = "gate_panic_allow_bypass.rs"

    def fixture_sites(self):
        return self.sites(f"crates/demo/src/lib.rs", fixture(self.FIXTURE))

    def codes(self, sites):
        return [site.split(":", 2)[2] for site in sites]

    def test_a_comment_mentioning_allow_does_not_hide_a_site(self):
        """Fails before: any line containing the substring "allow" was skipped,
        so `value.unwrap() // allow` defeated the gate."""
        codes = self.codes(self.fixture_sites())
        self.assertIn("value.unwrap() // allow", codes)

    def test_the_word_allow_inside_a_message_does_not_hide_a_site(self):
        """Fails before: same substring bypass, through an expect message."""
        codes = self.codes(self.fixture_sites())
        self.assertIn('value.expect("allowlisted by a comment only")', codes)

    def test_todo_and_unimplemented_are_sites(self):
        """Fails before: PANIC_RE covered neither abort-on-purpose macro."""
        codes = self.codes(self.fixture_sites())
        self.assertIn('todo!("not written yet");', codes)
        self.assertIn('unimplemented!("not written yet");', codes)

    def test_a_genuine_lint_allowance_still_suppresses(self):
        """Fails before in the other direction: the old substring rule skipped
        the call line but not because of the attribute, and the hardened rule
        must keep honouring real `#[allow]` / `#[expect]` panic lints."""
        codes = self.codes(self.fixture_sites())
        self.assertNotIn("let inline = value.unwrap();", codes)
        self.assertNotIn("let preceding = value.unwrap();", codes)

    def test_an_unrelated_allowance_suppresses_nothing(self):
        """Guard: `#[allow(dead_code)]` is not a panic-family lint."""
        codes = self.codes(self.fixture_sites())
        self.assertEqual(codes.count("value.unwrap()"), 2)

    def test_duplicate_sites_need_one_baseline_entry_each(self):
        """Fails before: the comparison was a set difference, so three
        identical panic lines passed against a single baseline entry."""
        self.write("crates/demo/src/lib.rs", fixture(self.FIXTURE))
        errors = self.panic('crates/demo/src/lib.rs:1:panic!("duplicated counterexample payload");\n')
        matched = [e for e in errors if "duplicated counterexample payload" in e]
        self.assertEqual(len(matched), 1, errors)
        self.assertIn("occurs 3x", matched[0])
        self.assertIn("lists it 1x", matched[0])

    def test_a_fully_inventoried_duplicate_passes(self):
        """Guard: three identical sites against three baseline entries pass even
        though the recorded line numbers have drifted."""
        self.write("crates/demo/src/lib.rs", fixture(self.FIXTURE))
        drifted = "\n".join(
            f'crates/demo/src/lib.rs:{900 + n}:panic!("duplicated counterexample payload");'
            for n in range(3)
        )
        errors = self.panic(drifted + "\n")
        self.assertEqual([e for e in errors if "duplicated counterexample payload" in e], [])

    def test_write_baseline_and_check_panic_share_one_scope(self):
        """Fails before: `--write-baseline` skipped only `/tests/` while
        `check_panic` also skipped `tests.rs`, so regenerating the baseline
        changed what was gated."""
        self.write("crates/demo/src/lib.rs", "fn a() { Some(1).unwrap(); }\n")
        self.write("crates/demo/src/tests.rs", "fn b() { Some(2).unwrap(); }\n")
        self.write("crates/demo/tests/it.rs", "fn c() { Some(3).unwrap(); }\n")
        self.write("crates/demo/src/contests/rules.rs", "fn d() { Some(4).unwrap(); }\n")
        baseline = self.root / "panic_baseline.txt"
        written = check_gates.write_baseline(baseline, self.crates, self.apps, self.root)
        text = baseline.read_text(encoding="utf-8")
        self.assertEqual(written, 2)
        self.assertIn("crates/demo/src/lib.rs", text)
        self.assertIn("crates/demo/src/contests/rules.rs", text)
        self.assertNotIn("tests.rs", text)
        self.assertNotIn("demo/tests/it.rs", text)
        self.assertEqual(check_gates.check_panic(baseline, self.crates, self.apps, self.root), [])

    def test_missing_baseline_entries_is_exact(self):
        """Fails before: `missing_baseline_entries` did not exist. A top-up must
        append exactly the surplus, counting duplicates per (file, code)."""
        self.write(
            "crates/demo/src/lib.rs",
            'fn a() {\n    panic!("same");\n    panic!("same");\n    panic!("same");\n}\n',
        )
        baseline = self.write("baseline.txt", 'crates/demo/src/lib.rs:9:panic!("same");\n')
        appended = check_gates.missing_baseline_entries(
            baseline, self.crates, self.apps, self.root
        )
        self.assertEqual(len(appended), 2)
        self.assertEqual(
            sorted(appended),
            ['crates/demo/src/lib.rs:3:panic!("same");', 'crates/demo/src/lib.rs:4:panic!("same");'],
        )
        baseline.write_text(
            baseline.read_text(encoding="utf-8") + "\n".join(appended) + "\n", encoding="utf-8"
        )
        self.assertEqual(check_gates.check_panic(baseline, self.crates, self.apps, self.root), [])

    def test_repo_baseline_covers_every_gated_site(self):
        """Integration: the committed baseline satisfies the multiplicity gate."""
        errors = check_gates.check_panic(ROOT / "scripts" / "panic_baseline.txt")
        self.assertEqual(errors, [])

    def test_repo_baseline_has_no_entry_for_code_that_is_gone(self):
        """Integration: a stale entry would let the identical line be
        reintroduced without review."""
        live = {check_gates.panic_payload(site) for site in check_gates.gated_panic_sites()}
        stale = [
            line
            for line in check_gates.read_baseline(ROOT / "scripts" / "panic_baseline.txt")
            if check_gates.panic_payload(line) not in live
        ]
        self.assertEqual(stale, [])


class TestModuleRanges(unittest.TestCase):
    def test_mask_rust_hides_braces_in_literals_and_comments(self):
        lines = [
            "fn f() {",
            '    let a = "}{";',
            "    let b = '}';",
            "    // }",
            "    /* } { */",
            '    let c = r#"}"#;',
            "}",
            "",
        ]
        text = "\n".join(lines)
        masked = check_gates.mask_rust(text)
        self.assertEqual(len(masked), len(text))
        self.assertEqual(masked.count("\n"), text.count("\n"))
        self.assertEqual(masked.count("{"), 1)
        self.assertEqual(masked.count("}"), 1)

    def test_inline_module_range_is_brace_matched(self):
        text = '#[cfg(test)]\nmod tests {\n    fn t() { assert!(true); }\n}\nfn after() {}\n'
        self.assertEqual(check_gates.test_module_ranges(text), [(1, 4)])

    def test_cfg_test_on_a_use_statement_hides_nothing(self):
        """`apps/joyword/src/document_canvas.rs:6` is exactly this shape; the old
        split discarded the remaining 1939 lines of production code."""
        text = 'use a::b;\n#[cfg(test)]\nuse c::d;\nfn f() { std::fs::read("x"); }\n'
        self.assertEqual(check_gates.test_module_ranges(text), [])

    def test_out_of_line_module_covers_only_its_declaration(self):
        """`apps/joyword/src/main.rs:2467-2469` is exactly this shape."""
        text = '#[cfg(test)]\n#[path = "tests.rs"]\nmod find_bar_tests;\nfn after() {}\n'
        self.assertEqual(check_gates.test_module_ranges(text), [(1, 3)])

    def test_cfg_all_test_on_a_fn_is_not_a_module(self):
        text = '#[cfg(all(test, unix))]\nfn helper() { std::fs::read("x"); }\n'
        self.assertEqual(check_gates.test_module_ranges(text), [])

    def test_production_lines_skip_several_modules(self):
        text = (
            "fn before() {}\n"
            "#[cfg(test)]\nmod a {\n    fn t() {}\n}\n"
            "fn middle() {}\n"
            "#[cfg(test)]\nmod b {\n    fn t() {}\n}\n"
            "fn after() {}\n"
        )
        numbers = [number for number, _ in check_gates.production_lines(text)]
        self.assertEqual(numbers, [1, 6, 11, 12])


class FilesystemGate(TempTree):
    def test_production_code_below_a_test_module_is_scanned(self):
        """Fails before: everything after the first `#[cfg(test)]` was dropped,
        so the process spawn and the `OpenOptions`/`File::open`/`read_dir` calls
        below the test module were never seen."""
        self.write("crates/demo/src/lib.rs", fixture("gate_filesystem_hidden.rs"))
        errors = self.filesystem()
        self.assertTrue(errors, "the fixture must be rejected")
        joined = "\n".join(errors)
        self.assertIn("use std::process::Command;", joined)
        self.assertIn("Command::new(program)", joined)
        self.assertIn("std::fs::OpenOptions::new()", joined)
        self.assertIn("std::fs::File::open(path)", joined)
        self.assertIn("std::fs::read_dir(path)", joined)
        self.assertIn("std::fs::hard_link(path, path)", joined)
        self.assertIn("std::fs::remove_dir_all(path)", joined)

    def test_only_the_test_module_body_is_skipped(self):
        """The fixture's test module touches the filesystem on purpose; those
        lines must not be reported while the production lines are."""
        text = fixture("gate_filesystem_hidden.rs")
        self.write("crates/demo/src/lib.rs", text)
        errors = self.filesystem()
        self.assertTrue(errors)
        ranges = check_gates.test_module_ranges(text)
        self.assertEqual(len(ranges), 1)
        start, end = ranges[0]
        reported = [
            number
            for number, line in check_gates.production_lines(text)
            if check_gates.FS_RE.search(line)
        ]
        self.assertTrue(any(number > end for number in reported), reported)
        self.assertFalse(any(start <= number <= end for number in reported), reported)
        self.assertEqual(len(reported), len(errors))

    def test_env_set_var_is_an_entry_point(self):
        """Fails before: mutating the process environment was not scanned."""
        self.write(
            "crates/demo/src/lib.rs",
            'fn f() { std::env::set_var("JOYWORD_HOME", "x"); }\n',
        )
        self.assertTrue(self.filesystem())

    def test_a_real_test_directory_is_still_skipped(self):
        """Guard: the tightened filter must not start gating integration tests."""
        self.write("crates/demo/tests/it.rs", fixture("gate_filesystem_lookalike.rs"))
        self.assertEqual(self.filesystem(), [])

    def test_a_tests_rs_module_is_still_skipped(self):
        """Fails before: the filesystem gate had no `tests.rs` rule at all, so a
        test module reached through `#[path]` was gated as production code (the
        panic gate already skipped it). `apps/joyword/src/tests.rs` is this
        shape."""
        self.write("crates/demo/src/tests.rs", fixture("gate_filesystem_lookalike.rs"))
        self.assertEqual(self.filesystem(), [])
        self.assertTrue(check_gates.panic_scope("crates/demo/src/lib.rs"))
        self.assertFalse(check_gates.panic_scope("crates/demo/src/tests.rs"))

    def test_examples_are_still_skipped(self):
        """Guard: unchanged exclusion."""
        self.write("crates/demo/examples/tool.rs", fixture("gate_filesystem_lookalike.rs"))
        self.assertEqual(self.filesystem(), [])

    def test_a_lookalike_tests_path_is_not_skipped(self):
        """Fails before: `"tests/" in shown` skipped any path containing that
        substring, so `src/contests/` was treated as a test directory."""
        self.write("crates/demo/src/contests/rules.rs", fixture("gate_filesystem_lookalike.rs"))
        errors = self.filesystem()
        self.assertTrue(errors)
        self.assertIn("crates/demo/src/contests/rules.rs", "\n".join(errors))
        self.assertFalse(check_gates.is_test_path("crates/demo/src/contests/rules.rs"))
        self.assertTrue(check_gates.is_test_path("crates/demo/tests/rules.rs"))
        self.assertTrue(check_gates.is_test_path("apps/joyword/src/tests.rs"))

    def test_repo_offenders_are_only_the_reported_one(self):
        """Integration ratchet: the hardened scan surfaces exactly one offender
        on main, reported for human review instead of silently allowlisted."""
        errors = check_gates.check_filesystem()
        unexpected = [error for error in errors if REPORTED_FS_OFFENDER not in error]
        self.assertEqual(unexpected, [])

    def test_repo_files_below_a_test_module_are_really_scanned(self):
        """Integration: the old `split("#[cfg(test)]")[0]` truncation hid most of
        these files; the hardened scan sees the production code below."""
        for relative in (
            "crates/joyword-shell/src/storage/source.rs",
            "crates/joyword-engine/src/session.rs",
            "apps/joyword/src/linux_files.rs",
        ):
            text = (ROOT / relative).read_text(encoding="utf-8")
            visible = len(check_gates.production_lines(text))
            truncated = text.split("#[cfg(test)]", 1)[0].count("\n") + 1
            self.assertGreaterEqual(visible, truncated, relative)
        # `linux_files.rs` carries its marker late, so it only gains from the
        # extended regex; these two had nearly the whole file discarded.
        for relative in (
            "crates/joyword-shell/src/storage/source.rs",
            "crates/joyword-engine/src/session.rs",
        ):
            text = (ROOT / relative).read_text(encoding="utf-8")
            visible = len(check_gates.production_lines(text))
            truncated = text.split("#[cfg(test)]", 1)[0].count("\n") + 1
            self.assertGreater(visible, truncated * 4, relative)


class CounterexampleCommand(TempTree):
    def test_legacy_counterexample_still_trips_all_three_checks(self):
        """Guards `.github/workflows/ci.yml:46`, which must keep working
        unchanged."""
        problems = check_gates.counterexample(FIXTURES / "gate_counterexample.rs")
        self.assertTrue(problems)
        joined = "\n".join(problems)
        self.assertIn("depends on joyword-shell", joined)
        self.assertIn("filesystem access outside the allowlist", joined)
        self.assertIn("new panic site", joined)

    def test_layer_fixture_is_rejected_through_the_shared_tree(self):
        """Fails before: the throwaway crate only ever carried the bare
        `name = { path = ... }` spelling."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            check_gates.counterexample_tree(
                root, "fn unused() {}\n", fixture("gate_layers_counterexample.toml")
            )
            errors = check_gates.check_layers(root / "crates", root / "apps")
        self.assertTrue(any("joyword-shell" in error for error in errors), errors)
        self.assertTrue(any("dev-depends on joyword-corpus" in error for error in errors), errors)

    def test_manifest_layer_problems_understands_the_dotted_form(self):
        """Fails before: the fragment helper shared the blind `DEP_RE`."""
        problems = check_gates.manifest_layer_problems(fixture("gate_layers_counterexample.toml"))
        self.assertIn("joyword-model depends on joyword-shell, which its layer does not allow", problems)


if __name__ == "__main__":
    unittest.main()
