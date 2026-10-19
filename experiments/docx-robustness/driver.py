#!/usr/bin/env python3
"""Bounded mutation testing plus corpus-expanding fuzz (stdlib only).

The campaign/smoke commands are fixed-seed mutation testing. The fuzz command
adds a feedback loop: the harness reports observable stage facts (sanitized
variant names and size buckets, never document content) and inputs that reach
previously unseen facts join the mutation corpus. This is behavioral feature
feedback, not code coverage; no coverage engine is installed or required."""
import argparse
import collections
import hashlib
import io
import json
import math
import os
from pathlib import Path
import platform
import random
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import time
import zipfile

import matrix
try:
    import resource
except ImportError:
    resource = None

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
TARGET_SHA = '8b7f9c2b5e5b03c0ee6ac9a56c550a8e715f4d32'
MAX_INPUT = 1 << 20
MAX_OUTPUT = 65536
FAILURES = {'crash', 'timeout', 'invariant', 'protocol'}

def sha(data):
    return hashlib.sha256(data).hexdigest()

def bounded_read(path):
    with open(path, 'rb') as stream:
        data = stream.read(MAX_INPUT + 1)
    if len(data) > MAX_INPUT:
        raise ValueError('input exceeds 1 MiB')
    return data

def git(*args):
    return subprocess.check_output(['git', *args], cwd=str(REPO), timeout=10).decode().strip()

