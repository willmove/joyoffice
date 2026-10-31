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
import importlib.util
import json
import subprocess
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
    parser.add_argument("--profile", choices=["debug", "release"], default="release")
    parser.add_argument("--source-snapshot", type=Path, help="source hash map captured before the product build")
    parser.add_argument("--cli", type=Path, help="matching joyword-cli executable to include")
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
    root = Path(__file__).resolve().parents[1]
    spec = importlib.util.spec_from_file_location("products", root / "tools/xtask/product.py")
    products = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(products)
    if args.version != products.manifest_version("joyword", root):
        sys.exit("Package version must match apps/joyword/Cargo.toml")
    if args.cli and not args.cli.is_file():
        sys.exit("CLI binary not found: " + str(args.cli))
    with tempfile.TemporaryDirectory() as temp:
        staging = Path(temp)
        readme = staging / "README-joyword.txt"
        readme.write_text(README.format(version=args.version, platform=args.platform,
                                        arch=args.arch), encoding="utf-8")
        # A product package contains only its selected normal/build graph,
        # licenses and executable(s), even though the workspace lock is shared.
        script_dir = root / "scripts/joydeck"
        sys.path.insert(0, str(script_dir))
        try:
            import sbom
            rustc = subprocess.check_output(["rustc", "-vV"], cwd=root, text=True, encoding='utf-8')
            target = next(line.split(": ", 1)[1] for line in rustc.splitlines() if line.startswith("host:"))
            roots = ("joyword", "joyword-cli") if args.cli else ("joyword",)
            dependency = sbom.generate(staging / "SBOM.spdx.json", target, profile=args.profile, roots=roots)
        finally:
            sys.path.pop(0)
        hashes = dependency.pop("sourceHashes")
        if args.source_snapshot:
            sbom.require_unchanged_sources(json.loads(args.source_snapshot.read_text(encoding='utf-8')), hashes)
        hashes["scripts/package_release.py"] = sha256(Path(__file__))
        (staging / "SOURCE-HASHES.json").write_text(json.dumps(hashes, indent=2) + "\n", encoding='utf-8')
        (staging / "BUILD-INFO.json").write_text(json.dumps({"product": "joyword", "version": args.version,
            "target": target, "profile": args.profile, "packageRoots": list(roots),
            "binarySha256": sha256(binary), "cliSha256": sha256(args.cli) if args.cli else None, "cargoLockSha256": dependency["lockSha256"],
            "dependencyPackages": dependency["packages"], "sourceSnapshotVerified": args.source_snapshot is not None, "officialSignatureVerified": False}, indent=2) + "\n", encoding='utf-8')
        materials = [("README-joyword.txt", readme, False), ("LICENSE", root / "LICENSE", False)]
        materials.extend((p.relative_to(staging).as_posix(), p, False)
                         for p in sorted(staging.rglob("*"))
                         if p.is_file() and p != readme)
        if args.platform == "macos":
            bundle = app_bundle(binary, args.version, staging)
            if args.cli:
                shutil.copy2(args.cli, bundle / "Contents/MacOS/joyword-cli")
            entries = [(p.relative_to(staging).as_posix(), p, "MacOS" in p.parts)
                       for p in sorted(bundle.rglob("*")) if p.is_file()]
            make_zip(entries + materials, asset)
        elif args.platform == "windows":
            entries = [("joyword.exe", binary, True)]
            if args.cli:
                entries.append(("joyword-cli.exe", args.cli, True))
            make_zip(entries + materials, asset)
        else:
            entries = [("joyword", binary, True)]
            if args.cli:
                entries.append(("joyword-cli", args.cli, True))
            make_tar_gz(entries + materials, asset)
    sidecar = write_sidecar(asset)
    print(asset)
    print(sidecar)
    return 0


if __name__ == "__main__":
    sys.exit(main())
