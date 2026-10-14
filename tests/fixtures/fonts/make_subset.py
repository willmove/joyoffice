#!/usr/bin/env python3
"""Rebuild the Latin test face from the pinned OFL Noto Serif source.

Requires fonttools==4.60.1 only when regenerating; normal tests need no Python
font libraries. Download/source identity is documented in provenance.json.
"""

import hashlib
import json
from pathlib import Path
import sys

import fontTools
from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: make_subset.py <pinned-source-NotoSerif.ttf>")
    if fontTools.__version__ != "4.60.1":
        raise SystemExit("Regeneration requires fonttools==4.60.1")
    directory = Path(__file__).resolve().parent
    source = Path(sys.argv[1])
    provenance = directory / "provenance.json"
    if provenance.exists():
        wanted = json.loads(provenance.read_text(encoding="utf-8"))["source_sha256"]
        if hashlib.sha256(source.read_bytes()).hexdigest() != wanted:
            raise SystemExit("Source font checksum mismatch")
    font = TTFont(source, recalcTimestamp=False)
    timestamp = font["head"].modified
    instantiateVariableFont(font, {"wght": 400, "wdth": 100}, inplace=True)
    options = subset.Options()
    options.layout_features = ["*"]
    options.name_IDs = [0, 1, 2, 3, 4, 5, 6, 13, 14, 16, 17]
    options.name_languages = ["*"]
    options.notdef_glyph = True
    options.notdef_outline = True
    options.recommended_glyphs = True
    options.recalc_timestamp = False
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(text="AVfi ")
    subsetter.subset(font)
    # This is a renamed test-only derivative, not a full Noto distribution.
    names = {
        1: "JoyOffice Test Serif", 2: "Regular",
        3: "JoyOfficeTestSerif-Regular-1", 4: "JoyOffice Test Serif Regular",
        6: "JoyOfficeTestSerif-Regular", 16: "JoyOffice Test Serif", 17: "Regular",
    }
    for record in font["name"].names:
        if record.nameID in names:
            record.string = names[record.nameID].encode(record.getEncoding())
    font["head"].modified = timestamp
    font.recalcTimestamp = False
    output = directory / "joyoffice-test-serif.ttf"
    font.save(output)
    print(output.name, output.stat().st_size, hashlib.sha256(output.read_bytes()).hexdigest())


if __name__ == "__main__":
    main()
