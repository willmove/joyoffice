"""Independent public-format readback of the actual final-package desktop Save As."""
from pathlib import Path
import hashlib
import json
import shutil
import zipfile
from pptx import Presentation
from pptx.enum.dml import MSO_COLOR_TYPE, MSO_THEME_COLOR

OUT = Path(__file__).resolve().parent
runtime = json.loads((OUT/'package-runtime.json').read_text(encoding='utf-8'))
saved = Path(json.loads((OUT/'desktop-path.json').read_text(encoding='utf-8'))['path'])
source = Path(runtime['source'])
p = Presentation(saved)
assert len(p.slides) == 3
shape = next(s for s in p.slides[0].shapes if s.has_text_frame and s.text.startswith('JoyDeck'))
assert shape.text == 'JoyDeck · 真实文稿预览'
runs = list(shape.text_frame.paragraphs[0].runs)
assert runs[0].text == 'J'
assert runs[0].font.color.type == MSO_COLOR_TYPE.SCHEME
assert runs[0].font.color.theme_color == MSO_THEME_COLOR.ACCENT_2
assert all(r.font.name == 'Arial' and r.font.size.pt == 36 for r in runs)
assert all(r.font.color.type != MSO_COLOR_TYPE.SCHEME or r.font.color.theme_color != MSO_THEME_COLOR.ACCENT_2 for r in runs[1:])
with zipfile.ZipFile(source) as a, zipfile.ZipFile(saved) as b:
    assert set(a.namelist()) == set(b.namelist())
    changed = [n for n in a.namelist() if a.read(n) != b.read(n)]
    assert changed == ['ppt/slides/slide1.xml'], changed
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(source) == runtime['source_sha256']
(OUT/'artifacts').mkdir(exist_ok=True)
shutil.copy2(saved, OUT/'artifacts/desktop-formatted.pptx')
report = dict(path=str(saved), sha256=sha(saved), source_unchanged=True, pages=3,
              text=shape.text, font='Arial', size=36, theme_color='accent2',
              theme_color_range='first character J only', changed_parts=changed,
              runs=[dict(text=r.text, font=r.font.name, size=r.font.size.pt,
                         color_type=str(r.font.color.type)) for r in runs])
(OUT/'desktop-public-read.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print('Desktop PPTX independently verified: Arial / 36pt / first-character theme accent2 / source unchanged')
