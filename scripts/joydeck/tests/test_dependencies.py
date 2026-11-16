"""Reject mixed GPUI snapshots before compiling desktop or automation code."""
import copy
import importlib.util
from pathlib import Path
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    'check_dependencies', ROOT / 'scripts/joydeck/check_dependencies.py')
dependencies = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dependencies)


class SnapshotBaseline(unittest.TestCase):
    def setUp(self):
        self.lock = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))

    def test_current_lock_keeps_the_renamed_reqwest_version(self):
        self.assertEqual(dependencies.check_lock(self.lock), [])

    def test_stale_platform_or_support_snapshot_is_rejected(self):
        for name in ('gpui-pre-linux', 'gpui-pre-bench-metrics'):
            with self.subTest(name=name):
                lock = copy.deepcopy(self.lock)
                next(p for p in lock['package'] if p['name'] == name)['version'] = '0.3.10'
                self.assertTrue(any(name in error for error in dependencies.check_lock(lock)))

    def test_second_component_or_unreviewed_snapshot_source_is_rejected(self):
        lock = copy.deepcopy(self.lock)
        lock['package'].append({'name': 'gpui-base', 'version': '0.8.1'})
        self.assertTrue(any('gpui-base' in error for error in dependencies.check_lock(lock)))
        package = next(p for p in self.lock['package'] if p['name'] == 'gpui-pre-windows')
        package['source'] = 'git+https://example.invalid/unreviewed'
        self.assertTrue(any('gpui-pre-windows' in error for error in dependencies.check_lock(self.lock)))


if __name__ == '__main__':
    unittest.main()
