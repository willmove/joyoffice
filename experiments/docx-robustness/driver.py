#!/usr/bin/env python3
"""Bounded mutation testing, behavioral-feature fuzz, and LLVM region fuzz.

smoke and campaign are fixed-seed mutation testing. fuzz without --feedback
(the default, behavioral) grows a corpus from the harness's sanitized stage
facts. That signal is not code coverage, and those summaries keep
coverage_feedback false.

fuzz --feedback coverage does not build the harness. Point --binary at a
second build made with RUSTFLAGS="-C instrument-coverage". Each child gets
its own LLVM profile. llvm-profdata merges those profiles and llvm-cov
exports source regions; newly covered regions under crates/ feed the same
corpus scheduler. That mode records coverage_feedback true and
feedback_signal llvm-source-regions. The llvm-tools come from the Rust
toolchain distribution (LLVM, Apache-2.0 WITH LLVM-exception). They are not
vendored, not a Cargo or Python dependency, and the driver only uses them
to merge and read profiles."""
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

def production_tree():
    return git('rev-parse', 'HEAD:crates')

# Same identity build.rs embeds in the harness; a binary built against any
# other crates tree is classified as a protocol failure.
TARGET_SHA = production_tree()

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

def run_process(command, data, timeout=3.0, memory_mib=0, env=None):
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
                                 start_new_session=os.name == 'posix', preexec_fn=limit, env=env)
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

def invoke(binary, target, case, seed, data, timeout, memory_mib, env=None):
    command = [str(binary), target, case, str(seed), '-']
    result = run_process(command, data, timeout, memory_mib, env=env)
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

def crates_source_path(filename):
    """Repository-relative crates/ path, or None when the file is outside that tree."""
    if not isinstance(filename, str) or not filename:
        return None
    normalized = filename.replace('\\', '/')
    marker = '/crates/'
    index = normalized.find(marker)
    if index >= 0:
        relative = normalized[index + 1:]
    elif normalized.startswith('crates/'):
        relative = normalized
    else:
        return None
    parts = relative.split('/')
    if any(part in ('', '.', '..') for part in parts):
        return None
    return relative

def _as_int(value):
    if isinstance(value, bool):
        raise ValueError('bool is not a coverage counter')
    return int(value)

def _region_row(filenames, region):
    """Kind-0 code region or kind-4 branch region under crates/."""
    if not isinstance(region, (list, tuple)) or len(region) < 5:
        return None
    try:
        line_s, col_s, line_e, col_e = (_as_int(region[0]), _as_int(region[1]), _as_int(region[2]), _as_int(region[3]))
        count = _as_int(region[4])
        file_id = _as_int(region[5]) if len(region) > 5 else 0
        kind = _as_int(region[7]) if len(region) > 7 else 0
    except (TypeError, ValueError):
        return None
    if file_id < 0 or file_id >= len(filenames):
        return None
    relative = crates_source_path(filenames[file_id])
    if relative is None or kind not in (0, 4):
        return None
    return ('region' if kind == 0 else 'branch', relative, line_s, col_s, line_e, col_e, count)

def _branch_row(filenames, branch):
    """One llvm-cov branch record: true and false counters for a source range."""
    if not isinstance(branch, (list, tuple)) or len(branch) < 6:
        return None
    try:
        line_s, col_s, line_e, col_e = (_as_int(branch[0]), _as_int(branch[1]), _as_int(branch[2]), _as_int(branch[3]))
        true_count, false_count = _as_int(branch[4]), _as_int(branch[5])
        file_id = _as_int(branch[6]) if len(branch) > 6 else 0
    except (TypeError, ValueError):
        return None
    if file_id < 0 or file_id >= len(filenames):
        return None
    relative = crates_source_path(filenames[file_id])
    if relative is None:
        return None
    return (relative, line_s, col_s, line_e, col_e, true_count, false_count)

