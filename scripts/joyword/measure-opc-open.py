"""Compare session imports of the same DOCX in alternating fresh CLI processes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess
import sys
import tempfile
import time
import zipfile

ROOT = Path(__file__).resolve().parents[2]


def measured(binary, doc):
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        start = time.perf_counter()
        with subprocess.Popen([binary, "text", "--session", "--", doc], stdout=stdout, stderr=stderr) as child:
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
        elapsed = (time.perf_counter() - start) * 1000
        if child.returncode:
            stderr.seek(0)
            raise RuntimeError(stderr.read().decode())
        stdout.seek(0)
        return {
            "ms": round(elapsed, 3),
            "rssMiB": round(usage.ru_maxrss / (1048576 if sys.platform == "darwin" else 1024), 3),
            "textSha256": hashlib.sha256(stdout.read()).hexdigest(),
        }


def add_parts(source, target):
    rng = random.Random(731)
    with zipfile.ZipFile(source) as old, zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as new:
        for entry in old.infolist():
            data = old.read(entry)
            if entry.filename == "[Content_Types].xml":
                data = data.replace(b"</Types>", b'<Default Extension="opaque" ContentType="application/octet-stream"/></Types>')
            new.writestr(entry, data)
        for i in range(12):
            chunk = rng.randbytes(256 * 1024)
            info = zipfile.ZipInfo(f"vendor/part{i}.opaque", (2020, 1, 1, 0, 0, 0))
            new.writestr(info, chunk + bytes(768 * 1024), compress_type=zipfile.ZIP_DEFLATED)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("baseline", "candidate", "output"):
        parser.add_argument(arg, type=Path)
    args = parser.parse_args()
    samples = ROOT / "assets/samples"
    with tempfile.TemporaryDirectory() as tmp:
        large = Path(tmp) / "opaque-12MiB.docx"
        add_parts(samples / "letter.docx", large)
        report = {"platform": platform.platform(), "build": "dev", "cases": {}}
        for path in (samples / "mixed-media.docx", samples / "handbook.docx", large):
            case = {"baseline": [], "candidate": []}
            original = hashlib.sha256(path.read_bytes()).hexdigest()
            order = [("baseline", args.baseline.resolve()), ("candidate", args.candidate.resolve())]
            for _, binary in order:
                measured(binary, path)
            for pair in range(6):
                for key, binary in (reversed(order) if pair % 2 else order):
                    case[key].append(measured(binary, path))
            assert len({s["textSha256"] for key, _ in order for s in case[key]}) == 1
            assert hashlib.sha256(path.read_bytes()).hexdigest() == original
            with zipfile.ZipFile(path) as archive:
                case["expandedBytes"] = sum(i.file_size for i in archive.infolist())
            case["sourceSha256"] = original
            case["summary"] = {key: {"medianMs": round(statistics.median(s["ms"] for s in case[key]), 3),
                                      "maxRssMiB": max(s["rssMiB"] for s in case[key])} for key, _ in order}
            report["cases"][path.name] = case
        args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
