"""X11 native Word regression for path-dialog layout and document focus recovery.

Requires xdotool, ImageMagick import and pdftotext on an unlocked X11 desktop.
Uses OS synthetic keys, isolated test profiles and disposable DOCX copies.
It does not certify the system IME, physical input or other desktop protocols.
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


def document_text(path):
    with zipfile.ZipFile(path) as package:
        root = ET.fromstring(package.read("word/document.xml"))
    return "".join(node.text or "" for node in root.iter("{http://schemas.openxmlformats.org/wordprocessingml/2006/main}t"))


def run(binary, output):
    if not os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"):
        raise RuntimeError("This regression requires a native X11 session")
    for tool in ("xdotool", "import", "pdftotext"):
        if not shutil.which(tool):
            raise RuntimeError(f"Required tool missing: {tool}")
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    source = output / "a-long-directory-name-for-native-path-dialog-regression" / "original-document.docx"
    source.parent.mkdir()
    shutil.copyfile(ROOT / "tests/fixtures/docx/DOCX-MIN-001.docx", source)
    original = source.read_bytes()
    saved = source.with_name("saved-copy.docx")
    pdf = output / "native-export.pdf"
    environment = os.environ.copy()
    environment.update(JOYWORD_PROFILE="test", JOYWORD_PROFILE_DIR=str(output / "profile"),
                       JOYWORD_RECOVERY_DIR=str(output / "recovery"))
    report = {"passed": False, "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "eventSource": "X11 OS synthetic keys and actual window pixels", "checks": [],
              "nativeImeVerified": False}
    process = None
    window = None

    def command(*args):
        return subprocess.check_output(list(args), text=True, timeout=15).strip()

    def wait(predicate, label):
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"Word exited {process.returncode}: {label}")
            if predicate():
                return
            time.sleep(.1)
        raise TimeoutError(label)

    def keys(*values):
        command("xdotool", "windowactivate", "--sync", window)
        command("xdotool", "key", "--clearmodifiers", *values)
        time.sleep(.15)

    def text(value):
        command("xdotool", "type", "--clearmodifiers", "--delay", "20", value)

    def capture(name):
        command("xdotool", "windowactivate", "--sync", window)
        time.sleep(.2)
        command("import", "-window", window, str(output / name))

    try:
        with (output / "application.log").open("w", encoding="utf-8") as log:
            process = subprocess.Popen([str(binary), str(source)], env=environment, cwd=ROOT,
                                       stdout=log, stderr=subprocess.STDOUT)

            def ready():
                nonlocal window
                try:
                    windows = command("xdotool", "search", "--onlyvisible", "--pid", str(process.pid)).split()
                except subprocess.CalledProcessError:
                    return False
                if len(windows) != 1:
                    return False
                window = windows[0]
                return source.name in command("xdotool", "getwindowname", window)

            wait(ready, "source document window")
            time.sleep(.5)
            capture("01-open.png")
            keys("ctrl+Home")
            text("Z")
            keys("ctrl+z")
            keys("ctrl+y")
            keys("ctrl+shift+s")
            capture("02-long-path.png")
            keys("ctrl+a")
            text(str(saved))
            keys("Return")
            wait(saved.is_file, "save-copy completion")
            if document_text(saved) != "Z" + document_text(source):
                raise AssertionError("Saved copy lost the edit, history or original text")
            report["checks"].append("native typing, undo/redo, long-path Save As and independent DOCX readback")
            # No pointer click is allowed here: opening this next dialog by key
            # must work immediately after the former path field disappears.
            keys("ctrl+e")
            capture("03-keyboard-export-dialog.png")
            keys("ctrl+a")
            text(str(pdf))
            keys("Return")
            wait(pdf.is_file, "keyboard PDF export after Save As")
            extracted = command("pdftotext", str(pdf), "-")
            if "".join(extracted.split()) != "".join(document_text(saved).split()):
                raise AssertionError("PDF text differs from the saved DOCX")
            (output / "pdf-text.txt").write_text(extracted, encoding="utf-8")
            report["checks"].append("document focus restored after Save As; immediate keyboard PDF export and Poppler readback")
            keys("ctrl+e")
            capture("04-keyboard-after-export.png")
            keys("Escape")
            if source.read_bytes() != original:
                raise AssertionError("Save As changed the source document")
            report["checks"].append("export completion retains keyboard focus; cancellation and original bytes preserved")
            report["passed"] = True
    except Exception as error:
        report["error"] = str(error)
        if window is not None and process is not None and process.poll() is None:
            try:
                capture("failure.png")
            except (OSError, subprocess.SubprocessError):
                pass
        raise
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="New evidence directory; must not exist")
    args = parser.parse_args()
    run(args.binary.resolve(strict=True), args.output)
