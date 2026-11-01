"""Portable native GPUI regression: synthetic input, real engine and window pixels.

Run on an unlocked desktop with an automation build. This does not certify a
physical keyboard, system IME, screen reader, display hotplug or device printing.
Only the launched process and its isolated configuration/files are modified.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import socket
import subprocess
import time


class Control:
    def __init__(self, port, profile):
        self.port = port
        self.profile = profile

    def call(self, method, **params):
        token = (self.profile / f"control-{self.port}.token").read_text(encoding="utf-8").strip()
        with socket.create_connection(("127.0.0.1", self.port), timeout=15) as connection:
            with connection.makefile("rw", encoding="utf-8") as stream:
                def exchange(value):
                    stream.write(json.dumps(value, ensure_ascii=False) + "\n")
                    stream.flush()
                    reply = json.loads(stream.readline())
                    if not reply.get("ok"):
                        raise RuntimeError(f"{method}: {reply.get('error')}")
                    return reply["result"]
                exchange({"id": 0, "method": "auth", "params": {"token": token}})
                return exchange({"id": 1, "method": method, "params": params})

    def wait(self, predicate, label, process, timeout=20):
        deadline = time.monotonic() + timeout
        last = None
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"Application exited {process.returncode} while waiting for {label}")
            try:
                last = self.call("ui.inspect")
            except (OSError, ValueError):
                time.sleep(.1)
                continue
            if predicate(last):
                return last
            time.sleep(.05)
        raise TimeoutError(f"{label}; last view: {last}")


def run(binary, output):
    output = output.resolve()
    # Refuse to overwrite evidence or accidentally reuse a user's configuration.
    output.mkdir(parents=True, exist_ok=False)
    profile = output / "profile"
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    environment = os.environ.copy()
    environment["JOYSHEET_CONFIG_DIR"] = str(profile)
    for name in ("JOYSHEET_CONTROL_TOKEN", "GRIDCRAFT_CONTROL_TOKEN"):
        environment.pop(name, None)
    checks = []
    report = {"passed": False, "platform": platform.platform(), "checks": checks,
              "binary": str(binary), "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "eventSource": "production GPUI synthetic input",
              "nativeImeVerified": False, "gpuPresentationMeasured": False}
    process = None
    control = Control(port, profile)
    shortcut = "cmd" if platform.system() == "Darwin" else "ctrl"
    try:
        with (output / "application.log").open("w", encoding="utf-8") as log:
            process = subprocess.Popen([str(binary), "--sample", "sales", "--control", str(port), "--diagnostics"],
                                       cwd=binary.parent, env=environment, stdout=log, stderr=subprocess.STDOUT)

            def wait(predicate, label):
                return control.wait(predicate, label, process)

            def cell(address, expected):
                actual = control.call("cell.get", cell=address).get("value")
                if actual != expected:
                    raise AssertionError(f"{address}: expected {expected!r}, got {actual!r}")

            def key(value, modified=False):
                return control.call("ui.key", key=value, **({shortcut: True} if modified else {}))

            def type_cell(address, text):
                control.call("ui.cell.click", cell=address)
                control.call("ui.text", text=text)
                key("enter")
                wait(lambda v: v["editor"] is None, "input commit")

            def capture(name):
                path = output / name
                if initial["capabilities"]["screenshot"]:
                    result = control.call("ui.screenshot", path=str(path))
                elif platform.system() == "Linux" and os.environ.get("DISPLAY"):
                    # Linux GPUI has no render_to_image implementation. Capture
                    # the actual X11 window instead; never label it GPU readback.
                    xdotool, image_import = shutil.which("xdotool"), shutil.which("import")
                    if not xdotool or not image_import:
                        raise RuntimeError("X11 screenshots require xdotool and ImageMagick import in PATH")
                    windows = subprocess.check_output([xdotool, "search", "--onlyvisible", "--pid", str(process.pid)], text=True).split()
                    if not windows:
                        raise RuntimeError("No visible test-process window")
                    active = subprocess.check_output([xdotool, "getactivewindow"], text=True).strip()
                    if len(windows) == 1:
                        active = windows[0]
                    elif active not in windows:
                        raise RuntimeError("The multi-window test process lost foreground focus")
                    subprocess.run([xdotool, "windowactivate", "--sync", active], check=True, timeout=10)
                    time.sleep(.2)
                    subprocess.run([image_import, "-window", active, str(path)], check=True, timeout=15)
                    result = {"source": "X11 window pixels via ImageMagick", "window": active}
                else:
                    raise RuntimeError("No supported native window capture backend")
                if not path.is_file() or path.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
                    raise AssertionError(f"GPU capture missing or invalid: {path}")
                report.setdefault("captures", {})[name] = result

            initial = wait(lambda v: v["grid"]["height"] > 100 and v["perf"]["frame"] > 1, "first native frame")
            if not initial["capabilities"].get("syntheticIme", initial["capabilities"]["screenshot"]):
                raise RuntimeError("Build joysheet with --features joysheet/automation for native verification")
            report["display"] = initial["display"]
            if platform.system() == "Linux":
                if initial["capabilities"]["screenshot"] or initial["capabilities"]["mcpTargetedScreenshots"]:
                    raise AssertionError("Linux must not advertise unsupported GPUI GPU readback")
                unsupported = output / "unsupported-gpu.png"
                try:
                    control.call("ui.screenshot", path=str(unsupported))
                except RuntimeError as error:
                    if "render_to_image" not in str(error):
                        raise
                else:
                    raise AssertionError("Unsupported GPU screenshot reported success")
                if unsupported.exists():
                    raise AssertionError("Rejected GPU screenshot created an output file")
                checks.append("Linux GPU capability is accurate; unsupported request rejects without output")
            capture("01-sales.png")
            checks.append("native sales window, charts and window screenshot")
            control.call("app.new")
            wait(lambda v: len(v["windows"]) == 2 and v["grid"]["height"] > 100, "new workbook")
            type_cell("A1", "中文报表😀")
            type_cell("B1", "10")
            type_cell("C1", "=B1*2")
            cell("A1", "中文报表😀")
            cell("C1", 20)
            type_cell("B1", "20")
            cell("C1", 40)
            checks.append("Unicode/emoji, pointer/text input and dependent formula recalculation")
            key("z", True)
            wait(lambda v: v["editor"] is None, "undo")
            cell("C1", 20)
            key("y", True)
            wait(lambda v: v["editor"] is None, "redo")
            cell("C1", 40)
            checks.append("platform keyboard undo and redo")
            control.call("ui.cell.click", cell="D1")
            control.call("ui.ime.mark", text="组合😀")
            wait(lambda v: v["composing"], "synthetic marked input")
            key("enter")
            marked = control.call("ui.inspect")
            if marked["editor"] is None or not marked["composing"]:
                raise AssertionError("Enter prematurely committed marked input")
            cell("D1", None)
            control.call("ui.ime.unmark")
            key("enter")
            wait(lambda v: v["editor"] is None, "composition commit")
            cell("D1", "组合😀")
            checks.append("synthetic composition blocks premature Enter; commit retains UTF-16 text")
            control.call("engine.execute", command="selection.set", params={"range": "B1:C1"})
            key("c", True)
            control.call("ui.cell.click", cell="B6")
            key("v", True)
            wait(lambda v: v["editor"] is None and not v["error"], "clipboard paste")
            cell("C6", 40)
            type_cell("B6", "7")
            cell("C6", 14)
            checks.append("system clipboard and relative formula references")
            control.call("ui.cell.click", cell="G2")
            control.call("ui.text", text="=SU")
            key("tab")
            if control.call("ui.inspect")["editor"]["text"] != "=SUM(":
                raise AssertionError("Function completion did not insert SUM(")
            control.call("ui.text", text="B1)")
            key("enter")
            wait(lambda v: v["editor"] is None, "function completion")
            cell("G2", 20)
            checks.append("registered function completion and calculation")
            control.call("engine.execute", command="view.freezeTopRow", params={})
            control.call("engine.execute", command="view.zoom", params={"percent": 125})
            control.call("ui.dialog", name="home.fontSize")
            control.call("ui.dialog.set", field="size", value=14)
            capture("02-format-dialog.png")
            control.call("ui.dialog.confirm")
            wait(lambda v: not v["dialog"] and not v["error"], "format dialog")
            checks.append("freeze, zoom and real formatting form")
            workbook = output / "中文 空格.xlsx"
            control.call("app.save", path=str(workbook))
            if not workbook.is_file():
                raise AssertionError("Save reported success without a workbook")
            type_cell("F2", "关闭取消保留")
            key("w", True)
            wait(lambda v: v["dialog"] == "saveChanges", "unsaved close prompt")
            control.call("ui.dialog.cancel")
            cell("F2", "关闭取消保留")
            checks.append("cancelled close retains unsaved edits")
            control.call("app.save", path=str(workbook))
            control.call("engine.execute", command="file.close", params={"force": True, "document": True})
            control.call("app.open", path=str(workbook))
            wait(lambda v: any(w["path"] == str(workbook) for w in v["windows"]), "XLSX reopen")
            for address, value in [("A1", "中文报表😀"), ("C1", 40), ("C6", 14), ("D1", "组合😀"), ("F2", "关闭取消保留")]:
                cell(address, value)
            capture("03-roundtrip.png")
            checks.append("Chinese/space path XLSX save, close, reopen and retained formulas/text")
            report["finalView"] = control.call("ui.inspect")
            report["passed"] = True
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"passed": True, "checks": checks, "report": str(output / "report.json")}, ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="New evidence directory; must not exist")
    arguments = parser.parse_args()
    run(arguments.binary.resolve(strict=True), arguments.output)
