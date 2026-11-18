"""Verify the first-round native pilots in an isolated JoyOffice window on macOS."""
import argparse
import json
from pathlib import Path
import platform
import secrets
import select
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET
import zipfile

from probe_native_ime import mac_keys, mac_target, run, select_mac_pinyin, system_events

from native_pilot_support import ROOT, SCRIPTS, SheetControl, app, digest, port, wait, word_request

SPELLING = ["6", "4", "31", "45", "5", "13", "14", "45"]


def candidates(pid):
    # Only inspect windows belonging to the launched process. AppKit hosts its candidate panel.
    query = ('ObjC.import("CoreGraphics"); let w=ObjC.deepUnwrap(ObjC.castRefToObject('
             '$.CGWindowListCopyWindowInfo(1,0))); JSON.stringify(w.filter(x=>x.kCGWindowOwnerPID==='
             + str(pid) + ').map(x=>({id:x.kCGWindowNumber,level:x.kCGWindowLayer,b:x.kCGWindowBounds})));')
    return json.loads(run("osascript", "-l", "JavaScript", "-e", query))


def pinyin(process, helper, out, report, after_commit, after_cancel):
    report["inputMethod"] = select_mac_pinyin(helper, process)
    pid = process.pid
    previous = {w["id"] for w in candidates(pid) if w["level"] == 20}
    mac_keys(pid, SPELLING)

    def new_panel():
        windows = candidates(pid)
        main = [w for w in windows if w["level"] == 0]
        panels = [w for w in windows if w["level"] == 20 and w["id"] not in previous]
        if len(main) == 1 and len(panels) == 1:
            return main[0], panels[0]

    window, panel = wait(new_panel, "system candidate panel", process)
    screen, box = window["b"], panel["b"]
    # Window coordinates are logical display points. Retain the placement relative to the app.
    x, y = box["X"] - screen["X"], box["Y"] - screen["Y"]
    if not (0 <= x < screen["Width"] and 0 <= y < screen["Height"]):
        raise AssertionError("candidate panel is outside the focused test window")
    run("screencapture", "-x", "-o", "-l", str(panel["id"]), str(out / "candidate.png"))
    # Capture the adjacent editor region only while the test window owns keyboard focus.
    if system_events(f"return frontmost of {mac_target(pid)}") != "true":
        raise RuntimeError("lost focus before candidate capture")
    left, top = max(0, x - 95), max(0, y - 75)
    width, height = min(box["Width"] + 115, screen["Width"] - left), min(box["Height"] + 110, screen["Height"] - top)
    rect = ",".join(str(int(v)) for v in [left + screen["X"], top + screen["Y"], width, height])
    run("screencapture", "-x", "-R", rect, str(out / "editor.png"))
    report["candidateBoundsInWindow"] = [x, y, box["Width"], box["Height"]]
    mac_keys(pid, ["49"])
    wait(after_commit, "selected Chinese", process)
    mac_keys(pid, SPELLING[:3])
    wait(lambda: any(w["level"] == 20 for w in candidates(pid)), "second candidate panel", process)
    mac_keys(pid, ["53"])
    wait(lambda: all(w["level"] != 20 for w in candidates(pid)) and after_cancel(), "cancelled composition", process)
    report["checks"] += ["candidate shown", "selected Chinese", "Escape discarded second preedit"]


