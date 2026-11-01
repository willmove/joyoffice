"""macOS production-window preset insertion, parameter, history and save acceptance."""
import argparse
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path
from paths import ROOT, SCRIPTS, binary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=binary())
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    helper = out / 'preset-ax'
    subprocess.run(['swiftc', str(SCRIPTS / 'unified-ui-ax.swift'), '-o', str(helper)], check=True)
    cli = args.binary.resolve().with_name('joydeck-cli')
    fixture = out / 'working.joydeck'
    subprocess.run([str(cli), '--write-root', str(out), 'run', json.dumps([{'method': 'document.save', 'params': {'path': str(fixture)}}])], check=True, capture_output=True)
    state = out / 'state'
    state.mkdir()
    (state / 'preferences.json').write_text(json.dumps(dict(schema=2, generation=0, recent=[], zoom=1., inspector=True, notes=False, width=1400, height=900)), encoding='utf-8')
    log = (out / 'native.log').open('wb')
    process = subprocess.Popen([str(args.binary.resolve()), str(fixture)], cwd=ROOT, env=dict(os.environ, JOYDECK_STATE_DIR=str(state)), stdout=log, stderr=subprocess.STDOUT)

    def ax(action, *values):
        return json.loads(subprocess.check_output([str(helper), str(process.pid), action, *values], text=True))

    def flatten(nodes):
        for n in nodes:
            yield n
            yield from flatten(n.get('children', []))

    def wait_for(name, present=True):
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            nodes = list(flatten(ax('tree')))
            found = any(name in [n.get('id'), n.get('title'), n.get('label')] for n in nodes)
            if found == present:
                return nodes
            if process.poll() is not None:
                raise RuntimeError('Application exited')
            time.sleep(.1)
        raise TimeoutError('UI state not reached: ' + name)

    def press(name):
        subprocess.run(['osascript', '-e', f'tell application "System Events" to tell (first application process whose unix id is {process.pid}) to set frontmost to true'], check=True, capture_output=True)
        ax('press', name)
        time.sleep(.35)

    def capture(name):
        script = f'ObjC.import("CoreGraphics"); JSON.stringify(ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo(1,0))).filter(x=>x.kCGWindowOwnerPID==={process.pid}&&x.kCGWindowLayer===0));'
        windows = json.loads(subprocess.check_output(['osascript', '-l', 'JavaScript', '-e', script], text=True))
        assert windows and windows[0]['kCGWindowIsOnscreen']
        subprocess.run(['screencapture', '-x', '-o', '-l', str(windows[0]['kCGWindowNumber']), str(out / name)], check=True)

    try:
        wait_for('插入')
        subprocess.run(['osascript', '-e', f'tell application "System Events" to tell (first application process whose unix id is {process.pid}) to set frontmost to true'], check=True, capture_output=True)
        press('插入')
        wait_for('shape-gallery')
        ax('focus', 'shape-gallery')
        subprocess.run(['osascript', '-e', 'tell application "System Events" to key code 36'], check=True, capture_output=True)
        wait_for('三角形')
        capture('01-gallery.png')
        press('三角形')
        wait_for('形状参数 1 增加')
        time.sleep(.5)
        capture('02-inserted.png')
        def save_check(name, expected):
            before = hashlib.sha256(fixture.read_bytes()).hexdigest()
            press('save')
            deadline = time.monotonic() + 12
            while hashlib.sha256(fixture.read_bytes()).hexdigest() == before:
                if time.monotonic() >= deadline:
                    raise TimeoutError('Save not published: ' + name)
                time.sleep(.1)
            result = subprocess.check_output([str(cli), '--read-root', str(out), 'inspect', str(fixture)], text=True)
            (out / (name + '-inspection.json')).write_text(result, encoding='utf-8')
            objects = json.loads(result)['result']['deck']['slides'][0]['objects']
            assert len(objects) == 1, name
            assert objects[0]['preset']['name'] == 'triangle', name
            assert objects[0]['preset'].get('adjustments', []) == expected, (name, objects[0]['preset'])

        press('形状参数 1 增加')
        save_check('adjusted', [55000.0])
        capture('03-adjusted.png')
        press('undo')
        save_check('undo', [])
        press('redo')
        save_check('redo', [55000.0])
        report = {'passed': True, 'checks': ['Real gallery inserts and selects triangle', 'Parameter increment, undo and redo via native controls', 'Native save independently reopened by CLI with triangle adjustment 55000'], 'binarySha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'scope': 'macOS AX actions and synthetic keyboard events; no physical-keyboard, screen-reader or other-platform certification'}
        (out / 'result.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        print(json.dumps(report, ensure_ascii=False))
    except Exception:
        (out / 'failure-accessibility.json').write_text(json.dumps(ax('tree'), ensure_ascii=False, indent=2), encoding='utf-8')
        raise
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        log.close()


if __name__ == '__main__':
    main()