def parse_llvm_cov_export(payload):
    """Parse an llvm-cov JSON export into crates/ region and branch sets.

    A code region is a kind-0 mapping region. A branch site is either a
    branch record (true and false counters) or, when that record is absent,
    a kind-4 region. Files outside crates/ are ignored. Totals count unique
    ranges, including ranges whose counter is still zero. Covered region ids
    are the ranges with a positive counter; covered branch ids are the taken
    sides, so the other side becoming taken is new coverage.
    """
    if isinstance(payload, (bytes, bytearray)):
        payload = payload.decode('utf-8')
    if isinstance(payload, str):
        try:
            document = json.loads(payload)
        except json.JSONDecodeError as error:
            raise ValueError('coverage export is not JSON') from error
    elif isinstance(payload, dict):
        document = payload
    else:
        raise ValueError('coverage export has an unsupported type')
    if document.get('type') != 'llvm.coverage.json.export':
        raise ValueError('coverage export is not llvm.coverage.json.export')
    region_counts = {}
    branch_sides = {}
    branch_regions = {}
    for entry in document.get('data') or []:
        if not isinstance(entry, dict):
            continue
        for function in entry.get('functions') or []:
            if not isinstance(function, dict):
                continue
            names = function.get('filenames') or []
            if not isinstance(names, list):
                continue
            if not any(crates_source_path(name) for name in names):
                continue
            for region in function.get('regions') or []:
                parsed = _region_row(names, region)
                if parsed is None:
                    continue
                label, relative, line_s, col_s, line_e, col_e, count = parsed
                key = (relative, line_s, col_s, line_e, col_e)
                if label == 'region':
                    region_counts[key] = max(region_counts.get(key, 0), count)
                else:
                    branch_regions[key] = max(branch_regions.get(key, 0), count)
            for branch in function.get('branches') or []:
                parsed = _branch_row(names, branch)
                if parsed is None:
                    continue
                relative, line_s, col_s, line_e, col_e, true_count, false_count = parsed
                key = (relative, line_s, col_s, line_e, col_e)
                previous = branch_sides.get(key, (0, 0))
                branch_sides[key] = (max(previous[0], true_count), max(previous[1], false_count))
        for file_obj in entry.get('files') or []:
            if not isinstance(file_obj, dict):
                continue
            name = file_obj.get('filename')
            if crates_source_path(name) is None:
                continue
            for branch in file_obj.get('branches') or []:
                parsed = _branch_row([name], branch)
                if parsed is None:
                    continue
                relative, line_s, col_s, line_e, col_e, true_count, false_count = parsed
                key = (relative, line_s, col_s, line_e, col_e)
                previous = branch_sides.get(key, (0, 0))
                branch_sides[key] = (max(previous[0], true_count), max(previous[1], false_count))
    for key, count in branch_regions.items():
        if key not in branch_sides:
            branch_sides[key] = (count, 0)
    region_ids = {('region', *key) for key, count in region_counts.items() if count > 0}
    branch_ids = set()
    branches_covered = 0
    for key, (true_count, false_count) in branch_sides.items():
        if true_count > 0 or false_count > 0:
            branches_covered += 1
        if true_count > 0:
            branch_ids.add(('branch-true', *key))
        if false_count > 0:
            branch_ids.add(('branch-false', *key))
    return {
        'region_ids': region_ids,
        'branch_ids': branch_ids,
        'regions_total': len(region_counts),
        'regions_covered': sum(count > 0 for count in region_counts.values()),
        'branches_total': len(branch_sides),
        'branches_covered': branches_covered,
    }

def locate_llvm_tools(profdata=None, cov=None):
    """llvm-profdata and llvm-cov from the active Rust sysroot, unless paths are given."""
    sysroot = None
    host = None
    def resolve(name, explicit):
        nonlocal sysroot, host
        if explicit:
            path = Path(explicit)
            if not path.is_file():
                raise ValueError('{} not found: {}'.format(name, path))
            return path
        if sysroot is None:
            sysroot = subprocess.check_output(['rustc', '--print', 'sysroot'], text=True, timeout=30).strip()
            details = subprocess.check_output(['rustc', '-vV'], text=True, timeout=30)
            for line in details.splitlines():
                if line.startswith('host: '):
                    host = line.split(' ', 1)[1].strip()
            if not host:
                raise ValueError('rustc -vV did not report a host triple')
        path = Path(sysroot) / 'lib' / 'rustlib' / host / 'bin' / name
        if not path.is_file():
            raise ValueError(
                '{} not found at {}; install it from the Rust toolchain with '
                'rustup component add llvm-tools --toolchain 1.98.1'.format(name, path))
        return path
    return {'llvm-profdata': resolve('llvm-profdata', profdata), 'llvm-cov': resolve('llvm-cov', cov)}

