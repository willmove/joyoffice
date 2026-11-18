"""Exercise the three editors with real X11 key events and the selected Fcitx5 Chinese input method."""
import argparse
import json
import os
from pathlib import Path
import secrets
import select
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET
import zipfile

from native_pilot_support import ROOT, SheetControl, app, digest, port, wait, word_request
from probe_native_ime import run, x11_selection_owner


class Input:
    def __init__(self, p):
        self.p = p
        self.win = wait(lambda: run("xdotool", "search", "--onlyvisible", "--pid", str(p.pid)).splitlines()[0], "X11 window", p)
        run("xdotool", "windowactivate", "--sync", self.win)
        geometry = run("xdotool", "getwindowgeometry", "--shell", self.win)
        self.geom = dict(line.split("=", 1) for line in geometry.splitlines())

    def key(self, *keys):
        if run("xdotool", "getactivewindow") != self.win:
            raise RuntimeError("test editor lost X11 focus")
        run("xdotool", "key", "--clearmodifiers", "--delay", "90", *keys)

    def activate(self, method):
        # A fresh XIM client associates its focused window with the input context on the
        # first event. Switch the engine only after that event reaches this client.
        self.key("Left")
        time.sleep(.3)
        run("fcitx5-remote", "-s", method)
        run("fcitx5-remote", "-o")
        wait(lambda: run("fcitx5-remote", "-n") == method, "active input method", self.p)

    def compose(self, out, committed, marked=None):
        self.key(*"zhongwen")
        time.sleep(.45)
        if marked is not None:
            wait(marked, "product marked range", self.p)
        g = self.geom
        crop = f'{g["WIDTH"]}x{g["HEIGHT"]}{int(g["X"]):+d}{int(g["Y"]):+d}'
        run("import", "-window", "root", "-crop", crop, str(out / "candidate.png"))
        self.key("space")
        wait(committed, "selected 中文", self.p)
        self.key(*"zho")
        self.key("Escape")
        wait(committed, "cancel retained the first selection", self.p)


def word(binary, out, method):
    source = out/"验证.docx"
    shutil.copyfile(ROOT/"assets/samples/letter.docx", source)
    number, secret = port(), secrets.token_hex(32)
    env = {"JOYWORD_CONFIG_DIR":str(out/"profile"),"JOYWORD_CONTROL_KEY":secret}
    args = ["--control",str(number),"--",str(source)]
    inspect = lambda: word_request(number, "document.inspect", key=secret).get("result")
    with app(binary,args,out,env) as p:
        start = wait(inspect,"loaded Word",p)
        key = Input(p)
        key.activate(method)
        command = {"command":"caret.docEnd","doc":start["doc"],"version":start["version"],"params":{}}
        assert word_request(number,"engine.execute",command,secret)["ok"]
        initial, expected = start["text"], start["text"]+"中文"
        key.compose(out, lambda: inspect()["text"] == expected)
        key.key("ctrl+z")
        wait(lambda: inspect()["text"] == initial,"undo",p)
        key.key("ctrl+y")
        wait(lambda: inspect()["text"] == expected,"redo",p)
        key.key("ctrl+n")
        time.sleep(.2)
        key.key("Escape")
        wait(lambda: inspect()["text"] == expected, "focus returned to the edited document", p)
        key.key("ctrl+n")
        time.sleep(.25)
        key.key("Return")
        time.sleep(.25)
        key.key("Return")
        def saved():
            with zipfile.ZipFile(source) as z:
                return "中文" in "".join(ET.fromstring(z.read("word/document.xml")).itertext())
        wait(saved,"DOCX XML readback",p)
    with app(binary,args,out,env,"reopen.log") as p:
        wait(lambda: (inspect() or {}).get("text") == expected,"reopened Word",p)
    return source


def deck(binary, out, method):
    source, target = out/"source.pptx", out/"验证.pptx"
    shutil.copyfile(ROOT/"tests/fixtures/joydeck/r1-preview.pptx",source)
    env = {"JOYDECK_CONFIG_DIR":str(out/"profile"),"XDG_STATE_HOME":str(out/"state")}
    count = 0
    def call(p, command, params=None, guarded=True):
        nonlocal count
        status = call(p, "document.status", guarded=False) if guarded else None
        count += 1
        msg = {"protocolVersion":"joydeck-control-1","requestId":str(count),"method":command,"params":params or {}}
        if status:
            msg.update(target=status["documentId"], expectedRevision=status["revision"])
        p.stdin.write(json.dumps(msg)+"\n")
        p.stdin.flush()
        if not select.select([p.stdout],[],[],5)[0]:
            raise TimeoutError(command)
        result = json.loads(p.stdout.readline())
        if "error" in result:
            raise ValueError(str(result["error"]))
        return result["result"]
    notes = lambda p: call(p,"document.inspect")["slides"][0]["notes"]
    with app(binary,[str(source),"--control-stdio="+str(out)],out,env) as p:
        initial = wait(lambda: notes(p),"Deck notes",p)
        key = Input(p)
        x, y = int(key.geom["WIDTH"])//2, int(key.geom["HEIGHT"])-51
        run("xdotool","mousemove","--window",key.win,str(x),str(y),"click","1")
        key.activate(method)
        key.key("ctrl+a")
        key.compose(out, lambda: notes(p) == "中文")
        key.key("Tab")
        key.key("ctrl+z")
        wait(lambda: notes(p) == initial,"undo notes",p)
        key.key("ctrl+y")
        wait(lambda: notes(p) == "中文","redo notes",p)
        call(p,"document.save",{"path":str(target)})
    with zipfile.ZipFile(target) as z:
        assert "中文" in list(ET.fromstring(z.read("ppt/notesSlides/notesSlide1.xml")).itertext())
    with app(binary,[str(target),"--control-stdio="+str(out)],out,env,"reopen.log") as p:
        wait(lambda: notes(p) == "中文","reopened notes",p)
    return target


