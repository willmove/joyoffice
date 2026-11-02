"""Regression checks for rejecting an unwanted window stack or source override."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('dependency_gate', Path(__file__).resolve().parents[1] / 'check_dependencies.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class DependencyGates(unittest.TestCase):
    def baseline(self):
        return {'package': [{'name': name, 'version': version, 'source': 'registry+https://github.com/rust-lang/crates.io-index'} for name, version in gate.EXPECTED.items()]}

    def test_same_version_git_override_is_rejected(self):
        lock = self.baseline()
        lock['package'][0]['source'] = 'git+https://example.invalid/fork'
        self.assertTrue(gate.check_lock(lock))

    def test_font_only_egui_dependency_is_rejected(self):
        lock = self.baseline()
        lock['package'].append({'name': 'epaint_default_fonts', 'version': '0.37.0'})
        self.assertTrue(gate.check_lock(lock))

    def test_original_baseline_passes(self):
        self.assertFalse(gate.check_lock(self.baseline()))
