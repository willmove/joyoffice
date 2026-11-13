from pathlib import Path
import sys
sys.path.insert(0,str(Path('target/corpus-tools').resolve()))
from pptx import Presentation
from pptx.util import Inches,Pt
from pptx.dml.color import RGBColor
from pptx.oxml.xmlchemy import OxmlElement
p=Presentation();p.slide_width=Inches(13.333333);p.slide_height=Inches(7.5)
for i in range(2):
 s=p.slides.add_slide(p.slide_layouts[6])
 for y,size,text in [(0.7,36,'JoyDeck DPI / 放映清晰度'),(1.7,22,'Native pixels · 矢量文字与细线'),(2.6,16,'中文测试：办公演示 0123456789 ABCDEFG abcdefg'),(3.3,12,'12 pt: 小字号、标点、边缘 / Small text, crisp strokes'),(4.0,10,'10 pt: 显示比例改变后应重新生成画面 — DPI 100% / 150% / 200%')]:
  box=s.shapes.add_textbox(Inches(.8), Inches(y), Inches(11.8), Inches(.7))
  para=box.text_frame.paragraphs[0];para.text=text
  para.font.name='Microsoft YaHei';para.font.size=Pt(size);para.font.color.rgb=RGBColor(24,36,48)
 for j in range(8):
  shape=s.shapes.add_shape(1,Inches(.9+j*1.45),Inches(5.2),Inches(1.1), Inches(.65))
  shape.fill.solid();shape.fill.fore_color.rgb=RGBColor(230,240,250);shape.line.width=Pt(.5);shape.line.color.rgb=RGBColor(40,80,120)
 if i:
  tr=OxmlElement('p:transition');tr.set('dur','2000');tr.set('spd','slow');tr.append(OxmlElement('p:fade'));s._element.append(tr)
p.save('target/joydeck-dpi-check/clarity.pptx')
print('Created two-page clarity/transition fixture')
