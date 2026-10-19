import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import driver
import matrix

class DriverTests(unittest.TestCase):
    def test_deterministic_byte_and_structural_mutations(self):
        data=matrix.archive()
        a=random.Random(42);b=random.Random(42)
        first=[driver.mutate(data,a,i) for i in range(30)]
        self.assertEqual(first,[driver.mutate(data,b,i) for i in range(30)])
        self.assertTrue(all(len(data)<=driver.MAX_INPUT for data,_ in first))
        self.assertTrue(any(method=='xml-depth' for _,method in first))
    def test_matrix_regenerates_exact_manifest_hashes(self):
        with tempfile.TemporaryDirectory() as d:
            entries=matrix.generate(d)
            self.assertEqual(len(entries),27)
            for entry in entries:self.assertEqual(driver.sha((Path(d)/entry['file']).read_bytes()),entry['sha256'])
    def test_minimization_preserves_exact_failure_class_and_budget(self):
        classify=lambda b:'invariant' if b'BUG' in b else ('crash' if b'B' in b else 'ok')
        small,runs=driver.minimize(b'paddingBUGpadding',classify,'invariant',100)
        self.assertEqual(small,b'BUG');self.assertLessEqual(runs,100)
        with self.assertRaises(ValueError):driver.minimize(b'normal',classify,'invariant')
    def test_timeout_reaps_child_and_collects_output(self):
        code='import sys,time;print("ready",flush=True);time.sleep(20)'
        result=driver.run_process([sys.executable,'-c',code],b'',timeout=0.15)
        self.assertTrue(result['timed_out']);self.assertTrue(result['reaped']);self.assertIn('ready',result['stdout'])
        self.assertEqual(driver.classify(result),'timeout')
    def test_nonzero_exit_protocol_and_binary_input(self):
        result=driver.run_process([sys.executable,'-c','import sys;assert sys.stdin.buffer.read()==b"\\x00\\xff";sys.exit(17)'],b'\x00\xff')
        self.assertEqual(result['exit'],17);self.assertEqual(driver.classify(result),'crash')
        malformed=driver.run_process([sys.executable,'-c','print("not JSON")'],b'')
        self.assertEqual(driver.classify(malformed),'protocol')
    def test_unicode_space_paths_and_bounded_output(self):
        with tempfile.TemporaryDirectory(prefix='中文 space ') as d:
            script=Path(d)/'child 程序.py';script.write_text('print("x"*100000)')
            result=driver.run_process([sys.executable,str(script)],b'')
            self.assertEqual(driver.classify(result),'protocol');self.assertLessEqual(len(result['stdout']),driver.MAX_OUTPUT)
    def test_result_schema_status_and_source_are_checked(self):
        result={'timed_out':False,'exit':0,'stdout_bytes':1,'stderr_bytes':0,'stdout':json.dumps({'schema':1,'target_sha':driver.TARGET_SHA,'status':'ok'})}
        self.assertEqual(driver.classify(result),'ok')
        result['exit']=2;self.assertEqual(driver.classify(result),'protocol')
    def test_resource_probe_reports_capability_without_assuming_platform(self):
        capability=driver.memory_capability();self.assertIsInstance(capability['enforced'],bool);self.assertIn('reason',capability)
    def test_size_limit_is_checked_before_starting_a_child(self):
        with self.assertRaises(ValueError):driver.run_process(['does-not-exist'],b'0'*(driver.MAX_INPUT+1))
    def test_smoke_requires_complete_matrix_within_total_time_budget(self):
        for smoke, expired, expected_exit, complete in [(True, True, 1, False), (True, False, 0, True), (False, True, 0, None)]:
            with self.subTest(smoke=smoke, expired=expired), tempfile.TemporaryDirectory() as directory:
                root = Path(directory); (root/'seed.docx').write_bytes(matrix.archive())
                args = SimpleNamespace(iterations=1, seconds=120, timeout=3, memory_mib=512,
                                       binary=Path('unused'), out=root/'out', seeds=root, seed=42)
                times = [0, 121, 121] if expired else [0, 0, 0, 0, 0]
                with mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}), \
                     mock.patch.object(driver, 'environment', return_value={}), \
                     mock.patch.object(driver, 'invoke', return_value={'class': 'ok'}), \
                     mock.patch.object(driver.time, 'monotonic', side_effect=times), \
                     mock.patch('builtins.print'):
                    self.assertEqual(driver.campaign(args, smoke), expected_exit)
                summary = json.loads((root/'out'/'summary.json').read_text())
                self.assertEqual(summary['matrix_complete'], complete)
                self.assertEqual(summary['executed'], 0 if expired else 3)
    def test_extract_features_prefers_harness_list_and_falls_back(self):
        result = {'class': 'ok', 'harness_report': {'features': ['a.b', 'c.d'], 'checks': 9}}
        feats, signal = driver.extract_features(result)
        self.assertEqual(feats, {'a.b', 'c.d'}); self.assertEqual(signal, 'harness-features')
        result = {'class': 'ok', 'harness_report': {'checks': 9, 'reason': 'word/document.xml: boom'}}
        feats, signal = driver.extract_features(result)
        self.assertEqual(signal, 'status-reason-fallback')
        self.assertIn('status.ok', feats); self.assertIn('checks.8-15', feats); self.assertIn('reason.word-document-xml', feats)
        result = {'class': 'crash', 'harness_report': {}}
        feats, signal = driver.extract_features(result)
        self.assertEqual(feats, {'status.crash'}); self.assertEqual(signal, 'status-reason-fallback')

    def test_fuzz_expands_corpus_on_novelty_deterministically(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root/'seed.docx').write_bytes(matrix.archive())
            args = SimpleNamespace(iterations=40, seconds=120, timeout=3, memory_mib=512,
                                   binary=Path('unused'), out=root/'out', seeds=root, seed=7, max_corpus=32)
            tick = iter(i * 0.001 for i in range(100000))
            summaries = []
            for run in range(2):
                calls = iter(range(1000))
                scripted = lambda *a, **k: {'class': 'ok',
                                            'harness_report': {'features': ['f.{}'.format(next(calls) % 17)]}}
                with mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}), \
                     mock.patch.object(driver, 'environment', return_value={}), \
                     mock.patch.object(driver, 'invoke', side_effect=scripted), \
                     mock.patch.object(driver.time, 'monotonic', side_effect=lambda: next(tick)), \
                     mock.patch('builtins.print'):
                        self.assertEqual(driver.fuzz(args), 0)
                summaries.append(json.loads((root/'out'/'summary.json').read_text()))
                import shutil; shutil.rmtree(root/'out')
            first, second = summaries
            self.assertGreater(first['features_discovered'], 1)
            self.assertGreater(first['corpus_size'], 1)
            self.assertLessEqual(first['corpus_size'], 32)
            self.assertEqual(first['features_discovered'], second['features_discovered'])
            self.assertEqual(first['corpus_size'], second['corpus_size'])
            self.assertEqual(first['novelty_events'], second['novelty_events'])

    def test_fuzz_enforces_corpus_cap_and_anomaly_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root/'seed.docx').write_bytes(matrix.archive())
            args = SimpleNamespace(iterations=12, seconds=120, timeout=3, memory_mib=512,
                                   binary=Path('unused'), out=root/'out', seeds=root, seed=1, max_corpus=8)
            tick = iter(i * 0.001 for i in range(100000))
            seq = [{'class': 'ok', 'reproduce': ['x', 'y'],
                    'harness_report': {'features': ['f.{}'.format(i)]}} for i in range(20)]
            with mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}), \
                 mock.patch.object(driver, 'environment', return_value={}), \
                 mock.patch.object(driver, 'invoke', side_effect=seq), \
                 mock.patch.object(driver.time, 'monotonic', side_effect=lambda: next(tick)), \
                 mock.patch('builtins.print'):
                    self.assertEqual(driver.fuzz(args), 0)
            summary = json.loads((root/'out'/'summary.json').read_text())
            self.assertEqual(summary['corpus_size'], 8)
            self.assertEqual(summary['features_discovered'], 12)
            bad = [dict(item, harness_report={'features': ['f.x']}) for item in seq]
            bad[3] = {'class': 'crash', 'reproduce': ['x', 'y']}
            args.out = root/'out2'
            with mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}), \
                 mock.patch.object(driver, 'environment', return_value={}), \
                 mock.patch.object(driver, 'invoke', side_effect=bad), \
                 mock.patch.object(driver.time, 'monotonic', side_effect=lambda: next(tick)), \
                 mock.patch('builtins.print'):
                    self.assertEqual(driver.fuzz(args), 1)
            self.assertTrue((root/'out2'/'failure-3.docx').exists())

    def test_fuzz_budget_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root/'seed.docx').write_bytes(matrix.archive())
            args = SimpleNamespace(iterations=0, seconds=120, timeout=3, memory_mib=512,
                                   binary=Path('unused'), out=root/'out', seeds=root, seed=1, max_corpus=8)
            with self.assertRaises(ValueError): driver.fuzz(args)
            args.iterations = 10; args.max_corpus = 4
            with self.assertRaises(ValueError): driver.fuzz(args)

    @unittest.skipUnless(os.name == 'posix', 'shebang stub binary; Windows native execution is a separate runner concern')
    def test_fuzz_end_to_end_with_stub_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root/'seed.docx').write_bytes(matrix.archive())
            stub = root/'stub.py'
            stub.write_text(
                '#!/usr/bin/env python3\n'
                'import sys, json, hashlib\n'
                'data = sys.stdin.buffer.read()\n'
                'features = ["stub.base", "stub.pk" if data[:2] == b"PK" else "stub.raw",\n'
                '            "stub.len-{}".format(min(len(data) // 200, 4))]\n'
                'print(json.dumps({"schema": 1, "target_sha": "' + driver.TARGET_SHA + '",'
                ' "status": "ok", "checks": 1, "features": features,'
                ' "input_sha256": hashlib.sha256(data).hexdigest(),'
                ' "target": sys.argv[1], "case_id": sys.argv[2], "seed": int(sys.argv[3])}))\n')
            stub.chmod(0o755)
            args = SimpleNamespace(iterations=9, seconds=60, timeout=3, memory_mib=512,
                                   binary=stub, out=root/'out', seeds=root,
                                   seed=5, max_corpus=16)
            with mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}):
                self.assertEqual(driver.fuzz(args), 0)
            summary = json.loads((root/'out'/'summary.json').read_text())
            self.assertEqual(summary['feedback_signal'], 'harness-features')
            self.assertIs(summary['coverage_feedback'], False)
            self.assertNotIn('regions_covered', summary)
            self.assertGreaterEqual(summary['features_discovered'], 3)
            self.assertEqual(summary['corpus_size'], 1 + summary['novelty_events'])
            self.assertTrue((root/'out'/'corpus.json').exists())
            self.assertEqual(summary['counts'], {'ok': 9})


