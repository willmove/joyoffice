"""Stage the selected product's desktop icon and launcher resources."""
from pathlib import Path
import plistlib
import shutil
from install_linux_desktop import PRODUCTS as DESKTOP_ENTRIES, desktop_entry


BRAND = Path(__file__).resolve().parents[1] / "assets/brand"


def copy_logo(directory, product):
    logo = directory / "assets/brand" / f"{product}.svg"
    logo.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(BRAND / logo.name, logo)


def bundle_icon(contents, product):
    name = f"{product}.icns"
    resources = contents / "Resources"
    resources.mkdir(parents=True, exist_ok=True)
    shutil.copy2(BRAND / name, resources / name)
    return name


def linux_launcher(prefix, product):
    icon = prefix / "share/icons/hicolor/scalable/apps" / f"{product}.svg"
    icon.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(BRAND / icon.name, icon)
    desktop = prefix / "share/applications" / f"{product}.desktop"
    desktop.parent.mkdir(parents=True, exist_ok=True)
    desktop.write_text(desktop_entry(product), encoding="utf-8", newline="\n")
    return desktop, icon


def copy_linux_installer(directory):
    shutil.copy2(Path(__file__).with_name("install_linux_desktop.py"), directory / "install-desktop.py")


def development_bundle(binary, product, version, staging):
    """Run macOS development builds inside a bundle so Dock can read the ICNS."""
    name = DESKTOP_ENTRIES[product][0]
    contents = Path(staging) / f"{name}.app/Contents"
    executable = contents / "MacOS" / product
    executable.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(binary, executable)
    # Keep Deck's optional native helpers beside the copied development binary.
    for helper in Path(binary).parent.glob(f"{product}-*-macos"):
        if helper.is_file():
            shutil.copy2(helper, executable.parent / helper.name)
    identifier = "org.joysheet.desktop" if product == "joysheet" else f"dev.joyoffice.{product}"
    document = {"CFBundleTypeRole": "Editor", "LSHandlerRank": "Alternate"}
    if product == "joysheet":
        document.update(CFBundleTypeName="Spreadsheet workbook",
                        CFBundleTypeExtensions=["xlsx", "xls", "xlsb", "ods", "csv", "tsv"])
    else:
        document.update(CFBundleTypeName="Word document" if product == "joyword" else "PowerPoint presentation",
                        LSItemContentTypes=["org.openxmlformats." +
                            ("wordprocessingml.document" if product == "joyword" else "presentationml.presentation")])
    (contents / "Info.plist").write_bytes(plistlib.dumps({
        "CFBundleExecutable": product, "CFBundleIdentifier": identifier,
        "CFBundleName": name, "CFBundleDisplayName": name, "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version, "CFBundleVersion": version,
        "CFBundleIconFile": bundle_icon(contents, product), "NSHighResolutionCapable": True,
        "CFBundleDocumentTypes": [document],
    }))
    return executable
