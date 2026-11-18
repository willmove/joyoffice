#!/usr/bin/env python3
"""Install a user's desktop launcher for a JoyOffice portable/development binary."""
import argparse
import os
from pathlib import Path
import shutil


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


def desktop_entry(product, executable=None):
    name, category = PRODUCTS[product]
    command = quote_exec(executable) if executable is not None else product
    if executable is not None and "%" in str(executable):
        # GIO checks the first Exec token before expanding %% field escapes.
        # Pass this unusual path as an argument to sh, without interpolating it
        # into shell code. exec preserves the application's process identity.
        command = f"/bin/sh -c {quote_exec('exec \"$@\"')} {product} {command}"
    return (f"[Desktop Entry]\nType=Application\nName={name}\nExec={command} %f\n"
            f"Icon={product}\nStartupWMClass={product}\nTerminal=false\nCategories=Office;{category};\n")


def install(product, binary, logo, data_home):
    binary, logo = Path(binary).resolve(strict=True), Path(logo).resolve(strict=True)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("Not an executable file: " + str(binary))
    entry = desktop_entry(product, binary)
    data_home = Path(data_home)
    icon = data_home / "icons/hicolor/scalable/apps" / f"{product}.svg"
    desktop = data_home / "applications" / f"{product}.desktop"
    icon.parent.mkdir(parents=True, exist_ok=True)
    desktop.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(logo, icon)
    desktop.write_text(entry, encoding="utf-8", newline="\n")
    return desktop


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", required=True, choices=PRODUCTS)
    parser.add_argument("--binary", type=Path, help="default: executable beside this script or in AppDir usr/bin")
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
        desktop = install(args.product, binary, logo, data_home)
    except (ValueError, OSError) as error:
        parser.exit(1, f"{error}\n")
    print(f"Installed {desktop}. Keep the application at {binary.resolve()}.")


if __name__ == "__main__":
    main()
