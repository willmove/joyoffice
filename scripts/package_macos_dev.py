#!/usr/bin/env python3
"""Wrap the existing debug executable for local UI validation; not a signed release."""
import sys
import re
from pathlib import Path
from package_release import app_bundle

root = Path(__file__).resolve().parents[1]
manifest = (root / "apps/joyword/Cargo.toml").read_text(encoding='utf-8')
section = re.search(r"(?ms)^\[package\]\s*(.*?)(?=^\[|\Z)", manifest)
match = re.search(r'^version\s*=\s*"([^"]+)"', section[1] if section else "", re.M)
if not match:
    sys.exit("JoyWord must declare an independent application version")
version = match[1]
binary = root / "target/debug/joyword"
if sys.platform != "darwin":
    sys.exit("This development wrapper is only for macOS.")
if not binary.is_file():
    sys.exit("Run cargo build -p joyword --features dev-runtime-shaders --locked first.")
print(app_bundle(binary, version, root / "target/dev"))