class CoverageTests(unittest.TestCase):
    def test_parser_counts_only_crates_regions_and_branch_sides(self):
        payload = {
            'type': 'llvm.coverage.json.export',
            'version': '3.1.0',
            'data': [{
                'files': [
                    {'filename': '/src/proj/crates/joyword-docx/src/lib.rs',
                     'branches': [[10, 1, 10, 8, 4, 0, 0, 0, 4], [11, 1, 11, 8, 0, 0, 0, 0, 4]]},
                    {'filename': '/usr/local/cargo/registry/src/index.crates.io-abc/regex/src/lib.rs',
                     'branches': [[1, 1, 1, 2, 9, 9, 0, 0, 4]]},
                    {'filename': 'C:\\work\\crates\\joyword-model\\src\\lib.rs',
                     'branches': [[2, 1, 2, 4, 0, 3, 0, 0, 4]]},
                ],
                'functions': [
                    {'filenames': ['/src/proj/crates/joyword-docx/src/lib.rs',
                                   '/usr/local/cargo/registry/src/index.crates.io-abc/regex/src/lib.rs',
                                   '/src/proj/crates/../secret.rs'],
                     'regions': [
                         [4, 1, 4, 10, 2, 0, 0, 0],
                         [5, 1, 5, 10, 0, 0, 0, 0],
                         [4, 1, 4, 10, 1, 0, 0, 0],
                         [6, 1, 6, 10, 9, 1, 0, 0],
                         [8, 1, 8, 4, 0, 0, 0, 2],
                         [9, 1, 9, 4, 7, 2, 0, 0],
                         [12, 1, 12, 6, 3, 0, 0, 4],
                     ],
                     'branches': [[14, 1, 14, 6, 0, 2, 0, 0, 4]]},
                    {'filenames': ['C:\\work\\crates\\joyword-model\\src\\lib.rs'],
                     'regions': [[3, 1, 3, 8, 1, 0, 0, 0]]},
                ],
            }],
        }
        view = driver.parse_llvm_cov_export(payload)
        self.assertEqual(view['regions_total'], 3)
        self.assertEqual(view['regions_covered'], 2)
        self.assertEqual(view['region_ids'], {
            ('region', 'crates/joyword-docx/src/lib.rs', 4, 1, 4, 10),
            ('region', 'crates/joyword-model/src/lib.rs', 3, 1, 3, 8),
        })
        self.assertEqual(view['branches_total'], 5)
        self.assertEqual(view['branches_covered'], 4)
        self.assertIn(('branch-true', 'crates/joyword-docx/src/lib.rs', 10, 1, 10, 8), view['branch_ids'])
        self.assertIn(('branch-false', 'crates/joyword-docx/src/lib.rs', 14, 1, 14, 6), view['branch_ids'])
        self.assertIn(('branch-true', 'crates/joyword-docx/src/lib.rs', 12, 1, 12, 6), view['branch_ids'])
        self.assertIn(('branch-false', 'crates/joyword-model/src/lib.rs', 2, 1, 2, 4), view['branch_ids'])
        self.assertIsNone(driver.crates_source_path('/usr/local/cargo/registry/src/index.crates.io-abc/regex/src/lib.rs'))
        with self.assertRaises(ValueError):
            driver.parse_llvm_cov_export('{"type":"other"}')
        with self.assertRaises(ValueError):
            driver.parse_llvm_cov_export('not-json')

    def _coverage_args(self, root, **overrides):
        values = dict(iterations=12, seconds=120, timeout=3, memory_mib=512, binary=Path('unused'),
                      out=root / 'out', seeds=root, seed=20261020, max_corpus=8, feedback='coverage')
        values.update(overrides)
        return SimpleNamespace(**values)

    def _run_scripted(self, args, regions_for_call):
        calls = {'n': 0}
        def account(tools, binary, profraw, cumulative):
            regions = regions_for_call(calls['n'])
            calls['n'] += 1
            return {'region_ids': {('region', 'crates/joyword-docx/src/lib.rs', region, 1, region, 2)
                                    for region in regions},
                    'branch_ids': set(), 'regions_total': 40, 'regions_covered': len(regions),
                    'branches_total': 0, 'branches_covered': 0}
        tick = iter(i * 0.001 for i in range(100000))
        chain = {'tools': {}, 'llvm_profdata_version': 'LLVM version 22.1.8-rust-1.98.1-stable',
                 'llvm_cov_version': 'LLVM version 22.1.8-rust-1.98.1-stable',
                 'rustc_version': 'rustc 1.98.1\nrelease: 1.98.1\nhost: x86_64-unknown-linux-gnu\nLLVM version: 22.1.8'}
        with mock.patch.object(driver, 'coverage_toolchain', return_value=chain), \
             mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}), \
             mock.patch.object(driver, 'environment', return_value={'schema': 1}), \
             mock.patch.object(driver, 'account_profile', side_effect=account), \
             mock.patch.object(driver, 'invoke', return_value={'class': 'ok', 'reproduce': ['b', 't', 'c', '0', 'in']}), \
             mock.patch.object(driver.time, 'monotonic', side_effect=lambda: next(tick)), \
             mock.patch('builtins.print'):
            code = driver.fuzz(args)
        return code, json.loads((Path(args.out) / 'summary.json').read_text())

    def test_new_regions_are_novelty_and_repeat_regions_are_not(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'seed.docx').write_bytes(matrix.archive())
            sequence = [{1}, {1}, {1, 2}, {1, 2}]
            code, summary = self._run_scripted(self._coverage_args(root, iterations=3, max_corpus=16),
                                               lambda n: sequence[n])
            self.assertEqual(code, 0)
            self.assertEqual(summary['baseline_runs'], 1)
            self.assertEqual(summary['executed'], 3)
            self.assertEqual(summary['novelty_events'], 1)
            self.assertEqual(summary['corpus_size'], 2)
            self.assertEqual(summary['regions_covered'], 2)
            self.assertEqual(summary['feedback_signal'], 'llvm-source-regions')
            self.assertIs(summary['coverage_feedback'], True)
            self.assertEqual(summary['profiles_read'], 4)
            self.assertLessEqual(summary['corpus_bytes_written'], driver.FUZZ_MAX_CORPUS_BYTES)

    def test_coverage_fuzz_is_deterministic_and_honors_corpus_cap(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'seed.docx').write_bytes(matrix.archive())
            summaries = []
            for run in range(2):
                code, summary = self._run_scripted(
                    self._coverage_args(root, out=root / 'out{}'.format(run), iterations=12, max_corpus=8, seed=11),
                    lambda n: {n})
                self.assertEqual(code, 0)
                summaries.append(summary)
            first, second = summaries
            self.assertEqual(first['corpus_size'], 8)
            self.assertEqual(first['novelty_events'], 12)
            self.assertEqual(first['regions_covered'], 13)
            self.assertEqual(first['executed'], 12)
            for key in ('novelty_events', 'corpus_size', 'regions_covered', 'executed', 'baseline_runs'):
                self.assertEqual(first[key], second[key])

    def test_coverage_fuzz_stops_at_eight_failures_and_at_the_time_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'seed.docx').write_bytes(matrix.archive())
            args = self._coverage_args(root, iterations=30, max_corpus=16)
            crashes = {'class': 'crash', 'reproduce': ['b', 't', 'c', '0', 'in']}
            def account(tools, binary, profraw, cumulative):
                return {'region_ids': {('region', 'crates/joyword-docx/src/lib.rs', 1, 1, 1, 2)},
                        'branch_ids': set(), 'regions_total': 4, 'regions_covered': 1,
                        'branches_total': 0, 'branches_covered': 0}
            tick = iter(i * 0.001 for i in range(100000))
            chain = {'tools': {}, 'llvm_profdata_version': 'LLVM version test',
                     'llvm_cov_version': 'LLVM version test', 'rustc_version': 'rustc test'}
            with mock.patch.object(driver, 'coverage_toolchain', return_value=chain), \
                 mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}), \
                 mock.patch.object(driver, 'environment', return_value={}), \
                 mock.patch.object(driver, 'account_profile', side_effect=account), \
                 mock.patch.object(driver, 'invoke', return_value=crashes), \
                 mock.patch.object(driver.time, 'monotonic', side_effect=lambda: next(tick)), \
                 mock.patch('builtins.print'):
                self.assertEqual(driver.fuzz(args), 1)
            summary = json.loads((root / 'out' / 'summary.json').read_text())
            self.assertEqual(summary['baseline_runs'] + summary['executed'], 8)
            self.assertEqual(len(list((root / 'out').glob('failure-*.docx'))), 8)
            args.out = root / 'timed'
            clock = iter([0, 1000, 1000, 1000])
            with mock.patch.object(driver, 'coverage_toolchain', return_value=chain), \
                 mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}), \
                 mock.patch.object(driver, 'environment', return_value={}), \
                 mock.patch.object(driver, 'invoke', return_value=crashes), \
                 mock.patch.object(driver.time, 'monotonic', side_effect=lambda: next(clock)), \
                 mock.patch('builtins.print'):
                self.assertEqual(driver.fuzz(args), 0)
            timed = json.loads((root / 'timed' / 'summary.json').read_text())
            self.assertEqual(timed['executed'], 0)
            self.assertEqual(timed['baseline_runs'], 0)
            self.assertEqual(timed['profiles_read'], 0)

    def test_coverage_budget_and_tool_errors(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'seed.docx').write_bytes(matrix.archive())
            args = self._coverage_args(root, iterations=0)
            with self.assertRaises(ValueError):
                driver.fuzz(args)
            args.iterations = 4
            args.max_corpus = 3
            with self.assertRaises(ValueError):
                driver.fuzz(args)
            args.max_corpus = 8
            args.feedback = 'edges'
            with self.assertRaises(ValueError):
                driver.fuzz(args)
            args.feedback = 'coverage'
            args.llvm_profdata = Path('/no/such/llvm-profdata')
            with self.assertRaises(ValueError):
                driver.fuzz(args)
            self.assertFalse((root / 'out').exists())

    def test_sanitize_and_comparison_keep_only_campaign_numbers(self):
        coverage = {'coverage_feedback': True, 'feedback_signal': 'llvm-source-regions', 'campaign_seed': 5,
                    'requested_runs': 10, 'executed': 10, 'baseline_runs': 2, 'novelty_events': 3,
                    'corpus_size': 6, 'regions_covered': 9, 'regions_total': 20,
                    'branches_covered': 1, 'branches_total': 4, 'counts': {'ok': 10},
                    'baseline_counts': {'crash': 0}, 'llvm_profdata_version': 'LLVM version 22.1.8-rust-1.98.1-stable'}
        behavioral = {'campaign_seed': 5, 'executed': 10, 'novelty_events': 4, 'corpus_size': 7,
                      'features_discovered': 12, 'counts': {'ok': 9, 'crash': 1}}
        cleaned = driver.sanitize_coverage_summary(dict(coverage, reproduce=['/tmp/a.docx'], input_sha256='ab'))
        self.assertNotIn('reproduce', cleaned)
        self.assertNotIn('input_sha256', cleaned)
        self.assertEqual(cleaned['regions_covered'], 9)
        with self.assertRaises(ValueError):
            driver.sanitize_coverage_summary(dict(coverage, os='/tmp/secret'))
        compared = driver.campaign_comparison(coverage, behavioral)
        self.assertEqual(compared['coverage']['regions_covered'], 9)
        self.assertEqual(compared['behavioral']['anomalies'], 1)
        self.assertEqual(compared['coverage']['anomalies'], 0)
        behavioral['campaign_seed'] = 6
        with self.assertRaises(ValueError):
            driver.campaign_comparison(coverage, behavioral)

    def test_fuzz_cli_defaults_remain_behavioral(self):
        args = driver.build_parser().parse_args(['fuzz', '--binary', 'b', '--out', 'o'])
        self.assertEqual(args.feedback, 'behavioral')
        self.assertEqual(args.max_corpus, 256)
        self.assertEqual(args.iterations, 300)
        self.assertEqual(args.seed, 20261015)
        smoke = driver.build_parser().parse_args(['smoke', '--binary', 'b', '--out', 'o'])
        self.assertFalse(hasattr(smoke, 'feedback'))

    def test_unprofiled_runs_do_not_claim_a_coverage_result(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'seed.docx').write_bytes(matrix.archive())
            args = self._coverage_args(root, iterations=2, max_corpus=8)
            chain = {'tools': {}, 'llvm_profdata_version': 'LLVM version test',
                     'llvm_cov_version': 'LLVM version test', 'rustc_version': 'rustc test'}
            tick = iter(i * 0.001 for i in range(1000))
            with mock.patch.object(driver, 'coverage_toolchain', return_value=chain), \
                 mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}), \
                 mock.patch.object(driver, 'environment', return_value={}), \
                 mock.patch.object(driver, 'account_profile', return_value=None), \
                 mock.patch.object(driver, 'invoke', return_value={'class': 'ok', 'reproduce': ['b']}), \
                 mock.patch.object(driver.time, 'monotonic', side_effect=lambda: next(tick)), \
                 mock.patch('builtins.print'):
                with self.assertRaises(ValueError):
                    driver.fuzz(args)
            self.assertTrue((root / 'out' / 'summary.json').exists())

    @unittest.skipUnless(os.name == 'posix', 'instrumented stub is compiled for this POSIX runner')
    def test_coverage_end_to_end_with_instrumented_stub(self):
        try:
            driver.locate_llvm_tools()
        except (ValueError, OSError, subprocess.SubprocessError):
            if os.environ.get('JOYWORD_REQUIRE_LLVM_TOOLS') == '1':
                self.fail('llvm-tools required but not found')
            self.skipTest('llvm-tools not installed')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            # Short non-zip seed: truncation crosses the stub's length regions.
            (root / 'seed.docx').write_bytes(b'PK' + bytes(range(40)))
            source = root / 'crates' / 'covstub.rs'
            source.parent.mkdir()
            source.write_text(COVERAGE_STUB)
            binary = root / 'covstub'
            subprocess.check_call(['rustc', '--edition', '2024', '-C', 'instrument-coverage',
                                   '-o', str(binary), str(source)], timeout=60)
            summaries = []
            for run in range(2):
                args = self._coverage_args(root, binary=binary, out=root / 'run{}'.format(run),
                                           iterations=8, seconds=60, max_corpus=16, seed=20261020, memory_mib=512)
                with mock.patch.object(driver, 'memory_capability', return_value={'enforced': False}), \
                     mock.patch('builtins.print'):
                    self.assertEqual(driver.fuzz(args), 0)
                summaries.append(json.loads((root / 'run{}'.format(run) / 'summary.json').read_text()))
            summary = summaries[0]
            self.assertIs(summary['coverage_feedback'], True)
            self.assertEqual(summary['feedback_signal'], 'llvm-source-regions')
            self.assertGreater(summary['profiles_read'], 0)
            self.assertGreater(summary['regions_total'], 0)
            self.assertGreater(summary['regions_covered'], 0)
            self.assertLessEqual(summary['regions_covered'], summary['regions_total'])
            self.assertGreaterEqual(summary['novelty_events'], 1)
            self.assertGreater(summary['corpus_size'], 1)
            self.assertTrue(summary['coverage_merge_matches_scheduler'])
            self.assertIn('1.98.1', summary['rustc_version'])
            self.assertTrue(summary['llvm_profdata_version'].startswith('LLVM version '))
            self.assertEqual(summary['llvm_profdata_version'], summary['llvm_cov_version'])
            driver.sanitize_coverage_summary(summary)
            for key in ('regions_covered', 'regions_total', 'novelty_events', 'corpus_size', 'executed', 'profiles_read'):
                self.assertEqual(summary[key], summaries[1][key])


