"""Native Sheet bundle adapters; signing remains controlled by the existing environment."""
from pathlib import Path
import os
import json
import platform
import plistlib
import shutil
import subprocess

from project import ROOT, run


def bundle(directory, version):
    system = platform.system()
    target = json.loads((directory / "BUILD.json").read_text(encoding="utf-8"))["target"]
    if system == "Windows":
        run(["pwsh.exe", "-NoProfile", "-File", "scripts/joysheet/package-windows.ps1",
             "-Directory", str(directory), "-Output", str(directory.parent / (directory.name + "-setup.exe"))])
    elif system == "Darwin":
        if target != "aarch64-apple-darwin":
            raise ValueError("Initial macOS distribution requires Apple Silicon")
        app = directory / "Joysheet.app/Contents"
        binaries = app / "MacOS"
        binaries.mkdir(parents=True)
        for name in ("joysheet", "joysheet-cli"):
            shutil.copy2(directory / name, binaries / name)
        resources = app / "Resources"
        resources.mkdir()
        for name in ("docs", "assets", "scripts", "dependency-licenses"):
            shutil.copytree(directory / name, resources / name)
        for name in ("BUILD.json", "BUILD-INFO.json", "SOURCE-HASHES.json", "SBOM.spdx.json", "LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "ATTRIBUTION.md"):
            shutil.copy2(directory / name, resources / name)
        shutil.copy2(ROOT / "assets/joysheet/joysheet.icns", resources / "joysheet.icns")
        with (app / "Info.plist").open("wb") as output:
            plistlib.dump({"CFBundleIdentifier": "org.joysheet.desktop", "CFBundleName": "Joysheet",
                          "CFBundleExecutable": "joysheet", "CFBundlePackageType": "APPL",
                          "CFBundleShortVersionString": version, "CFBundleVersion": version,
                          "LSMinimumSystemVersion": "15.0", "NSHighResolutionCapable": True,
                          "CFBundleIconFile": "joysheet.icns",
                          "CFBundleDocumentTypes": [{"CFBundleTypeName": "Spreadsheet workbook",
                              "CFBundleTypeRole": "Editor", "LSHandlerRank": "Alternate",
                              "CFBundleTypeExtensions": ["xlsx", "xls", "xlsb", "ods", "csv", "tsv"]}]}, output)
        identity = os.environ.get("JOYSHEET_MAC_SIGN_IDENTITY", "-")
        run(["codesign", "--force", "--deep", "--sign", identity,
             *(["--options", "runtime", "--timestamp"] if identity != "-" else []), str(app.parent)])
        image = directory.parent / (directory.name + ".dmg")
        run(["hdiutil", "create", "-ov", "-format", "UDZO", "-volname", "Joysheet", "-srcfolder", str(app.parent), str(image)])
        if os.environ.get("JOYSHEET_NOTARY_PROFILE"):
            run(["xcrun", "notarytool", "submit", str(image), "--keychain-profile", os.environ["JOYSHEET_NOTARY_PROFILE"], "--wait"])
            run(["xcrun", "stapler", "staple", str(image)])
    elif system == "Linux":
        app = directory / "Joysheet.AppDir"
        deb = directory / "debian"
        for stage in (app, deb):
            binaries = stage / "usr/bin"
            binaries.mkdir(parents=True)
            for name in ("joysheet", "joysheet-cli"):
                shutil.copy2(directory / name, binaries / name)
            shutil.copytree(directory / "dependency-licenses", stage / "usr/share/doc/joysheet/licenses")
            icon = stage / "usr/share/icons/hicolor/512x512/apps/joysheet.png"
            icon.parent.mkdir(parents=True)
            shutil.copy2(ROOT / "assets/joysheet/joysheet.png", icon)
            desktop = stage / "usr/share/applications/joysheet.desktop"
            desktop.parent.mkdir(parents=True)
            desktop.write_text("[Desktop Entry]\nType=Application\nName=Joysheet\nExec=joysheet %f\nIcon=joysheet\nTerminal=false\nCategories=Office;Spreadsheet;\n", encoding="utf-8")
        launcher = app / "AppRun"
        launcher.write_text('#!/bin/sh\nHERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexec "$HERE/usr/bin/joysheet" "$@"\n', encoding="utf-8")
        launcher.chmod(0o755)
        control = deb / "DEBIAN/control"
        control.parent.mkdir()
        architecture = {"x86_64": "amd64", "aarch64": "arm64"}[target.split("-", 1)[0]]
        control.write_text(f"Package: joysheet\nVersion: {version}\nArchitecture: {architecture}\nMaintainer: Joysheet contributors\nSection: editors\nPriority: optional\nDepends: libc6 (>= 2.39), libfontconfig1, libx11-xcb1, libxkbcommon-x11-0, libssl3t64, libvulkan1, libwayland-client0, libzstd1, poppler-utils, xdg-utils\nDescription: Joysheet GPUI spreadsheet desktop preview\n", encoding="utf-8")
        run(["dpkg-deb", "--build", "--root-owner-group", str(deb), str(directory.parent / (directory.name + ".deb"))])
        run(["tar", "-czf", str(directory.parent / (directory.name + "-appdir.tar.gz")), "-C", str(directory), "Joysheet.AppDir"])
    else:
        raise ValueError("Unsupported native platform: " + system)
