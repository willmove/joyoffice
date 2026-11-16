"""Ribbon dialog pointer regression, with isolated profile, binaries and evidence.

Covers advanced galleries and wizards through the production GPUI pointer/key path. This supplements native Windows mouse
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
    parser.add_argument("--focused-only", action="store_true", help="Run dialog state regressions without repeating the full control inventory")
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
    checks, inventory = [], []
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

    def tab(name):
        click("ribbon-tab:" + name)
        return wait(lambda v: v["ribbon"]["tab"] == name, name)

    def execute(command, params=None):
        result=send("engine.execute", {"command": command, "params": params or {}})
        send("ui.awaitIdle")
        return result

    def set_field(field, value):
        send("ui.dialog.set", {"field": field, "value": value})

    def dialog(name):
        send("ui.dialog", {"name": name})
        return wait(lambda v: v.get("commandDialog", {}).get("command") == name, name)

    def choose(key,value):
        click("choice:"+key)
        wait(lambda v:v["excelControls"]["popupOpen"],"choice popup open")
        click("choice:"+key+":"+value)
        wait(lambda v:not v["excelControls"]["popupOpen"],"choice popup close")

    def revision():
        return send("ui.inspect")["windows"][0]["revision"]

    try:
        wait(lambda v:v["grid"]["height"]>100,"first frame")
        send("ui.resize",{"width":1700,"height":1100})
        execute("range.setValues",{"range":"A1","values":[["Region","Amount","Code"],["West",20,"001"],["East",10,"002"],["West",5,"003"]]})
        execute("selection.set",{"range":"A1:C4"})
        tab("Home")
        before=revision()
        click("ribbon:font-name")
        wait(lambda v:any(k.startswith("font-item:") for k in v["advancedBounds"]),"installed font list")
        shot("font-dropdown")
        send("ui.key",{"key":"escape"})
        assert revision()==before
        click("ribbon-menu:fill")
        shot("theme-palette")
        click("ribbon-theme:2:4")
        send("ui.awaitIdle")
        assert revision()>before
        execute("edit.undo")
        dialog("home.fillColor")
        click("dialog-tab:1")
        for k,n in [("red",18),("green",52),("blue",86)]:set_field(k,n)
        shot("rgb-colors")
        click("dialog-ok")
        send("ui.awaitIdle")
        checks.append("Installed font popup; theme swatch; custom RGB apply and undo")
        dialog("home.formatCells")
        click("dialog-tab:2")
        assert send("ui.inspect")["excelControls"]["fontCount"]>10
        shot("format-font-preview")
        click("dialog-tab:3")
        before=revision()
        click("border-preset:outside")
        click("border-preview:borderDown")
        shot("border-preview")
        click("dialog-cancel")
        assert revision()==before
        dialog("home.formatCells")
        click("dialog-tab:3")
        click("border-preset:outside")
        click("dialog-ok")
        send("ui.awaitIdle")
        assert revision()>before
        execute("edit.undo")
        checks.append("Font preview, graphical border click/preset; Cancel unchanged; grouped format apply/undo")
        click("ribbon:format-as-table")
        shot("table-gallery")
        click("table-style:TableStyleMedium4")
        assert send("ui.inspect")["commandDialog"]["fields"]["style"]=="TableStyleMedium4"
        click("dialog-ok")
        wait(lambda v:v["worksheet"]["tables"]==1,"table created")
        execute("edit.undo")
        checks.append("Table gallery grouped thumbnails; selected style applied after Create Table confirmation")
        for command,value in [("insert.shape","ellipse"),("insert.chart","pie"),("insert.smartArt","cycle")]:
            dialog(command)
            click("gallery:"+value)
            shot(command.replace(".","-"))
            click("dialog-cancel")
        checks.append("Shape/chart/SmartArt visual selectors and Cancel")
        dialog("data.sort")
        click("sort-add")
        assert len(send("ui.inspect")["excelControls"]["sortLevels"])==2
        choose("sort-0-column","A")
        choose("sort-1-column","B")
        choose("sort-1-order","desc")
        click("sort-copy")
        click("sort-delete")
        click("sort-up")
        click("sort-down")
        shot("sort-levels")
        before=revision()
        click("dialog-cancel")
        assert revision()==before
        dialog("data.sort")
        click("sort-add")
        choose("sort-1-column","B")
        choose("sort-1-order","desc")
        click("dialog-ok")
        send("ui.awaitIdle")
        assert execute("cell.get",{"cell":"A2"})["value"]=="East"
        assert execute("cell.get",{"cell":"B3"})["value"]==20
        execute("edit.undo")
        checks.append("Multi-level sort add/copy/delete/reorder, cancel, actual two-key order and undo")
        execute("cell.set",{"cell":"G1","input":'001,ignore,31/12/2026,"quoted,field"'})
        execute("selection.set",{"range":"G1"})
        dialog("data.textToColumns")
        shot("text-columns-step1")
        click("wizard-next")
        set_field("comma",True)
        shot("text-columns-step2")
        click("wizard-next")
        choose("columnFormat","text")
        click("split-column:1")
        choose("columnFormat","skip")
        click("split-column:2")
        choose("columnFormat","dateDMY")
        set_field("destination","I1")
        shot("text-columns-step3")
        click("wizard-back")
        click("wizard-next")
        click("dialog-ok")
        send("ui.awaitIdle")
        assert execute("cell.get",{"cell":"I1"})["value"]=="001"
        assert isinstance(execute("cell.get",{"cell":"J1"})["value"],(int,float))
        assert execute("cell.get",{"cell":"K1"})["value"]=="quoted,field"
        execute("edit.undo")
        checks.append("Three-step delimiter wizard, quotes, text leading zeros, skipped column, DMY date, back/next and undo")
        dialog("data.textToColumns")
        choose("mode","fixed")
        click("wizard-next")
        click("split-break:3")
        click("split-break:3")
        click("split-break:2")
        shot("text-columns-fixed-width")
        before=revision()
        click("dialog-cancel")
        assert revision()==before
        checks.append("Fixed-width ruler adds/removes character breaks; cancel unchanged")
        dialog("data.getData")
        set_field("text","Name,Amount\nAlice,42\nBob,7")
        before=revision()
        click("preview-import")
        wait(lambda v:(v["excelControls"]["importPreview"] or {}).get("rows")==3,"import preview")
        assert send("ui.inspect")["commandDialog"]["command"]=="data.getData"
        assert revision()==before
        shot("import-preview")
        click("dialog-cancel")
        assert revision()==before
        checks.append("CSV table preview stays in dialog and leaves workbook unchanged")
        execute("formulas.defineName",{"name":"Revenue","refersTo":"=Sheet1!$B$2:$B$4","scope":"Workbook"})
        tab("Formulas")
        click("ribbon:name-manager")
        wait(lambda v:v["panel"]==19,"names manager")
        shot("name-manager")
        click("name-edit")
        set_field("comment","Mouse regression")
        click("dialog-ok")
        wait(lambda v:v["panel"]==19,"return to manager")
        click("name-delete")
        click("name-delete-cancel")
        click("manager-close")
        checks.append("Name manager columns, edit returns to manager, delete confirmation cancel")
        execute("selection.set",{"range":"B2:B4"})
        tab("Home")
        send("ui.command",{"command":"home.manageRules"})
        wait(lambda v:v["panel"]==23,"rules manager")
        before=revision()
        click("cf-new")
        set_field("value","10")
        shot("new-formatting-rule")
        click("dialog-ok")
        assert len(send("ui.inspect")["excelControls"]["rulesDraft"])==1
        assert revision()==before
        shot("cf-manager-draft")
        click("cf-cancel")
        assert revision()==before
        send("ui.command",{"command":"home.manageRules"})
        wait(lambda v:v["panel"]==23,"rules reopen")
        click("cf-new")
        click("dialog-ok")
        click("cf-apply")
        wait(lambda v:v["worksheet"]["conditionalFormats"]==1 and v["excelControls"]["rulesTarget"] is not None,"rules applied")
        click("cf-edit")
        set_field("value","25")
        click("dialog-ok")
        wait(lambda v:v["panel"]==23 and v["commandDialog"]["command"] is None,"rule editor accepted")
        click("cf-ok")
        wait(lambda v:v["panel"]==0,"rules OK")
        execute("edit.undo")
        checks.append("Rules drafts cancel unchanged; Apply; Edit; OK; undo")
        before=revision()
        tab("File")
        # The backstage Print button goes through the same ribbon command path.
        send("ui.command",{"command":"file.print","params":{}})
        wait(lambda v:v.get("printStatus")=="preview ready","print backstage")
        shot("print-backstage")
        assert revision()==before
        choose("print-orientation","landscape")
        wait(lambda v:v.get("printStatus")=="preview ready" and v.get("printWork",{}).get("current",False),"updated print preview")
        shot("print-landscape")
        click("preview-close")
        checks.append("Print opens settings and actual PDF preview; orientation refreshes preview; close")
        send("app.save",{"path":str(output/"result.xlsx")})
        send("ui.awaitIdle")
        report={"passed":len(checks),"checks":checks,"binarySha256":hashlib.sha256((binaries/"joysheet.exe").read_bytes()).hexdigest()}
        (output/"report.json").write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding="utf-8")
        print(json.dumps(report,ensure_ascii=False))
    except BaseException:
        try:
            shot("failure")
            (output/"failure-state.json").write_text(json.dumps(send("ui.inspect"),ensure_ascii=False,indent=2),encoding="utf-8")
        except Exception:
            pass
        raise
    finally:
        if channel is not None:channel.close()
        if connection is not None:connection.close()
        if not args.keep_open:
            app.terminate()
            app.wait(timeout=20)
        log.close()


if __name__=="__main__":main()
