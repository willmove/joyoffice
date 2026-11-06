#!/usr/bin/env python3
"""Exercise the Windows portable upgrade and rollback in an isolated directory."""
import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import time
from zipfile import ZipFile


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read_package(archive, destination):
    expected = archive.with_name(archive.name + ".sha256").read_text(encoding="ascii").split()[0]
    with archive.open("rb") as stream:
        fingerprint = hashlib.file_digest(stream, "sha256").hexdigest()
        if fingerprint != expected.lower():
            raise ValueError(f"Package checksum mismatch: {archive.name}")
        stream.seek(0)
        with ZipFile(stream) as package:
            with_targets = ("joyword.exe", "joyword-cli.exe")
            if any(package.namelist().count(name) != 1 for name in with_targets):
                raise ValueError("Package must contain exactly one GUI and one CLI binary")
            destination.mkdir()
            for name in with_targets:
                with package.open(name) as source, (destination / name).open("xb") as target:
                    shutil.copyfileobj(source, target)
    return fingerprint


@contextmanager
def candidate_in_place(install, staged, previous, retired):
    install.rename(previous)
    try:
        staged.rename(install)
    except OSError:
        previous.rename(install)
        raise
    try:
        yield
    finally:
        install.rename(retired)
        previous.rename(install)


def association():
    import winreg

    name = r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.docx\UserChoice"
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, name) as key:
            return winreg.QueryValueEx(key, "ProgId")[0]
    except FileNotFoundError:
        return None


def cli(bin_dir, env, *args):
    output = subprocess.check_output([bin_dir / "joyword-cli.exe", *args], cwd=bin_dir,
                                     env=env, encoding="utf-8", timeout=30)
    return json.loads(output)


def request(port, method, params=None, key=None):
    payload = {"id": 1, "method": method, "params": params or {}}
    if key is not None:
        payload["key"] = key
    with socket.create_connection(("127.0.0.1", port), timeout=3) as sock:
        sock.sendall(json.dumps(payload).encode() + b"\n")
        with sock.makefile("rb") as reader:
            reply = json.loads(reader.readline(16 << 20))
    require(reply["id"] == 1,
            'Control response has an unexpected request ID')
    return reply


def start(executable, env, *args):
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = 0
    return subprocess.Popen([executable, *args], cwd=executable.parent, env=env, startupinfo=startup)


def stop(process):
    if process.poll() is None:
        process.terminate()
    process.wait(timeout=10)


def verify_control(bin_dir, env, source):
    with socket.socket() as port_probe:
        port_probe.bind(("127.0.0.1", 0))
        port = port_probe.getsockname()[1]
    key = secrets.token_hex(32)
    env = {**env, "JOYWORD_CONTROL_KEY": key}
    app = start(bin_dir / "joyword.exe", env, "--control", str(port), "--", source)
    try:
        limit = time.monotonic() + 30
        while True:
            require(app.poll() is None,
                    f"JoyWord exited with {app.returncode}")
            try:
                inspect = request(port, "document.inspect", key=key)
                if inspect["ok"]:
                    break
            except OSError:
                pass
            if time.monotonic() >= limit:
                raise TimeoutError("Control never exposed an active document")
            time.sleep(.2)
        require(request(port, "document.inspect")["error"] == "unauthorized",
                'Control did not reject a missing key')
        require(request(port, "document.inspect", key="0" * 64)["error"] == "unauthorized",
                'Control did not reject an incorrect key')
        before = inspect["result"]
        capabilities = request(port, "capabilities", key=key)["result"]
        require(capabilities["versionRequired"] and not capabilities["fileAccess"],
                'Control version or file-access contract changed')
        if capabilities["uiAutomation"]:
            require(set(capabilities["uiCommands"]) == {"edit.goto", "view.zoom"},
                    'Unexpected UI command capabilities')
            target = {"doc": before["doc"], "version": before["version"]}
            for command, invalid, valid in [
                ("edit.goto", {"page": 0}, {"page": 1}),
                ("view.zoom", {"value": "onePage"}, {"value": 125}),
            ]:
                params = {**target, "command": command}
                require(not request(port, "engine.execute", {**params, "params": invalid}, key)["ok"],
                        'Control accepted invalid UI parameters')
                reply = request(port, "engine.execute", {**params, "params": valid}, key)
                require(reply["ok"],
                        reply)
                if command == "view.zoom":
                    require(reply["result"]["zoom"] == 1.25,
                            'Control returned an incorrect zoom')
        args = {"doc": before["doc"], "version": before["version"], "command": "text.insert",
                "params": {"text": "R8 控制验证 "}}
        require(not request(port, "engine.execute", {**args, "doc": before["doc"] + 1}, key)["ok"],
                'Control accepted the wrong document')
        require(not request(port, "engine.execute", {**args, "command": "file.open"}, key)["ok"],
                'Control accepted file access')
        require(request(port, "engine.execute", args, key)["ok"],
                'Control rejected a valid edit')
        require(not request(port, "engine.execute", args, key)["ok"],
                'Control accepted a stale document version')
        after = request(port, "document.inspect", key=key)["result"]
        require(after["text"].count("R8 控制验证 ") == 1 and after["version"] != before["version"],
                'Edit text or document version did not advance exactly once')
        mcp = {"jsonrpc": "2.0", "id": 1, "method": "tools/call",
               "params": {"name": "inspect_document", "arguments": {}}}
        output = subprocess.check_output(
            [bin_dir / "joyword-cli.exe", "mcp", "--connect", f"127.0.0.1:{port}"],
            input=json.dumps(mcp) + "\n", cwd=bin_dir, env=env, encoding="utf-8", timeout=15)
        response = json.loads(output)
        require(not response["result"].get("isError", False),
                'MCP returned an error')
        require("R8 控制验证 " in response["result"]["content"][0]["text"],
                'MCP did not read back the edit')
    finally:
        stop(app)


