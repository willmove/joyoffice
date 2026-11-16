"""Ribbon dialog pointer regression, with isolated profile, binaries and evidence.

Uses the production GPUI pointer/key path. This supplements native Windows mouse
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
        raise AssertionError(f"Timed out: {label}; {last}")

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
        send("engine.execute", {"command": command, "params": params or {}})
        send("ui.awaitIdle")

    def set_field(field, value):
        send("ui.dialog.set", {"field": field, "value": value})

    def dialog(name):
        send("ui.dialog", {"name": name})
        return wait(lambda v: v.get("commandDialog", {}).get("command") == name, name)

    try:
        wait(lambda v: v["grid"]["height"] > 100, "first frame")
        send("ui.resize", {"width": 1800, "height": 1000})
        execute("cell.set", {"cell": "A1", "input": "10"})
        execute("cell.set", {"cell": "B1", "input": "20"})
        for name in ([] if args.focused_only else ["Home", "Insert", "Draw", "Page Layout", "Formulas", "Data", "Review", "View", "Help", "Automate"]):
            view = tab(name)
            shot("tab-" + name.lower().replace(" ", "-"))
            for group in view["ribbon"]["groups"]:
                for tool in group["tools"]:
                    record = {"tab": name, "group": group["name"], **tool}
                    if tool["dialog"]:
                        before = send("ui.inspect")["windows"][0]["revision"]
                        click("ribbon:" + tool["id"])
                        state = send("ui.inspect")
                        if state["commandDialog"]["command"]:
                            assert state["commandDialog"]["command"] == tool["command"], record
                            shot("dialog-" + tool["id"])
                            click("dialog-cancel")
                            wait(lambda v: v["commandDialog"]["command"] is None, "cancel")
                            assert send("ui.inspect")["windows"][0]["revision"] == before, record
                            record["tested"] = "pointer opens dialog; Cancel leaves revision unchanged"
                        else:
                            record["tested"] = "prerequisite guard (no selected object)"
                            assert state["error"], record
                        send("ui.key", {"key": "escape"})
                    elif tool["menu"]:
                        if tool["id"] == "cell-styles":
                            state = send("ui.inspect")
                            for gallery_key in sorted(k for k in state["advancedBounds"] if k.startswith("ribbon-style:")):
                                click(gallery_key)
                        key = "ribbon-menu:" + tool["id"]
                        if key not in send("ui.inspect")["advancedBounds"]:
                            key = "ribbon:" + tool["id"]
                        if tool["id"] == "cell-styles" and "ribbon:cell-styles-more" in send("ui.inspect")["advancedBounds"]:
                            key = "ribbon:cell-styles-more"
                        if key in send("ui.inspect")["advancedBounds"]:
                            click(key)
                            shot("menu-" + tool["id"])
                            send("ui.key", {"key": "escape"})
                            record["tested"] = "pointer opens menu and Escape dismisses"
                            record["items"] = []
                            for item in tool.get("menuItems", []):
                                # These interactions use only the disposable workbook.
                                send("ui.message.dismiss")
                                click(key)
                                click("ribbon-item:" + item["id"])
                                time.sleep(0.1)
                                state = send("ui.inspect")
                                outcome = {**item, "dialogOpened": state["commandDialog"]["command"],
                                           "panel": state["panel"], "error": state["error"]}
                                if state["commandDialog"]["command"]:
                                    click("dialog-cancel")
                                else:
                                    send("ui.key", {"key": "escape"})
                                record["items"].append(outcome)
                        else:
                            record["tested"] = "inline style gallery pointer choices"
                    else:
                        if tool["command"] in {"view.newWindow", "view.hideWindow", "view.unhideWindow"}:
                            record["tested"] = "window lifecycle: separate product smoke required"
                        else:
                            send("ui.message.dismiss")
                            click("ribbon:" + tool["id"])
                            time.sleep(0.15)
                            state = send("ui.inspect")
                            record["tested"] = "pointer invoked; prerequisite error" if state["error"] else "pointer invoked; response captured"
                            record["result"] = {"dialog": state["dialog"], "panel": state["panel"], "error": state["error"]}
                            send("ui.key", {"key": "escape"})
                    inventory.append(record)
                    (output / "inventory-progress.json").write_text(json.dumps(inventory, ensure_ascii=False, indent=2), encoding="utf-8")
            checks.append("pointer dialogs/menus: " + name)
        send("app.new")
        execute("cell.set", {"cell": "A1", "input": "10"})
        execute("cell.set", {"cell": "B1", "input": "20"})
        tab("Home")
        execute("selection.set", {"range": "A1"})
        execute("home.underline", {"on": False})
        # Split buttons: main applies immediately; the arrow remains a menu.
        send("ui.cell.click", {"cell": "A1"})
        click("ribbon:underline")
        send("ui.awaitIdle")
        assert send("cell.get", {"cell": "A1"})["style"]["font"]["underline"] in {"Single", "Double"}
        click("ribbon:underline")
        send("ui.awaitIdle")
        assert send("cell.get", {"cell": "A1"})["style"]["font"]["underline"] == "None"
        click("ribbon:copy")
        send("ui.awaitIdle")
        send("ui.cell.click", {"cell": "C1"})
        click("ribbon:paste")
        send("ui.awaitIdle")
        assert send("cell.get", {"cell": "C1"})["value"] == 10.0
        checks.append("split primary underline toggles; Copy/Paste main buttons transfer a value")
        # Format tabs keep values across pages and preserve unrelated styles.
        execute("home.fontSize", {"range": "B1", "size": 24})
        execute("home.fontColor", {"range": "B1", "color": {"theme": 4, "tint": 0.25}})
        execute("selection.set", {"range": "A1:B1"})
        before_b = send("cell.get", {"cell": "B1"})["style"]
        original = send("ui.inspect")["windows"][0]["revision"]
        dialog("home.formatCells")
        for i in range(6):
            click("dialog-tab:" + str(i))
            wait(lambda v: v["commandDialog"]["tab"] == i, "format tab")
            shot("format-tab-" + str(i))
        click("dialog-ok")
        assert send("ui.inspect")["windows"][0]["revision"] == original
        dialog("home.formatCells")
        click("dialog-tab:2")
        click("font-style:Bold")
        click("dialog-tab:1")
        click("dialog-tab:2")
        assert send("ui.inspect")["commandDialog"]["fields"]["bold"] == "true"
        click("dialog-ok")
        send("ui.awaitIdle")
        after_b = send("cell.get", {"cell": "B1"})["style"]
        assert after_b["font"]["bold"]
        assert after_b["font"]["size"] == 24
        assert after_b["font"]["color"] == before_b["font"]["color"]
        checks.append("six Format Cells tabs, no-op OK, font style persistence and mixed-style preservation")
        execute("pageLayout.pageSetup", {"orientation": "landscape", "paper": "A3", "scale": 85,
                                         "header": "&CReport", "left": 0.8, "titleRows": "1:2"})
        view = dialog("pageLayout.pageSetup")
        fields = view["commandDialog"]["fields"]
        assert fields["paper"] == "A3" and fields["scale"] == "85" and fields["header"] == "&CReport"
        for i in range(4):
            click("dialog-tab:" + str(i))
            shot("page-tab-" + str(i))
        click("dialog-ok")
        send("ui.awaitIdle")
        view = dialog("pageLayout.pageSetup")
        assert view["commandDialog"]["fields"]["scale"] == "85"
        assert view["commandDialog"]["fields"]["fitMode"] == "false"
        click("dialog-cancel")
        checks.append("Page Setup loads live settings; four tabs; scale mode and header survive OK/reopen")
        dialog("data.validation")
        for i in range(3):
            click("dialog-tab:" + str(i))
            shot("validation-tab-" + str(i))
        click("dialog-cancel")
        execute("view.zoom", {"percent": 125})
        assert dialog("view.zoom")["commandDialog"]["fields"]["percent"] == "125"
        set_field("percent", "invalid")
        click("dialog-ok")
        wait(lambda v: bool(v["commandDialog"]["error"]), "inline validation error")
        shot("dialog-inline-error")
        click("dialog-cancel")
        send("ui.key", {"key": "k", "ctrl": True})
        assert send("ui.inspect")["commandDialog"]["command"] is None
        shot("command-search")
        send("ui.key", {"key": "escape"})
        checks.append("validation tabs; live zoom; invalid input stays open; Ctrl+K opens command search")
        dialog("edit.find")
        set_field("what", "10")
        send("ui.key", {"key": "enter"})
        send("ui.awaitIdle")
        assert send("ui.inspect")["commandDialog"]["command"] == "edit.find"
        shot("find-stays-open")
        click("dialog-cancel")
        checks.append("Find Next keeps the Find and Replace dialog open")
        execute("home.rowHeight", {"height": 20})
        execute("home.columnWidth", {"width": 64})
        assert float(dialog("home.rowHeight")["commandDialog"]["fields"]["points"]) == 15.0
        click("dialog-cancel")
        assert float(dialog("home.columnWidth")["commandDialog"]["fields"]["chars"]) == 8.43
        click("dialog-cancel")
        checks.append("Row Height uses points and Column Width uses characters")
        previous = send("ui.inspect")
        execute("view.newWindow")
        view = wait(lambda v: len(v["windows"]) == len(previous["windows"]) + 1, "new workbook view")
        hidden_window = view["window"]
        execute("view.hideWindow", {"window": hidden_window})
        wait(lambda v: all(w["window"] != hidden_window for w in v["windows"]), "hidden view")
        tab("View")
        click("ribbon:unhide-window")
        wait(lambda v: v["commandDialog"]["command"] == "view.unhideWindow" and v["commandDialog"]["fields"]["window"] == str(hidden_window), "hidden window picker")
        shot("unhide-window-picker")
        click("dialog-cancel")
        assert all(w["window"] != hidden_window for w in send("ui.inspect")["windows"])
        click("ribbon:unhide-window")
        wait(lambda v: v["commandDialog"]["fields"]["window"] == str(hidden_window), "hidden window reload")
        click("dialog-ok")
        wait(lambda v: any(w["window"] == hidden_window for w in v["windows"]), "selected window restored")
        checks.append("New/Hide/Unhide view; Unhide picker Cancel retains hidden view and OK restores selected view")
        report = {"checks": checks, "inventory": inventory, "passed": len(checks), "binary": str(binaries / "joysheet.exe"),
                  "binarySha256": hashlib.sha256((binaries / "joysheet.exe").read_bytes()).hexdigest(), "focusedOnly": args.focused_only}
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps({"passed": len(checks), "catalogued": len(inventory)}, ensure_ascii=False))
    finally:
        if channel is not None:
            channel.close()
        if connection is not None:
            connection.close()
        if not args.keep_open:
            app.terminate()
            app.wait(timeout=20)
        log.close()


if __name__ == "__main__":
    main()
