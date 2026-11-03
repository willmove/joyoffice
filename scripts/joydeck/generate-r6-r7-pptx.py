"""Reproduce original public OOXML delivery corpus; copied legacy/media sources are ledgered separately."""
from pathlib import Path
import sys,zipfile
ROOT=Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'target/corpus-tools'))
from pptx import Presentation
from pptx.util import Inches,Pt
from pptx.dml.color import RGBColor
folder=ROOT/'tests/fixtures/joydeck/r6-r7'
p=Presentation();p.slide_width=Inches(10);p.slide_height=Inches(5.625)
for i in range(3):
    s=p.slides.add_slide(p.slide_layouts[6]);s.background.fill.solid();s.background.fill.fore_color.rgb=RGBColor.from_string(['EEF4FA','DFE9F5','FFF1DD'][i])
    b=s.shapes.add_textbox(Inches(.5),Inches(.4),Inches(9),Inches(1.1));b.text=f'R6 / R7 · 真实 PPTX {i+1}';b.text_frame.paragraphs[0].font.size=Pt(30);b.text_frame.paragraphs[0].font.name='Microsoft YaHei'
    b=s.shapes.add_textbox(Inches(.5),Inches(1.5),Inches(9),Inches(.7));b.text='中文 · e\u0301 · עברית · public OOXML';b.text_frame.paragraphs[0].font.size=Pt(19)
    s.notes_slide.notes_text_frame.text=f'PRIVATE SPEAKER NOTES {i+1} / 中文备注：仅演示者可见。'
    if i==0:
        run=s.shapes.add_textbox(Inches(.5),Inches(4.5),Inches(8),Inches(.5)).text_frame.paragraphs[0].add_run();run.text='Official project documentation';run.hyperlink.address='https://example.com/joydeck';run.font.size=Pt(18)
        s.shapes.add_movie(str(folder/'original-h264-aac.mp4'),Inches(3),Inches(2.5),Inches(4),Inches(1.6),poster_frame_image=str(ROOT/'tests/fixtures/joydeck/r4-r5/shapes.png'),mime_type='video/mp4')
        for ct in s._element.xpath('.//p:video/p:cMediaNode/p:cTn'):ct.set('repeatCount','indefinite')
    if i==1:s._element.set('show','0')
    if i==2:s.shapes.add_textbox(Inches(.5),Inches(3),Inches(9),Inches(1)).text='End / 结束（隐藏的第二页应被跳过）'
p.core_properties.author='JoyDeck original verification';p.core_properties.title='R6 R7 public interchange';p.save(folder/'delivery-media.pptx')
with zipfile.ZipFile(folder/'delivery-media.pptx','a',zipfile.ZIP_DEFLATED) as z:z.writestr('vendor/retain-r6.xml',b'<original>exact source payload</original>')
print(folder/'delivery-media.pptx')
