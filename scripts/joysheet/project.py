"""JoySheet's product tools, invoked by the common JoyOffice xtask runner."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
APPLICATIONS = ("joysheet", "joysheet-cli")


def run(command):
    print("+ " + " ".join(map(str, command)), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)


def host_target():
    output = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True, encoding="utf-8")
    return next(line.removeprefix("host: ") for line in output.splitlines() if line.startswith("host: "))


def build(profile="debug", validation=False):
    # Cargo's artifact messages honor target directories, executable suffixes and
    # explicit host selection; do not guess target/release paths.
    command = ["cargo", "build", "--locked", "--target", host_target(),
               "-p", "joysheet", "-p", "joysheet-cli", "--message-format=json-render-diagnostics"]
    if profile == "release":
        command.append("--release")
    if validation:
        command += ["--features", "joysheet/automation"]
    output = subprocess.check_output(command, cwd=ROOT, text=True, encoding="utf-8")
    artifacts = {}
    for line in output.splitlines():
        value = json.loads(line)
        if value.get("reason") == "compiler-artifact" and value.get("executable"):
            artifacts[value["target"]["name"]] = Path(value["executable"])
    if set(APPLICATIONS) - artifacts.keys():
        raise ValueError("Cargo did not report both Sheet executables")
    return {name: artifacts[name] for name in APPLICATIONS}


def helper(command, stage="M8"):
    args = ["cargo", "run", "--locked", "-p", "joysheet-tools", "--", command]
    if command == "smoke":
        args += ["--stage", stage]
    run(args)


def smoke(stage, engine_only=False):
    helper("smoke", stage)
    if engine_only:
        return
    if platform.system() != "Windows":
        raise ValueError("Native smoke currently requires Windows; use --engine-only")
    artifacts = build(validation=True)
    number = int(stage[1:])
    command = ["pwsh.exe", "-NoProfile", "-File", "scripts/joysheet/desktop-smoke.ps1",
               "-BinaryDirectory", str(artifacts["joysheet"].parent),
               "-Stage", "M3" if number >= 4 else stage]
    for milestone in range(4, 9):
        command += [f"-M{milestone}Task", "4" if number >= milestone else "0"]
    if number >= 8:
        command += ["-WindowsTask", "4", "-WindowsQualityTask", "4"]
    run(command)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["ci", "smoke", "inventory", "bench", "wasm"])
    parser.add_argument("--stage", choices=[f"M{n}" for n in range(9)], default="M8")
    parser.add_argument("--engine-only", action="store_true")
    args = parser.parse_args(argv)
    if args.command == "ci":
        run([sys.executable, "scripts/check_gates.py"])
        run([sys.executable, "scripts/check_docs.py"])
        run(["cargo", "xtask", "check", "joysheet", *(["--headless"] if args.engine_only else ["--validation"])])
    elif args.command == "smoke":
        smoke(args.stage, args.engine_only)
    elif args.command == "wasm":
        run(["cargo", "check", "--locked", "--target", "wasm32-unknown-unknown",
             "-p", "joysheet-engine", "-p", "joysheet-chart", "-p", "joysheet-pdf"])
    else:
        helper(args.command)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print("error: " + str(error), file=sys.stderr)
        sys.exit(1)
