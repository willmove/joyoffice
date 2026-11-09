"""Stage the selected product's desktop icon and launcher resources."""
from pathlib import Path
import shutil


BRAND = Path(__file__).resolve().parents[1] / "assets/brand"
DESKTOP_ENTRIES = {
    "joyword": ("JoyWord", "WordProcessor"),
    "joydeck": ("JoyDeck", "Presentation"),
    "joysheet": ("JoySheet", "Spreadsheet"),
}


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
    name, category = DESKTOP_ENTRIES[product]
    icon = prefix / "share/icons/hicolor/scalable/apps" / f"{product}.svg"
    icon.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(BRAND / icon.name, icon)
    desktop = prefix / "share/applications" / f"{product}.desktop"
    desktop.parent.mkdir(parents=True, exist_ok=True)
    desktop.write_text(
        f"[Desktop Entry]\nType=Application\nName={name}\nExec={product} %f\n"
        f"Icon={product}\nStartupWMClass={product}\nTerminal=false\nCategories=Office;{category};\n",
        encoding="utf-8", newline="\n",
    )
    return desktop, icon
