"""Run frozen and candidate Deck output on the same machine and independently read the results."""
import argparse
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import subprocess

from PIL import Image, ImageChops
from pypdf import PdfReader


ROOT = Path(__file__).resolve().parents[2]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(*args):
    subprocess.run(args, check=True, capture_output=True, timeout=90)


def raster(path):
    with Image.open(path) as image:
        rgb = image.convert("RGB")
        return {"size": rgb.size, "sha256": digest(rgb.tobytes())}


def pdf_readback(path):
    reader = PdfReader(path, strict=True)
    pages = []
    for page in reader.pages:
        fonts = page["/Resources"].get("/Font", {})
        if hasattr(fonts, "get_object"):
            fonts = fonts.get_object()
        embedded = {}
        for entry in fonts.values():
            font = entry.get_object()
            descendants = font.get("/DescendantFonts", [font])
            for child in descendants:
                child = child.get_object()
                descriptor = child.get("/FontDescriptor")
                assert descriptor, (path, child.get("/BaseFont"))
                descriptor = descriptor.get_object()
                stream = next(descriptor[k] for k in ("/FontFile", "/FontFile2", "/FontFile3") if k in descriptor)
                assert stream.get_object().get_data(), path
                # A subset name contains an implementation-specific six-letter prefix.
                name = str(child.get("/BaseFont", "")).split("+")[-1]
                embedded[name] = embedded.get(name, 0) + 1
        text = page.extract_text()
        assert not text.strip() or embedded, path
        links = []
        for reference in page.get("/Annots", []):
            link = reference.get_object()
            assert link["/Subtype"] == "/Link", path
            dest = link.get("/A")
            dest = dest if dest is None else dest.get_object()
            links.append((tuple(map(float, link["/Rect"])), str((dest or {}).get("/URI", ""))))
        pages.append({
            "size": [float(page.mediabox.width), float(page.mediabox.height)],
            "text": text,
            "fonts": embedded,
            "links": links,
        })
    prefix = path.parent / f"{path.stem}-raster"
    run("pdftoppm", "-r", "72", "-png", str(path), str(prefix))
    images = sorted(path.parent.glob(f"{prefix.name}-*.png"))
    assert len(images) == len(pages), path
    for row, image in zip(pages, images):
        row["raster"] = raster(image)
    outline = [(item.title, reader.get_destination_page_number(item)) for item in reader.outline]
    return {"pages": pages, "outline": outline}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frozen", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("metafiles", type=Path, help="Directory holding the frozen wmf.pptx and emf.pptx")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    decks = ROOT / "tests/fixtures/joydeck"
    delivery = decks / "r6-r7/delivery-media.pptx"
    independent = decks / "independent.pptx"
    cases = [
        ("slides", delivery, "slides", "3,1", 2),
        ("notes", delivery, "notes", "1,2,3", 3),
        ("handouts-3", delivery, "handouts-3", "1,2,3,1", 2),
        ("handouts-6", delivery, "handouts-6", "1,2,3,1,2,3,1", 2),
        ("mixed-content", independent, "slides", "1,2,3", 3),
        ("wmf", args.metafiles / "wmf.pptx", "slides", "2", 1),
        ("emf", args.metafiles / "emf.pptx", "slides", "2", 1),
    ]
    results = {}
    for build, cli in (("frozen", args.frozen), ("candidate", args.candidate)):
        folder = args.output / build
        folder.mkdir()
        results[build] = {}
        for name, path, layout, selection, expected in cases:
            output = folder / f"{name}.pdf"
            run(str(cli.resolve()), "export", str(path.resolve()), "--format", "pdf",
                "--dpi", "72", "--layout", layout, "--pages", selection, "--output", str(output.resolve()))
            data = pdf_readback(output)
            assert len(data["pages"]) == expected, (name, len(data["pages"]))
            assert len(data["outline"]) == len(selection.split(",")), name
            data["fixtureSha256"] = digest(path.read_bytes())
            if name in ("wmf", "emf"):
                pngs = folder / f"{name}-scene"
                run(str(cli.resolve()), "render", str(path.resolve()), "--page", "2", "--output", str(pngs.resolve()))
                image, = pngs.glob("*.png")
                with Image.open(image) as bitmap:
                    green = sum(amount for amount, rgb in bitmap.convert("RGB").getcolors(16_000_000) if rgb == (19, 177, 97))
                assert green > 10, name
                data.update({"scene": raster(image), "greenPixels": green})
            results[build][name] = data
    exact = deepcopy(results)
    footer_fixes = {}
    for case in ("handouts-3", "handouts-6"):
        old = exact["frozen"][case]["pages"][0]
        new = exact["candidate"][case]["pages"][0]
        assert old["text"].endswith("\n2") and new["text"].endswith("\n1"), case
        with Image.open(args.output / "frozen" / f"{case}-raster-1.png") as a, \
                Image.open(args.output / "candidate" / f"{case}-raster-1.png") as b:
            changed = ImageChops.difference(a.convert("RGB"), b.convert("RGB")).getbbox()
            assert changed and changed[0] >= b.width - 72 and changed[1] >= b.height - 44, changed
        footer_fixes[case] = {"firstPageBefore": 2, "firstPageAfter": 1, "changedPixelsWithin": changed}
        old["text"] = old["text"][:-1] + "1"
        old["raster"] = new["raster"]
    differences = [case for case in exact["candidate"] if exact["frozen"][case] != exact["candidate"][case]]
    assert any(page["fonts"] and page["text"].strip() for item in results["candidate"].values() for page in item["pages"])
    report = {
        "frozenCliSha256": digest(args.frozen.read_bytes()),
        "candidateCliSha256": digest(args.candidate.read_bytes()),
        "differentCases": differences,
        "intentionalFooterFixes": footer_fixes,
        "cases": results,
    }
    (args.output / "report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    assert not differences, differences
    print(f"{len(cases)} PDF comparisons, {sum(c[-1] for c in cases)} pages, "
          f"2 page footer fixes and 2 exact metafile scenes")


if __name__ == "__main__":
    main()