def tool_version(path):
    result = subprocess.run([str(path), '--version'], stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, timeout=30)
    if result.returncode != 0 or not result.stdout.strip():
        raise ValueError('{} --version failed'.format(path))
    return result.stdout

def short_llvm_version(text):
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith('LLVM version '):
            return stripped
    raise ValueError('llvm tool version text has no LLVM version line')

def short_rustc_version(text):
    kept = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(('rustc ', 'release: ', 'host: ', 'LLVM version: ')):
            kept.append(stripped)
    if not any(line.startswith('rustc ') for line in kept):
        raise ValueError('rustc -Vv did not report a version')
    return '\n'.join(kept)

def coverage_toolchain(args):
    tools = locate_llvm_tools(getattr(args, 'llvm_profdata', None), getattr(args, 'llvm_cov', None))
    rustc_text = subprocess.check_output(['rustc', '-Vv'], text=True, timeout=30)
    return {
        'tools': tools,
        'llvm_profdata_version': short_llvm_version(tool_version(tools['llvm-profdata'])),
        'llvm_cov_version': short_llvm_version(tool_version(tools['llvm-cov'])),
        'rustc_version': short_rustc_version(rustc_text),
    }

COVERAGE_IGNORE = ('/registry/', '/git/checkouts/', '/rustc/', 'docx-robustness')

def _run_tool(command, timeout):
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
    if result.returncode != 0:
        detail = result.stderr.decode('utf-8', errors='replace')[:500]
        raise ValueError('{} failed: {}'.format(Path(command[0]).name, detail))
    return result

def merge_profiles(profdata_bin, inputs, output):
    command = [str(profdata_bin), 'merge', '-sparse', '-failure-mode=any', '-o', str(output)]
    command.extend(str(path) for path in inputs)
    _run_tool(command, 60)

def export_profile(tools, binary, profdata):
    command = [str(tools['llvm-cov']), 'export', '-format=text', '--unify-instantiations', '--skip-expansions',
               '-instr-profile={}'.format(profdata), str(binary)]
    for pattern in COVERAGE_IGNORE:
        command.append('--ignore-filename-regex={}'.format(pattern))
    result = _run_tool(command, 120)
    return parse_llvm_cov_export(result.stdout)

def account_profile(tools, binary, profraw, cumulative):
    """Merge one child profraw into the cumulative profile and return that child's view.

    A missing or empty profile (timeout, kill, or a non-instrumented exit) returns None.
    """
    profraw = Path(profraw)
    cumulative = Path(cumulative)
    if not profraw.is_file() or profraw.stat().st_size == 0:
        return None
    indexed = profraw.with_suffix('.profdata')
    merge_profiles(tools['llvm-profdata'], [profraw], indexed)
    view = export_profile(tools, binary, indexed)
    merged_tmp = cumulative.with_name(cumulative.name + '.tmp')
    sources = [indexed]
    if cumulative.is_file():
        sources.append(cumulative)
    merge_profiles(tools['llvm-profdata'], sources, merged_tmp)
    merged_tmp.replace(cumulative)
    profraw.unlink(missing_ok=True)
    indexed.unlink(missing_ok=True)
    return view

def _coverage_label(item):
    kind, relative, line_s, col_s, line_e, col_e = item
    return '{}:{}:{}:{}-{}:{}'.format(kind, relative, line_s, col_s, line_e, col_e)

