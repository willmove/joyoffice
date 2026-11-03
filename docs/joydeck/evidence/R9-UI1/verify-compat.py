"""Run both shipped transports against public PPTX; no live desktop binding."""
import argparse
import hashlib
import json
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
from pptx import Presentation

ROOT = Path(__file__).resolve().parents[4]
ap = argparse.ArgumentParser(description=__doc__)
ap.add_argument('--cli', type=Path, required=True)
args = ap.parse_args()
exe = args.cli.resolve()
work = Path(tempfile.mkdtemp(prefix='joydeck-ui1-compat-')) / '中文 空格'
work.mkdir()
source = work / '原稿.pptx'
shutil.copy2(ROOT / 'tests/fixtures/joydeck/r1-preview.pptx', source)
original = hashlib.sha256(source.read_bytes()).hexdigest()
records = []
for transport in ['control', 'mcp']:
    p = subprocess.Popen([str(exe), transport, '--root', str(work)], stdin=subprocess.PIPE,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE, encoding='utf-8')
    lines = queue.Queue()
    threading.Thread(target=lambda: [lines.put(line) for line in p.stdout], daemon=True).start()
    count = 0

    def wire(payload, receive=True):
        p.stdin.write(json.dumps(payload, ensure_ascii=False) + '\n')
        p.stdin.flush()
        response = json.loads(lines.get(timeout=30)) if receive else None
        records.append(dict(transport=transport, request=payload, response=response))
        return response

    if transport == 'mcp':
        assert wire(dict(jsonrpc='2.0', id=0, method='initialize', params={}))['result']['serverInfo']['name'] == 'joydeck'
        wire(dict(jsonrpc='2.0', method='notifications/initialized'), False)

    def request(method, params=None, revision=None, target=None, request_id=None):
        global count
        count += 1
        body = dict(protocolVersion='joydeck-control-1', requestId=request_id or f'{transport}-{count}',
                    method=method, params=params or {})
        if target is not None:
            body['target'] = target
        if revision is not None:
            body['expectedRevision'] = revision
        response = wire(body if transport == 'control' else dict(jsonrpc='2.0', id=count,
                            method='tools/call', params=dict(name='joydeck.control', arguments=body)))
        return response if transport == 'control' else response['result']['structuredContent']

    cap = request('engine.commands')['result']
    assert cap['commandSchemaVersion'] == 2 and cap['legacyCommandSchemaSupported'] is False
    assert all(m in cap['methods'] for m in ['command.validate', 'document.undo', 'document.redo'])
    opened = request('document.open', dict(path=str(source)))['result']
    target = opened['documentId']
    assert opened['pages'] == 3 and not opened['dirty']
    valid = request('command.validate', dict(command=dict(command='new-slide')), 0, target)['result']
    assert valid['valid'] and valid['wouldChange'] and valid['sideEffects'] is False
    assert request('document.status')['result'] == opened
    assert 'error' in request('command.validate', dict(command=dict(command='select', page=99, shapes=[])), 0, target)
    assert 'error' in request('command.validate', dict(command=dict(command='new-slide')), 99, target)
    assert 'error' in request('command.validate', dict(command=dict(schemaVersion=1, commandId='page.add', arguments={})), 0, target)
    assert request('document.status')['result'] == opened
    edited = request('command.execute', dict(command=dict(command='new-slide')), 0, target)['result']
    assert edited['pages'] == 4 and edited['canUndo']
    preview = request('command.validate', dict(command=dict(command='undo')), 1, target)['result']
    assert preview['wouldChange'] and request('document.status')['result'] == edited
    undone = request('document.undo', {}, 1, target, 'same-undo')
    assert undone['result']['pages'] == 3 and not undone['result']['dirty'] and undone['result']['canRedo']
    assert request('document.undo', {}, 1, target, 'same-undo') == undone
    assert 'error' in request('document.redo', {}, 1, target)
    redone = request('document.redo', {}, 2, target)['result']
    assert redone['pages'] == 4 and redone['revision'] == 3
    out = work / f'{transport}-saved.pptx'
    assert request('document.save', dict(path=str(out)), 3, target)['result']['dirty'] is False
    assert len(Presentation(out).slides) == 4
    assert request('document.open', dict(path=str(out)))['result']['pages'] == 4
    assert hashlib.sha256(source.read_bytes()).hexdigest() == original
    p.stdin.close()
    assert p.wait(timeout=30) == 0
    assert not p.stderr.read().strip()

report = dict(cli=str(exe), cli_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(), work=str(work),
              source_sha256=original, source_unchanged=True, exchanges=len(records), records=records)
(Path(__file__).parent/'compat.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(f'Control/MCP passed: {len(records)} exchanges; saved PPTX independently read; original unchanged')