def memory_capability():
    if resource is None or not hasattr(resource, 'RLIMIT_AS'):
        return {'enforced': False, 'reason': 'RLIMIT_AS unavailable; no Windows Job Object backend'}
    code = '''import resource,sys
limit=64<<20
try: resource.setrlimit(resource.RLIMIT_AS,(limit,limit))
except (ValueError,OSError): sys.exit(3)
try: block=bytearray(128<<20)
except MemoryError: sys.exit(0)
sys.exit(4)
'''
    try:
        result = subprocess.run([sys.executable, '-c', code], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
        return {'enforced': result.returncode == 0, 'probe_exit': result.returncode,
                'reason': '64 MiB address-space probe; virtual space, not RSS' if result.returncode == 0 else 'OS rejected/failed address-space limit probe'}
    except subprocess.TimeoutExpired:
        return {'enforced': False, 'reason': 'limit probe timeout'}

def run_process(command, data, timeout=3.0, memory_mib=0):
    if timeout <= 0 or timeout > 30:
        raise ValueError('timeout must be in (0, 30] seconds')
    if len(data) > MAX_INPUT:
        raise ValueError('input exceeds 1 MiB')
    limit = None
    if memory_mib:
        if resource is None or not (32 <= memory_mib <= 2048):
            raise ValueError('unsupported memory limit')
        def set_limit():
            resource.setrlimit(resource.RLIMIT_AS, (memory_mib << 20, memory_mib << 20))
        limit = set_limit
    started = time.monotonic()
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=stdout, stderr=stderr,
                                 start_new_session=os.name == 'posix', preexec_fn=limit)
        timed_out = False
        try:
            child.communicate(data, timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            if os.name == 'posix':
                # Only the new process group created for this invocation.
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            elif shutil.which('taskkill'):
                subprocess.run(['taskkill', '/PID', str(child.pid), '/T', '/F'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
            else:
                child.kill()
            child.communicate(timeout=5)
        stdout.seek(0, 2); out_bytes = stdout.tell(); stdout.seek(0)
        stderr.seek(0, 2); err_bytes = stderr.tell(); stderr.seek(0)
        out = stdout.read(MAX_OUTPUT + 1); err = stderr.read(MAX_OUTPUT + 1)
    return {'schema': 1, 'elapsed_seconds': time.monotonic() - started, 'exit': child.returncode,
            'timed_out': timed_out, 'reaped': child.poll() is not None, 'stdout_bytes': out_bytes,
            'stderr_bytes': err_bytes, 'stdout': out[:MAX_OUTPUT].decode('utf-8', errors='replace'),
            'stderr': err[:MAX_OUTPUT].decode('utf-8', errors='replace')}

def classify(result):
    if result['timed_out']:
        return 'timeout'
    if result['exit'] not in (0, 2, 3):
        return 'crash'
    if result['stdout_bytes'] > MAX_OUTPUT or result['stderr_bytes'] > MAX_OUTPUT:
        return 'protocol'
    try:
        value = json.loads(result['stdout'])
        if value['schema'] != 1 or value['target_sha'] != TARGET_SHA:
            return 'protocol'
        expected = {0: 'ok', 2: 'rejected', 3: 'invariant'}[result['exit']]
        return expected if value['status'] == expected else 'protocol'
    except (ValueError, KeyError, TypeError):
        return 'protocol'

def invoke(binary, target, case, seed, data, timeout, memory_mib):
    command = [str(binary), target, case, str(seed), '-']
    result = run_process(command, data, timeout, memory_mib)
    result.update({'class': classify(result), 'target': target, 'case_id': case, 'seed': seed,
                   'input_sha256': sha(data), 'target_sha': TARGET_SHA,
                   'reproduce': [str(binary), target, case, str(seed), 'INPUT_FILE']})
    # A valid-looking result for a different input/target must not pass.
    if result['class'] in ('ok', 'rejected', 'invariant'):
        value = json.loads(result['stdout'])
        result['harness_report'] = value
        if (value.get('input_sha256'), value.get('target'), value.get('case_id'), value.get('seed')) != (sha(data), target, case, seed):
            result['class'] = 'protocol'
    return result

def mutate(data, rng, iteration):
    mode = iteration % 12
    value = bytearray(data)
    if mode == 0:
        if not value:
            return b'PK', 'bytes-insert'
        for _ in range(1 + rng.randrange(4)):
            value[rng.randrange(len(value))] ^= 1 << rng.randrange(8)
        return bytes(value), 'bytes-flip'
    if mode == 1:
        return bytes(value[:rng.randrange(len(value) + 1)]), 'bytes-truncate'
    if mode in (2, 3):
        pos = value.find(b'PK\x01\x02')
        if pos >= 0 and pos + 46 <= len(value):
            struct.pack_into('<I', value, pos + (16 if mode == 2 else 24), rng.getrandbits(32))
            return bytes(value), 'zip-crc' if mode == 2 else 'zip-size'
    try:
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            info = z.infolist()
            if len(info) > 128 or sum(i.file_size for i in info) > (4 << 20):
                raise ValueError('mutator expansion budget')
            parts = [(i.filename, z.read(i)) for i in info]
        values = dict(parts)
        main = values.get('word/document.xml', b'')
        if mode == 4:
            parts.append(('../bad.bin', b'path'))
        elif mode == 5:
            parts.append(('word/document.xml', main))
        elif mode == 6:
            values['word/document.xml'] = main.replace(b'<w:p>', b'<w:p w:futureSemantic="keep">', 1)
            parts = list(values.items())
        elif mode == 7:
            values['word/document.xml'] = main.replace(b'<w:body>', b'<w:body>'+b'<x:n xmlns:x="urn:w09">'*40, 1).replace(b'</w:body>', b'</x:n>'*40+b'</w:body>', 1)
            parts = list(values.items())
        elif mode == 8:
            values['word/document.xml'] = b'<!DOCTYPE w:document [<!ENTITY x "data">]>'+main
            parts = list(values.items())
        elif mode == 10:
            parts.append(('word/media/fuzz-{:04x}.bin'.format(rng.getrandbits(16)),
                          bytes([rng.randrange(256)]) * rng.randrange(1, 64)))
        elif mode == 11:
            values['word/document.xml'] = main.replace(b'<w:body>', b'<w:body><w:p/>', 1)
            parts = list(values.items())
        else:
            values['word/document.xml'] = main.replace(b'</w:t>', (' '+str(rng.getrandbits(32))+' 甲👩‍💻</w:t>').encode(), 1)
            parts = list(values.items())
        out = io.BytesIO()
        import warnings
        with warnings.catch_warnings():
            warnings.simplefilter('ignore', UserWarning)
            with zipfile.ZipFile(out, 'w', compression=zipfile.ZIP_STORED) as z:
                for name, content in parts:
                    i = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0)); z.writestr(i, content)
        result = out.getvalue()
        return result[:MAX_INPUT], ['flip', 'truncate', 'crc', 'size', 'zip-path', 'zip-duplicate', 'xml-attribute', 'xml-depth', 'xml-entity', 'xml-text', 'zip-extra-part', 'xml-extra-paragraph'][mode]
    except (ValueError, KeyError, RuntimeError, NotImplementedError, zipfile.BadZipFile):
        pos = rng.randrange(len(value) + 1)
        value[pos:pos] = bytes([rng.randrange(256)])
        return bytes(value[:MAX_INPUT]), 'bytes-insert-fallback'

def feat_token(text):
    """First whitespace token reduced to a bounded identifier (matches the harness)."""
    token = text.split()[0] if text.split() else 'none'
    out = []
    last_dash = False
    for ch in token[:24].lower():
        if ch.isalnum() or ch in '_-':
            out.append(ch)
            last_dash = False
        elif out and not last_dash:
            out.append('-')
            last_dash = True
    value = ''.join(out).strip('-')
    return value or 'none'

def checks_bucket(n):
    n = int(n)
    for cap, label in ((0, '0'), (1, '1'), (2, '2'), (3, '3'), (7, '4-7'), (15, '8-15')):
        if n <= cap:
            return label
    return '16+'

def extract_features(result):
    """Behavioral feedback set for one run, and which signal produced it.

    Preferred: the harness schema-1 `features` list (stage variant names and
    size buckets). Fallback for older binaries: coarse status/checks/reason
    classes. This is not code coverage.
    """
    value = result.get('harness_report') or {}
    features = value.get('features')
    if isinstance(features, list) and features and all(isinstance(f, str) and f for f in features):
        return set(features), 'harness-features'
    feats = {'status.' + result['class']}
    if result['class'] in ('ok', 'rejected', 'invariant'):
        feats.add('checks.' + checks_bucket(value.get('checks', 0)))
        feats.add('reason.' + feat_token(str(value.get('reason') or 'none')))
    return feats, 'status-reason-fallback'

FUZZ_MAX_CORPUS_BYTES = 32 << 20

def fuzz(args):
    """Corpus-expanding fuzz: inputs reaching unseen behavioral features join
    the mutation corpus (newest-first, square-biased scheduling). Hard budgets:
    wall clock, iterations, per-child timeout/address space, input and output
    caps, corpus size and corpus bytes; no network is used anywhere."""
    if not 1 <= args.iterations <= 100000 or not 0 < args.seconds <= 600:
        raise ValueError('fuzz budget out of range')
    if not 8 <= args.max_corpus <= 1024:
        raise ValueError('corpus limit out of range')
    memory = memory_capability(); cap = args.memory_mib if memory['enforced'] else 0
    info = environment(args.binary, memory)
    info['coverage_feedback'] = False
    info['feedback_signal'] = 'pending'
    out = Path(args.out); out.mkdir(parents=True, exist_ok=False)
    (out / 'corpus').mkdir()
    seeds = [(p.stem, bounded_read(p)) for p in sorted(Path(args.seeds).glob('*.docx'))]
    if not seeds:
        raise ValueError('no seeds')
    rng = random.Random(args.seed)
    counts = collections.Counter(); started = time.monotonic()
    anomalies = 0; executed = 0; novelty_events = 0; signal = 'none'
    seen = set()
    corpus = [{'name': 'seed-' + name, 'data': data, 'parent': None, 'iteration': -1}
              for name, data in seeds]
    corpus_bytes = 0
    for entry in corpus:
        if corpus_bytes + len(entry['data']) <= FUZZ_MAX_CORPUS_BYTES:
            (out / 'corpus' / (entry['name'] + '.docx')).write_bytes(entry['data'])
            corpus_bytes += len(entry['data'])
    with (out / 'results.jsonl').open('w') as log:
        for i in range(args.iterations):
            remaining = args.seconds - (time.monotonic() - started)
            if remaining <= 0:
                break
            # Newest corpus entries first; squaring biases draws toward them.
            parent = corpus[int(rng.random() ** 2 * len(corpus))]
            data, method = mutate(parent['data'], rng, i)
            target = ('opc', 'import', 'roundtrip')[i % 3]
            event_seed = rng.getrandbits(32)
            case = 'W09-fuzz-{}-{}'.format(parent['name'], i)
            result = invoke(args.binary, target, case, event_seed, data, min(args.timeout, remaining), cap)
            result['mutation'] = method; result['campaign_seed'] = args.seed
            feats, sig = extract_features(result)
            if sig != 'none':
                signal = sig
            new = sorted(feats - seen)
            if new:
                seen.update(new); novelty_events += 1
                if len(corpus) < args.max_corpus:
                    entry = {'name': 'corpus{:04d}'.format(len(corpus)), 'data': data,
                             'parent': parent['name'], 'iteration': i}
                    corpus.insert(0, entry)
                    if corpus_bytes + len(data) <= FUZZ_MAX_CORPUS_BYTES:
                        (out / 'corpus' / (entry['name'] + '.docx')).write_bytes(data)
                        corpus_bytes += len(data)
                result['novel_features'] = new
            counts[result['class']] += 1; executed += 1
            if result['class'] in FAILURES:
                anomalies += 1
                path = out / ('failure-{}.docx'.format(i)); path.write_bytes(data)
                result['reproduce'][-1] = str(path)
                (out / ('failure-{}.json'.format(i))).write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
            log.write(json.dumps(result, ensure_ascii=False) + '\n'); log.flush()
            if anomalies >= 8:
                break
    peak = None
    if resource is not None:
        peak = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * (1 if sys.platform == 'darwin' else 1024)
    info['feedback_signal'] = signal
    summary = dict(info, command='fuzz', campaign_seed=args.seed, requested_runs=args.iterations,
                   requested_seconds=args.seconds, executed=executed, counts=dict(counts),
                   elapsed_seconds=time.monotonic() - started, timeout_seconds=args.timeout,
                   effective_address_space_mib=cap, aggregate_children_peak_rss_bytes=peak,
                   peak_scope='OS cumulative maximum across child processes; includes capability probe, not per-case',
                   features_discovered=len(seen), novelty_events=novelty_events,
                   corpus_size=len(corpus), corpus_limit=args.max_corpus,
                   corpus_bytes_written=corpus_bytes, corpus_bytes_cap=FUZZ_MAX_CORPUS_BYTES,
                   network='none: the harness never resolves external relationship targets and the driver opens no sockets')
    (out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n')
    (out / 'corpus.json').write_text(json.dumps(
        [{'name': e['name'], 'parent': e['parent'], 'iteration': e['iteration'],
          'bytes': len(e['data']), 'sha256': sha(e['data'])} for e in corpus],
        ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(summary, ensure_ascii=False))
    return 1 if anomalies else 0

def minimize(data, evaluate, failure_class, max_runs=128):
    if not 1 <= max_runs <= 1000:
        raise ValueError('minimizer run budget out of range')
    if failure_class not in FAILURES:
        raise ValueError('minimization requires an anomaly class')
    if evaluate(data) != failure_class:
        raise ValueError('initial failure does not reproduce')
    current, granularity, runs = data, 2, 1
    while current and runs < max_runs:
        chunk = max(1, math.ceil(len(current) / granularity))
        changed = False
        for start in range(0, len(current), chunk):
            if runs >= max_runs:
                break
            candidate = current[:start] + current[start+chunk:]
            runs += 1
            if evaluate(candidate) == failure_class:
                current = candidate; changed = True; granularity = max(2, granularity - 1); break
        if not changed:
            if granularity >= len(current):
                break
            granularity = min(len(current), granularity * 2)
    return current, runs

def environment(binary, memory):
    if subprocess.run(['git', 'diff', '--quiet', TARGET_SHA, '--', 'crates'], cwd=str(REPO)).returncode:
        raise ValueError('production target tree changed; C08 must approve a fixed new revision')
    return {'schema': 1, 'target_sha': TARGET_SHA, 'harness_git_sha': git('rev-parse', 'HEAD'),
            'harness_binary_sha256': sha(Path(binary).read_bytes()), 'harness_worktree_dirty': bool(git('status','--porcelain','--','experiments/docx-robustness')), 'os': platform.platform(),
            'python': platform.python_version(), 'memory_limit': memory,
            'process_tree_timeout_kill': 'process group' if os.name == 'posix' else ('taskkill /T' if shutil.which('taskkill') else 'direct child only'),
            'coverage_feedback': False}

def campaign(args, smoke=False):
    if not 1 <= args.iterations <= 100000 or not 0 < args.seconds <= 600:
        raise ValueError('campaign bound out of range')
    memory = memory_capability(); cap = args.memory_mib if memory['enforced'] else 0
    info = environment(args.binary, memory); out = Path(args.out); out.mkdir(parents=True, exist_ok=False)
    seeds = [(p.stem, bounded_read(p)) for p in sorted(Path(args.seeds).glob('*.docx'))]
    if not seeds:
        raise ValueError('no seeds')
    rng = random.Random(args.seed); counts = collections.Counter(); started = time.monotonic(); anomalies = 0
    items = [(name, data, target, i, 'matrix') for i,(name,data) in enumerate(seeds) for target in ('opc','import','roundtrip')] if smoke else None
    with (out/'results.jsonl').open('w') as log:
        for i in range(len(items) if items is not None else args.iterations):
            remaining = args.seconds - (time.monotonic() - started)
            if remaining <= 0:
                break
            if items is None:
                name, source = seeds[rng.randrange(len(seeds))]
                data, method = mutate(source, rng, i); target = ('opc','import','roundtrip')[i % 3]; event_seed = rng.getrandbits(32)
            else:
                name, data, target, event_seed, method = items[i]
            case = 'W09-{}-{}'.format(name,i)
            result = invoke(args.binary, target, case, event_seed, data, min(args.timeout, remaining), cap)
            result['mutation'] = method; result['campaign_seed'] = args.seed
            counts[result['class']] += 1
            if result['class'] in FAILURES:
                anomalies += 1
                path=out/('failure-{}.docx'.format(i)); path.write_bytes(data)
                result['reproduce'][-1] = str(path)
                (out/('failure-{}.json'.format(i))).write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
            log.write(json.dumps(result, ensure_ascii=False)+'\n'); log.flush()
            if anomalies >= 8:
                break
    peak = None
    if resource is not None:
        peak = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * (1 if sys.platform == 'darwin' else 1024)
    matrix_complete = None if items is None else sum(counts.values()) == len(items)
    summary = dict(info, campaign_seed=args.seed, requested_runs=args.iterations if items is None else len(items),
                   requested_seconds=args.seconds, executed=sum(counts.values()), counts=dict(counts),
                   matrix_complete=matrix_complete,
                   elapsed_seconds=time.monotonic()-started, timeout_seconds=args.timeout,
                   effective_address_space_mib=cap, aggregate_children_peak_rss_bytes=peak,
                   peak_scope='OS cumulative maximum across child processes; includes capability probe, not per-case')
    (out/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(summary,ensure_ascii=False))
    return 1 if anomalies or matrix_complete is False else 0

def main():
    parser = argparse.ArgumentParser(description=__doc__); sub=parser.add_subparsers(dest='command',required=True)
    for command in ['smoke', 'campaign', 'fuzz']:
        p = sub.add_parser(command); p.add_argument('--binary', type=Path, required=True); p.add_argument('--seeds', type=Path, default=HERE/'seeds'); p.add_argument('--out', required=True)
        p.add_argument('--seed', type=int, default=20261015); p.add_argument('--iterations', type=int, default=300); p.add_argument('--seconds', type=float, default=120); p.add_argument('--timeout', type=float, default=3); p.add_argument('--memory-mib', type=int, default=512)
        if command == 'fuzz':
            p.add_argument('--max-corpus', type=int, default=256)
    for command in ['replay','minimize']:
        p=sub.add_parser(command);p.add_argument('--binary',type=Path,required=True);p.add_argument('--input',type=Path,required=True);p.add_argument('--target',choices=['opc','import','roundtrip'],required=True);p.add_argument('--seed',type=int,required=True);p.add_argument('--timeout',type=float,default=3)
        if command=='minimize':p.add_argument('--class',dest='failure_class',choices=sorted(FAILURES),required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--max-runs',type=int,default=128)
    args = parser.parse_args()
    if args.command in ('smoke', 'campaign'):
        return campaign(args, args.command == 'smoke')
    if args.command == 'fuzz':
        return fuzz(args)
    data=bounded_read(args.input);binary=args.binary.resolve()
    evaluate=lambda value:invoke(binary,args.target,'W09-replay',args.seed,value,args.timeout,0)
    if args.command=='replay':
        result=evaluate(data);print(json.dumps(result,ensure_ascii=False));return 1 if result['class'] in FAILURES else 0
    if not 1 <= args.max_runs <= 1000:raise ValueError('minimizer budget out of range')
    small,runs=minimize(data,lambda value:evaluate(value)['class'],args.failure_class,args.max_runs)
    with args.out.open('xb') as out:out.write(small)
    print(json.dumps({'schema':1,'class':args.failure_class,'runs':runs,'before':len(data),'after':len(small),'sha256':sha(small),'replay':evaluate(small)},ensure_ascii=False))
    return 0

if __name__=='__main__':
    try:
        sys.exit(main())
    except (ValueError,OSError,subprocess.SubprocessError) as error:
        print(str(error),file=sys.stderr);sys.exit(64)