def word(binary, out, helper, report):
    source = out / "source.docx"
    shutil.copyfile(ROOT / "tests/fixtures/docx/DOCX-MIN-001.docx", source)
    source_hash = digest(source)
    target = out / "重构 副本.docx"
    accessibility = out / "native-ax"
    run("swiftc", str(SCRIPTS / "joydeck/unified-ui-ax.swift"), "-o", str(accessibility))
    number, token = port(), secrets.token_hex(32)
    env = {"JOYWORD_CONFIG_DIR": str(out / "profile"), "JOYWORD_CONTROL_KEY": token, "JOYWORD_SAVE_TRACE": "1"}

    def inspect():
        reply = word_request(number, "document.inspect", key=token)
        if not reply["ok"]:
            return None
        return reply["result"]

    def command(name, *, wait_for_load=False, **params):
        state = inspect()
        reply = word_request(number, "engine.execute", {"command": name, "doc": state["doc"],
                              "version": state["version"], "params": params}, token)
        if not reply["ok"]:
            if wait_for_load and reply["error"] == "finish the active dialog or composition first":
                return False
            raise RuntimeError(str(reply["error"]))
        return True

    with app(binary, ["--control", str(number), "--", str(source)], out, env) as p:
        baseline = wait(inspect, "Word document", p)["text"]
        wait(lambda: command("caret.docEnd", wait_for_load=True), "Word editing focus after opening", p)
        expected = baseline + "中文"
        key = lambda code: mac_keys(p.pid, [code])
        pinyin(p, helper, out, report, lambda: inspect()["text"] == expected, lambda: inspect()["text"] == expected)
        key("6 using command down")
        wait(lambda: inspect()["text"] == baseline, "undo one commit", p)
        key("6 using {command down, shift down}")
        wait(lambda: inspect()["text"] == expected, "redo one commit", p)
        report["checks"].append("one native undo and redo")

        focus = f'(value of attribute "AXFocusedUIElement" of {mac_target(p.pid)})'
        def panel():
            key("1 using {command down, shift down}")
            wait(lambda: system_events(f"return role of {focus}") in ("AXTextField", "AXComboBox"), "Save As name field", p)
        def save_name(name):
            system_events(f'set value of attribute "AXValue" of {focus} to "{name}"')
            key("36")
        def trace():
            return [s.split(" ", 1)[1] for s in (out / "desktop.log").read_text().splitlines() if s.startswith("joyword-save ")]

        panel()
        key("53")
        wait(lambda: "picker-cancelled" in trace() and inspect()["text"] == expected, "cancel kept draft", p)
        panel()
        save_name("格式错误.txt")
        wait(lambda: "job-rejected" in trace() and inspect()["text"] == expected, "wrong extension rejected", p)
        assert not (out / "格式错误.txt").exists()
        panel()
        save_name(target.name)
        wait(lambda: "readback-verified" in trace() and target.exists(), "Save As readback", p)
        with zipfile.ZipFile(target) as z:
            root = ET.fromstring(z.read("word/document.xml"))
        assert "".join(e.text or "" for e in root.iter("{http://schemas.openxmlformats.org/wordprocessingml/2006/main}t")).endswith("中文")
        command("caret.docEnd")
        command("text.insert", text="复核")
        key("1 using command down")
        time.sleep(.4)
        if trace().count("readback-verified") == 1:
            # Ordinary save exposes a metadata-protection banner in the main window.
            def confirm():
                labels = ("Confirm replace original", "确认替换原文件")
                for label in labels:
                    attempt = subprocess.run([accessibility, str(p.pid), "press", label], capture_output=True, timeout=5)
                    if attempt.returncode == 0:
                        return True
                return False
            wait(confirm, "confirm replacement of the test copy", p)
        wait(lambda: trace().count("readback-verified") == 2, "ordinary save after Save As", p)
        assert digest(source) == source_hash
        report["trace"] = trace()
    number = port()
    with app(binary, ["--control", str(number), "--", str(target)], out, env, "reopen.log") as reopened:
        wait(lambda: (inspect() or {}).get("text") == expected + "复核", "fresh Word session readback", reopened)
    report["checks"] += ["cancel kept draft", "rejected format kept draft", "retry to Chinese/space path",
                         "ordinary save to copy", "original hash unchanged", "fresh desktop process readback"]
    report["sourceSha256"], report["copySha256"] = source_hash, digest(target)


def deck(binary, out, helper, report):
    source = out / "source.pptx"
    shutil.copyfile(ROOT / "tests/fixtures/joydeck/r1-preview.pptx", source)
    initial_hash = digest(source)
    target = out / "演讲 备注.pptx"
    env = {"JOYDECK_CONFIG_DIR": str(out / "profile"), "XDG_STATE_HOME": str(out / "state")}
    sequence = 0
    def call(process, method, params=None, guard=True):
        nonlocal sequence
        state = call(process, "document.status", guard=False) if guard else None
        sequence += 1
        message = {"protocolVersion": "joydeck-control-1", "requestId": str(sequence),
                   "method": method, "params": params or {}}
        if state:
            message.update(target=state["documentId"], expectedRevision=state["revision"])
        process.stdin.write(json.dumps(message) + "\n")
        process.stdin.flush()
        if not select.select([process.stdout], [], [], 5)[0]:
            raise TimeoutError(method)
        result = json.loads(process.stdout.readline())
        if "error" in result:
            raise ValueError(str(result["error"]))
        return result["result"]

    def notes(process):
        return call(process, "document.inspect")["slides"][0]["notes"]
    args = ["--control-stdio=" + str(out)]
    with app(binary, [str(source), *args], out, env) as p:
        initial = wait(lambda: notes(p), "Deck notes", p)
        system_events(f"set frontmost of {mac_target(p.pid)} to true")
        bounds = system_events(f"tell {mac_target(p.pid)} to return (position of window 1) & (size of window 1)")
        x, y, w, h = map(int, bounds.split(", "))
        # R9's notes pane is immediately above the bottom status bar.
        run("swift", "-e", 'import Cocoa; import CoreGraphics; '
            + f'guard NSWorkspace.shared.frontmostApplication?.processIdentifier == {p.pid} else {{ exit(1) }}; '
            + f'let pt = CGPoint(x:{x+w//2},y:{y+h-51}); '
            + 'for t in [CGEventType.leftMouseDown, .leftMouseUp] { CGEvent(mouseEventSource:nil, mouseType:t, mouseCursorPosition:pt, mouseButton:.left)?.post(tap:.cghidEventTap) }')
        mac_keys(p.pid, ["0 using command down"])
        pinyin(p, helper, out, report, lambda: notes(p) == "中文", lambda: notes(p) == "中文")
        mac_keys(p.pid, ["48"])
        mac_keys(p.pid, ["6 using command down"])
        wait(lambda: notes(p) == initial, "undo notes after focus change", p)
        mac_keys(p.pid, ["6 using {command down, shift down}"])
        wait(lambda: notes(p) == "中文", "redo notes", p)
        call(p, "document.save", {"path": str(target)})
    with zipfile.ZipFile(target) as z:
        xml = ET.fromstring(z.read("ppt/notesSlides/notesSlide1.xml"))
    assert any(e.text == "中文" for e in xml.iter("{http://schemas.openxmlformats.org/drawingml/2006/main}t"))
    with app(binary, [str(target), *args], out, env, "reopen.log") as reopened:
        wait(lambda: notes(reopened) == "中文", "fresh Deck session notes", reopened)
    assert digest(source) == initial_hash
    report["checks"] += ["notes persisted after Tab", "one native undo and redo", "independent PPTX notes XML",
                         "original hash unchanged", "fresh desktop process readback"]
    report["sourceSha256"], report["copySha256"] = initial_hash, digest(target)


