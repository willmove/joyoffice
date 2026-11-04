"""Compile the owned AVFoundation helper for the same architecture as JoyDeck."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
ARCHITECTURES = {"aarch64-apple-darwin": "arm64", "x86_64-apple-darwin": "x86_64"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", choices=ARCHITECTURES)
    args = parser.parse_args()
    arch = ARCHITECTURES[args.target] if args.target else platform.machine()
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=args.output) as scratch:
        binary = Path(scratch) / "joydeck-video-macos"
        subprocess.run([
            "xcrun", "swiftc", "-O", "-target", f"{arch}-apple-macos12.0",
            ROOT / "crates/joydeck-shell/native/video-macos.swift",
            "-o", binary,
        ], check=True)
        binary.replace(args.output / binary.name)


if __name__ == "__main__":
    main()
