"""Time middle-page edits through the JoyWord desktop session and its native keyboard."""

import argparse
import ctypes
import json
from pathlib import Path
import platform
import runpy
import secrets
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import app, digest, port, wait, word_request
from probe_native_ime import mac_target, run, system_events


def native_keyboard(process):
    """Return a focus check and a key sender scoped to the test window."""
    if sys.platform == "darwin":
        target = mac_target(process.pid)
        wait(lambda: system_events(f"return exists window 1 of {target}") == "true",
             "Word window", process)
        system_events(f"set frontmost of {target} to true")
        cg = ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
        cf = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
        cg.CGEventCreateKeyboardEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint16, ctypes.c_bool]
        cg.CGEventCreateKeyboardEvent.restype = ctypes.c_void_p
        cg.CGEventPostToPid.argtypes = [ctypes.c_int, ctypes.c_void_p]
        cg.CGEventPostToPid.restype = None
        cf.CFRelease.argtypes = [ctypes.c_void_p]
        cf.CFRelease.restype = None

        def send(insert):
            key = 26 if insert else 51  # ANSI 7 or Delete to the left
            for down in (True, False):
                event = cg.CGEventCreateKeyboardEvent(None, key, down)
                if not event:
                    raise RuntimeError("Core Graphics could not create a key event")
                cg.CGEventPostToPid(process.pid, event)
                cf.CFRelease(event)

        return lambda: system_events(f"return frontmost of {target}") == "true", send

    win = wait(lambda: run("xdotool", "search", "--onlyvisible", "--pid",
                           str(process.pid)).splitlines()[0], "Word X11 window", process)
    run("xdotool", "windowactivate", "--sync", win)
    return (lambda: run("xdotool", "getactivewindow") == win,
            lambda insert: run("xdotool", "key", "--clearmodifiers", "--delay", "0",
                               "7" if insert else "BackSpace"))


def correct_edit(original, actual, inserted):
    # The very same character may occur elsewhere. Compare the entire document and the
    # insertion's position between the surrounding page labels.
    lo, hi = original.index("sample 0499."), original.index("sample 0501.")
    if len(actual) != len(original) + len(inserted):
        return False
    return any(actual == original[:i] + inserted + original[i:] for i in range(lo, hi + 1))


def timed_samples(edit):
    samples = [edit(i % 2 == 0) for i in range(22)][2:]
    return {"samplesMs": samples, "p95Ms": sorted(samples)[18]}


def measure(binary, out):
    fixture = out / "long-document.docx"
    runpy.run_path(str(Path(__file__).with_name("verify-r8-long-doc.py")))["make_fixture"](fixture)
    source_hash = digest(fixture)
    number, key = port(), secrets.token_hex(32)

    def inspect():
        return word_request(number, "document.inspect", key=key).get("result")

    def command(state, name, **params):
        response = word_request(number, "engine.execute", {
            "doc": state["doc"], "version": state["version"], "command": name, "params": params,
        }, key=key)
        if not response["ok"]:
            raise ValueError(response["error"])

    marker = "Alpha beta gamma delta. " * 10
    config = {"JOYWORD_CONFIG_DIR": str(out / "profile"), "JOYWORD_CONTROL_KEY": key}
    with app(binary, ["--control", str(number), "--", str(fixture)], out, config) as process:
        original = wait(inspect, "thousand-page document", process)
        assert len(original["summary"]["pages"]) == 1000
        command(original, "edit.goto", page=500)

        def edit_command(insert):
            state = inspect()
            start = time.monotonic_ns()
            if insert:
                command(state, "text.insert", text=marker)
            else:
                command(state, "edit.undo")
            acknowledged = time.monotonic_ns()
            after = inspect()
            assert len(after["summary"]["pages"]) == 1000
            assert (correct_edit(original["text"], after["text"], marker)
                    if insert else after["text"] == original["text"])
            return round((acknowledged - start) / 1e6, 4)

        command_report = timed_samples(edit_command)
        focused, send = native_keyboard(process)

        def edit_key(insert):
            if not focused():
                raise RuntimeError("Word lost keyboard focus")
            start = time.monotonic_ns()
            send(insert)
            while (time.monotonic_ns() - start) < 5_000_000_000:
                after = inspect()
                observed = time.monotonic_ns()
                if after and (correct_edit(original["text"], after["text"], "7")
                              if insert else after["text"] == original["text"]):
                    assert len(after["summary"]["pages"]) == 1000
                    return round((observed - start) / 1e6, 4)
                time.sleep(.002)
            raise TimeoutError("did not observe the expected key edit on page 500")

        keyboard_report = timed_samples(edit_key)
        assert digest(fixture) == source_hash
    return {
        "platform": platform.platform(), "binarySha256": digest(binary),
        "fixtureSha256": source_hash, "pages": 1000, "editedPage": 500, "originalUnchanged": True,
        "sessionEdit": {
            "operation": f"insert {len(marker)} characters then undo",
            "start": "before dispatch to the desktop's guarded edit command",
            "end": "desktop confirms the document edit", **command_report,
        },
        "systemKeyboard": {
            "operation": "digit 7 then Backspace in the focused editor",
            "start": "before dispatching a native key to the desktop window",
            "end": "first complete readback containing the expected edit",
            "includesReadbackAndScheduling": True, **keyboard_report,
        },
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    dest = args.output.resolve()
    dest.mkdir(parents=True, exist_ok=False)
    (dest / "report.json").write_text(json.dumps(measure(args.binary.resolve(), dest), indent=2) + "\n")