def _coverage_limits(args):
    if not 1 <= args.iterations <= 100000 or not 0 < args.seconds <= 600:
        raise ValueError('fuzz budget out of range')
    if not 8 <= args.max_corpus <= 1024:
        raise ValueError('corpus limit out of range')

def _anomaly_count(counts):
    return sum((counts or {}).get(name, 0) for name in FAILURES)

def sanitize_coverage_summary(summary):
    """Reject document bytes, filesystem paths and per-input hashes before a summary is kept."""
    if not isinstance(summary, dict):
        raise ValueError('summary must be an object')
    if summary.get('coverage_feedback') is not True:
        raise ValueError('not a coverage-guided summary')
    if summary.get('feedback_signal') != 'llvm-source-regions':
        raise ValueError('coverage summary signal is not llvm-source-regions')
    dropped = {'reproduce', 'input_sha256', 'stdout', 'stderr', 'harness_report', 'novel_coverage'}
    def walk(value):
        if isinstance(value, dict):
            return {key: walk(item) for key, item in value.items() if key not in dropped}
        if isinstance(value, list):
            return [walk(item) for item in value]
        if isinstance(value, str):
            if len(value) > 2000:
                raise ValueError('summary string is unexpectedly long')
            if '://' in value or value.startswith(('/', '\\')) or '\\' in value or value.endswith('.docx'):
                raise ValueError('summary contains a path or document reference')
        return value
    return walk(summary)

def campaign_comparison(coverage_summary, behavioral_summary):
    """Numeric comparison of one coverage campaign and one behavioral campaign."""
    if coverage_summary.get('campaign_seed') != behavioral_summary.get('campaign_seed'):
        raise ValueError('comparison seed mismatch')
    return {
        'campaign_seed': coverage_summary['campaign_seed'],
        'iterations': coverage_summary.get('requested_runs'),
        'behavioral': {
            'executed': behavioral_summary['executed'],
            'novelty_events': behavioral_summary['novelty_events'],
            'corpus_size': behavioral_summary['corpus_size'],
            'features_discovered': behavioral_summary['features_discovered'],
            'anomalies': _anomaly_count(behavioral_summary.get('counts')),
        },
        'coverage': {
            'executed': coverage_summary['executed'],
            'baseline_runs': coverage_summary.get('baseline_runs'),
            'novelty_events': coverage_summary['novelty_events'],
            'corpus_size': coverage_summary['corpus_size'],
            'regions_covered': coverage_summary['regions_covered'],
            'regions_total': coverage_summary['regions_total'],
            'branches_covered': coverage_summary['branches_covered'],
            'branches_total': coverage_summary['branches_total'],
            'anomalies': _anomaly_count(coverage_summary.get('counts')) + _anomaly_count(coverage_summary.get('baseline_counts')),
        },
    }

FUZZ_MAX_CORPUS_BYTES = 32 << 20

