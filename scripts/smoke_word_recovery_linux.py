"""T104 native X11 crash/restore regression on UOS at 125% desktop scale.

Uses isolated disposable sources/profiles, real window pixels, OS synthetic
input and SIGKILL of the owned test process. This is not a power-loss test.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import time
import xml.etree.ElementTree as ET
import zipfile
import zlib

ROOT = Path(__file__).resolve().parents[1]
W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"


def journal_record(path):
    data = path.read_bytes()
    assert len(data) >= 29 and data[:8] in (b"JWREC002", b"JWREC003")
    assert zlib.crc32(data[:-4]) == struct.unpack_from("<I", data, len(data) - 4)[0]
    generation, kind, version = struct.unpack_from("<QBQ", data, 8)
    position = 25

    def blob():
        nonlocal position
        length = struct.unpack_from("<Q", data, position)[0]
        position += 8
        assert length <= len(data) - 4 - position
        result = data[position:position + length]
        position += length
        return result

    if kind == 2:
        return {"generation": generation, "version": version, "discarded": True}
    baseline, text = blob(), blob().decode("utf-8")
    snapshot = blob() if data[:8] == b"JWREC003" else None
    assert position == len(data) - 4
    return {"generation": generation, "version": version, "baseline": baseline,
            "text": text, "snapshot": snapshot, "discarded": False}


def document_state(path):
    with zipfile.ZipFile(path) as package:
        root = ET.fromstring(package.read("word/document.xml"))
    table = root.find(W + "body/" + W + "tbl")
    assert table is not None
    return {"rows": len(table.findall(W + "tr")),
            "grid": [int(column.get(W + "w")) for column in table.find(W + "tblGrid")],
            "text": "".join(node.text or "" for node in root.iter(W + "t")),
            "fill": table.find(W + "tr/" + W + "tc/" + W + "tcPr/" + W + "shd").get(W + "fill"),
            "bold": len(root.findall(".//" + W + "b")),
            "center": len([node for node in root.iter(W + "jc") if node.get(W + "val") == "center"])}


def run(binary, output):
    if not os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"):
        raise RuntimeError("Requires an unlocked native X11 desktop")
    for tool in ("xdotool", "import"):
        if not shutil.which(tool):
            raise RuntimeError(f"Missing tool: {tool}")
    output.mkdir(parents=True, exist_ok=False)
    source = output / "formal-contract.docx"
    shutil.copyfile(ROOT / "docs/joyoffice/validation/20261101-t201/native/20-reopened.docx", source)
    formal = source.read_bytes()
    report = {"passed": False, "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "sourceSha256": hashlib.sha256(formal).hexdigest(), "desktopScale": 1.25,
              "eventSource": "X11 OS synthetic keys/pointer; real windows; owned process SIGKILL",
              "powerLossTested": False, "nativeImeVerified": False, "checks": []}
    process, window, logfile = None, None, None

    def command(*args):
        return subprocess.check_output(list(map(str, args)), text=True, timeout=20).strip()

    def wait(predicate, label):
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            if predicate():
                return
            if process.poll() is not None:
                raise RuntimeError(f"Word exited {process.returncode}: {label}")
            time.sleep(.1)
        raise TimeoutError(label)

    def stop(kill=False):
        if process is not None and process.poll() is None:
            process.kill() if kill else process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=10)
        if logfile is not None:
            logfile.close()

    def launch(name, journal, path=None):
        nonlocal process, window, logfile
        environment = os.environ.copy()
        environment.update(JOYWORD_PROFILE="test", JOYWORD_PROFILE_DIR=str(output / ("profile-" + name)),
                           JOYWORD_RECOVERY_DIR=str(journal))
        logfile = (output / (name + ".log")).open("w", encoding="utf-8")
        process = subprocess.Popen([str(binary), *([str(path)] if path else [])], cwd=ROOT,
                                   env=environment, stdout=logfile, stderr=subprocess.STDOUT)

        def ready():
            nonlocal window
            try:
                windows = command("xdotool", "search", "--onlyvisible", "--pid", process.pid).split()
            except subprocess.CalledProcessError:
                return False
            if len(windows) != 1:
                return False
            window = windows[0]
            return path is None or path.name in command("xdotool", "getwindowname", window)

        wait(ready, "window " + name)
        command("xdotool", "windowactivate", "--sync", window)
        time.sleep(.7)
        geometry = command("xdotool", "getwindowgeometry", "--shell", window)
        assert "WIDTH=1500\nHEIGHT=1063" in geometry or "WIDTH=1500\nHEIGHT=1064" in geometry, geometry

    def click(x, y):
        command("xdotool", "mousemove", "--window", window, x, y, "click", "1")
        time.sleep(.25)

    def key(*keys):
        command("xdotool", "key", "--clearmodifiers", *keys)
        time.sleep(.2)

    def text(value):
        command("xdotool", "type", "--clearmodifiers", "--delay", "10", value)
        time.sleep(.2)

    def capture(name):
        command("import", "-window", window, output / (name + ".png"))

    def save(name):
        path = output / (name + ".docx")
        key("ctrl+shift+s")
        key("ctrl+a")
        text(str(path))
        key("Return")
        wait(path.is_file, name)
        return path

    try:
        journal = output / "journal"
        launch("edit", journal, source)
        click(124, 82)
        click(450, 430)
        # Drag the first internal boundary 24 physical pixels at 80% zoom.
        command("xdotool", "mousemove", "--window", window, 713, 264, "mousedown", "1")
        time.sleep(.15)
        command("xdotool", "mousemove", "--window", window, 737, 264)
        time.sleep(.2)
        command("xdotool", "mouseup", "1")
        time.sleep(.3)
        click(360, 158)
        click(497, 193)
        click(760, 854)
        key("ctrl+a")
        text("FFDDCC")
        click(1024, 950)
        click(360, 158)
        click(497, 235)  # Insert row, then type without a body click.
        text("RecoveredRow")
        key("Home", "shift+End", "ctrl+b")
        click(55, 82)
        click(644, 179)  # Paragraph centre alignment.
        click(500, 456)
        expected = save("01-before-kill")
        state = document_state(expected)
        assert state["rows"] == 3 and state["grid"] == [4488, 1752, 3120], state
        assert state["fill"] == "FFDDCC" and state["bold"] > 0 and state["center"] > 0, state
        assert "RecoveredRow" in state["text"], state
        capture("01-before-kill")
        slots = sorted([(path, journal_record(path)) for path in journal.glob("recovery-?.jwr")],
                       key=lambda item: item[1]["generation"])
        assert len(slots) == 2
        latest = slots[-1][1]
        assert latest["snapshot"] is not None, "Only original package + text was journaled; rich edits cannot recover"
        assert latest["baseline"] == formal
        assert latest["snapshot"] == expected.read_bytes(), "Checkpoint differs from independent Save As"
        report["lastCommitted"] = {"generation": latest["generation"], "version": latest["version"], "state": state}
        saved_slots = [(path.name, path.read_bytes(), record) for path, record in slots]
        stop(kill=True)
        report["checks"].append("real table row/column-width/fill/bold/alignment edits; checkpoint equals independent Save As; SIGKILL after last commit")

        launch("restore", journal)
        wait(lambda: "recovered-" in command("xdotool", "getwindowname", window), "rich recovery")
        capture("02-restored")
        restored = save("02-restored")
        assert restored.read_bytes() == latest["snapshot"]
        assert document_state(restored) == state
        assert source.read_bytes() == formal
        report["checks"].append("native startup restores exact complete DOCX, including formatting and table structure; formal source unchanged")
        key("ctrl+w")
        capture("03-discard-prompt")
        click(963, 590)
        wait(lambda: journal_record(max(journal.glob("recovery-?.jwr"), key=lambda p: journal_record(p)["generation"]))["discarded"], "explicit discard tombstone")
        stop()
        launch("after-discard", journal)
        capture("04-after-discard")
        assert "recovered-" not in command("xdotool", "getwindowname", window)
        stop()
        report["checks"].append("recovered rich document keeps unsaved choice after Save As; explicit discard publishes tombstone; restart does not resurrect it")

        fallback = output / "fallback-journal"
        fallback.mkdir()
        for name, data, _ in saved_slots:
            (fallback / name).write_bytes(data)
        latest_name = saved_slots[-1][0]
        corrupt = b"deliberately truncated latest generation"
        (fallback / latest_name).write_bytes(corrupt)
        launch("fallback", fallback)
        wait(lambda: "recovered-" in command("xdotool", "getwindowname", window), "fallback recovery")
        capture("05-fallback-warning")
        fallback_copy = save("03-fallback")
        assert fallback_copy.read_bytes() == saved_slots[0][2]["snapshot"]
        assert (fallback / latest_name).read_bytes() == corrupt
        assert (fallback / saved_slots[0][0]).read_bytes() == saved_slots[0][1]
        stop()
        report["checks"].append("corrupt latest slot restores exact prior rich generation; visible fallback notice; damaged and valid slots retained")

        broken = output / "broken-journal"
        broken.mkdir()
        for name, _, _ in saved_slots:
            (broken / name).write_bytes(b"corrupt both slots")
        launch("broken", broken)
        capture("06-recovery-failure-warning")
        assert "recovered-" not in command("xdotool", "getwindowname", window)
        assert not list(broken.glob("recovered-*.docx"))
        assert all((broken / name).read_bytes() == b"corrupt both slots" for name, _, _ in saved_slots)
        assert source.read_bytes() == formal
        stop()
        report["checks"].append("both corrupt slots refuse recovery, show notice and preserve evidence; no fabricated recovered package")
        report["passed"] = True
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
        if process is not None and process.poll() is None and window is not None:
            try:
                capture("failure")
            except subprocess.SubprocessError as capture_error:
                report["captureError"] = str(capture_error)
        raise
    finally:
        stop()
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/joyword")
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    run(arguments.binary.resolve(), arguments.output.resolve())
