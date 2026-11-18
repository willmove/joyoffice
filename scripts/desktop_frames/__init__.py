"""Observe the first OS-presented frame containing a calibrated window region."""
from collections import deque
from contextlib import contextmanager
import platform
import subprocess
from threading import Condition, Thread

from probe_native_ime import run


@contextmanager
def display_stream(helper, pid, crop, directory):
    mac = platform.system() == "Darwin"
    target = str(pid) if mac else run("xdotool", "search", "--onlyvisible", "--pid", str(pid)).splitlines()[0]
    frames = deque(maxlen=1024)
    condition = Condition()
    with (directory / "display.log").open("w") as log:
        proc = subprocess.Popen([helper,target,*map(str,crop)], stdout=subprocess.PIPE, stderr=log, text=True)
        def read():
            for line in proc.stdout:
                fields = line.split()
                if len(fields) == 2:
                    with condition:
                        frames.append(tuple(map(int, fields)))
                        condition.notify_all()
        reader = Thread(target=read, daemon=True)
        reader.start()
        def match(after, wanted=None, timeout=5):
            with condition:
                def latest():
                    if proc.poll() is not None:
                        raise RuntimeError(f"display observer exited {proc.returncode}")
                    return next(((ns,h) for ns,h in frames if ns >= after and (wanted is None or h == wanted)), None)
                if not condition.wait_for(latest, timeout=timeout):
                    raise TimeoutError(f"no matching displayed sample, got {list(frames)[-5:]}")
                return latest()
        try:
            yield frames, match
        finally:
            proc.terminate()
            proc.wait(timeout=5)
            reader.join(2)
