"""Exercise every registered edit over real local/remote MCP, with validate, replay and undo."""
import argparse
import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import time
import wave

from paths import ROOT, binary


class Client:
    def __init__(self, command, prefix):
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, text=True, encoding='utf-8')
        self.prefix, self.serial, self.target = prefix, 0, None
        response = self.rpc('initialize', {'protocolVersion': '2025-11-25', 'capabilities': {},
                                        'clientInfo': {'name': 'catalog-verifier', 'version': '1'}})
        assert response['result']['protocolVersion'] == '2025-11-25', response
        self.process.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
        self.process.stdin.flush()

    def rpc(self, method, params):
        self.serial += 1
        request = {'jsonrpc': '2.0', 'id': self.serial, 'method': method, 'params': params}
        self.process.stdin.write(json.dumps(request, ensure_ascii=False) + '\n')
        self.process.stdin.flush()
        response = json.loads(self.process.stdout.readline())
        assert response['id'] == self.serial, response
        return response

    def call(self, method, params=None, operation=None):
        operation = operation or f'{self.prefix}-{self.serial}'
        args = {'operationId': operation, 'method': method, 'params': params or {}}
        if self.target is not None:
            args['target'] = self.target
        response = self.rpc('tools/call', {'name': 'joydeck_dispatch', 'arguments': args})
        result = json.loads(response['result']['content'][0]['text'])
        if method in ('document.new', 'document.open') and 'result' in result:
            self.target = result['result']['documentId']
        return result

    def success(self, method, params=None, operation=None):
        result = self.call(method, params, operation)
        assert 'result' in result, result
        return result['result']

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=8)
        assert self.process.returncode == 0, self.process.stderr.read()


def shape(kind):
    return {'kind': kind, 'x': 50, 'y': 60, 'width': 224, 'height': 88, 'fontSize': 24}


def seed(client):
    client.success('document.new', {'dirtyPolicy': 'discard'})
    client.success('command.execute', {'command': 'page.add'})
    for kind in ('rect', 'ellipse', 'table'):
        obj = shape(kind)
        if kind == 'table':
            cell_style = dict.fromkeys(('fill', 'color', 'family', 'size', 'bold', 'align', 'border', 'border_width', 'padding'))
            style = {'header': True, 'banded': True, 'border': '#d8dfec', 'border_width': 1,
                     'header_fill': '@accent', 'body_fill': '#ffffff', 'band_fill': '#f2f4fa',
                     'color': '#243247', 'header_color': '#ffffff', 'family': '@body',
                     'size': 18, 'padding': 6, 'align': 'left'}
            obj['table'] = {'rows': [44, 44], 'columns': [112, 112], 'style': style,
                            'cells': [{'text': '', 'runs': [], 'span': [1, 1], 'style': cell_style} for _ in range(4)]}
        client.success('command.execute', {'command': 'object.add', 'page': 0, 'object': obj})
    return client.success('document.inspect')


def cases(deck):
    changed = copy.deepcopy(deck['slides'][0]['objects'][0])
    changed['x'], changed['fill'] = 200, '#aabbcc'
    pcm = io.BytesIO()
    with wave.open(pcm, 'wb') as wav:
        wav.setnchannels(1); wav.setsampwidth(2); wav.setframerate(8000)
        wav.writeframes(b'\0\0' * 200)
    asset_bytes = pcm.getvalue()
    asset_id = hashlib.sha256(asset_bytes).hexdigest()
    asset = {'extension': 'wav', 'bytes': list(asset_bytes), 'info': {
        'profiles': [], 'fps': 0, 'duration': .025, 'codecs': ['lpcm'], 'width': 0,
        'height': 0, 'adapter': 'model-pcm-fixture'}}
    return [
        {'command': 'page.add'}, {'command': 'page.copy', 'pages': [0]},
        {'command': 'page.delete', 'pages': [1]}, {'command': 'page.reorder', 'pages': [0], 'target': 2},
        {'command': 'page.visibility', 'pages': [0]}, {'command': 'page.size', 'width': 800, 'height': 600},
        {'command': 'object.add', 'page': 0, 'object': shape('text')},
        {'command': 'object.update', 'page': 0, 'objects': [changed]},
        {'command': 'object.delete', 'page': 0, 'objects': [0, 1]},
        {'command': 'object.reorder', 'page': 0, 'objects': [0], 'target': 3},
        {'command': 'review.edit', 'action': {'type': 'add', 'id': 'thread-one', 'slide': deck['slides'][0]['id'],
                                            'object': None, 'author': '作者', 'text': '批注'}},
        {'command': 'table.edit', 'page': 0, 'object': deck['slides'][0]['objects'][2]['id'],
         'action': {'type': 'setCell', 'value': [0, 0, '自动化表格', []]}},
        {'command': 'document.design', 'action': {'type': 'notes', 'value': [0, '自动化备注']}},
        {'command': 'presentation.motion', 'presentation': {'loop_show': True}},
        {'command': 'media.edit', 'playback': {'assets': {asset_id: asset}, 'clips': [{
            'id': 'clip-one', 'slide': deck['slides'][0]['id'], 'asset': asset_id, 'start': 0, 'end': .025,
            'volume': .5, 'looping': False, 'autoplay': False, 'cross_slide': False}]}},
        {'command': 'transaction.batch', 'commands': [{'command': 'page.add'},
            {'command': 'document.design', 'action': {'type': 'notes', 'value': [0, 'batch备注']}}]},
    ]


