"""Drive native Save As panels on scratch workbooks and presentations, including late returns."""
import argparse
import json
from pathlib import Path
import posixpath
import platform
import shutil
import time
import xml.etree.ElementTree as ET
import zipfile

from native_pilot_support import ROOT, DeckControl, SheetControl, app, digest, port, wait
from probe_native_ime import mac_keys, mac_target, run, system_events


class Panel:
    def __init__(self, process):
        self.process = process
        self.mac = platform.system() == "Darwin"
        if self.mac:
            self.owner = mac_target(process.pid)
            self.focus = f'(value of attribute "AXFocusedUIElement" of {self.owner})'
        else:
            self.window = wait(lambda: run("xdotool", "search", "--onlyvisible", "--pid", str(process.pid)).splitlines()[0],
                               "application window", process)

    def visible(self):
        if self.mac:
            return system_events(f'return role of {self.focus}') in ("AXTextField", "AXComboBox")
        window = run("xdotool", "getactivewindow")
        if window == self.window:
            return None
        name = run("xdotool", "getwindowname", window).lower()
        owner = run("xdotool", "getwindowpid", window)
        executable = Path(f"/proc/{owner}/comm").read_text().strip()
        if executable == "dde-file-dialog" or "另存" in name or "save" in name:
            return window
        return None

    def open(self):
        if self.mac:
            system_events(f"set frontmost of {self.owner} to true")
            mac_keys(self.process.pid, ["1 using {command down, shift down}"])
        else:
            run("xdotool", "windowactivate", "--sync", self.window)
            run("xdotool", "key", "--clearmodifiers", "ctrl+shift+s")
        self.dialog = wait(self.visible, "native save panel", self.process)

    def xevent(self, *args):
        if self.visible() != self.dialog:
            raise RuntimeError("save panel lost focus")
        run("xdotool", *args)

    def cancel(self):
        if self.mac:
            mac_keys(self.process.pid, ["53"])
        else:
            self.xevent("key", "Escape")
        wait(lambda: not self.visible(), "panel dismissal", self.process)

    def submit(self, path):
        if self.mac:
            system_events(f'set value of attribute "AXValue" of {self.focus} to {json.dumps(path.name, ensure_ascii=False)}')
            mac_keys(self.process.pid, ["36"])
        else:
            time.sleep(.4)
            self.xevent("key", "--clearmodifiers", "Home", "shift+End")
            for char in path.name:
                self.xevent("type", "--clearmodifiers", char)
                if ord(char) > 127:
                    time.sleep(.1)
            self.xevent("key", "--clearmodifiers", "Return")
        wait(lambda: not self.visible(), "panel acceptance", self.process)


def sheet(binary, out):
    cfg, number = out / "profile", port()
    client = SheetControl(number, cfg)
    call = client.call
    files = [out / name for name in ("原件.xlsx", "过时 副本.xlsx", "有效 副本.xlsx")]
    original, stale, saved = files
    with app(binary, ["--sample", "sales", "--control", str(number), "--diagnostics"], out,
             {"JOYSHEET_CONFIG_DIR": str(cfg)}, unset=("JOYSHEET_CONTROL_TOKEN", "GRIDCRAFT_CONTROL_TOKEN")) as p:
        client.wait(lambda v: v["window"] is not None, "workbook", p)
        call("app.save", path=str(original))
        initial_hash = digest(original)
        change = lambda text: call("engine.execute", command="cell.set", params={"cell":"A1","input":text})
        change("第一版")
        call("ui.awaitIdle")
        panel = Panel(p)
        panel.open()
        panel.cancel()
        panel.open()
        change("另存期间修改")
        panel.submit(stale)
        client.wait(lambda v: v["error"], "writer rejected late result", p)
        assert not stale.exists() and call("cell.get", cell="A1")["value"] == "另存期间修改"
        call("ui.message.dismiss")
        panel.open()
        panel.submit(saved)
        client.wait(lambda v: any(w["path"] == str(saved) for w in v["windows"]), "saved copy", p)
    with zipfile.ZipFile(saved) as z:
        assert any("另存期间修改" in z.read(n).decode("utf-8") for n in z.namelist() if n.startswith("xl/") and n.endswith(".xml"))
    with app(binary, [str(saved), "--control", str(number), "--diagnostics"], out, {"JOYSHEET_CONFIG_DIR": str(cfg)},
             "reopen.log", unset=("JOYSHEET_CONTROL_TOKEN", "GRIDCRAFT_CONTROL_TOKEN")) as p:
        client.wait(lambda v: any(w["path"] == str(saved) for w in v["windows"]), "fresh process", p)
        assert call("cell.get", cell="A1")["value"] == "另存期间修改"
    assert digest(original) == initial_hash
    return ["cancel then retry", "late result after edit rejected before file creation", "independent XLSX and fresh process readback", "original hash unchanged"]


def deck(binary, out):
    original, target = out / "原件.pptx", out / "演示 副本.pptx"
    shutil.copyfile(ROOT / "tests/fixtures/joydeck/independent.pptx", original)
    initial_hash = digest(original)
    env = {"JOYDECK_CONFIG_DIR":str(out / "profile"), "JOYDECK_RECOVERY_DIR":str(out / "recovery")}
    args = ["--control-stdio=" + str(out)]

    with app(binary, [str(original), *args], out, env) as p:
        client = DeckControl(p)
        slides = wait(client.inspect, "presentation", p)["slides"]
        panel = Panel(p)
        panel.open()
        panel.cancel()
        panel.open()
        client.edit({"command":"page.add"})
        assert len(client.inspect()["slides"]) == len(slides) + 1
        stale = out / "过时 副本.pptx"
        panel.submit(stale)
        panel.open()
        assert not stale.exists()
        panel.submit(target)
        wait(target.is_file, "PowerPoint copy", p)
    with zipfile.ZipFile(target) as z:
        ids = ET.fromstring(z.read("ppt/presentation.xml")).findall(".//{*}sldId")
        targets = {rel.get("Id"):rel.get("Target") for rel in ET.fromstring(z.read("ppt/_rels/presentation.xml.rels"))}
        slide_rel = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id"
        paths = {posixpath.normpath(posixpath.join("ppt", targets[item.attrib[slide_rel]])).lstrip("/") for item in ids}
        assert len(paths) == 4 and all(ET.fromstring(z.read(path)).tag.endswith("}sld") for path in paths)
    with app(binary, [str(target), *args], out, env, "reopen.log") as p:
        opened = wait(DeckControl(p).inspect, "fresh presentation", p)["slides"]
        assert len(slides) == 3 and len(opened) == 4 and opened[0]["notes"] == slides[0]["notes"]
    assert not stale.exists()
    assert digest(original) == initial_hash
    return ["cancel then retry", "late result after edit rejected before file creation", "four slides in independent PPTX inspection and fresh process readback", "original hash unchanged"]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", choices=("sheet", "deck"))
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    out, binary = args.output.resolve(), args.binary.resolve()
    out.mkdir(parents=True, exist_ok=False)
    result = {"platform":platform.platform(),"product":args.product,"binarySha256":digest(binary),"passed":False}
    try:
        result["checks"] = (sheet if args.product == "sheet" else deck)(binary, out)
        result["passed"] = True
    finally:
        (out / "report.json").write_text(json.dumps(result, indent=2) + "\n")
