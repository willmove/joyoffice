"""Rasterize the original file-preview geometry used by XLSX OLE pictures.

Matches crates/joysheet-xlsx/assets/embedded-file.svg; requires Pillow.
The filename is supplied separately as VML text, never baked into this asset.
"""
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[2]
scale = 4
image = Image.new("RGBA", (140 * scale, 100 * scale))
draw = ImageDraw.Draw(image)
points = [(50, 8), (78, 8), (90, 20), (90, 65), (50, 65), (50, 8)]
pixels = lambda p: [(x * scale, y * scale) for x, y in p]
draw.polygon(pixels(points), fill="#f6f8fa")
draw.line(pixels(points), fill="#176582", width=5, joint="curve")
for line in [[(78, 8), (78, 20), (90, 20)],
             [(58, 32), (82, 32)], [(58, 42), (82, 42)], [(58, 52), (82, 52)]]:
    draw.line(pixels(line), fill="#176582", width=5)
image.resize((140, 100), Image.Resampling.LANCZOS).save(
    root / "crates/joysheet-xlsx/assets/embedded-file.png", optimize=True)