def coverage_fuzz(args):
    """Coverage-guided fuzz. Novelty is a newly covered LLVM source region under crates/.

    Seeds are executed once on import first, so their regions are the baseline
    and do not count as novelty. Mutations then use the same newest-first
    square-biased scheduler as behavioral fuzz. Hard budgets match that mode.
    """
    _coverage_limits(args)
    chain = coverage_toolchain(args)
    memory = memory_capability()
    cap = args.memory_mib if memory['enforced'] else 0
    info = environment(args.binary, memory)
    info['coverage_feedback'] = True
    info['feedback_signal'] = 'llvm-source-regions'
    info['llvm_profdata_version'] = chain['llvm_profdata_version']
    info['llvm_cov_version'] = chain['llvm_cov_version']
    info['rustc_version'] = chain['rustc_version']
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=False)
    (out / 'corpus').mkdir()
    (out / 'profiles').mkdir()
    seeds = [(path.stem, bounded_read(path)) for path in sorted(Path(args.seeds).glob('*.docx'))]
    if not seeds:
        raise ValueError('no seeds')
    cumulative = out / 'coverage.profdata'
    seen_regions, seen_branches = set(), set()
    regions_total = branches_total = 0
    profiles_read = profile_errors = 0
    counts = collections.Counter()
    baseline_counts = collections.Counter()
    started = time.monotonic()
    anomalies = novelty_events = executed = baseline_runs = 0
    corpus = [{'name': 'seed-' + name, 'data': data, 'parent': None, 'iteration': -1} for name, data in seeds]
    corpus_bytes = 0
    for entry in corpus:
        if corpus_bytes + len(entry['data']) <= FUZZ_MAX_CORPUS_BYTES:
            (out / 'corpus' / (entry['name'] + '.docx')).write_bytes(entry['data'])
            corpus_bytes += len(entry['data'])

    def observe(index, data, target, case, event_seed, timeout):
        nonlocal profiles_read, profile_errors, regions_total, branches_total
        profraw = out / 'profiles' / '{}.profraw'.format(index)
        env = os.environ.copy()
        env['LLVM_PROFILE_FILE'] = str(profraw)
        result = invoke(args.binary, target, case, event_seed, data, timeout, cap, env=env)
        try:
            view = account_profile(chain['tools'], args.binary, profraw, cumulative)
        except (ValueError, OSError, subprocess.SubprocessError):
            profile_errors += 1
            view = None
        fresh = []
        if view is not None:
            profiles_read += 1
            regions_total = view['regions_total']
            branches_total = view['branches_total']
            fresh_regions = view['region_ids'] - seen_regions
            fresh_branches = view['branch_ids'] - seen_branches
            seen_regions.update(view['region_ids'])
            seen_branches.update(view['branch_ids'])
            fresh = sorted(_coverage_label(item) for item in (fresh_regions | fresh_branches))
        return result, fresh

    def note_failure(result, data, index):
        nonlocal anomalies
        anomalies += 1
        path = out / 'failure-{}.docx'.format(index)
        path.write_bytes(data)
        result['reproduce'][-1] = str(path)
        (out / 'failure-{}.json'.format(index)).write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')

    with (out / 'results.jsonl').open('w') as log:
        for name, data in seeds:
            remaining = args.seconds - (time.monotonic() - started)
            if remaining <= 0 or anomalies >= 8:
                break
            result, fresh = observe('seed-{}'.format(name), data, 'import', 'W09-cov-seed-{}'.format(name),
                                    args.seed, min(args.timeout, remaining))
            result['mutation'] = 'seed-baseline'
            result['campaign_seed'] = args.seed
            if fresh:
                result['novel_coverage'] = fresh[:64]
                result['novel_coverage_count'] = len(fresh)
            baseline_counts[result['class']] += 1
            baseline_runs += 1
            if result['class'] in FAILURES:
                note_failure(result, data, 'seed-{}'.format(name))
            log.write(json.dumps(result, ensure_ascii=False) + '\n')
            log.flush()
        rng = random.Random(args.seed)
        for i in range(args.iterations):
            remaining = args.seconds - (time.monotonic() - started)
            if remaining <= 0 or anomalies >= 8:
                break
            parent = corpus[int(rng.random() ** 2 * len(corpus))]
            data, method = mutate(parent['data'], rng, i)
            target = ('opc', 'import', 'roundtrip')[i % 3]
            event_seed = rng.getrandbits(32)
            case = 'W09-fuzz-{}-{}'.format(parent['name'], i)
            result, fresh = observe(i, data, target, case, event_seed, min(args.timeout, remaining))
            result['mutation'] = method
            result['campaign_seed'] = args.seed
            if fresh:
                novelty_events += 1
                if len(corpus) < args.max_corpus:
                    entry = {'name': 'corpus{:04d}'.format(len(corpus)), 'data': data,
                             'parent': parent['name'], 'iteration': i}
                    corpus.insert(0, entry)
                    if corpus_bytes + len(data) <= FUZZ_MAX_CORPUS_BYTES:
                        (out / 'corpus' / (entry['name'] + '.docx')).write_bytes(data)
                        corpus_bytes += len(data)
                result['novel_coverage'] = fresh[:64]
                result['novel_coverage_count'] = len(fresh)
            counts[result['class']] += 1
            executed += 1
            if result['class'] in FAILURES:
                note_failure(result, data, i)
            log.write(json.dumps(result, ensure_ascii=False) + '\n')
            log.flush()
    scheduler_regions = len(seen_regions)
    scheduler_branches = len({item[1:] for item in seen_branches})
    regions_covered = scheduler_regions
    branches_covered = scheduler_branches
    merge_matches = True
    if cumulative.is_file() and profiles_read:
        try:
            final = export_profile(chain['tools'], args.binary, cumulative)
        except (ValueError, OSError, subprocess.SubprocessError):
            final = None
            merge_matches = False
        if final is not None:
            regions_total = final['regions_total']
            branches_total = final['branches_total']
            regions_covered = final['regions_covered']
            branches_covered = final['branches_covered']
            merge_matches = (final['regions_covered'] == scheduler_regions and
                             final['branches_covered'] == scheduler_branches)
    peak = None
    if resource is not None:
        peak = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss * (1 if sys.platform == 'darwin' else 1024)
    summary = dict(info, command='fuzz', feedback='coverage', campaign_seed=args.seed,
                   requested_runs=args.iterations, requested_seconds=args.seconds, executed=executed,
                   baseline_runs=baseline_runs, baseline_target='import', counts=dict(counts),
                   baseline_counts=dict(baseline_counts), elapsed_seconds=time.monotonic() - started,
                   timeout_seconds=args.timeout, effective_address_space_mib=cap,
                   aggregate_children_peak_rss_bytes=peak,
                   peak_scope='OS cumulative maximum across child processes; includes capability probe, not per-case',
                   novelty_events=novelty_events, corpus_size=len(corpus), corpus_limit=args.max_corpus,
                   corpus_bytes_written=corpus_bytes, corpus_bytes_cap=FUZZ_MAX_CORPUS_BYTES,
                   regions_covered=regions_covered, regions_total=regions_total,
                   branches_covered=branches_covered, branches_total=branches_total,
                   scheduler_regions_covered=scheduler_regions, scheduler_branches_covered=scheduler_branches,
                   coverage_merge_matches_scheduler=merge_matches, profiles_read=profiles_read,
                   profile_errors=profile_errors,
                   network='none: the harness never resolves external relationship targets and the driver opens no sockets')
    (out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n')
    (out / 'corpus.json').write_text(json.dumps(
        [{'name': entry['name'], 'parent': entry['parent'], 'iteration': entry['iteration'],
          'bytes': len(entry['data']), 'sha256': sha(entry['data'])} for entry in corpus],
        ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(summary, ensure_ascii=False))
    if (baseline_runs + executed) > 0 and profiles_read == 0:
        raise ValueError(
            'coverage mode read no LLVM source profiles; build the harness with '
            'RUSTFLAGS="-C instrument-coverage" and install llvm-tools for Rust 1.98.1')
    return 1 if anomalies else 0

def cover(args):
    """Replay each .docx once on import through an instrumented binary and report crates/ regions."""
    if not 0 < args.timeout <= 30:
        raise ValueError('timeout must be in (0, 30] seconds')
    chain = coverage_toolchain(args)
    memory = memory_capability()
    cap = args.memory_mib if memory['enforced'] else 0
    info = environment(args.binary, memory)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=False)
    files = sorted(Path(args.inputs).glob('*.docx'))
    if not files:
        raise ValueError('no inputs')
    cumulative = out / 'coverage.profdata'
    profiles_read = anomalies = 0
    for index, path in enumerate(files):
        if anomalies >= 8:
            break
        data = bounded_read(path)
        profraw = out / '{}.profraw'.format(index)
        env = os.environ.copy()
        env['LLVM_PROFILE_FILE'] = str(profraw)
        result = invoke(args.binary, 'import', 'W09-cover-{}'.format(index), 0, data, args.timeout, cap, env=env)
        view = account_profile(chain['tools'], args.binary, profraw, cumulative)
        if view is not None:
            profiles_read += 1
        if result['class'] in FAILURES:
            anomalies += 1
    final = export_profile(chain['tools'], args.binary, cumulative) if cumulative.is_file() else None
    summary = dict(info, command='cover', coverage_feedback=True, feedback_signal='llvm-source-regions',
                   inputs=len(files), profiles_read=profiles_read, anomalies=anomalies,
                   regions_covered=0 if final is None else final['regions_covered'],
                   regions_total=0 if final is None else final['regions_total'],
                   branches_covered=0 if final is None else final['branches_covered'],
                   branches_total=0 if final is None else final['branches_total'],
                   llvm_profdata_version=chain['llvm_profdata_version'],
                   llvm_cov_version=chain['llvm_cov_version'], rustc_version=chain['rustc_version'],
                   network='none: the harness never resolves external relationship targets and the driver opens no sockets')
    (out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(summary, ensure_ascii=False))
    if profiles_read == 0:
        raise ValueError('cover read no LLVM source profiles')
    return 1 if anomalies else 0

def fuzz(args):
    """Corpus-expanding fuzz: inputs reaching unseen behavioral features join
    the mutation corpus (newest-first, square-biased scheduling). Hard budgets:
    wall clock, iterations, per-child timeout/address space, input and output
    caps, corpus size and corpus bytes; no network is used anywhere.

    --feedback coverage delegates to coverage_fuzz. The default behavioral
    mode is unchanged and keeps coverage_feedback false.
    """
    feedback = getattr(args, 'feedback', 'behavioral')
    if feedback == 'coverage':
        return coverage_fuzz(args)
    if feedback != 'behavioral':
        raise ValueError('feedback must be behavioral or coverage')
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
    if git('status', '--porcelain', '--untracked-files=all', '--', 'crates') or production_tree() != TARGET_SHA:
        raise ValueError('production crates differ from the committed tree; commit them and rebuild the harness')
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

def build_parser():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    for command in ['smoke', 'campaign', 'fuzz']:
        item = sub.add_parser(command)
        item.add_argument('--binary', type=Path, required=True)
        item.add_argument('--seeds', type=Path, default=HERE / 'seeds')
        item.add_argument('--out', required=True)
        item.add_argument('--seed', type=int, default=20261015)
        item.add_argument('--iterations', type=int, default=300)
        item.add_argument('--seconds', type=float, default=120)
        item.add_argument('--timeout', type=float, default=3)
        item.add_argument('--memory-mib', type=int, default=512)
        if command == 'fuzz':
            item.add_argument('--max-corpus', type=int, default=256)
            item.add_argument('--feedback', choices=('behavioral', 'coverage'), default='behavioral')
            item.add_argument('--llvm-profdata', type=Path)
            item.add_argument('--llvm-cov', type=Path)
    for command in ['replay', 'minimize']:
        item = sub.add_parser(command)
        item.add_argument('--binary', type=Path, required=True)
        item.add_argument('--input', type=Path, required=True)
        item.add_argument('--target', choices=['opc', 'import', 'roundtrip'], required=True)
        item.add_argument('--seed', type=int, required=True)
        item.add_argument('--timeout', type=float, default=3)
        if command == 'minimize':
            item.add_argument('--class', dest='failure_class', choices=sorted(FAILURES), required=True)
            item.add_argument('--out', type=Path, required=True)
            item.add_argument('--max-runs', type=int, default=128)
    cover_parser = sub.add_parser('cover')
    cover_parser.add_argument('--binary', type=Path, required=True)
    cover_parser.add_argument('--inputs', type=Path, required=True)
    cover_parser.add_argument('--out', required=True)
    cover_parser.add_argument('--timeout', type=float, default=3)
    cover_parser.add_argument('--memory-mib', type=int, default=512)
    cover_parser.add_argument('--llvm-profdata', type=Path)
    cover_parser.add_argument('--llvm-cov', type=Path)
    return parser

def main():
    args = build_parser().parse_args()
    if args.command in ('smoke', 'campaign'):
        return campaign(args, args.command == 'smoke')
    if args.command == 'fuzz':
        return fuzz(args)
    if args.command == 'cover':
        return cover(args)
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
