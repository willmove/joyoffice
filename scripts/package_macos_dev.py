#!/usr/bin/env python3
"""Wrap the existing debug executable for local UI validation; not a signed release."""
import sys
from pathlib import Path
from product_icons import development_bundle

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / "tools/xtask"))
from product import manifest_version
version = manifest_version("joyword")
binary = root / "target/debug/joyword"
if sys.platform != "darwin":
    sys.exit("This development wrapper is only for macOS.")
if not binary.is_file():
    sys.exit("Run cargo build -p joyword --features dev-runtime-shaders --locked first.")
print(development_bundle(binary, "joyword", version, root / "target/dev").parents[2])
