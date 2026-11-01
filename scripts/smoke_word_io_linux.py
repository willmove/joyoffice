"""T109 native X11 open/save race acceptance using real channel-gated workers.

Requires a Word validation build, JOYWORD_VALIDATION_IO_GATE opt-in (set here),
UOS/X11 at 125% desktop scale, xdotool and ImageMagick. Uses disposable files.
The gate controls timing only; parsing, snapshots, save IO and callbacks are real.
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


def make_source(path, prefix, paragraphs=1):
    body = "".join(f"<w:p><w:r><w:t>{prefix}_{i:04d} contract line</w:t></w:r></w:p>" for i in range(paragraphs))
    xml = f'<w:document xmlns:w="{W[1:-1]}"><w:body>{body}</w:body></w:document>'
    with zipfile.ZipFile(ROOT / "tests/fixtures/docx/DOCX-MIN-001.docx") as original, zipfile.ZipFile(path, "w") as copy:
        for name in original.namelist():
            copy.writestr(name, xml.encode() if name == "word/document.xml" else original.read(name))


def read_doc(path):
    with zipfile.ZipFile(path) as package:
        root = ET.fromstring(package.read("word/document.xml"))
    return {"text": "\n".join("".join(node.text or "" for node in paragraph.iter(W + "t"))
                             for paragraph in root.iter(W + "p")),
            "paragraphs": len(list(root.iter(W + "p"))), "bold": len(list(root.iter(W + "b")))}


def run(binary, output):
    if not os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"):
        raise RuntimeError("Requires unlocked native X11")
    for tool in ("xdotool", "import"):
        if not shutil.which(tool):
            raise RuntimeError(f"Missing tool: {tool}")
    output.mkdir(parents=True, exist_ok=False)
    initial, slow_a, slow_b, other = [output / name for name in ["initial.docx", "slow-a.docx", "slow-b.docx", "other.docx"]]
    for path, prefix, count in [(initial, "INITIAL", 1), (slow_a, "A", 2000), (slow_b, "B", 2000), (other, "C", 1)]:
        make_source(path, prefix, count)
    originals = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in [initial, slow_a, slow_b, other]}
    report = {"passed": False, "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "eventSource": "X11 OS synthetic keys/pointer and actual window pixels",
              "timingControl": "validation feature and explicit flag; channel gates on real production workers",
              "checks": [], "sourceHashes": originals, "paragraphsInLargeInput": 2000,
              "desktopScale": 1.25, "nativeImeVerified": False}
    process, window = None, None

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

    def key(*keys):
        command("xdotool", "key", "--clearmodifiers", *keys)
        time.sleep(.2)

    def text(value):
        command("xdotool", "type", "--clearmodifiers", "--delay", "10", value)
        time.sleep(.3)

    def capture(name):
        command("import", "-window", window, output / (name + ".png"))

    def release(newest=True):
        # Wait for acknowledgement of the actual click. A busy renderer may
        # paint the validation strip after the path sheet has disappeared.
        log_path = output / "application.log"
        before = log_path.read_text(encoding="utf-8").count("validation.io released")

        def click_acknowledged():
            if log_path.read_text(encoding="utf-8").count("validation.io released") > before:
                return True
            command("xdotool", "mousemove", "--window", window, 1400 if newest else 1240, 307, "click", "1")
            time.sleep(.2)
            return log_path.read_text(encoding="utf-8").count("validation.io released") > before

        wait(click_acknowledged, "native release button acknowledgement")
        time.sleep(.3)

    def open_path(path, dirty=False):
        key("ctrl+o")
        if dirty:
            command("xdotool", "mousemove", "--window", window, 965, 590, "click", "1")
            time.sleep(.3)  # Explicit Don't Save; Enter intentionally does not discard.
        key("ctrl+a")
        text(str(path))
        key("Return")
        time.sleep(.4)

    def start_copy(name):
        path = output / (name + ".docx")
        key("ctrl+shift+s")
        key("ctrl+a")
        text(str(path))
        key("Return")
        time.sleep(.3)
        assert not path.exists(), "The real save worker should still be paused before IO"
        return path

    try:
        environment = os.environ.copy()
        environment.update(JOYWORD_PROFILE="test", JOYWORD_PROFILE_DIR=str(output / "profile"),
                           JOYWORD_RECOVERY_DIR=str(output / "recovery"), JOYWORD_VALIDATION_IO_GATE="1")
        with (output / "application.log").open("w", encoding="utf-8") as log:
            process = subprocess.Popen([str(binary), str(initial)], cwd=ROOT, env=environment,
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
                return True

            wait(ready, "native window")
            command("xdotool", "windowactivate", "--sync", window)
            time.sleep(.8)
            geometry = command("xdotool", "getwindowgeometry", "--shell", window)
            assert "WIDTH=1500\nHEIGHT=1063" in geometry or "WIDTH=1500\nHEIGHT=1064" in geometry, geometry
            capture("00-startup-gate")
            release()
            wait(lambda: initial.name in command("xdotool", "getwindowname", window), "initial open")
            open_path(slow_a)
            text("BUF_A")
            key("ctrl+b")  # Modifying command must stay unavailable during open.
            capture("01-opening-readonly")
            key("Escape")
            cancelled = start_copy("01-cancelled-a")
            release()  # Newest is the copy; cancelled open A remains held.
            wait(cancelled.is_file, "cancel preserves buffered input on old document")
            cancelled_state = read_doc(cancelled)
            assert cancelled_state["text"].startswith("BUF_AINITIAL"), cancelled_state
            assert cancelled_state["bold"] == 0, cancelled_state
            report["checks"].append("large open held read-only; typing buffered; format refused; cancel keeps input on old document; original unchanged")

            open_path(slow_b, dirty=True)
            text("BUF_B")
            capture("02-two-opens")
            release(newest=False)  # Old A returns while newer B is still waiting.
            text("LATE_B")
            capture("03-old-result-new-open-still-pending")
            release()
            wait(lambda: slow_b.name in command("xdotool", "getwindowname", window), "newer B survives A's late completion")
            time.sleep(.5)
            report["checks"].append("cancelled A's late result cannot clear B's open gate or consume its buffered input")

            key("ctrl+Home")
            text("SAVE_N")
            saved_n = start_copy("02-save-n")
            text("PLUS")
            key("ctrl+w")
            capture("04-close-during-save")
            key("Escape")
            release()
            wait(saved_n.is_file, "save version N")
            state_n = read_doc(saved_n)
            assert state_n["text"].startswith("SAVE_NBUF_BLATE_BB_0000"), state_n["text"][:100]
            assert "PLUS" not in state_n["text"] and state_n["paragraphs"] == 2000
            capture("05-save-n-with-newer-edits")
            saved_later = start_copy("03-save-later")
            release()
            wait(saved_later.is_file, "save later version")
            state_later = read_doc(saved_later)
            assert state_later["text"].startswith("SAVE_NPLUSBUF_BLATE_BB_0000"), state_later["text"][:100]
            report["checks"].append("save IO paused after capture N; native typing continues to N+1; close asks and Cancel keeps document; output N excludes N+1; later copy includes it")

            late = start_copy("04-late-b-copy")
            open_path(other, dirty=True)
            release()  # Newest is open C; save B remains held.
            wait(lambda: other.name in command("xdotool", "getwindowname", window), "switch to C during B save")
            text("NEW_C")
            release(newest=False)
            wait(late.is_file, "late B copy")
            assert read_doc(late) == state_later
            assert other.name in command("xdotool", "getwindowname", window)
            capture("06-stale-save-new-session")
            saved_c = start_copy("05-current-c")
            release()
            wait(saved_c.is_file, "new C copy")
            assert read_doc(saved_c)["text"].startswith("NEW_CC_0000"), read_doc(saved_c)
            key("ctrl+w")
            capture("07-new-session-still-dirty")
            key("Escape")
            report["checks"].append("switch to C while B save is paused; late B output stays B and cannot change C's content/identity/unsaved choice")
            for path in [initial, slow_a, slow_b, other]:
                assert hashlib.sha256(path.read_bytes()).hexdigest() == originals[path.name]
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
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    run(arguments.binary.resolve(), arguments.output.resolve())
