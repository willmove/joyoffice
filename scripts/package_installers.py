#!/usr/bin/env python3
"""Shared macOS DMG, Linux DEB and portable archive builders for JoyOffice products."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import zipfile

_SCRIPTS_DIR = str(Path(__file__).resolve().parent)
if _SCRIPTS_DIR not in sys.path:
    sys.path.insert(0, _SCRIPTS_DIR)
from product_icons import linux_launcher

DEB_ARCH = {"x86_64": "amd64", "aarch64": "arm64"}

LINUX_DEB_DEPENDS = (
    "libc6 (>= 2.28)",
    "libgcc-s1 | libgcc1",
    "libxcb1",
    "libxkbcommon0",
    "libxkbcommon-x11-0",
    "libfontconfig1",
    "libvulkan1",
    "libwayland-client0",
    "libx11-xcb1",
    "xdg-utils",
)

PRODUCT_META = {
    "joyword": {
        "display_name": "JoyWord",
        "app_name": "JoyWord",
        "maintainer": "JoyOffice contributors",
        "description": "JoyWord desktop word processor",
        "recommends": (),
    },
    "joydeck": {
        "display_name": "JoyDeck",
        "app_name": "JoyDeck",
        "maintainer": "JoyOffice contributors",
        "description": "JoyDeck desktop presentation editor",
        "recommends": ("xclip | wl-clipboard",),
    },
    "joysheet": {
        "display_name": "JoySheet",
        "app_name": "Joysheet",
        "maintainer": "Joysheet contributors",
        "description": "JoySheet desktop spreadsheet editor",
        "recommends": ("poppler-utils",),
    },
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_sha256_sidecar(asset: Path) -> Path:
    sidecar = asset.with_name(asset.name + ".sha256")
    sidecar.write_text(f"{sha256_file(asset)}  {asset.name}\n", encoding="ascii")
    return sidecar


def deb_control(product: str, version: str, arch: str) -> str:
    if product not in PRODUCT_META:
        raise ValueError(f"Unknown product: {product}")
    if arch not in DEB_ARCH:
        raise ValueError(f"Unsupported Linux architecture: {arch}")
    meta = PRODUCT_META[product]
    lines = [
        f"Package: {product}",
        f"Version: {version}",
        f"Architecture: {DEB_ARCH[arch]}",
        f"Maintainer: {meta['maintainer']}",
        "Section: editors",
        "Priority: optional",
        "Homepage: https://github.com/willmove/joyoffice",
        f"Depends: {', '.join(LINUX_DEB_DEPENDS)}",
    ]
    if meta["recommends"]:
        lines.append(f"Recommends: {', '.join(meta['recommends'])}")
    lines.append(f"Description: {meta['description']}")
    return "\n".join(lines) + "\n"


def _is_executable(path: Path) -> bool:
    return bool(path.stat().st_mode & 0o111)


def _zip_info(arcname: str, executable: bool) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(arcname)
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = ((stat.S_IFREG | (0o755 if executable else 0o644)) << 16)
    return info


def create_zip_from_directory(directory: Path, archive: Path, *, root_prefix: str = "") -> Path:
    directory, archive = directory.resolve(), archive.resolve()
    temporary = archive.with_name(archive.name + ".partial")
    if archive.exists() or temporary.exists():
        raise FileExistsError(f"Existing archive is preserved: {archive}")
    archive.parent.mkdir(parents=True, exist_ok=True)
    prefix = root_prefix.rstrip("/") + "/" if root_prefix else ""
    try:
        with zipfile.ZipFile(temporary, "x", compression=zipfile.ZIP_DEFLATED, strict_timestamps=False) as bundle:
            for path in sorted(directory.rglob("*")):
                if path.is_symlink():
                    raise ValueError(f"Portable archive may not contain symlinks: {path}")
                if path.is_file():
                    arcname = prefix + path.relative_to(directory).as_posix()
                    with path.open("rb") as src, bundle.open(_zip_info(arcname, _is_executable(path)), "w") as dst:
                        shutil.copyfileobj(src, dst)
        temporary.rename(archive)
    except Exception:
        temporary.unlink(missing_ok=True)
        raise
    return archive


def create_tar_gz_from_directory(directory: Path, archive: Path, *, root_prefix: str = "") -> Path:
    directory, archive = directory.resolve(), archive.resolve()
    temporary = archive.with_name(archive.name + ".partial")
    if archive.exists() or temporary.exists():
        raise FileExistsError(f"Existing archive is preserved: {archive}")
    archive.parent.mkdir(parents=True, exist_ok=True)
    prefix = root_prefix.rstrip("/") + "/" if root_prefix else ""
    try:
        with tarfile.open(temporary, "w:gz") as bundle:
            for path in sorted(directory.rglob("*")):
                if path.is_symlink():
                    raise ValueError(f"Portable archive may not contain symlinks: {path}")
                if path.is_file():
                    arcname = prefix + path.relative_to(directory).as_posix()
                    info = bundle.gettarinfo(str(path), arcname=arcname)
                    info.mode = 0o755 if _is_executable(path) else 0o644
                    info.uid = info.gid = 0
                    info.uname = info.gname = ""
                    with path.open("rb") as src:
                        bundle.addfile(info, src)
        temporary.rename(archive)
    except Exception:
        temporary.unlink(missing_ok=True)
        raise
    return archive


def _default_run(command):
    subprocess.run(command, check=True)


def build_linux_deb(
    product: str,
    version: str,
    arch: str,
    output_deb: Path,
    *,
    binary: Path,
    cli_binary: Path,
    doc_entries: list[tuple[str, Path]] = (),
    staging_dir: Path | None = None,
    runner=None,
) -> Path:
    """Stage a Debian package layout and build an xz-compressed .deb for UOS 20/25 & Debian/Ubuntu."""
    output_deb = output_deb.resolve()
    if output_deb.exists() or output_deb.with_name(output_deb.name + ".sha256").exists():
        raise FileExistsError(f"Existing Debian package is preserved: {output_deb}")
    output_deb.parent.mkdir(parents=True, exist_ok=True)
    run_cmd = runner or _default_run

    def _stage_and_build(stage: Path) -> None:
        stage.mkdir(parents=True, exist_ok=True)
        bin_dir = stage / "usr/bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        for name, source in ((product, binary), (f"{product}-cli", cli_binary)):
            target = bin_dir / name
            shutil.copy2(source, target)
            target.chmod(0o755)
        linux_launcher(stage / "usr", product, system_install=True)
        doc_root = stage / "usr/share/doc" / product
        for rel, source in doc_entries:
            target = doc_root / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
            target.chmod(0o644)
        debian_dir = stage / "DEBIAN"
        debian_dir.mkdir(parents=True, exist_ok=True)
        debian_dir.chmod(0o755)
        control = debian_dir / "control"
        control.write_text(deb_control(product, version, arch), encoding="utf-8", newline="\n")
        control.chmod(0o644)
        # Use xz compression so older dpkg (e.g. UOS 20 / Debian 10 dpkg 1.19) can install it.
        run_cmd(["dpkg-deb", "-Zxz", "--build", "--root-owner-group", str(stage), str(output_deb)])

    if staging_dir is not None:
        _stage_and_build(staging_dir)
    else:
        with tempfile.TemporaryDirectory(prefix=f"{product}-deb-", dir=output_deb.parent) as temp:
            _stage_and_build(Path(temp) / "debian")

    if output_deb.is_file():
        write_sha256_sidecar(output_deb)
    return output_deb


def build_macos_dmg(
    product: str,
    app_bundle: Path,
    output_dmg: Path,
    *,
    sign_identity: str | None = None,
    notary_profile: str | None = None,
    runner=None,
) -> Path:
    """Codesign (ad-hoc by default) a macOS .app bundle and package it into a UDZO DMG."""
    if product not in PRODUCT_META:
        raise ValueError(f"Unknown product: {product}")
    output_dmg = output_dmg.resolve()
    if output_dmg.exists() or output_dmg.with_name(output_dmg.name + ".sha256").exists():
        raise FileExistsError(f"Existing DMG is preserved: {output_dmg}")
    output_dmg.parent.mkdir(parents=True, exist_ok=True)
    run_cmd = runner or _default_run

    env_prefix = product.upper()
    identity = (
        sign_identity
        or os.environ.get(f"{env_prefix}_MAC_SIGN_IDENTITY")
        or os.environ.get("JOYOFFICE_MAC_SIGN_IDENTITY")
        or "-"
    )
    notary = (
        notary_profile
        or os.environ.get(f"{env_prefix}_NOTARY_PROFILE")
        or os.environ.get("JOYOFFICE_NOTARY_PROFILE")
    )
    run_cmd([
        "codesign", "--force", "--deep", "--sign", identity,
        *(["--options", "runtime", "--timestamp"] if identity != "-" else []),
        str(app_bundle),
    ])
    volname = PRODUCT_META[product]["app_name"]
    with tempfile.TemporaryDirectory(prefix=f"{product}-dmg-", dir=output_dmg.parent) as temp:
        dmg_stage = Path(temp) / volname
        dmg_stage.mkdir()
        shutil.copytree(app_bundle, dmg_stage / app_bundle.name, symlinks=True)
        os.symlink("/Applications", dmg_stage / "Applications")
        run_cmd([
            "hdiutil", "create", "-ov", "-format", "UDZO",
            "-volname", volname, "-srcfolder", str(dmg_stage), str(output_dmg),
        ])
    if notary:
        run_cmd(["xcrun", "notarytool", "submit", str(output_dmg), "--keychain-profile", notary, "--wait"])
        run_cmd(["xcrun", "stapler", "staple", str(output_dmg)])
    if output_dmg.is_file():
        write_sha256_sidecar(output_dmg)
    return output_dmg
