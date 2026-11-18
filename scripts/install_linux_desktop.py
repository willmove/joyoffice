#!/usr/bin/env python3
"""Install a user's desktop launcher for a JoyOffice portable/development binary."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess


PRODUCTS = {
    "joyword": ("JoyWord", "WordProcessor"),
    "joydeck": ("JoyDeck", "Presentation"),
    "joysheet": ("JoySheet", "Spreadsheet"),
}


def quote_exec(path):
    # Desktop Entry Exec quoting is followed by the general string escaping.
    # Escape field codes as well: a percent sign can occur in a real path.
    value = str(path)
    if any(ord(char) < 32 for char in value):
        raise ValueError("Desktop executable path cannot contain control characters")
    value = value.replace("%", "%%")
    for char in ('\\', '"', '`', '$'):
        value = value.replace(char, "\\" + char)
    return '"' + value.replace("\\", "\\\\") + '"'


def icon_value(icon):
    value = str(icon)
    if any(ord(char) < 32 for char in value):
        raise ValueError("Desktop icon path cannot contain control characters")
    # Icon is an iconstring, not an Exec argument: no quotes or %% expansion.
    return value.replace("\\", "\\\\")


def desktop_entry(product, executable=None, icon=None):
    name, category = PRODUCTS[product]
    command = quote_exec(executable) if executable is not None else product
    if executable is not None and "%" in str(executable):
        # GIO checks the first Exec token before expanding %% field escapes.
        # Pass this unusual path as an argument to sh, without interpolating it
        # into shell code. exec preserves the application's process identity.
        command = f"/bin/sh -c {quote_exec('exec \"$@\"')} {product} {command}"
    return (f"[Desktop Entry]\nType=Application\nName={name}\nExec={command} %f\n"
            f"Icon={icon_value(icon if icon is not None else product)}\n"
            f"StartupWMClass={product}\nTerminal=false\nCategories=Office;{category};\n")


def icon_destination(product, data_home):
    return Path(data_home).resolve() / "icons/hicolor/scalable/apps" / f"{product}.svg"


def copy_icon(logo, icon):
    icon.parent.mkdir(parents=True, exist_ok=True)
    if logo != icon.resolve():
        shutil.copy2(logo, icon)


def install(product, binary, logo, data_home):
    # Preserve symlink invocation names: a wrapper may dispatch on argv[0].
    binary, logo = Path(os.path.abspath(binary)), Path(logo).resolve(strict=True)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("Not an executable file: " + str(binary))
    data_home = Path(data_home).resolve()
    icon = icon_destination(product, data_home)
    entry = desktop_entry(product, binary, icon)
    desktop = data_home / "applications" / f"{product}.desktop"
    desktop.parent.mkdir(parents=True, exist_ok=True)
    copy_icon(logo, icon)
    desktop.write_text(entry, encoding="utf-8", newline="\n")
    return desktop


def refresh_icon(product, logo, data_home):
    """Repair an existing launcher's icon without changing its custom command."""
    desktop = Path(data_home).resolve() / "applications" / f"{product}.desktop"
    lines = desktop.read_bytes().decode("utf-8").splitlines(keepends=True)
    icon = icon_destination(product, data_home)
    value = icon_value(icon)
    section = None
    active = replaced = False
    for index, line in enumerate(lines):
        if line.strip().startswith("["):
            active = line.strip() == "[Desktop Entry]"
            if active:
                section = index
        elif active and line.partition("=")[0].strip() == "Icon":
            newline = "\r\n" if line.endswith("\r\n") else "\n"
            lines[index] = f"Icon={value}{newline}"
            replaced = True
    if section is None:
        raise ValueError("Missing [Desktop Entry]: " + str(desktop))
    if not replaced:
        newline = "\r\n" if lines[section].endswith("\r\n") else "\n"
        if not lines[section].endswith("\n"):
            lines[section] += newline
        lines.insert(section + 1, f"Icon={value}{newline}")
    copy_icon(Path(logo).resolve(strict=True), icon)
    desktop.write_bytes("".join(lines).encode("utf-8"))
    return desktop


def notify_desktop():
    # DDE 25 does not reliably reload user launchers after a file-only install.
    # Do not start a desktop service or require DDE on other Linux desktops.
    if shutil.which("busctl"):
        try:
            subprocess.run(["busctl", "--user", "--auto-start=no", "--timeout=2", "call",
                            "org.desktopspec.ApplicationManager1", "/org/desktopspec/ApplicationManager1",
                            "org.desktopspec.ApplicationManager1", "ReloadApplications"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3, check=False)
        except (OSError, subprocess.TimeoutExpired):
            pass  # The files remain usable without this optional notification.


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", required=True, choices=PRODUCTS)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--binary", type=Path, help="default: executable beside this script or in AppDir usr/bin")
    mode.add_argument("--refresh-icon", action="store_true", help="repair an existing launcher, preserving its command")
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    binary = args.binary if args.binary is not None else here / args.product
    if args.binary is None and not binary.is_file():
        binary = here / "usr/bin" / args.product
    logo = here / "assets/brand" / f"{args.product}.svg"
    if not logo.is_file():
        logo = here / f"{args.product}.svg"
    if not logo.is_file():
        logo = here.parent / "assets/brand" / f"{args.product}.svg"
    data_home = os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share"
    try:
        desktop = (refresh_icon(args.product, logo, data_home) if args.refresh_icon
                   else install(args.product, binary, logo, data_home))
    except (ValueError, OSError) as error:
        parser.exit(1, f"{error}\n")
    notify_desktop()
    print(f"Updated {desktop}. Close and reopen existing application windows to refresh the taskbar icon.")
    if not args.refresh_icon:
        print(f"Keep the application at {Path(os.path.abspath(binary))}.")


if __name__ == "__main__":
    main()
