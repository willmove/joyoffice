"""Shared launch, file and control helpers for isolated desktop regression runs."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import runpy
import select
import socket
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS / "joysheet"))
word_request = runpy.run_path(str(SCRIPTS / "joyword/verify-r8-rollback.py"))["request"]
SheetControl = runpy.run_path(str(SCRIPTS / "joysheet/smoke-native.py"))["Control"]



class DeckControl:
    """Use the desktop's single editor and guard each write against its current revision."""

    def __init__(self, proc):
        self.proc = proc
        self.serial = 0

    def request(self, method, **args):
        self.serial += 1
        packet = {"protocolVersion":"joydeck-control-1", "requestId":str(self.serial),
                  "method":method, "params":{}, **args}
        self.proc.stdin.write(json.dumps(packet) + "\n")
        self.proc.stdin.flush()
        if not select.select([self.proc.stdout], [], [], 5)[0]:
            raise TimeoutError(method)
        reply = json.loads(self.proc.stdout.readline())
        if reply.get("requestId") != str(self.serial) or "error" in reply:
            raise ValueError(str(reply))
        return reply["result"]

    def guard(self):
        state = self.request("document.status")
        return {"target":state["documentId"], "expectedRevision":state["revision"]}

    def inspect(self):
        return self.request("document.inspect", **self.guard())

    def edit(self, command):
        return self.request("command.execute", params=command, **self.guard())


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def wait(check, label, process):
    deadline = time.monotonic() + 15
    last_error = ""
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"application exited while waiting for {label}")
        try:
            value = check()
            if value:
                return value
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            last_error = str(error)
        time.sleep(.1)
    raise TimeoutError(f"{label}: {last_error}")


def port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


@contextmanager
def app(binary, args, out, env, log_name="desktop.log", unset=()):
    environment = {**os.environ, **env}
    for name in unset:
        environment.pop(name, None)
    with (out / log_name).open("w") as log:
        process = subprocess.Popen([binary, *args], env=environment, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=log, text=True, bufsize=1)
        try:
            yield process
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            process.stdin.close()
            process.stdout.close()
