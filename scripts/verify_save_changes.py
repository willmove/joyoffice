"""Exercise the shared save prompt in real Word and Sheet windows using scratch copies."""
import argparse
import json
from pathlib import Path
import platform
import secrets
import shutil
import time
import xml.etree.ElementTree as ET
import zipfile

from native_pilot_support import ROOT, SheetControl, app, digest, port, wait, word_request
from probe_native_ime import mac_keys, mac_target, run, system_events


def word_keys(process):
    if platform.system() == "Darwin":
        target = mac_target(process.pid)
        wait(lambda: system_events(f"return exists window 1 of {target}") == "true", "Word window", process)
        system_events(f"set frontmost of {target} to true")
        codes = {"new": "45 using command down", "cancel": "53", "tab": "48",
                 "backtab": "48 using shift down", "enter": "36"}

        def key(name):
            mac_keys(process.pid, [codes[name]])
            time.sleep(.2)
    else:
        window = wait(lambda: run("xdotool", "search", "--onlyvisible", "--pid", str(process.pid)).splitlines()[0],
                      "Word window", process)
        run("xdotool", "windowactivate", "--sync", window)
        codes = {"new": "ctrl+n", "cancel": "Escape", "tab": "Tab", "backtab": "shift+Tab", "enter": "Return"}

        def key(name):
            if run("xdotool", "getactivewindow") != window:
                raise RuntimeError("Word no longer has keyboard focus")
            run("xdotool", "key", "--clearmodifiers", codes[name])
            time.sleep(.2)
    return key


def verify_word(binary, out):
    source = out / "保存测试 原件.docx"
    shutil.copyfile(ROOT / "assets/samples/letter.docx", source)
    before = digest(source)
    number, token = port(), secrets.token_hex(32)
    env = {"JOYWORD_CONFIG_DIR": str(out / "profile"), "JOYWORD_CONTROL_KEY": token}
    marker = "共享提示框保存测试"

    def inspect():
        reply = word_request(number, "document.inspect", key=token)
        return reply.get("result")

    def command(name, **params):
        state = inspect()
        reply = word_request(number, "engine.execute", {"command": name, "doc": state["doc"],
                             "version": state["version"], "params": params}, token)
        if not reply["ok"]:
            raise RuntimeError(str(reply["error"]))

    def saved():
        with zipfile.ZipFile(source) as package:
            xml = ET.fromstring(package.read("word/document.xml"))
        return marker in "".join(el.text or "" for el in xml.iter("{http://schemas.openxmlformats.org/wordprocessingml/2006/main}t"))

    with app(binary, ["--control", str(number), "--", str(source)], out, env) as process:
        initial = wait(inspect, "Word document", process)
        key = word_keys(process)
        command("caret.docEnd")
        command("text.insert", text=marker)
        key("new")
        key("tab")
        key("backtab")
        key("cancel")
        assert inspect()["doc"] == initial["doc"] and marker in inspect()["text"] and digest(source) == before
        key("new")
        key("enter")
        key("enter")
        wait(saved, "DOCX readback after replacement confirmation", process)
        wait(lambda: inspect() and inspect()["doc"] != initial["doc"], "pending New completed", process)
    with app(binary, ["--control", str(number), "--", str(source)], out, env, "reopen.log") as process:
        wait(lambda: inspect() and marker in inspect()["text"], "Word fresh process readback", process)
    return ["Tab / Shift+Tab / Escape retained the edit and original file",
            "save and original replacement confirmed in the prompt", "DOCX XML and fresh process agree"]


def verify_sheet(binary, out):
    source = out / "工作簿 副本.xlsx"
    number = port()
    profile = out / "profile"
    ctrl = SheetControl(number, profile)
    call = ctrl.call
    with app(binary, ["--sample", "sales", "--control", str(number), "--diagnostics"], out,
             {"JOYSHEET_CONFIG_DIR": str(profile)}, unset=("JOYSHEET_CONTROL_TOKEN", "GRIDCRAFT_CONTROL_TOKEN")) as process:
        def view(predicate, label):
            return ctrl.wait(predicate, label, process)

        def command(name, **params):
            call("engine.execute", command=name, params=params)
            call("ui.awaitIdle")

        def path_open(v):
            return any(w["window"] == v["window"] and w["path"] == str(source) for w in v["windows"])

        def cell_value():
            return call("cell.get", cell="A1").get("value")

        def prompt():
            call("ui.command", command="file.close")
            return view(lambda v: v["dialog"] == "saveChanges" and v["closeButtons"].get("save", {}).get("width", 0) > 0,
                        "rendered save prompt")

        def click(name):
            state = call("ui.inspect")
            assert state["dialog"] == "saveChanges" and state["window"] == target
            b = state["closeButtons"][name]
            call("ui.click", x=b["x"] + b["width"] / 2, y=b["y"] + b["height"] / 2)

        other = view(lambda v: v["window"] is not None, "initial Sheet window")["window"]
        command("file.new")
        view(lambda v: v["window"] != other, "separate workbook")
        command("cell.set", cell="A1", input="原始")
        call("app.save", path=str(source))
        target = view(path_open, "saved workbook")["window"]
        before = digest(source)
        command("cell.set", cell="A1", input="保留的修改")
        prompt()
        call("ui.key", key="tab")
        call("ui.key", key="tab", shift=True)
        call("ui.key", key="escape")
        view(lambda v: not v["dialog"] and v["window"] == target, "Escape")
        prompt()
        click("cancel")
        view(lambda v: not v["dialog"], "Cancel button")
        assert cell_value() == "保留的修改" and digest(source) == before
        prompt()
        # Occupy only our private write staging path to exercise a real I/O failure.
        staging = source.with_name(source.name + ".sctmp")
        with staging.open("xb") as f:
            f.write(b"test owner")
        click("save")
        view(lambda v: v["error"] and v["dialog"] == "saveChanges" and not v["closeSaving"], "failed save retained prompt")
        assert staging.read_bytes() == b"test owner" and digest(source) == before
        staging.unlink()
        click("save")
        view(lambda v: v["window"] == other and all(w["window"] != target for w in v["windows"]), "only target closed")
        call("app.open", path=str(source))
        target = view(path_open, "reopened workbook")["window"]
        assert cell_value() == "保留的修改"
        saved_hash = digest(source)
        command("cell.set", cell="A1", input="放弃的修改")
        prompt()
        click("discard")
        view(lambda v: v["window"] == other and all(w["window"] != target for w in v["windows"]), "Discard button")
        assert digest(source) == saved_hash
    return ["Escape and Cancel preserved workbook", "failed write retained draft, target and private staging file",
            "retry closed only the intended window and XLSX reopened with edits", "Discard preserved the saved workbook"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", choices=("word", "sheet"))
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    binary, out = args.binary.resolve(), args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    report = {"product": args.product, "platform": platform.platform(), "sha256": digest(binary), "passed": False}
    try:
        report["checks"] = (verify_word if args.product == "word" else verify_sheet)(binary, out)
        report["passed"] = True
    finally:
        (out / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