def exercise(client, comparison_directory):
    described = client.success('engine.commands')
    assert 'command.validate' in described['methods']
    outcomes = {}
    for index in range(len(described['commands'])):
        before = seed(client)
        params = cases(before['deck'])[index]
        params['expectedRevision'] = before['revision']
        assert client.success('command.validate', params)['sideEffects'] is False
        assert client.success('document.inspect') == before
        operation = f'{client.prefix}-case-{index}'
        first = client.call('command.execute', params, operation)
        assert first['result']['revision'] == before['revision'] + 1, first
        assert client.call('command.execute', params, operation) == first
        after = client.success('document.inspect')
        client.success('document.undo')
        undone = client.success('document.inspect')
        assert undone['deck'] == before['deck'], params
        client.success('document.redo')
        assert client.success('document.inspect')['deck'] == after['deck'], params
        outcomes[params['command']] = after['deck']
    assert set(outcomes) == set(described['commands']) == set(described['commandParameters'])
    state = client.success('document.inspect')
    assert 'error' in client.call('command.execute', {'command': 'page.add', 'expectedRevision': state['revision'] - 1})
    assert 'error' in client.call('command.validate', {'command': 'page.add', 'typo': True})
    assert client.success('document.inspect') == state
    if comparison_directory:
        client.success('document.open', {'path': str(comparison_directory / 'image-base.joydeck'), 'dirtyPolicy': 'discard'})
        before = client.success('document.inspect')
        report = client.success('document.compare', {'path': str(comparison_directory / 'image-reference.joydeck')})
        index = next(i for i, row in enumerate(report['differences']) if row['kind'] == 'object.added')
        params = {'path': str(comparison_directory / 'image-reference.joydeck'), 'index': index,
                  'baseSha256': report['base_sha256'], 'referenceSha256': report['reference_sha256']}
        assert 'error' in client.call('document.acceptObject', params)
        assert client.success('document.inspect') == before
        client.success('document.acceptObject', dict(params, includeResources=True))
        assert not client.success('document.compare', {'path': params['path']})['differences']
        client.success('document.undo')
        assert client.success('document.inspect')['deck'] == before['deck']
    return outcomes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=binary('joydeck-cli'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--comparison-directory', type=Path)
    args = parser.parse_args()
    executable = str(args.binary.resolve())
    with tempfile.TemporaryDirectory(prefix='joydeck-command-catalog-') as temporary:
        root = Path(temporary)
        options = ['--read-root', str(root), '--write-root', str(root)]
        if args.comparison_directory:
            args.comparison_directory = args.comparison_directory.resolve()
            options += ['--read-root', str(args.comparison_directory)]
        local = Client([executable, *options, 'mcp'], 'local')
        try:
            local_outcomes = exercise(local, args.comparison_directory)
        finally:
            local.close()
        descriptor = root / 'control.json'
        server = subprocess.Popen([executable, *options, 'serve', str(descriptor)], stdin=subprocess.PIPE,
                                  stderr=subprocess.PIPE, text=True, encoding='utf-8')
        try:
            for _ in range(100):
                if descriptor.exists():
                    break
                assert server.poll() is None
                time.sleep(.05)
            remote = Client([executable, '--connect', str(descriptor), 'mcp'], 'remote')
            try:
                remote_outcomes = exercise(remote, args.comparison_directory)
            finally:
                remote.close()
        finally:
            server.stdin.write('\n'); server.stdin.flush(); server.wait(timeout=8)
        def canonical(deck):
            names = {o['id']: f'object-{page}-{i}' for page, slide in enumerate(deck['slides'])
                     for i, o in enumerate(slide['objects'])}
            def visit(value):
                if isinstance(value, str):
                    return names.get(value, value)
                if isinstance(value, list):
                    return [visit(item) for item in value]
                if isinstance(value, dict):
                    return {names.get(key, key): visit(item) for key, item in value.items()}
                return value
            return visit(deck)
        assert {key: canonical(value) for key, value in local_outcomes.items()} == {
            key: canonical(value) for key, value in remote_outcomes.items()}
    result = {'passed': True, 'binarySha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
              'commands': sorted(local_outcomes), 'commandCount': len(local_outcomes),
              'localAndRemoteMcpEqual': True, 'validatePreservesDocumentAndHistory': True,
              'replayAndSingleUndoRedo': True, 'staleRevisionAndUnknownParametersRejected': True,
              'imageDependencyAcceptedUndo': bool(args.comparison_directory)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