COVERAGE_STUB = r'''
fn sha256(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut words = [0u32; 64];
        for index in 0..16 {
            words[index] = u32::from_be_bytes([
                chunk[index * 4], chunk[index * 4 + 1], chunk[index * 4 + 2], chunk[index * 4 + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7) ^ words[index - 15].rotate_right(18) ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17) ^ words[index - 2].rotate_right(19) ^ (words[index - 2] >> 10);
            words[index] = words[index - 16].wrapping_add(s0).wrapping_add(words[index - 7]).wrapping_add(s1);
        }
        let mut a: u32 = state[0];
        let mut b: u32 = state[1];
        let mut c: u32 = state[2];
        let mut d: u32 = state[3];
        let mut e: u32 = state[4];
        let mut f: u32 = state[5];
        let mut g: u32 = state[6];
        let mut h: u32 = state[7];
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let t1 = h.wrapping_add(s1).wrapping_add(choice).wrapping_add(K[index]).wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            control if control.is_control() => out.push_str(&format!("\\u{code:04x}", code = control as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn classify(data: &[u8]) -> u32 {
    if data.len() < 2 {
        return 1;
    }
    if data.len() < 8 {
        return 2;
    }
    if data.len() < 32 {
        return 3;
    }
    if data[0] != b'P' {
        return 4;
    }
    if data[1] != b'K' {
        return 5;
    }
    6
}

fn main() {
    let mut args = std::env::args();
    let _program = args.next();
    let target = args.next().expect("target");
    let case_id = args.next().expect("case");
    let seed: u64 = args.next().expect("seed").parse().expect("seed number");
    let mut data = Vec::new();
    std::io::Read::read_to_end(&mut std::io::stdin(), &mut data).expect("stdin");
    let checks = classify(&data);
    println!(
        "{{\"schema\":1,\"target\":{target},\"case_id\":{case},\"seed\":{seed},\"target_sha\":\"TARGET\",\"input_sha256\":\"{digest}\",\"bytes\":{length},\"status\":\"ok\",\"reason\":\"none\",\"checks\":{checks},\"features\":[\"stub.{checks}\"]}}",
        target = json_string(&target),
        case = json_string(&case_id),
        seed = seed,
        digest = sha256(&data),
        length = data.len(),
        checks = checks,
    );
}
'''.replace('TARGET', driver.TARGET_SHA)


