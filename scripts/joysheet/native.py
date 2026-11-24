"""Native Sheet bundle adapters; signing remains controlled by the existing environment."""
from pathlib import Path
import os
import json
import platform
import plistlib
import shutil
import subprocess
import sys

from project import ROOT, run
if str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))
from package_installers import build_macos_dmg, deb_control
from product_icons import bundle_icon, copy_linux_installer, linux_launcher


def windows_installer(directory, *, prepare_only=False):
    run(["pwsh.exe", "-NoProfile", "-File", "scripts/joysheet/package-windows.ps1",
         "-Directory", str(directory), "-Output", str(directory.parent / (directory.name + "-setup.exe")),
         *(["-PrepareOnly"] if prepare_only else [])])


def bundle(directory, version):
    system = platform.system()
    target = json.loads((directory / "BUILD.json").read_text(encoding="utf-8"))["target"]
    if system == "Windows":
        windows_installer(directory, prepare_only=True)
    elif system == "Darwin":
        app = directory / "Joysheet.app/Contents"
        binaries = app / "MacOS"
        binaries.mkdir(parents=True)
        for name in ("joysheet", "joysheet-cli"):
            shutil.copy2(directory / name, binaries / name)
        resources = app / "Resources"
        resources.mkdir()
        for name in ("docs", "assets", "scripts", "dependency-licenses"):
            shutil.copytree(directory / name, resources / name)
        for name in ("BUILD.json", "BUILD-INFO.json", "SOURCE-HASHES.json", "SBOM.spdx.json", "LICENSE-MIT", "LICENSE-APACHE", "LICENSE-JOYOFFICE", "NOTICE", "ATTRIBUTION.md"):
            shutil.copy2(directory / name, resources / name)
        with (app / "Info.plist").open("wb") as output:
            plistlib.dump({"CFBundleIdentifier": "org.joysheet.desktop", "CFBundleName": "Joysheet",
                          "CFBundleExecutable": "joysheet", "CFBundlePackageType": "APPL",
                          "CFBundleShortVersionString": version, "CFBundleVersion": version,
                          "LSMinimumSystemVersion": "15.0", "NSHighResolutionCapable": True,
                          "CFBundleIconFile": bundle_icon(app, "joysheet"),
                          "CFBundleDocumentTypes": [{"CFBundleTypeName": "Spreadsheet workbook",
                              "CFBundleTypeRole": "Editor", "LSHandlerRank": "Alternate",
                              "CFBundleTypeExtensions": ["xlsx", "xls", "xlsb", "ods", "csv", "tsv"]}]}, output)
        image = directory.parent / (directory.name + ".dmg")
        build_macos_dmg("joysheet", app.parent, image, runner=run)
    elif system == "Linux":
        linux_launcher(directory, "joysheet")
        copy_linux_installer(directory)
        app = directory / "Joysheet.AppDir"
        deb = directory / "debian"
        for stage in (app, deb):
            binaries = stage / "usr/bin"
            binaries.mkdir(parents=True)
            for name in ("joysheet", "joysheet-cli"):
                shutil.copy2(directory / name, binaries / name)
                (binaries / name).chmod(0o755)
            licenses = stage / "usr/share/doc/joysheet/licenses"
            shutil.copytree(directory / "dependency-licenses", licenses)
            shutil.copy2(directory / "LICENSE-JOYOFFICE", licenses / "LICENSE-JOYOFFICE")
            for meta in ("LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "ATTRIBUTION.md",
                         "SBOM.spdx.json", "BUILD-INFO.json", "SOURCE-HASHES.json"):
                if (directory / meta).is_file():
                    shutil.copy2(directory / meta, stage / "usr/share/doc/joysheet" / meta)
            desktop, icon = linux_launcher(stage / "usr", "joysheet", system_install=stage == deb)
            if stage == app:
                shutil.copy2(desktop, app / desktop.name)
                shutil.copy2(icon, app / icon.name)
                copy_linux_installer(app)
        launcher = app / "AppRun"
        launcher.write_text('#!/bin/sh\nHERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexec "$HERE/usr/bin/joysheet" "$@"\n', encoding="utf-8")
        launcher.chmod(0o755)
        control = deb / "DEBIAN/control"
        control.parent.mkdir()
        control.parent.chmod(0o755)
        arch = target.split("-", 1)[0]
        control.write_text(deb_control("joysheet", version, arch), encoding="utf-8", newline="\n")
        control.chmod(0o644)
        run(["dpkg-deb", "-Zxz", "--build", "--root-owner-group", str(deb), str(directory.parent / (directory.name + ".deb"))])
        run(["tar", "-czf", str(directory.parent / (directory.name + "-appdir.tar.gz")), "-C", str(directory), "Joysheet.AppDir"])
    else:
        raise ValueError("Unsupported native platform: " + system)
