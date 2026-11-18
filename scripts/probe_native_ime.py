"""Compare a stock GPUI field with the product IME probes. Requires an unlocked desktop."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import platform
import subprocess
import time


def run(*args):
    return subprocess.check_output(args, text=True, timeout=12).strip()


def system_events(body):
    return run("osascript", "-e", 'tell application "System Events"\n' + body + '\nend tell')


def mac_target(pid):
    return f"(first application process whose unix id is {pid})"


def mac_keys(pid, codes):
    # Deliver one spelling in order. Recheck focus before each key in that sequence.
    target = mac_target(pid)
    body = []
    for code in codes:
        body += [f'if not frontmost of {target} then error "test window lost focus"',
                 f"key code {code}", "delay 0.05"]
    system_events("\n".join(body))


def select_mac_pinyin(helper, process):
    target = mac_target(process.pid)
    wait_for(lambda: system_events(f"return exists window 1 of {target}") == "true", "visible test window", process)
    system_events(f"set frontmost of {target} to true")
    wait_for(lambda: system_events(f"return frontmost of {target}") == "true", "keyboard focus", process)
    wanted = "com.apple.inputmethod.SCIM.ITABC"
    if run(helper) != wanted:
        mac_keys(process.pid, ["49 using control down"])
        time.sleep(.4)
        if run(helper) != wanted:
            run(helper, wanted)
    wait_for(lambda: run(helper) == wanted, "macOS Pinyin", process)
    return wanted


def wait_for(predicate, label, process):
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"probe exited {process.returncode}")
        if predicate():
            return
        time.sleep(0.1)
    raise RuntimeError(f"timed out waiting for {label}")


def x11_selection_owner(atom_name):
    # XIM_SERVERS can retain an atom after its last owner disconnects.
    x11 = ctypes.CDLL("libX11.so.6")
    pointer, string, integer, xid = ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int, ctypes.c_ulong
    x11.XOpenDisplay.argtypes, x11.XOpenDisplay.restype = [string], pointer
    x11.XInternAtom.argtypes, x11.XInternAtom.restype = [pointer, string, integer], xid
    x11.XGetSelectionOwner.argtypes, x11.XGetSelectionOwner.restype = [pointer, xid], xid
    x11.XCloseDisplay.argtypes, x11.XCloseDisplay.restype = [pointer], integer
    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("cannot open the X11 display to inspect the XIM selection")
    try:
        atom = x11.XInternAtom(display, atom_name.encode("utf-8"), 1)
        return x11.XGetSelectionOwner(display, atom) if atom else 0
    finally:
        x11.XCloseDisplay(display)


def probe(binary, output, sogou_shift=False, method=None, server_preedit=False):
    output.mkdir(parents=True, exist_ok=False)
    log = output / "input.log"
    report = {"passed": False, "system": platform.platform(), "events": "OS synthetic keys",
              "binary": str(binary), "checks": []}
    mac = platform.system() == "Darwin"
    if sogou_shift and mac:
        raise ValueError("--sogou-shift requires the UOS X11 input method")
    if not mac and (platform.system() != "Linux" or not os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
        raise RuntimeError("use an X11 session or macOS")
    if mac:
        helper = output / "input-source"
        source = Path(__file__).parent / "joydeck" / "macos-input-source.c"
        run("clang", str(source), "-framework", "Carbon", "-framework", "CoreFoundation", "-o", str(helper))
        previous = run(helper)
    else:
        report["environment"] = {k: os.environ.get(k) for k in ("LANG", "LC_ALL", "LC_CTYPE", "XMODIFIERS", "XDG_SESSION_TYPE")}
        report["ximServers"] = run("xprop", "-root", "XIM_SERVERS")
        modifier = os.environ.get("XMODIFIERS", "")
        if modifier.startswith("@im="):
            report["ximSelectionOwner"] = x11_selection_owner("@server=" + modifier.removeprefix("@im="))
        report["inputMethod"] = run("fcitx5-remote", "-n")
        previous_method = report["inputMethod"]
        previous = run("fcitx5-remote")
    with log.open("w") as stream:
        process = subprocess.Popen([binary], stdout=stream, stderr=subprocess.STDOUT)
        toggled_mode = False
        try:
            if mac:
                wait_for(lambda: "focused=true" in log.read_text(), "focused GPUI input", process)
                report["inputMethod"] = select_mac_pinyin(helper, process)

                def keys(codes):
                    if run(helper) != report["inputMethod"]:
                        raise RuntimeError("input source changed during the IME probe")
                    mac_keys(process.pid, codes)
                spelling = ["6", "4", "31", "45", "5", "13", "14", "45"]
                select, cancel, undo = "49", "53", "6 using command down"
            else:
                def find_window():
                    result = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(process.pid)],
                                            text=True, capture_output=True, timeout=5)
                    return result.stdout.splitlines()
                wait_for(find_window, "probe window", process)
                window = find_window()[0]
                run("xdotool", "windowactivate", "--sync", window)
                if method:
                    run("xdotool", "key", "--clearmodifiers", "Left")
                    time.sleep(.3)
                    run("fcitx5-remote", "-s", method)
                run("fcitx5-remote", "-o")
                wait_for(lambda: run("fcitx5-remote") == "2", "active Fcitx", process)
                report["inputMethod"] = run("fcitx5-remote", "-n")

                def keys(codes):
                    if run("xdotool", "getactivewindow") != window:
                        raise RuntimeError("probe lost focus")
                    run("xdotool", "key", "--clearmodifiers", "--delay", "90", *codes)
                spelling = list("zhongwen")
                select, cancel, undo = "space", "Escape", "ctrl+z"
                if sogou_shift:
                    if report["inputMethod"] != "com.sogou.ime.ng.fcitx5.uos":
                        raise RuntimeError("--sogou-shift expects the UOS Sogou input method")
                    keys(["Shift_L"])
                    toggled_mode = True
                    report["temporaryModeToggle"] = "Shift_L"

            def until(marker, offset=0):
                wait_for(lambda: marker in log.read_text()[offset:], marker, process)
                report["checks"].append(marker)
            time.sleep(.5)
            keys(spelling)
            if server_preedit:
                time.sleep(.4)
                if "utf16=0" not in log.read_text() or "utf16=1 " in log.read_text():
                    raise RuntimeError("server-side preedit must leave the input empty until selection")
                report["preedit"] = "server"
            else:
                until("focused=true marked=true")
            keys([select])
            until("marked=false utf16=2 chinese=true")
            pos = len(log.read_text())
            keys(spelling[:3])
            if server_preedit:
                time.sleep(.35)
            else:
                until("marked=true", pos)
            pos = len(log.read_text())
            keys([cancel])
            if server_preedit:
                # Cancelling server-side preedit does not send a client-side update.
                if "marked=false utf16=2 chinese=true" not in log.read_text().splitlines()[-1]:
                    raise RuntimeError("cancelling server-side preedit changed the text")
            else:
                until("marked=false utf16=2 chinese=true", pos)
            pos = len(log.read_text())
            keys([undo])
            until("marked=false utf16=0 chinese=false", pos)
            report["passed"] = True
        except Exception as error:
            report["error"] = str(error)
            raise
        finally:
            try:
                if mac:
                    run(helper, previous)
                else:
                    try:
                        if toggled_mode:
                            if run("xdotool", "getactivewindow") != window:
                                raise RuntimeError("cannot restore Sogou mode because the test window lost focus")
                            keys(["Escape", "Shift_L"])
                            report["restoredMode"] = True
                    finally:
                        if method and previous_method:
                            run("fcitx5-remote", "-s", previous_method)
                        if previous in ("1", "2"):
                            run("fcitx5-remote", "-o" if previous == "2" else "-c")
            finally:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--sogou-shift", action="store_true", help="A/B comparison after one temporary Sogou mode toggle on UOS")
    parser.add_argument("--method", help="Select this Fcitx5 input method in the focused X11 probe")
    parser.add_argument("--server-preedit", action="store_true", help="XIM handles preedit outside the client")
    args = parser.parse_args()
    probe(args.binary.resolve(), args.output.resolve(), args.sogou_shift, args.method, args.server_preedit)
