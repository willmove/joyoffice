"""Build original CC0-1.0 font identity fixtures with fontTools 4.62.1."""

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTCollection
from fontTools.ttLib.tables.TupleVariation import TupleVariation

ROOT = Path(__file__).parent
FAMILY = "JoyDeck Font Environment Test"


def face(family, advance, edge):
    pen = TTGlyphPen(None)
    pen.moveTo((100, 0))
    pen.lineTo((100, 700))
    pen.lineTo((edge, 700))
    pen.lineTo((edge, 0))
    pen.closePath()
    fb = FontBuilder(1000, isTTF=True)
    fb.font["head"].created = fb.font["head"].modified = 3866054400
    fb.font.recalcTimestamp = False
    fb.setupGlyphOrder([".notdef", "A"])
    fb.setupCharacterMap({ord("A"): "A"})
    fb.setupGlyf({".notdef": TTGlyphPen(None).glyph(), "A": pen.glyph()})
    fb.setupHorizontalMetrics({".notdef": (500, 0), "A": (advance, 100)})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable({
        "familyName": family,
        "styleName": "Regular",
        "uniqueFontIdentifier": family,
        "fullName": family,
        "psName": family.replace(" ", "") + "-Regular",
    })
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
    fb.setupPost()
    return fb


def main():
    face(FAMILY, 620, 520).save(ROOT / "environment-narrow.ttf")
    face(FAMILY, 900, 800).save(ROOT / "environment-wide.ttf")
    variable = face(FAMILY + " Variable", 620, 520)
    variable.setupFvar([("wdth", 75, 100, 125, "Width"), ("wght", 100, 400, 900, "Weight")], [])
    variable.setupGvar({
        "A": [TupleVariation(
            {"wdth": (0, 1, 1)},
            [(0, 0), (0, 0), (160, 0), (160, 0), (0, 0), (160, 0), (0, 0), (0, 0)],
        )],
    })
    collection = TTCollection()
    collection.fonts = [face(FAMILY + " Collection", 620, 520).font, variable.font]
    collection.save(ROOT / "environment.ttc")


if __name__ == "__main__":
    main()
