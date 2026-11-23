"""Drive native Save As panels on scratch workbooks and presentations, including late returns."""
import argparse
import json
from pathlib import Path
import posixpath
import platform
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET
import zipfile

from native_pilot_support import ROOT, DeckControl, SheetControl, app, digest, port, wait
from probe_native_ime import mac_keys, mac_target, run, system_events


class Panel:
    def __init__(self, process, x11_save_buttons=None):
        self.process = process
        self.x11_save_buttons = x11_save_buttons
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
            if self.x11_save_buttons:
                file_x, file_y, save_x, save_y = self.x11_save_buttons
                run("xdotool", "mousemove", "--window", self.window, str(file_x), str(file_y), "click", "1")
                time.sleep(.3)
                if run("xdotool", "getactivewindow") != self.window:
                    raise RuntimeError("test application lost focus before Save As")
                run("xdotool", "mousemove", "--window", self.window, str(save_x), str(save_y), "click", "1")
            else:
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
            # xdotool types literal filenames; DDE can otherwise interpret their punctuation as
            # part of a Chinese composition. Restore the active input method after acceptance.
            fcitx = shutil.which("fcitx5-remote")
            active = fcitx and subprocess.run([fcitx], capture_output=True, text=True).stdout.strip() == "2"
            try:
                if active:
                    subprocess.run([fcitx, "-c"], check=True)
                time.sleep(.4)
                self.xevent("key", "--clearmodifiers", "Home", "shift+End")
                for char in path.name:
                    self.xevent("type", "--clearmodifiers", char)
                    if ord(char) > 127:
                        time.sleep(.1)
                self.xevent("key", "--clearmodifiers", "Return")
                wait(lambda: not self.visible(), "panel acceptance", self.process)
            finally:
                if active:
                    subprocess.run([fcitx, "-o"], check=True)
            return
        wait(lambda: not self.visible(), "panel acceptance", self.process)


def sheet(binary, out, x11_save_buttons=None):
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
        panel = Panel(p, x11_save_buttons)
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


def deck(binary, out, other_view=False, x11_buttons=None, x11_save_buttons=None):
    original, target = out / "原件.pptx", out / "演示 副本.pptx"
    if other_view:
        target = out / "four-slides.pptx"
    shutil.copyfile(ROOT / "tests/fixtures/joydeck/independent.pptx", original)
    initial_hash = digest(original)
    env = {"XDG_CONFIG_HOME":str(out / "profile"), "XDG_STATE_HOME":str(out / "state")}
    args = ["--control-stdio=" + str(out)]

    with app(binary, [str(original), *args], out, env) as p:
        client = DeckControl(p)
        slides = wait(client.inspect, "presentation", p)["slides"]
        panel = Panel(p, x11_save_buttons)
        if other_view:
            if x11_buttons:
                for x, y in zip(x11_buttons[::2], x11_buttons[1::2]):
                    run("xdotool","windowactivate","--sync",panel.window)
                    run("xdotool","mousemove","--window",panel.window,str(x),str(y),"click","1")
                    time.sleep(.45)
            else:
                from verify_shared_ribbon import NativeWindow
                surface = NativeWindow(p, out)
                surface.press(wait(lambda: surface.find({"视图"}, "button") or surface.find({"视图"}), "View tab", p))
                surface.press(wait(lambda: surface.find({"shared-view","新建文稿视图"}, "button"), "New document view", p))
            if panel.mac:
                wait(lambda: int(system_events(f"return count of windows of {panel.owner}")) == 2, "two document views", p)
            else:
                first = panel.window
                panel.window = wait(lambda: next((w for w in run("xdotool", "search", "--onlyvisible", "--pid", str(p.pid)).splitlines() if w != first), None), "second view", p)
        panel.open()
        panel.cancel()
        panel.open()
        client.edit({"command":"page.add"})
        assert len(client.inspect()["slides"]) == len(slides) + 1
        stale = out / ("stale-copy.pptx" if other_view else "过时 副本.pptx")
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
    checks = ["cancel then retry", "late result after edit rejected before file creation", "four slides in independent PPTX inspection and fresh process readback", "original hash unchanged"]
    if other_view:
        checks.append("second document view owned the picker while the first edited the shared document")
    return checks


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", choices=("sheet", "deck"))
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--other-view", action="store_true", help="open the native Deck picker in a second view of the same document")
    parser.add_argument("--x11-save-buttons", type=int, nargs=4, metavar=("FILE_X", "FILE_Y", "SAVE_X", "SAVE_Y"), help="Window-relative File / Save As centers verified on this X11 desktop; avoids desktop shortcut interception")
    parser.add_argument("--x11-view-buttons", type=int, nargs=4, metavar=("VIEW_X","VIEW_Y","NEW_X","NEW_Y"), help="window-relative centers from the native accessibility tree on the same X11 desktop")
    args = parser.parse_args()
    if args.x11_save_buttons and platform.system() != "Linux":
        parser.error("--x11-save-buttons requires Linux X11")
    if args.x11_view_buttons and (platform.system() != "Linux" or not args.other_view):
        parser.error("--x11-view-buttons requires an X11 --other-view check")
    if args.product != "deck" and args.other_view:
        parser.error("--other-view requires deck")
    out, binary = args.output.resolve(), args.binary.resolve()
    out.mkdir(parents=True, exist_ok=False)
    result = {"platform":platform.platform(),"product":args.product,"binarySha256":digest(binary),"passed":False}
    try:
        result["checks"] = sheet(binary,out,args.x11_save_buttons) if args.product == "sheet" else deck(binary,out,args.other_view,args.x11_view_buttons,args.x11_save_buttons)
        result["activation"] = "file-menu" if args.x11_save_buttons else "keyboard"
        if args.x11_save_buttons:
            result["x11SaveButtons"] = args.x11_save_buttons
        result["passed"] = True
    finally:
        (out / "report.json").write_text(json.dumps(result, indent=2) + "\n")
