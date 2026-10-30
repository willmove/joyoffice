#!/usr/bin/env python3
"""Package the joyword release binary into a per-platform archive with a sha256 sidecar.

Runs on all three release runners (see .github/workflows/release.yml) and locally
after `cargo build -p joyword --release --locked`:

    python3 scripts/package_release.py \
        --binary target/release/joyword \
        --version 0.1.0 --platform macos --arch aarch64 --out dist

The archive name is the updater-facing contract: joyword-<version>-<platform>-<arch>.
Existing archives are refused rather than overwritten, matching the CLI convention.
"""
import argparse
import hashlib
import plistlib
import shutil
import stat
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

README = """JoyWord {version} ({platform}-{arch}) - technology preview build, unsigned
JoyWord {version}（{platform}-{arch}）—— 技术预览构建，未签名

Not a 1.0 product; DOCX declared subset only. MIT license.
不是 1.0 产品；仅支持已声明的 DOCX 子集。MIT 许可证。

Docs and system requirements:
https://github.com/willmove/joyoffice#readme
macOS: unsigned; right-click the app and choose Open to bypass Gatekeeper.
macOS：未签名；右键 App 选择「打开」以绕过 Gatekeeper。
Windows: unsigned; SmartScreen may require "More info -> Run anyway".
Windows：未签名；SmartScreen 可能需要「更多信息 → 仍要运行」。
Linux: GPUI system libraries (Vulkan/X11/Wayland stack) must be installed.
Linux：需预装 GPUI 系统依赖（Vulkan/X11/Wayland 等组件）。
"""


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--binary", required=True, type=Path,
                        help="release-built joyword binary to package")
    parser.add_argument("--version", required=True,
                        help="release version without the v prefix")
    parser.add_argument("--platform", required=True, choices=["macos", "windows", "linux"])
    parser.add_argument("--arch", required=True, choices=["x86_64", "aarch64"])
    parser.add_argument("--out", required=True, type=Path, help="output directory (dist/)")
    return parser.parse_args()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_sidecar(asset: Path) -> Path:
    sidecar = asset.with_name(asset.name + ".sha256")
    sidecar.write_text(f"{sha256(asset)}  {asset.name}\n", encoding="ascii")
    return sidecar


def zip_entry(name: str, executable: bool) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name)
    info.external_attr = ((stat.S_IFREG | (0o755 if executable else 0o644)) << 16)
    return info


def make_zip(entries, asset: Path):
    with zipfile.ZipFile(asset, "w", zipfile.ZIP_DEFLATED) as archive:
        for arcname, source, executable in entries:
            archive.writestr(zip_entry(arcname, executable), source.read_bytes())


def make_tar_gz(entries, asset: Path):
    with tarfile.open(asset, "w:gz") as archive:
        for arcname, source, executable in entries:
            info = archive.gettarinfo(str(source), arcname=arcname)
            info.mode = 0o755 if executable else 0o644
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            with source.open("rb") as handle:
                archive.addfile(info, handle)


def app_bundle(binary: Path, version: str, staging: Path) -> Path:
    bundle = staging / "JoyWord.app"
    executable = bundle / "Contents" / "MacOS" / "joyword"
    executable.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(binary, executable)
    with (bundle / "Contents" / "Info.plist").open("wb") as handle:
        plistlib.dump({
            "CFBundleExecutable": "joyword",
            "CFBundleIdentifier": "dev.joyoffice.joyword",
            "CFBundleName": "JoyWord",
            "CFBundleDisplayName": "JoyWord",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version,
            "CFBundleVersion": "1",
            "NSHighResolutionCapable": True,
        }, handle)
    return bundle


def main() -> int:
    args = parse_args()
    binary = args.binary
    if not binary.is_file():
        sys.exit(f"binary not found: {binary} (run cargo build -p joyword --release --locked)")
    extension = {"macos": "zip", "windows": "zip", "linux": "tar.gz"}[args.platform]
    asset = args.out / f"joyword-{args.version}-{args.platform}-{args.arch}.{extension}"
    if asset.exists():
        sys.exit(f"refusing to overwrite existing archive: {asset}")
    args.out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temp:
        staging = Path(temp)
        readme = staging / "README-joyword.txt"
        readme.write_text(README.format(version=args.version, platform=args.platform,
                                        arch=args.arch), encoding="utf-8")
        if args.platform == "macos":
            bundle = app_bundle(binary, args.version, staging)
            entries = [(f"JoyWord.app/Contents/MacOS/{item.name}", item, True)
                       for item in sorted((bundle / "Contents" / "MacOS").iterdir())]
            entries += [(f"JoyWord.app/Contents/{item.name}", item, False)
                        for item in sorted((bundle / "Contents").iterdir())
                        if item.name != "MacOS"]
            entries.append(("README-joyword.txt", readme, False))
            make_zip(entries, asset)
        elif args.platform == "windows":
            exe = staging / "joyword.exe"
            shutil.copy2(binary, exe)
            make_zip([("joyword.exe", exe, True), ("README-joyword.txt", readme, False)], asset)
        else:
            make_tar_gz([("joyword", binary, True), ("README-joyword.txt", readme, False)], asset)
    sidecar = write_sidecar(asset)
    print(asset)
    print(sidecar)
    return 0


if __name__ == "__main__":
    sys.exit(main())