class ProcessFailureTests(unittest.TestCase):
    def test_minimize_a_real_crashing_subprocess(self):
        code='import sys,json;data=sys.stdin.buffer.read();sys.exit(17) if b"BUG" in data else print(json.dumps({"schema":1,"target_sha":"'+driver.TARGET_SHA+'","status":"ok"}))'
        evaluate=lambda data:driver.classify(driver.run_process([sys.executable,'-c',code],data,timeout=2))
        small,runs=driver.minimize(b'prefixBUGsuffix',evaluate,'crash',64)
        self.assertEqual(small,b'BUG');self.assertLessEqual(runs,64)
    @unittest.skipUnless(os.name=='posix','POSIX process-group test; Windows requires native taskkill validation')
    def test_timeout_kills_descendants_in_its_own_group(self):
        import time
        with tempfile.TemporaryDirectory() as directory:
            marker=Path(directory)/'descendant_survived'
            child='import time,pathlib;time.sleep(0.6);pathlib.Path('+repr(str(marker))+').write_text("alive")'
            parent='import subprocess,sys,time;subprocess.Popen([sys.executable,"-c",'+repr(child)+']);time.sleep(20)'
            result=driver.run_process([sys.executable,'-c',parent],b'',timeout=0.1)
            self.assertEqual(driver.classify(result),'timeout');time.sleep(0.8);self.assertFalse(marker.exists())

if __name__=='__main__':unittest.main()
