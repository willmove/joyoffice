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
            self.assertGreaterEqual(summary['features_discovered'], 3)
            self.assertEqual(summary['corpus_size'], 1 + summary['novelty_events'])
            self.assertTrue((root/'out'/'corpus.json').exists())
            self.assertEqual(summary['counts'], {'ok': 9})


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
