#!/usr/bin/env python3
"""Exercise the Windows portable upgrade and rollback in an isolated directory."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import time
import winreg
from zipfile import ZipFile


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read_package(archive, destination):
    fingerprint = digest(archive)
    expected = archive.with_name(archive.name + ".sha256").read_text(encoding="ascii").split()[0]
    if fingerprint != expected.lower():
        raise ValueError(f"Package checksum mismatch: {archive.name}")
    with ZipFile(archive) as package:
        with_targets = ("joyword.exe", "joyword-cli.exe")
        if any(package.namelist().count(name) != 1 for name in with_targets):
            raise ValueError("Package must contain exactly one GUI and one CLI binary")
        destination.mkdir()
        for name in with_targets:
            with package.open(name) as source, (destination / name).open("xb") as target:
                shutil.copyfileobj(source, target)
    return fingerprint


def association():
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
    assert reply["id"] == 1
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
            assert app.poll() is None, f"JoyWord exited with {app.returncode}"
            try:
                inspect = request(port, "document.inspect", key=key)
                if inspect["ok"]:
                    break
            except OSError:
                pass
            if time.monotonic() >= limit:
                raise TimeoutError("Control never exposed an active document")
            time.sleep(.2)
        assert request(port, "document.inspect")["error"] == "unauthorized"
        assert request(port, "document.inspect", key="0" * 64)["error"] == "unauthorized"
        before = inspect["result"]
        capabilities = request(port, "capabilities", key=key)["result"]
        assert capabilities["versionRequired"] and not capabilities["fileAccess"]
        args = {"doc": before["doc"], "version": before["version"], "command": "text.insert",
                "params": {"text": "R8 控制验证 "}}
        assert not request(port, "engine.execute", {**args, "doc": before["doc"] + 1}, key)["ok"]
        assert not request(port, "engine.execute", {**args, "command": "file.open"}, key)["ok"]
        assert request(port, "engine.execute", args, key)["ok"]
        assert not request(port, "engine.execute", args, key)["ok"]
        after = request(port, "document.inspect", key=key)["result"]
        assert after["text"].count("R8 控制验证 ") == 1 and after["version"] != before["version"]
        mcp = {"jsonrpc": "2.0", "id": 1, "method": "tools/call",
               "params": {"name": "inspect_document", "arguments": {}}}
        output = subprocess.check_output(
            [bin_dir / "joyword-cli.exe", "mcp", "--connect", f"127.0.0.1:{port}"],
            input=json.dumps(mcp) + "\n", cwd=bin_dir, env=env, encoding="utf-8", timeout=15)
        response = json.loads(output)
        assert not response["result"].get("isError", False)
        assert "R8 控制验证 " in response["result"]["content"][0]["text"]
    finally:
        stop(app)


def verify(previous, candidate, fixtures, work):
    work.mkdir(parents=True)
    install = work / "install"
    previous_hash = read_package(previous, install)
    old_exe = digest(install / "joyword.exe")
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
        assert digest(install / "joyword.exe") == old_exe
    else:
        raise AssertionError("Damaged package was accepted")
    stage = work / "staged"
    candidate_hash = read_package(candidate, stage)
    assert previous_hash != candidate_hash
    saved = install.rename(work / "previous")
    stage.rename(install)
    try:
        verify_control(install, env, source)
        restored = work / "恢复 副本.docx"
        cli(install, env, "recover", recovery, "--save", restored)
        assert digest(restored) == digest(fixtures / "snapshot.docx")
    finally:
        install.rename(work / "candidate")
        saved.rename(install)
    assert digest(install / "joyword.exe") == old_exe
    assert cli(install, env, "inspect", "--session", source)["text"] == before
    assert cli(install, env, "inspect", "--session", restored)["pages"]
    app = start(install / "joyword.exe", env, "--", source)
    try:
        time.sleep(2)
        assert app.poll() is None, f"Rolled-back app exited with {app.returncode}"
    finally:
        stop(app)
    assert all(digest(p) == fingerprint for p, fingerprint in original.items())
    assert association() == default_app
    return {"previous_sha256": previous_hash, "candidate_sha256": candidate_hash,
            "rollback_gui_started": True, "control_and_mcp": True,
            "originals_unchanged": len(original), "system_default_unchanged": True}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("previous", "candidate", "fixtures", "out"):
        parser.add_argument(name, type=lambda path: Path(path).resolve())
    args = parser.parse_args()
    print(json.dumps(verify(args.previous, args.candidate, args.fixtures, args.out)))
