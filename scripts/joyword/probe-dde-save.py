"""Exercise cancel and retry on the UOS X11 native Save As panel using an isolated DOCX."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import secrets
import shutil
import socket
import subprocess
import time
import xml.etree.ElementTree as ET
import zipfile

request = runpy.run_path(str(Path(__file__).with_name("verify-r8-rollback.py")))["request"]


def xd(*args):
    return subprocess.check_output(["xdotool", *args], text=True, timeout=10).strip()


def visible(pid):
    result = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(pid)],
                            text=True, capture_output=True, timeout=5)
    return next(iter(result.stdout.splitlines()), None)


def active_dialog():
    try:
        active = xd("getactivewindow")
        pid = xd("getwindowpid", active)
        if Path(f"/proc/{pid}/comm").read_text().strip() == "dde-file-dialog":
            return active
    except (OSError, subprocess.CalledProcessError):
        return None
    return None


def wait(process, check, description, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"JoyWord exited {process.returncode}")
        value = check()
        if value:
            return value
        time.sleep(.1)
    raise TimeoutError(description)


def run(binary, output):
    if not os.environ.get("DISPLAY"):
        raise ValueError("run in the logged-in UOS X11 desktop session")
    output.mkdir(parents=True, exist_ok=False)
    fixture = Path(__file__).resolve().parents[2] / "tests/fixtures/docx/DOCX-MIN-001.docx"
    source = output / "source.docx"
    shutil.copyfile(fixture, source)
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    name = "重构 副本.docx"
    target = output / name
    marker = "JoyOffice 另存测试"
    report = {"passed": False, "sourceSha256": source_hash, "checks": []}
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    key = secrets.token_hex(32)
    env = dict(os.environ, JOYWORD_CONFIG_DIR=str(output / "profile"), JOYWORD_SAVE_TRACE="1",
               JOYWORD_CONTROL_KEY=key)
    log_path = output / "application.log"
    with log_path.open("w") as stream:
        process = subprocess.Popen([binary, "--control", str(port), "--", source], env=env,
                                   stdout=stream, stderr=subprocess.STDOUT)
        panel = None
        try:
            def control(method, **params):
                reply = request(port, method, params, key)
                if not reply["ok"]:
                    raise ValueError(reply.get("error", "control failed"))
                return reply["result"]

            def loaded():
                try:
                    return control("document.inspect")
                except (OSError, ValueError):
                    return None

            state = wait(process, loaded, "document not loaded")
            window = wait(process, lambda: visible(process.pid), "no JoyWord window")
            control("engine.execute", command="text.insert", doc=state["doc"], version=state["version"],
                    params={"text": marker})
            if marker not in control("document.inspect")["text"]:
                raise AssertionError("missing draft edit")

            def open_panel():
                xd("windowactivate", "--sync", window)
                if xd("getactivewindow") != window:
                    raise RuntimeError("JoyWord is not focused")
                xd("key", "--clearmodifiers", "F12")
                return wait(process, active_dialog, "DDE panel did not appear")

            def send(dialog, *args):
                if active_dialog() != dialog:
                    raise RuntimeError("native Save panel lost focus")
                xd(*args)

            def submit_name(dialog, filename, screenshot=False):
                # DDE initially selects the basename and may apply that selection
                # after presenting the window. Include the displayed suffix explicitly.
                time.sleep(.3)
                send(dialog, "key", "--clearmodifiers", "Home", "shift+End")
                # Let Qt consume each temporary X11 Unicode key mapping.
                for character in filename:
                    send(dialog, "type", "--clearmodifiers", character)
                    time.sleep(.12)
                if screenshot and shutil.which("import"):
                    subprocess.run(["import", "-window", dialog, str(output / "filename.png")], check=True, timeout=10)
                send(dialog, "key", "--clearmodifiers", "Return")

            panel = open_panel()
            send(panel, "key", "--clearmodifiers", "Escape")
            wait(process, lambda: "joyword-save picker-cancelled" in log_path.read_text(), "missing cancel callback")
            if marker not in control("document.inspect")["text"] or target.exists():
                raise AssertionError("cancel changed the draft or created a file")
            report["checks"].append("cancel preserves draft")

            panel = open_panel()
            rejected = "unsupported.txt"
            submit_name(panel, rejected)
            normalised = output / f"{rejected}.docx"
            wait(process, lambda: "joyword-save job-rejected" in log_path.read_text() or normalised.exists(),
                 "missing filename validation result")
            if marker not in wait(process, loaded, "document after filename validation")["text"] or (output / rejected).exists():
                raise AssertionError("filename validation lost the edit or wrote a disguised DOCX")
            if normalised.exists():
                report["checks"].append("DDE appended .docx to the supplied extension")
            else:
                report["checks"].append("unsupported extension preserves draft")

            panel = open_panel()
            submit_name(panel, name, screenshot=True)
            verified_count = 2 if normalised.exists() else 1
            wait(process, lambda: log_path.read_text().count("joyword-save readback-verified") == verified_count,
                 "native save did not complete", timeout=60)
            with zipfile.ZipFile(target) as archive:
                xml = ET.fromstring(archive.read("word/document.xml"))
            ns = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"
            if marker not in "".join(item.text or "" for item in xml.iter(ns + "t")):
                raise AssertionError("independent DOCX readback is missing the edit")
            if hashlib.sha256(source.read_bytes()).hexdigest() != source_hash:
                raise AssertionError("Save As modified the source")
            report["checks"] += ["edited DOCX independently read back", "source hash unchanged"]
            report["copySha256"] = hashlib.sha256(target.read_bytes()).hexdigest()
            report["passed"] = True
        except Exception as error:
            report["error"] = str(error)
            raise
        finally:
            try:
                if panel and active_dialog() == panel:
                    xd("key", "--clearmodifiers", "Escape")
            finally:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                report["trace"] = [line for line in log_path.read_text().splitlines() if line.startswith("joyword-save ")]
                (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    run(args.binary.resolve(), args.output.resolve())
