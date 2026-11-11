"""Exercise the production ribbon with GPUI pointer/key events and GPU readback.

Run after `cargo xtask build joysheet --validation`. The output directory must be
new; each run owns an isolated profile and process and preserves its evidence.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time
from ribbon_objects_smoke import exercise as exercise_objects


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary-directory", type=Path, default=Path("target/debug"))
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--excel-reference", type=Path,
                        help="Optional Excel-authored Date/Region/Sales three-row Timeline fixture")
    parser.add_argument("--excel-filtered-reference", type=Path,
                        help="Optional Excel-saved January 2026 Timeline fixture with a total of 10")
    parser.add_argument("--excel-objects-reference", type=Path,
                        help="Optional Excel-resaved five-object JoySheet interop fixture")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = args.binary_directory.resolve() / "joysheet.exe"
    cli = binary.with_name("joysheet-cli.exe")
    env = os.environ.copy()
    env["JOYSHEET_CONFIG_DIR"] = str(output / "profile")
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    log = (output / "stderr.log").open("w", encoding="utf-8")
    app = subprocess.Popen([str(binary), "--control", str(port)], cwd=binary.parent,
                           env=env, stderr=log, stdout=subprocess.DEVNULL,
                           creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    checks = []

    def send(method, params=None):
        result = subprocess.run([str(cli), "send", str(port), method,
                                 json.dumps(params or {}, ensure_ascii=False)],
                                env=env, capture_output=True, encoding="utf-8", timeout=30)
        if result.returncode:
            raise RuntimeError(f"{method}: {result.stderr}")
        return json.loads(result.stdout)

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
            time.sleep(0.08)
        raise AssertionError(f"Timed out: {label}; {last}")

    def click(key):
        view = wait(lambda v: key in v["advancedBounds"], key)
        b = view["advancedBounds"][key]
        send("ui.click", {"x": b["x"] + b["width"] / 2,
                          "y": b["y"] + b["height"] / 2})
        return send("ui.inspect")

    def tab(name):
        click("ribbon-tab:" + name)
        return wait(lambda v: v["ribbon"]["tab"] == name, name)

    def shot(name):
        send("ui.screenshot", {"path": str(output / (name + ".png"))})

    def execute(command, params=None):
        send("engine.execute", {"command": command, "params": params or {}})
        send("ui.awaitIdle")

    try:
        wait(lambda v: v["grid"]["height"] > 100 and "ribbon" in v, "first frame")
        if args.excel_objects_reference:
            reference = args.excel_objects_reference.resolve()
            objects_hash = hashlib.sha256(reference.read_bytes()).hexdigest()
            send("app.open", {"name": "excel-resaved-objects.xlsx", "base64":
                             base64.b64encode(reference.read_bytes()).decode("ascii")})
            view = wait(lambda v: any("Timeline" in (o["object"] or {})
                                     for o in v["drawingObjects"]["shapes"]), "Excel-resaved objects import")
            imported = view["drawingObjects"]["shapes"]
            assert len(imported) == 5, imported
            (output / "excel-imported-objects.json").write_text(json.dumps(imported, ensure_ascii=False, indent=2), encoding="utf-8")
            kinds = {kind for o in imported for kind in (o["object"] or {})}
            assert kinds == {"SmartArt", "WordArt", "Equation", "Embedded", "Timeline"}, kinds
            embedded = next(o for o in imported if "Embedded" in (o["object"] or {}))
            extracted = output / "excel-resaved-extracted.txt"
            execute("object.extract", {"id": embedded["id"], "path": str(extracted)})
            assert extracted.read_bytes() == b"Embedded bytes: JoyOffice\r\n"
            assert hashlib.sha256(reference.read_bytes()).hexdigest() == objects_hash
            shot("excel-resaved-objects-imported")
            checks.append("Excel-resaved five native objects reimport and embedded payload extracts byte-for-byte")
            send("app.new")
            wait(lambda v: not v["drawingObjects"]["shapes"], "new Ribbon workbook")
        for name in ["Home", "Insert", "Draw", "Page Layout", "Formulas", "Data", "Review", "View", "Help", "Automate"]:
            view = tab(name)
            assert view["ribbon"]["groups"], name
            shot("tab-" + name.lower().replace(" ", "-"))
            checks.append("native tab click and GPU capture: " + name)
        tab("File")
        shot("tab-file")
        send("ui.key", {"key": "escape"})
        wait(lambda v: v["ribbon"]["tab"] == "Home", "backstage escape")
        checks.append("File backstage and Escape return")

        execute("cell.set", {"cell": "A1", "input": "12.5"})
        send("ui.cell.click", {"cell": "A1"})
        click("ribbon:bold")
        send("ui.awaitIdle")
        assert send("cell.get", {"cell": "A1"})["style"]["font"]["bold"]
        send("ui.key", {"key": "z", "ctrl": True})
        send("ui.awaitIdle")
        assert not send("cell.get", {"cell": "A1"})["style"]["font"]["bold"]
        send("ui.key", {"key": "y", "ctrl": True})
        send("ui.awaitIdle")
        assert send("cell.get", {"cell": "A1"})["style"]["font"]["bold"]
        checks.append("Ribbon command returns cell focus for Ctrl+Z/Ctrl+Y")
        click("ribbon:format-painter")
        send("ui.cell.click", {"cell": "C3"})
        send("ui.awaitIdle")
        assert send("cell.get", {"cell": "C3"})["style"]["font"]["bold"]
        checks.append("Home Bold and Format Painter apply real styles")
        click("ribbon:fill-menu")
        shot("menu-fill")
        send("ui.key", {"key": "escape"})

        view = send("ui.inspect")
        old_height = view["grid"]["height"]
        send("ui.key", {"key": "f1", "ctrl": True})
        wait(lambda v: v["ribbon"]["collapsed"] and v["grid"]["height"] > old_height, "collapse")
        send("ui.key", {"key": "f1", "ctrl": True})
        wait(lambda v: not v["ribbon"]["collapsed"], "expand")
        checks.append("Ctrl+F1 changes the visible ribbon and grid extent")

        tab("Formulas")
        click("ribbon:functions-math")
        wait(lambda v: v["ribbon"]["functionCategory"] == "MathTrig", "function category")
        send("ui.text", {"text": "SUM"})
        wait(lambda v: "function:SUM" in v["advancedBounds"], "filtered SUM")
        shot("function-library")
        click("function:SUM")
        wait(lambda v: v["editor"] is not None and v["editor"]["text"] == "=SUM(", "function inserts editable formula")
        send("ui.key", {"key": "escape"})
        checks.append("Function search filters Math/Trig and SUM starts an editable formula")
        tab("View")
        click("ribbon:headings")
        send("ui.awaitIdle")
        shot("headings-hidden")
        send("ui.cell.click", {"cell": "B2"})
        send("ui.text", {"text": "Hidden headings hit test"})
        send("ui.key", {"key": "enter"})
        send("ui.awaitIdle")
        assert send("cell.get", {"cell": "B2"})["value"] == "Hidden headings hit test"
        click("ribbon:headings")
        send("ui.awaitIdle")
        checks.append("Hidden headings retain correct B2 editing coordinates")

        tab("Draw")
        click("ribbon:draw-blue")
        view = wait(lambda v: v["ribbon"]["inkTool"] == "pen", "pen")
        grid = view["grid"]
        start = {"x": grid["x"] + 250, "y": grid["y"] + 150}
        send("ui.drag", {**start, "toX": start["x"] + 180, "toY": start["y"] + 65})
        send("ui.awaitIdle")
        assert len(send("ui.inspect")["drawingObjects"]["shapes"]) == 1
        shot("ink-drawn")
        send("ui.key", {"key": "escape"})
        send("ui.key", {"key": "z", "ctrl": True})
        send("ui.awaitIdle")
        assert len(send("ui.inspect")["drawingObjects"]["shapes"]) == 0
        send("ui.key", {"key": "y", "ctrl": True})
        send("ui.awaitIdle")
        assert len(send("ui.inspect")["drawingObjects"]["shapes"]) == 1
        checks.append("Draw pointer gesture creates ink; keyboard Undo/Redo restores it")

        file = output / "ribbon-roundtrip.xlsx"
        send("app.save", {"path": str(file)})
        click("ribbon:draw-eraser")
        send("ui.click", {"x": start["x"] + 60, "y": start["y"] + 20})
        send("ui.awaitIdle")
        assert len(send("ui.inspect")["drawingObjects"]["shapes"]) == 0
        checks.append("Eraser removes ink objects")
        send("ui.key", {"key": "escape"})
        previous_doc = send("ui.inspect")["windows"][0]["doc"]
        send("app.open", {"name": "ribbon-reopened.xlsx", "base64": base64.b64encode(file.read_bytes()).decode("ascii")})
        wait(lambda v: any(w["doc"] != previous_doc and w.get("active") for w in v["windows"]), "reopen independent document")
        assert len(send("ui.inspect")["drawingObjects"]["shapes"]) == 1
        checks.append("Saved XLSX reopens with ink intact")
        tab("Help")
        click("ribbon:help")
        wait(lambda v: v["ribbon"]["help"], "help")
        shot("help-open")
        send("ui.key", {"key": "escape"})
        checks.append("Local help opens and closes with Escape")
        exercise_objects(send, wait, click, tab, shot, execute, checks, output)
        if args.excel_reference:
            reference = args.excel_reference.resolve()
            reference_hash = hashlib.sha256(reference.read_bytes()).hexdigest()
            send("app.open", {"name": "excel-authored-reference.xlsx", "base64":
                             base64.b64encode(reference.read_bytes()).decode("ascii")})
            view = wait(lambda v: any("Timeline" in (o["object"] or {})
                                     for o in v["drawingObjects"]["shapes"]),
                        "Excel-authored Timeline import")
            timeline = next(o for o in view["drawingObjects"]["shapes"]
                            if "Timeline" in (o["object"] or {}))
            execute("timeline.scroll", {"id": timeline["id"], "delta": -6})
            view = send("ui.inspect")
            b = next(b for b in view["objectBounds"] if b["kind"] == "shape"
                     and b["id"] == timeline["id"])
            send("ui.click", {"x": b["x"] + 20, "y": b["y"] + b["height"] - 40})
            send("ui.awaitIdle")
            selected = next(o for o in send("ui.inspect")["drawingObjects"]["shapes"]
                            if o["id"] == timeline["id"])["object"]["Timeline"]["selection"]
            assert selected is not None
            assert send("cell.get", {"cell": "B4"})["value"] == 10
            shot("excel-authored-timeline-filtered")
            checks.append("Excel-authored Timeline imports, scrolls back to source data and filters by pointer")
        if args.excel_filtered_reference:
            reference = args.excel_filtered_reference.resolve()
            filtered_hash = hashlib.sha256(reference.read_bytes()).hexdigest()
            send("app.open", {"name": "excel-filtered-reference.xlsx", "base64":
                             base64.b64encode(reference.read_bytes()).decode("ascii")})
            view = wait(lambda v: any("Timeline" in (o["object"] or {})
                                     for o in v["drawingObjects"]["shapes"]),
                        "Excel-saved January Timeline import")
            timeline = next(o for o in view["drawingObjects"]["shapes"]
                            if "Timeline" in (o["object"] or {}))
            start, end = timeline["object"]["Timeline"]["selection"]
            assert end - start + 1 == 31, "Excel's January end date is inclusive"
            assert send("cell.get", {"cell": "G4"})["value"] == 10
            execute("timeline.filter", {"id": timeline["id"], "clear": True})
            assert send("cell.get", {"cell": "G4"})["value"] == 30
            assert send("cell.get", {"cell": "G6"})["value"] == 60
            assert hashlib.sha256(reference.read_bytes()).hexdigest() == filtered_hash
            checks.append("Excel-saved January imports all 31 days; clearing restores the full PivotTable")
        tab("Home")
        shot("final-home")
        report = {"passed": True, "checks": checks, "binary": str(binary),
                  "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                  "source": "production GPUI synthetic input and native GPU readback",
                  "physicalInputVerified": False}
        if args.excel_reference:
            assert hashlib.sha256(args.excel_reference.read_bytes()).hexdigest() == reference_hash
            report["excelAuthoredReference"] = {
                "sha256": reference_hash,
                "sourceUnmodified": True,
                "excelAcceptanceOfJoySheetExports": False,
            }
        if args.excel_filtered_reference:
            report["excelFilteredReference"] = {"sha256": filtered_hash, "sourceUnmodified": True}
        if args.excel_objects_reference:
            report["excelResavedObjectsReference"] = {"sha256": objects_hash, "sourceUnmodified": True}
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False, indent=2))
    finally:
        app.terminate()
        app.wait(timeout=15)
        log.close()


if __name__ == "__main__":
    main()