def verify(previous, candidate, fixtures, work):
    require(os.name == "nt", "Full rollback verification requires Windows")
    work.mkdir(parents=True)
    install = work / "install"
    previous_hash = read_package(previous, install)
    old_binaries = {name: digest(install / name) for name in ("joyword.exe", "joyword-cli.exe")}
    profile = work / "用户 配置"
    profile.mkdir()
    preferences = profile / "preferences.conf"
    preferences.write_text("ui.locale=en\nautosave.minutes=0\n", encoding="utf-8")
    recovery = profile / "recovery"
    shutil.copytree(fixtures / "rich", recovery)
    source = work / "源 文档.docx"
    shutil.copyfile(fixtures / "source.docx", source)
    original = {p: digest(p) for p in (source, preferences, *recovery.iterdir()) if p.is_file()}
    env = {**os.environ, "JOYWORD_CONFIG_DIR": str(profile), "JOYWORD_RECOVERY_DIR": str(recovery)}
    env.pop("JOYWORD_CONTROL_KEY", None)
    env.pop("JOYWORD_PROFILE_DIR", None)
    default_app = association()
    before = cli(install, env, "inspect", "--session", source)["text"]
    # The failed update is rejected before replacing the active payload.
    bad = work / "damaged.zip"
    bad.write_bytes(b"invalid candidate")
    bad.with_name("damaged.zip.sha256").write_text("0" * 64, encoding="ascii")
    try:
        read_package(bad, work / "failed-stage")
    except ValueError:
        require(all(digest(install / name) == sha for name, sha in old_binaries.items()),
                'Damaged package changed installed binaries')
    else:
        raise AssertionError("Damaged package was accepted")
    stage = work / "staged"
    candidate_hash = read_package(candidate, stage)
    require(previous_hash != candidate_hash,
            'Candidate must differ from previous package')
    previous_dir = work / "previous"
    retired = work / "candidate"
    # A disappeared stage fails after the live directory has been renamed.
    try:
        with candidate_in_place(install, work / "missing-stage", previous_dir, retired):
            raise AssertionError("Missing stage was promoted")
    except FileNotFoundError:
        require(all(digest(install / name) == sha for name, sha in old_binaries.items()),
                'Failed promotion did not restore installed binaries')
        require(not previous_dir.exists() and not retired.exists(),
                'Failed promotion left rollback directories')
    with candidate_in_place(install, stage, previous_dir, retired):
        verify_control(install, env, source)
        restored = work / "恢复 副本.docx"
        cli(install, env, "recover", recovery, "--save", restored)
        require(digest(restored) == digest(fixtures / "snapshot.docx"),
                'Recovered snapshot changed')
    require(all(digest(install / name) == sha for name, sha in old_binaries.items()),
            'Rollback did not restore installed binaries')
    require(cli(install, env, "inspect", "--session", source)["text"] == before,
            'Rolled-back CLI changed source text')
    require(cli(install, env, "inspect", "--session", restored)["pages"],
            'Rolled-back CLI could not reopen recovered document')
    app = start(install / "joyword.exe", env, "--", source)
    try:
        time.sleep(2)
        require(app.poll() is None,
                f"Rolled-back app exited with {app.returncode}")
    finally:
        stop(app)
    require(all(digest(p) == fingerprint for p, fingerprint in original.items()),
            'Original documents, preferences or recovery files changed')
    require(association() == default_app,
            'DOCX default application changed')
    return {"previous_sha256": previous_hash, "candidate_sha256": candidate_hash,
            "rollback_gui_started": True, "control_and_mcp": True,
            "promotion_failure_restored_previous": True,
            "originals_unchanged": len(original), "system_default_unchanged": True}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("previous", "candidate", "fixtures", "out"):
        parser.add_argument(name, type=lambda path: Path(path).resolve())
    args = parser.parse_args()
    print(json.dumps(verify(args.previous, args.candidate, args.fixtures, args.out)))