def sheet(binary, out, method, server_preedit=False):
    target, cfg, number = out/"验证.xlsx", out/"profile", port()
    ctrl = SheetControl(number,cfg)
    args, env = ["--control",str(number),"--diagnostics"], {"JOYSHEET_CONFIG_DIR":str(cfg)}
    token_vars = ("JOYSHEET_CONTROL_TOKEN","GRIDCRAFT_CONTROL_TOKEN")
    with app(binary,args,out,env,unset=token_vars) as p:
        ctrl.wait(lambda v:v["perf"]["frame"]>1,"loaded Sheet",p)
        key = Input(p)
        ctrl.call("ui.cell.click",cell="A1")
        key.activate(method)
        def selected():
            v=ctrl.call("ui.inspect")
            return not v["composing"] and (v["editor"] or {}).get("text") == "中文"
        try:
            def pending():
                state = ctrl.call("ui.inspect")
                if server_preedit:
                    # With server-side preedit the app receives only the final commit.
                    return not state["composing"] and not (state["editor"] or {}).get("text")
                return state["composing"]
            key.compose(out, selected, pending)
        finally:
            state = ctrl.call("ui.inspect")
            (out/"cell-input.json").write_text(json.dumps({k:state.get(k) for k in ("editor","composing","inputGeometry","selection","focus","keyTrace")},ensure_ascii=False,indent=2)+"\n")
        key.key("Tab")
        value=lambda:ctrl.call("cell.get",cell="A1").get("value")
        wait(lambda: value()=="中文","committed cell",p)
        key.key("ctrl+z")
        wait(lambda: value() in (None,""),"undo cell",p)
        key.key("ctrl+y")
        wait(lambda: value()=="中文","redo cell",p)
        ctrl.call("app.save",path=str(target))
    with zipfile.ZipFile(target) as z:
        assert any("中文" in z.read(n).decode() for n in z.namelist() if n.startswith("xl/") and n.endswith(".xml"))
    with app(binary,[str(target),*args],out,env,"reopen.log",token_vars) as p:
        ctrl.wait(lambda v:v["perf"]["frame"]>1,"reopened Sheet",p)
        assert ctrl.call("cell.get",cell="A1")["value"]=="中文"
    return target


if __name__ == "__main__":
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument("product",choices=("word","deck","sheet"))
    ap.add_argument("binary",type=Path)
    ap.add_argument("output",type=Path)
    ap.add_argument("--method", default="pinyin")
    ap.add_argument("--server-preedit", action="store_true", help="The XIM server displays preedit and sends only committed text")
    opt=ap.parse_args()
    out,binary=opt.output.resolve(),opt.binary.resolve()
    out.mkdir(parents=True,exist_ok=False)
    initial=run("fcitx5-remote","-n")
    active=run("fcitx5-remote")
    report={"passed":False,"method":opt.method,"display":os.environ["DISPLAY"],"binarySha256":digest(binary),
            "ximSelectionOwner":x11_selection_owner("@server="+os.environ["XMODIFIERS"].removeprefix("@im="))}
    try:
        if not report["ximSelectionOwner"]:
            raise RuntimeError("the X11 input method has no owner on this display")
        report["preedit"] = "server" if opt.server_preedit else "client"
        if opt.product == "sheet":
            target = sheet(binary, out, opt.method, opt.server_preedit)
        else:
            target = {"word":word,"deck":deck}[opt.product](binary,out,opt.method)
        report.update(passed=True,fileSha256=digest(target),candidateSha256=digest(out/"candidate.png"))
    except Exception as error:
        report["error"]=str(error)
        raise
    finally:
        if initial:
            run("fcitx5-remote","-s",initial)
        if active in ("1","2"):
            run("fcitx5-remote","-o" if active=="2" else "-c")
        (out/"report.json").write_text(json.dumps(report,indent=2)+"\n")
