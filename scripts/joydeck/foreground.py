"""QA-only macOS foreground activation before collecting native window pixels."""
import json
import subprocess
import time


def focus_window(pid, timeout=3.0, settle=0.2):
    """Activate the tested process and verify the foreground PID, or fail.

    Pixel thresholds are unchanged: activation establishes the drawable-window
    precondition on an unlocked desktop, rather than accepting occluded pixels.
    """
    if isinstance(pid, bool) or not isinstance(pid, int) or pid <= 0:
        raise ValueError('A positive process ID is required for window capture')
    script = (
        'ObjC.import("AppKit"); '
        f'var app=$.NSRunningApplication.runningApplicationWithProcessIdentifier({pid}); '
        'var activated=app.activateWithOptions(2); '
        'JSON.stringify({activated:Boolean(activated), '
        'frontmostPid:Number($.NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier)});'
    )
    deadline = time.monotonic() + timeout
    attempts = 0
    last = None
    while time.monotonic() < deadline:
        last = json.loads(subprocess.check_output(
            ['osascript', '-l', 'JavaScript', '-e', script], text=True))
        attempts += 1
        if last.get('frontmostPid') == pid:
            # Allow the activation-triggered frame to be presented before the
            # existing CPU/GPUI pixel assertions inspect the window surface.
            time.sleep(settle)
            return {'requestedPid': pid, 'frontmostPid': pid, 'verified': True,
                    'activationRequested': True, 'attempts': attempts,
                    'settleSeconds': settle}
        time.sleep(0.05)
    raise RuntimeError(f'Tested process {pid} did not become foreground: {last}')
