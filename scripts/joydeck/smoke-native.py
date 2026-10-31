"""Native window adapter regressions; does not simulate a physical keyboard or IME."""
import argparse
import os
from pathlib import Path
import subprocess
import time

from paths import ROOT, SCRIPTS, FIXTURES, EVIDENCE, LICENSES, binary, native_helper
MARKERS = {
    'M0': ('--smoke', ['utf16_commit=true', 'composition_cancel=true', 'modal_open=true', 'second_window_closed=true', 'Diagnostic worker settled at 100%']),
    'M1': ('--viewer-smoke', ['M1_SMOKE passed=true']),
    'M2': ('--editor-smoke', ['M2_SMOKE complete=true']),
    'M3': ('--text-smoke', ['rich_roundtrip=true', 'complete=true', 'overlay_shape_cache=true']),
    'M4': ('--scene-smoke', ['M4_SMOKE move_no_page_raster=true', 'cache_rebuild_preserves_document=true', 'stale_document_job_discarded=true complete=true']),
}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('stages', nargs='*', choices=MARKERS, default=list(MARKERS))
    parser.add_argument('--binary', type=Path, default=binary('joydeck'))
    parser.add_argument('--timeout', type=float, default=40)
    args = parser.parse_args()
    for stage in args.stages:
        flag, markers = MARKERS[stage]
        log = ROOT / 'logs' / f'{stage.lower()}-native-{os.sys.platform}.log'
        log.parent.mkdir(exist_ok=True)
        with log.open('wb') as stream:
            process = subprocess.Popen([str(args.binary.resolve()), flag], cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + args.timeout
                while True:
                    content = log.read_text(errors='replace')
                    if 'M3_SMOKE blocked=' in content:
                        raise RuntimeError(f'{stage} native validation blocked; requires an active drawable window: {log}')
                    if 'JoyDeck startup/runtime failure:' in content or 'error=timeout' in content or 'M3_SMOKE blocked=' in content or any(line.startswith(('M1_SMOKE failed=true', 'M2_SMOKE error=', 'M4_SMOKE failed=')) for line in content.splitlines()):
                        raise RuntimeError(f'{stage} native failure: {log}')
                    if all(marker in content for marker in markers):
                        if process.poll() not in (None, 0):
                            raise RuntimeError(f'{stage} exited {process.returncode}: {log}')
                        print(f'{stage} PASS: {log}', flush=True)
                        break
                    if process.poll() is not None:
                        raise RuntimeError(f'{stage} exited {process.returncode} without success markers: {log}')
                    if time.monotonic() >= deadline:
                        raise RuntimeError(f'{stage} native timeout: {log}')
                    time.sleep(.1)
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()

if __name__ == '__main__':
    main()