def sheet(binary, out, helper, report):
    number = port()
    profile = out / "profile"
    env = {"JOYSHEET_CONFIG_DIR": str(profile)}
    unset_tokens = ("JOYSHEET_CONTROL_TOKEN", "GRIDCRAFT_CONTROL_TOKEN")
    control = SheetControl(number, profile)
    call = control.call
    target = out / "中文 单元格.xlsx"
    with app(binary, ["--control", str(number), "--diagnostics"], out, env, unset=unset_tokens) as p:
        control.wait(lambda v: v["perf"]["frame"] > 1, "first frame", p)
        call("ui.cell.click", cell="A1")
        assert call("ui.inspect")["editor"] is None, "expected a selected cell before typing"
        def committed():
            v = call("ui.inspect")
            return not v["composing"] and v["editor"]["text"] == "中文"
        pinyin(p, helper, out, report, committed, committed)
        mac_keys(p.pid, ["48"])
        call("ui.awaitIdle")
        def value():
            return call("cell.get", cell="A1").get("value")
        wait(lambda: value() == "中文", "Tab committed cell", p)
        for key, expected in [("6 using command down", (None, "")), ("6 using {command down, shift down}", ("中文",))]:
            mac_keys(p.pid, [key])
            wait(lambda: value() in expected, "native workbook history", p)
        call("app.save", path=str(target))
    number = port()
    reopened = SheetControl(number, profile)
    with app(binary, [str(target), "--control", str(number)], out, env, "reopen.log", unset_tokens) as p:
        reopened.wait(lambda v: v["perf"]["frame"] > 1, "first frame after reopening", p)
        wait(lambda: reopened.call("cell.get", cell="A1").get("value") == "中文", "fresh Sheet session readback", p)
    report["checks"] += ["typed directly into selected cell", "Tab committed cell", "one native undo and redo",
                         "fresh desktop process readback"]
    report["copySha256"] = digest(target)
    visual = subprocess.run(["swift", SCRIPTS / "joysheet/check-macos-preedit.swift", out / "editor.png"],
                            capture_output=True, text=True, timeout=60)
    if visual.returncode:
        raise RuntimeError(visual.stderr.strip() or "macOS preedit image check failed")
    report["checks"].append("complete Pinyin visible on one line")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", choices=("word", "deck", "sheet"))
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("requires an unlocked macOS desktop")
    out, binary = args.output.resolve(), args.binary.resolve()
    out.mkdir(parents=True, exist_ok=False)
    helper = out / "input-source"
    run("clang", str(SCRIPTS / "joydeck/macos-input-source.c"), "-framework", "Carbon",
        "-framework", "CoreFoundation", "-o", str(helper))
    previous = run(helper)
    report = {"passed": False, "product": args.product, "checks": [], "system": run("sw_vers", "-productVersion"),
              "binarySha256": digest(binary), "events": "System Events keyboard through macOS Pinyin"}
    try:
        {"word": word, "deck": deck, "sheet": sheet}[args.product](binary, out, helper, report)
        report["passed"] = True
    except Exception as e:
        report["error"] = str(e)
        raise
    finally:
        try:
            run(helper, previous)
        finally:
            (out / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
