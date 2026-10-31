"""Drive the real macOS Pinyin input method using OS keyboard events in a test window.
This is OS/IME integration coverage, not a physical-keyboard or other-platform test.
The prior input source is restored even when validation fails.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from paths import ROOT, SCRIPTS, FIXTURES, EVIDENCE, LICENSES, binary, native_helper

def command(args):
    return subprocess.check_output(args, text=True).strip()

def applescript(script):
    return command(['osascript', '-e', script])

def wait_marker(log, marker, process, timeout=12):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        content = log.read_text(errors='replace')
        if 'startup/runtime failure' in content or 'failed=timeout' in content:
            raise RuntimeError(f'IME regression failed: {log}')
        if marker in content:
            return
        if process.poll() is not None:
            raise RuntimeError(f'Application exited before {marker}: {log}')
        time.sleep(.05)
    raise TimeoutError(f'Missing {marker}: {log}')

def capture(pid, output, region=False):
    js = f'ObjC.import("CoreGraphics"); var list=ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo(1,0))); JSON.stringify(list.filter(x => x.kCGWindowOwnerPID === {pid} && x.kCGWindowLayer === 0).map(x => x.kCGWindowNumber));'
    ids = json.loads(command(['osascript', '-l', 'JavaScript', '-e', js]))
    if len(ids) != 1:
        raise RuntimeError(f'Expected one test window, found {ids}')
    if region:
        frontmost = applescript(f'tell application "System Events" to get frontmost of (first application process whose unix id is {pid})')
        if frontmost != 'true': raise RuntimeError('Test window lost focus before candidate capture')
        js = f'ObjC.import("CoreGraphics"); var list=ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo(1,0))); JSON.stringify(list.find(x => x.kCGWindowOwnerPID === {pid} && x.kCGWindowNumber === {ids[0]}).kCGWindowBounds);'
        bounds = json.loads(command(['osascript', '-l', 'JavaScript', '-e', js]))
        rect = ','.join(str(int(bounds[k])) for k in ['X', 'Y', 'Width', 'Height'])
        subprocess.run(['screencapture', '-x', '-R', rect, str(output)], check=True)
    else:
        subprocess.run(['screencapture', '-x', '-o', '-l', str(ids[0]), str(output)], check=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=binary('joydeck', 'debug'))
    parser.add_argument("--table", action="store_true")
    args = parser.parse_args()
    output = EVIDENCE / ('m8' if args.table else 'm3') / f'pinyin-{time.time_ns()}'
    output.mkdir(parents=True)
    log = output / 'native.log'
    with tempfile.TemporaryDirectory(prefix='joydeck-ime-') as temp:
        helper = str(Path(temp) / 'input-source')
        subprocess.run(['clang', str(SCRIPTS / 'macos-input-source.c'), '-framework', 'Carbon', '-framework', 'CoreFoundation', '-o', helper], check=True)
        previous = command([helper])
        env = dict(os.environ, JOYDECK_IME_EVIDENCE=str(output / 'roundtrip.joydeck'))
        with log.open('wb') as stream:
            process = subprocess.Popen([str(args.binary.resolve()), '--table-ime-smoke' if args.table else '--ime-smoke', '--hold'], cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT)
            try:
                wait_marker(log, 'M3_IME ready=true', process)
                applescript(f'tell application "System Events" to tell (first application process whose unix id is {process.pid}) to set frontmost to true')
                if command([helper]) != 'com.apple.inputmethod.SCIM.ITABC':
                    applescript('tell application "System Events" to key code 49 using control down')
                time.sleep(.4)
                selected = command([helper])
                (output / 'input-source.txt').write_text(selected + '\n')
                if selected != 'com.apple.inputmethod.SCIM.ITABC':
                    raise RuntimeError(f'Pinyin source is not active: {selected}')
                time.sleep(.4)
                applescript('tell application "System Events"\nrepeat with code in {6, 4, 31, 45, 5, 13, 14, 45}\nkey code code\ndelay 0.05\nend repeat\nend tell')
                time.sleep(.6)
                with (output / 'input-source.txt').open('a') as source_log: source_log.write(command([helper]) + '\n')
                capture(process.pid, output / 'candidate.png', region=True)
                applescript('tell application "System Events" to key code 49')
                wait_marker(log, 'M3_IME committed=true', process)
                applescript('tell application "System Events"\nrepeat with code in {12, 32, 7, 34, 0, 31}\nkey code code\ndelay 0.05\nend repeat\nend tell')
                wait_marker(log, 'M3_IME cancel_ready=true', process)
                applescript('tell application "System Events" to key code 53')
                wait_marker(log, 'complete=true', process)
                time.sleep(.4)
                capture(process.pid, output / 'committed.png')
                (output / 'result.json').write_text(json.dumps({'input_method':'com.apple.inputmethod.SCIM.ITABC', 'events':'System Events synthetic keyboard events', 'commit':True,'cancel':True,'single_undo':True,'roundtrip':True,'physical_keyboard':False}, indent=2)+'\n')
                print(f'PASS: macOS system Pinyin commit/cancel/undo/roundtrip; evidence: {output}', flush=True)
            finally:
                restore = subprocess.run([helper, previous])
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                if restore.returncode:
                    raise RuntimeError(f'Could not restore previous input source {previous}')

if __name__ == '__main__':
    main()
