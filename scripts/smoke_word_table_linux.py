"""Native X11 T201 acceptance: create/edit a contract table and read DOCX back.

Uses disposable copies, OS synthetic input and actual window pixels. Requires
UOS/X11 at 125% desktop scale and the default 1200x850 logical Word window.
Does not certify a system IME, physical input, or another desktop protocol.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"


def read_table(path):
    with zipfile.ZipFile(path) as package:
        root = ET.fromstring(package.read("word/document.xml"))
    table = root.find(f"{W}body/{W}tbl")
    if table is None:
        raise AssertionError("Saved document has no table")
    margins = table.find(W + "tblPr/" + W + "tblCellMar")
    return {"margins": {node.tag.removeprefix(W): int(node.get(W + "w")) for node in margins}
            if margins is not None else None,
            "grid": [int(col.get(W + "w")) for col in table.find(W + "tblGrid")],
            "rows": [[{"text": "".join(node.text or "" for node in cell.iter(W + "t")),
                       "span": int(cell.find(W + "tcPr/" + W + "gridSpan").get(W + "val"))
                       if cell.find(W + "tcPr/" + W + "gridSpan") is not None else 1,
                       "fill": cell.find(W + "tcPr/" + W + "shd").get(W + "fill")
                       if cell.find(W + "tcPr/" + W + "shd") is not None else None,
                       "border": cell.find(W + "tcPr/" + W + "tcBorders/" + W + "top").get(W + "val")
                       if cell.find(W + "tcPr/" + W + "tcBorders/" + W + "top") is not None else None}
                      for cell in row.findall(W + "tc")] for row in table.findall(W + "tr")]}


def run(binary, output):
    if not os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"):
        raise RuntimeError("Requires a native unlocked X11 session")
    for tool in ("xdotool", "import", "pdftotext"):
        if not shutil.which(tool):
            raise RuntimeError(f"Missing tool: {tool}")
    output.mkdir(parents=True, exist_ok=False)
    source = output / "blank-contract.docx"
    with zipfile.ZipFile(ROOT / "tests/fixtures/docx/DOCX-MIN-001.docx") as original, zipfile.ZipFile(source, "w") as copy:
        for name in original.namelist():
            data = original.read(name)
            if name == "word/document.xml":
                data = (f'<w:document xmlns:w="{W[1:-1]}"><w:body><w:p/><w:sectPr>'
                        '<w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" '
                        'w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>').encode()
            copy.writestr(name, data)
    original_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    environment = os.environ.copy()
    environment.update(JOYWORD_PROFILE="test", JOYWORD_PROFILE_DIR=str(output / "profile"),
                       JOYWORD_RECOVERY_DIR=str(output / "recovery"))
    report = {"passed": False, "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "sourceSha256": original_hash, "eventSource": "X11 OS synthetic keys/pointer; actual window pixels",
              "desktopScale": 1.25, "nativeImeVerified": False, "checks": [], "snapshots": {}}
    process, window = None, None

    def command(*args):
        return subprocess.check_output(list(map(str, args)), text=True, timeout=20).strip()

    def wait(predicate, label):
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"Word exited {process.returncode}: {label}")
            if predicate():
                return
            time.sleep(.1)
        raise TimeoutError(label)

    def click(x, y):
        command("xdotool", "mousemove", "--window", window, x, y, "click", "1")
        time.sleep(.25)

    def key(*keys):
        command("xdotool", "key", "--clearmodifiers", *keys)
        time.sleep(.2)

    def text(value):
        command("xdotool", "type", "--clearmodifiers", "--delay", "20", value)
        time.sleep(.2)

    def capture(name):
        command("import", "-window", window, output / (name + ".png"))

    def menu(index):
        click(360, 158)
        click(497, 193 + 42 * index)

    def save(name):
        path = output / (name + ".docx")
        key("ctrl+shift+s")
        key("ctrl+a")
        text(str(path))
        key("Return")
        wait(path.is_file, "Save As " + name)
        state = read_table(path)
        report["snapshots"][name] = state
        return state

    def drag(x, delta):
        command("xdotool", "mousemove", "--window", window, x, 264, "mousedown", "1")
        time.sleep(.15)
        command("xdotool", "mousemove", "--window", window, x + delta, 264)
        time.sleep(.2)
        command("xdotool", "mouseup", "1")
        time.sleep(.4)

    try:
        with (output / "application.log").open("w", encoding="utf-8") as log:
            process = subprocess.Popen([str(binary), str(source)], cwd=ROOT, env=environment,
                                       stdout=log, stderr=subprocess.STDOUT)

            def ready():
                nonlocal window
                try:
                    windows = command("xdotool", "search", "--onlyvisible", "--pid", process.pid).split()
                except subprocess.CalledProcessError:
                    return False
                if len(windows) != 1:
                    return False
                window = windows[0]
                return source.name in command("xdotool", "getwindowname", window)

            wait(ready, "open blank source")
            command("xdotool", "windowactivate", "--sync", window)
            geometry = command("xdotool", "getwindowgeometry", "--shell", window)
            report["windowGeometry"] = geometry
            if "WIDTH=1500\nHEIGHT=1063" not in geometry and "WIDTH=1500\nHEIGHT=1064" not in geometry:
                raise RuntimeError("This pixel regression requires the default 1500x1063/1064 physical window")
            time.sleep(.6)
            click(124, 82)
            click(216, 158)
            click(300, 245)  # Native 3-column x 2-row grid picker.
            text("Item")  # No body click: verifies focus after insertion.
            for x, y, value in [(665, 430, "Qty"), (870, 430, "Amount"),
                                (450, 456, "Service"), (665, 456, "2"), (870, 456, "100")]:
                click(x, y)
                text(value)
            baseline = save("01-created")
            assert baseline["grid"] == [3120] * 3, baseline
            assert baseline["margins"] is None, baseline
            assert [[cell["text"] for cell in row] for row in baseline["rows"]] == [
                ["Item", "Qty", "Amount"], ["Service", "2", "100"]], baseline
            capture("01-created")
            report["checks"].append("create a 3x2 table using native grid picker; immediate typing; six cell values read from DOCX")

            click(450, 430)
            menu(1)
            inserted = save("02-row-insert")
            assert len(inserted["rows"]) == 3
            key("ctrl+z")
            assert save("03-row-undo") == baseline
            key("ctrl+y")
            assert save("04-row-redo") == inserted
            click(450, 456)
            menu(2)
            assert save("05-row-delete") == baseline
            menu(3)
            assert len(save("06-column-insert")["grid"]) == 4
            click(610, 430)
            menu(4)
            assert save("07-column-delete") == baseline
            capture("02-table-menu-result")
            report["checks"].append("native row insert/delete and column insert/delete; row undo/redo preserves all content")

            click(450, 430)
            menu(6)
            merged = save("08-merged")
            assert [cell["span"] for cell in merged["rows"][0]] == [2, 1]
            assert merged["rows"][0][0]["text"] == "ItemQty"
            menu(8)
            split = save("09-split")
            assert [cell["span"] for cell in split["rows"][0]] == [1, 1, 1]
            key("ctrl+z")
            assert save("10-split-undo") == merged
            key("ctrl+z")
            assert save("11-merge-undo") == baseline
            capture("03-merge-split-history")
            report["checks"].append("merge right and split through visible menu; each is one undo; no content loss")

            drag(646, 48)
            resized = save("12-resized-80")
            assert resized["grid"] == [3840, 2400, 3120], resized
            key("ctrl+z")
            assert save("13-resize-undo") == baseline
            key("ctrl+y")
            assert save("14-resize-redo") == resized
            capture("04-resized-80")
            click(1404, 1044)
            click(1404, 1044)
            capture("05-zoom-100-before-drag")
            drag(680, 24)
            zoomed = save("15-resized-100")
            assert zoomed["grid"] == [4128, 2112, 3120], zoomed
            capture("06-resized-100")
            report["checks"].append("real ruler drags at 80% and 100% document zoom on a 125% desktop; exact twips; undo/redo")

            # Native properties: double border and only the caret cell's fill.
            click(380, 465)
            menu(0)
            click(760, 637)
            key("ctrl+a")
            text("double")
            click(760, 854)
            key("ctrl+a")
            text("DDEEFF")
            capture("07-fill-dialog")
            click(1024, 950)
            formatted = save("16-cell-fill")
            assert formatted["rows"][0][0]["fill"] == "DDEEFF", formatted
            assert formatted["margins"] == baseline["margins"], formatted
            assert formatted["rows"][0][1]["fill"] is None
            assert all(cell["border"] == "double" for row in formatted["rows"] for cell in row)
            key("ctrl+z")
            assert save("17-format-undo") == zoomed
            key("ctrl+y")
            assert save("18-format-redo") == formatted
            menu(0)
            click(760, 854)
            key("ctrl+a")
            text("FF0000")
            key("Escape")
            assert save("19-cancelled-fill") == formatted
            capture("08-final-table")
            report["checks"].append("cell fill and double border in one undo; adjacent cells unchanged; cancelled properties unchanged")

            final_path = output / "19-cancelled-fill.docx"
            key("ctrl+o")
            capture("09-discard-original-session")
            click(963, 590)  # Explicit Don't Save for this disposable source session.
            key("ctrl+a")
            text(str(final_path))
            key("Return")
            wait(lambda: final_path.name in command("xdotool", "getwindowname", window), "reopen saved contract")
            time.sleep(.5)
            assert save("20-reopened") == formatted
            key("ctrl+e")
            key("ctrl+a")
            pdf = output / "contract.pdf"
            text(str(pdf))
            key("Return")
            wait(pdf.is_file, "PDF export")
            extracted = command("pdftotext", pdf, "-")
            (output / "pdf-text.txt").write_text(extracted, encoding="utf-8")
            assert extracted.split() == ["Item", "Qty", "Amount", "Service", "2", "100"], extracted
            capture("10-reopened")
            assert hashlib.sha256(source.read_bytes()).hexdigest() == original_hash
            report["checks"].append("save and reopen contract in native app; unchanged grid/fill/borders/content; independent PDF text readback; original bytes intact")
            report["passed"] = True
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
        if window is not None and process is not None and process.poll() is None:
            capture("failure")
        raise
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=10)
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/joyword")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    run(args.binary.resolve(), args.output.resolve())
