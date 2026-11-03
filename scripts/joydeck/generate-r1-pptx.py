"""Independent OOXML fixture producer; run with python-pptx 1.0.2 in target/corpus-tools.
Original content under this repository's MIT license. No Office installation required.
"""
from pathlib import Path
import sys
from io import BytesIO

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'target/corpus-tools'))
from pptx import Presentation
from pptx.util import Inches, Pt
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_SHAPE
from PIL import Image, ImageDraw

destination = ROOT / 'tests/fixtures/joydeck/independent.pptx'
p = Presentation()
p.slide_width, p.slide_height = Inches(13.333333), Inches(7.5)
for index, title in enumerate(['Independent PPTX / 独立样本', 'Raster, table & geometry', 'Portrait-independent navigation']):
    slide = p.slides.add_slide(p.slide_layouts[6])
    def text(value, x, y, width, height, size):
        box = slide.shapes.add_textbox(Inches(x), Inches(y), Inches(width), Inches(height))
        paragraph = box.text_frame.paragraphs[0]
        paragraph.text = value
        paragraph.font.name = 'Microsoft YaHei'
        paragraph.font.size = Pt(size)
        paragraph.font.color.rgb = RGBColor.from_string('26352D')
    text(title, .65, .7, 12, 1, 34)
    text('Made with python-pptx 1.0.2 · actual public OOXML', .7, 1.8, 12, .7, 19)
    shape = slide.shapes.add_shape(MSO_SHAPE.ROUNDED_RECTANGLE, Inches(.7), Inches(3), Inches(3.3), Inches(1.7))
    shape.fill.solid(); shape.fill.fore_color.rgb = RGBColor.from_string('226F4C')
    shape.line.color.rgb = RGBColor.from_string('226F4C')
    if index == 1:
        image = Image.new('RGB', (180, 120), '#E8C170'); draw = ImageDraw.Draw(image)
        draw.ellipse((20, 20, 100, 100), fill='#315FA8'); draw.rectangle((110, 25, 160, 95), fill='#226F4C')
        data = BytesIO(); image.save(data, format='PNG'); data.seek(0)
        slide.shapes.add_picture(data, Inches(4.5), Inches(3), Inches(2.4), Inches(1.6))
        table = slide.shapes.add_table(3, 2, Inches(7.4), Inches(3), Inches(4.5), Inches(2)).table
        for r in range(3):
            for c in range(2): table.cell(r,c).text = f'Cell {r+1}/{c+1}'
    text(f'JOYOFFICE / JOYDECK R1                         {index+1:02}', .7, 6.5, 11, .6, 16)
    slide.notes_slide.notes_text_frame.text = f'Original independent speaker notes / 备注 {index+1}'
p.save(destination)
print(destination)
