"""macOS production-window UI acceptance; uses isolated state and a copied fixture."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
from paths import ROOT, SCRIPTS, FIXTURES, binary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=binary())
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    helper = out / 'unified-ui-ax'
    subprocess.run(['swiftc', str(SCRIPTS / 'unified-ui-ax.swift'), '-o', str(helper)], check=True)
    fixture = out / 'working-copy.joydeck'
    shutil.copyfile(FIXTURES / 'native-20.joydeck', fixture)
    state = out / 'state'
    state.mkdir(exist_ok=True)
    (state / 'preferences.json').write_text(json.dumps(dict(schema=2, generation=0, recent=[], zoom=1., inspector=False, notes=False, width=1200, height=800)), encoding='utf-8')
    process = subprocess.Popen([str(args.binary.resolve()), str(fixture)], cwd=ROOT,
                               env=dict(os.environ, JOYDECK_STATE_DIR=str(state)),
                               stdout=(out / 'native.log').open('wb'), stderr=subprocess.STDOUT)
    checks = []
    captured_windows = {}

    def ax(action, *values):
        return json.loads(subprocess.check_output([str(helper), str(process.pid), action, *values], text=True))

    def flatten(tree):
        for item in tree:
            yield item
            yield from flatten(item.get('children', []))

    def tree():
        return list(flatten(ax('tree')))

    def wait_for(predicate):
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            current = tree()
            if predicate(current):
                return current
            if process.poll() is not None:
                raise RuntimeError('Application exited')
            time.sleep(.1)
        raise TimeoutError('UI state not reached')

    def control(nodes, name):
        return next((n for n in nodes if name in [n.get('id'), n.get('title'), n.get('label')]), None)

    def press(name):
        ax('press', name)
        time.sleep(.15)

    def open_menu(name):
        ax('focus', name)
        subprocess.run(['osascript', '-e', f'tell application "System Events" to tell (first application process whose unix id is {process.pid}) to set frontmost to true', '-e', 'tell application "System Events" to key code 36'], check=True, capture_output=True)
        wait_for(lambda n: any(i.get('role') == 'AXMenu' for i in n))
        time.sleep(.2)

    def capture(name):
        js = f'ObjC.import("CoreGraphics"); JSON.stringify(ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo(1,0))).filter(x=>x.kCGWindowOwnerPID==={process.pid}&&x.kCGWindowLayer===0));'
        windows = json.loads(subprocess.check_output(['osascript', '-l', 'JavaScript', '-e', js], text=True))
        assert windows and windows[0]['kCGWindowIsOnscreen']
        captured_windows[name] = windows[0]['kCGWindowBounds']
        subprocess.run(['screencapture', '-x', '-o', '-l', str(windows[0]['kCGWindowNumber']), str(out / name)], check=True)

    def resize(width, height=800):
        script = f'tell application "System Events" to tell (first application process whose unix id is {process.pid})\nset frontmost to true\nset size of window 1 to {{{width},{height}}}\nend tell'
        subprocess.run(['osascript', '-e', script], check=True, capture_output=True)
        time.sleep(.2)

    try:
        initial = wait_for(lambda n: control(n, '插入') is not None and control(n, 'undo') is not None)
        resize(1200)
        assert control(initial, 'undo')['enabled'] == '0'
        checks.append('initial undo disabled; localized controls and stable IDs')
        capture('01-main.png')
        press('插入')
        wait_for(lambda n: control(n, 'insert-table') is not None)
        capture('02-insert.png')
        press('insert-table')
        wait_for(lambda n: control(n, 'insert-table-cancel') is not None)
        capture('03-table-dialog.png')
        press('insert-table-cancel')
        wait_for(lambda n: control(n, 'insert-table-cancel') is None)
        checks.append('table dialog opens and cancels without editing')
        press('开始')
        press('add-page')
        wait_for(lambda n: control(n, 'undo')['enabled'] == '1')
        press('undo')
        wait_for(lambda n: control(n, 'undo')['enabled'] == '0')
        press('redo')
        wait_for(lambda n: control(n, 'undo')['enabled'] == '1')
        press('undo')
        checks.append('real add-page button and undo/redo state transitions')
        press('设计')
        wait_for(lambda n: control(n, 'create-design') is not None)
        press('create-design')
        wait_for(lambda n: control(n, 'apply-layout') is not None and control(n, 'apply-layout')['enabled'] == '1')
        press('apply-layout')
        ax('set', '强调色 · 精确值', '#a63d24')
        press('应用主题配色')
        capture('04-design.png')
        checks.append('create design, apply layout and edit actual theme color')
        press('开始')
        open_menu('font-picker')
        capture('05-font-menu.png')
        subprocess.run(['osascript', '-e', 'tell application "System Events" to key code 53'], check=True, capture_output=True)
        open_menu('more-workflows')
        wait_for(lambda n: control(n, '比较文稿与接受更改') is not None)
        capture('06-workflows.png')
        subprocess.run(['osascript', '-e', 'tell application "System Events" to key code 53'], check=True, capture_output=True)
        checks.append('keyboard opens font picker and advanced workflow menu')
        for width in [1600, 960, 800]:
            resize(width, 800 if width == 1600 else 700)
            current = wait_for(lambda n: control(n, 'home-more' if width < 1100 else 'add-page') is not None)
            assert control(current, 'save')
            capture(f'07-width-{width}.png')
        checks.append('requested 1600/960/800 widths retain commands; actual window bounds recorded')
        resize(1200)
        press('save')
        deadline = time.monotonic() + 10
        initial_hash = hashlib.sha256((FIXTURES / 'native-20.joydeck').read_bytes()).hexdigest()
        while time.monotonic() < deadline:
            if hashlib.sha256(fixture.read_bytes()).hexdigest() != initial_hash:
                break
            time.sleep(.1)
        else:
            raise TimeoutError('Save not published')
        result = subprocess.check_output([str(args.binary.with_name('joydeck-cli').resolve()), '--read-root', str(out), 'inspect', str(fixture)], cwd=ROOT, text=True)
        (out / 'saved-inspection.json').write_text(result, encoding='utf-8')
        import zipfile
        with zipfile.ZipFile(fixture) as archive:
            document = json.loads(archive.read('document.json'))
        assert document['design']['themes']['theme-1']['colors']['accent'] == '#a63d24'
        assert len(document['slides']) == 20
        checks.append('save and independent CLI reopen preserve theme edit and page count')
        (out / 'final-accessibility.json').write_text(json.dumps(ax('tree'), ensure_ascii=False, indent=2), encoding='utf-8')
        report = dict(passed=True, capturedWindows=captured_windows, binarySha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(), checks=checks,
                      scope='macOS AX actions and OS synthetic events; not physical keyboard, IME or screen-reader certification')
        (out / 'result.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        print(json.dumps(report, ensure_ascii=False))
    except Exception:
        (out / 'failure-accessibility.json').write_text(json.dumps(ax('tree'), ensure_ascii=False, indent=2), encoding='utf-8')
        raise
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)


if __name__ == '__main__':
    main()
