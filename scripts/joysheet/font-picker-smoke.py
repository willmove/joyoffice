"""Editable Ribbon font regression, with isolated profile, binaries and evidence.

Covers editable fonts and scrollbar dragging through production GPUI input. This supplements native Windows mouse
checks; it is not a claim of complete Excel feature equivalence.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--binary-directory", type=Path, default=Path("target/debug"))
    parser.add_argument("--keep-open", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binaries = output / "bin"
    binaries.mkdir()
    for name in ["joysheet.exe", "joysheet-cli.exe"]:
        shutil.copy2(args.binary_directory / name, binaries / name)
    env = os.environ.copy()
    env["JOYSHEET_CONFIG_DIR"] = str(output / "profile")
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    log = (output / "stderr.log").open("w", encoding="utf-8")
    app = subprocess.Popen([str(binaries / "joysheet.exe"), "--control", str(port)],
                           env=env, cwd=binaries, stdout=subprocess.DEVNULL, stderr=log,
                           creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    checks = []
    (output / "session.json").write_text(json.dumps({"pid": app.pid, "port": port}), encoding="utf-8")

    connection = None
    channel = None

    def send(method, params=None):
        nonlocal connection, channel
        if channel is None:
            token = (output / "profile" / f"control-{port}.token").read_text(encoding="utf-8").strip()
            connection = socket.create_connection(("127.0.0.1", port), timeout=30)
            channel = connection.makefile("rwb")
            channel.write((json.dumps({"id": 0, "method": "auth", "params": {"token": token}}) + "\n").encode("utf-8"))
            channel.flush()
            assert json.loads(channel.readline())["ok"], "control authentication"
        channel.write((json.dumps({"id": 1, "method": method, "params": params or {}}, ensure_ascii=False) + "\n").encode("utf-8"))
        channel.flush()
        result = json.loads(channel.readline())
        if not result["ok"]:
            raise RuntimeError(f"{method}: {result.get('error')}")
        return result["result"]

    def wait(predicate, label):
        deadline = time.monotonic() + 20
        last = None
        while time.monotonic() < deadline:
            if app.poll() is not None:
                raise RuntimeError("JoySheet exited")
            try:
                last = send("ui.inspect")
                if predicate(last):
                    return last
            except (RuntimeError, OSError):
                pass
            time.sleep(0.06)
        raise AssertionError(f"Timed out: {label}; see failure-state.json")

    def click(key):
        view = wait(lambda v: key in v["advancedBounds"], key)
        b = view["advancedBounds"][key]
        send("ui.click", {"x": b["x"] + b["width"] / 2, "y": b["y"] + b["height"] / 2})
        time.sleep(0.06)

    def shot(name):
        send("ui.screenshot", {"path": str(output / f"{name}.png")})

    def execute(command, params=None):
        result=send("engine.execute", {"command": command, "params": params or {}})
        send("ui.awaitIdle")
        return result

    def revision():
        return send("ui.inspect")["windows"][0]["revision"]

    def type_font(name):
        click("font-input")
        wait(lambda v:v["fontPicker"]["focused"],"font name focus")
        send("ui.key",{"key":"a","ctrl":True})
        send("ui.text",{"text":name})
        wait(lambda v:v["fontPicker"]["text"]==name,"font text")

    def assert_font(name):
        wait(lambda v:not v["fontPicker"]["open"] and not v["fontPicker"]["focused"],"font committed")
        send("ui.awaitIdle")
        for cell in ["A1","B2"]:
            assert send("cell.get",{"cell":cell})["style"]["font"]["name"]==name
        assert send("cell.get",{"cell":"A1"})["value"]==42
        assert send("cell.get",{"cell":"C1"})["style"]["font"]["name"]!="Portable Font Test"

    try:
        wait(lambda v:v["grid"]["height"]>100,"first frame")
        send("ui.resize",{"width":1400,"height":900})
        execute("range.setValues",{"range":"A1","values":[[42,10],[20,30]]})
        execute("home.bold",{"range":"A1"})
        execute("selection.set",{"range":"A1:B2"})
        before=revision()
        type_font(" arial ")
        shot("typed-font")
        assert revision()==before and send("ui.inspect")["editor"] is None
        send("ui.key",{"key":"enter"})
        assert_font("Arial")
        assert send("cell.get",{"cell":"A1"})["style"]["font"]["bold"]
        assert not send("cell.get",{"cell":"B2"})["style"]["font"]["bold"]
        assert send("ui.inspect")["settings"]["recent_fonts"][0]=="Arial"
        send("ui.key",{"key":"z","ctrl":True})
        send("ui.awaitIdle")
        assert send("cell.get",{"cell":"A1"})["style"]["font"]["name"]!="Arial"
        send("ui.key",{"key":"y","ctrl":True})
        assert_font("Arial")
        checks.append("Direct typed font normalizes installed case/whitespace, applies only selected cells, preserves values/mixed styles, undo/redo")
        before=revision()
        type_font("Cancel Font")
        send("ui.key",{"key":"escape"})
        assert_font("Arial")
        assert revision()==before
        type_font("等线")
        send("ui.key",{"key":"enter"})
        assert_font("等线")
        checks.append("Escape cancels without revision change; Chinese font names can be entered directly")
        execute("pageLayout.themeFonts",{"major":"Georgia","minor":"Arial"})
        click("ribbon:font-name")
        state=wait(lambda v:v["fontPicker"]["open"],"font dropdown")
        rows=state["fontPicker"]["rows"]
        assert rows[0]["group"]=="theme"
        assert any(r["name"]=="Georgia" and r["group"]=="theme" for r in rows)
        assert any(r["name"]=="等线" and r["group"]=="recent" for r in rows)
        assert next(i for i,r in enumerate(rows) if r["group"]=="recent")<next(i for i,r in enumerate(rows) if r["group"]=="all")
        assert state["fontPicker"]["maxScrollY"]>1000
        shot("pinned-fonts-scrollbar")
        b=state["advancedBounds"]["font-list-viewport"]
        send("ui.drag",{"x":b["x"]+b["width"]-5,"y":b["y"]+10,"toX":b["x"]+b["width"]-5,"toY":b["y"]+b["height"]-20})
        state=wait(lambda v:v["fontPicker"]["scrollY"] < -500,"drag scrollbar thumb")
        shot("dragged-fonts")
        candidates=[r["name"] for r in state["fontPicker"]["rows"] if r["name"] and r["group"]=="all"]
        visible=[]
        for name in candidates:
            rect=state["advancedBounds"].get("font-item:all:"+name)
            if rect and rect["y"]>=b["y"] and rect["y"]+rect["height"]<=b["y"]+b["height"]:
                visible.append(name)
        assert visible and candidates.index(visible[-1])>20
        chosen=visible[-1]
        click("font-item:all:"+chosen)
        assert_font(chosen)
        assert send("ui.inspect")["settings"]["recent_fonts"][0]==chosen
        checks.append("Workbook theme and recent groups pinned above full catalog; visible scrollbar thumb drag reveals and selects later fonts")
        click("ribbon:font-name")
        wait(lambda v:v["fontPicker"]["open"],"reopen list")
        type_font("arial")
        state=wait(lambda v:v["fontPicker"]["filter"]=="arial","filter fonts")
        assert all("arial" in r["name"].lower() for r in state["fontPicker"]["rows"] if r["name"])
        shot("filtered-fonts")
        send("ui.key",{"key":"escape"})
        assert_font(chosen)
        click("ribbon:font-name")
        wait(lambda v:v["fontPicker"]["open"],"reopen pinned list")
        click("font-item:theme:Georgia")
        assert_font("Georgia")
        checks.append("Typing in an open list filters choices; Escape cancels; mouse theme-font selection applies actual font")
        type_font("Portable Font Test")
        send("ui.key",{"key":"enter"})
        assert_font("Portable Font Test")
        expected=send("ui.inspect")["settings"]["recent_fonts"]
        execute("review.protectSheet",{"formatCells":False})
        type_font("Arial")
        send("ui.key",{"key":"enter"})
        send("ui.awaitIdle")
        assert send("ui.inspect")["error"]
        assert send("ui.inspect")["settings"]["recent_fonts"]==expected
        execute("review.unprotectSheet")
        checks.append("Portable names retained; failed protected-sheet formatting does not pollute recent fonts")
        send("app.save",{"path":str(output/"result.xlsx")})
        send("ui.awaitIdle")
        # Restart only the isolated test process with the same preferences directory.
        channel.close();connection.close();channel=None;connection=None
        app.terminate();app.wait(timeout=20)
        with socket.socket() as probe:
            probe.bind(("127.0.0.1",0));port=probe.getsockname()[1]
        app=subprocess.Popen([str(binaries/"joysheet.exe"),"--control",str(port)],env=env,cwd=binaries,stdout=subprocess.DEVNULL,stderr=log,creationflags=getattr(subprocess,"CREATE_NO_WINDOW",0))
        (output/"session.json").write_text(json.dumps({"pid":app.pid,"port":port}),encoding="utf-8")
        wait(lambda v:v["grid"]["height"]>100,"restart")
        assert send("ui.inspect")["settings"]["recent_fonts"]==expected
        click("ribbon:font-name")
        wait(lambda v:v["fontPicker"]["open"],"persisted recent list")
        shot("persisted-recents")
        assert any(r["name"]=="Portable Font Test" and r["group"]=="recent" for r in send("ui.inspect")["fontPicker"]["rows"])
        send("ui.key",{"key":"escape"})
        checks.append("Recent font history persists across application restart")
        report={"passed":len(checks),"checks":checks,"binarySha256":hashlib.sha256((binaries/"joysheet.exe").read_bytes()).hexdigest(),"dragSelectedFont":chosen,"recentAfterRestart":expected}
        (output/"report.json").write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding="utf-8")
        print(json.dumps(report,ensure_ascii=False))
    except BaseException:
        try:
            shot("failure")
            (output/"failure-state.json").write_text(json.dumps(send("ui.inspect"),ensure_ascii=False,indent=2),encoding="utf-8")
        except Exception:pass
        raise
    finally:
        if channel is not None:channel.close()
        if connection is not None:connection.close()
        if not args.keep_open:
            app.terminate();app.wait(timeout=20)
        log.close()


if __name__=="__main__":main()
