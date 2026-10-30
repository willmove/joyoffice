#!/usr/bin/env python3
"""Wrap the existing debug executable for local UI validation; not a signed release."""
import plistlib
import shutil
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
binary = root / "target/debug/joyword"
bundle = root / "target/dev/JoyWord.app"
if sys.platform != "darwin":
    sys.exit("This development wrapper is only for macOS.")
if not binary.is_file():
    sys.exit("Run cargo build -p joyword --features dev-runtime-shaders --locked first.")
executable = bundle / "Contents/MacOS/joyword"
executable.parent.mkdir(parents=True, exist_ok=True)
shutil.copy2(binary, executable)
with (bundle / "Contents/Info.plist").open("wb") as handle:
    plistlib.dump({
        "CFBundleExecutable": "joyword",
        "CFBundleIdentifier": "dev.joyoffice.joyword",
        "CFBundleName": "JoyWord",
        "CFBundleDisplayName": "JoyWord",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": "0.1.0",
        "CFBundleVersion": "1",
        "NSHighResolutionCapable": True,
    }, handle)
print(bundle)
