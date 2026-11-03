"""Original independent R4/R5 corpus: python-pptx 1.0.2; no runtime dependency."""
from pathlib import Path
from io import BytesIO
import sys,zipfile
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'target/corpus-tools'))
from pptx import Presentation
from pptx.util import Inches,Pt
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_SHAPE
from pptx.enum.chart import XL_CHART_TYPE
from pptx.chart.data import CategoryChartData
from pptx.oxml.xmlchemy import OxmlElement
from PIL import Image,ImageDraw

folder=ROOT/'tests/fixtures/joydeck/r4-r5';folder.mkdir(exist_ok=True)
p=Presentation();p.slide_width=Inches(13.333333);p.slide_height=Inches(7.5)
image=Image.new('RGBA',(480,280),(0,0,0,0));draw=ImageDraw.Draw(image)
draw.rounded_rectangle((6,6,474,274),24,fill=(38,120,208,220));draw.ellipse((35,35,220,220),fill=(255,192,80,230));draw.polygon([(270,45),(435,230),(250,230)],fill=(30,135,100,190))
picture=BytesIO();image.save(picture,format='PNG');picture.seek(0);(folder/'shapes.png').write_bytes(picture.getvalue())
def text(slide,value,x,y,w,h,size=24):
    box=slide.shapes.add_textbox(Inches(x),Inches(y),Inches(w),Inches(h));para=box.text_frame.paragraphs[0];para.text=value;para.font.name='Microsoft YaHei';para.font.size=Pt(size);para.font.color.rgb=RGBColor.from_string('26352D');return box
for index in range(3):
    slide=p.slides.add_slide(p.slide_layouts[6]);text(slide,['GPUI scene / 原生对象场景','Master · theme · picture · chart','Complex content / 局部回退'][index],.55,.45,12,1,30)
    text(slide,'JoyDeck R4/R5 · independent public OOXML · 中文 / e\u0301 / עברית',.6,1.4,12,.7,17)
    slide.notes_slide.notes_text_frame.text=f'R4/R5 original notes {index+1}'
    if index==0:
        for i,kind in enumerate([MSO_SHAPE.RECTANGLE,MSO_SHAPE.ROUNDED_RECTANGLE,MSO_SHAPE.OVAL,MSO_SHAPE.CHEVRON]):
            s=slide.shapes.add_shape(kind,Inches(.7+i*3.1),Inches(2.8),Inches(2.4),Inches(1.6));s.fill.solid();s.fill.fore_color.theme_color=5+i;s.line.fill.background()
            s.text=f'Object {i+1}';s.text_frame.paragraphs[0].font.size=Pt(22)
        text(slide,'Drag objects: cached paths + exact glyph outlines',.7,5.4,11,1,26)
    if index==1:
        slide.shapes.add_picture(BytesIO(picture.getvalue()),Inches(.7),Inches(2.6),Inches(3.4),Inches(2.0))
        data=CategoryChartData();data.categories=['Alpha','Beta','Gamma','Delta'];data.add_series('Plan',[10,18,14,24]);data.add_series('Actual',[12,16,20,28])
        chart=slide.shapes.add_chart(XL_CHART_TYPE.COLUMN_CLUSTERED,Inches(4.7),Inches(2.3),Inches(7.7),Inches(3.9),data).chart
        chart.has_title=True;chart.chart_title.text_frame.text='Original chart / 可编辑数据';chart.has_legend=True
        text(slide,'Edit theme, crop, brightness and chart values; save & reopen',.7,6.5,12,.5,16)
    if index==2:
        shape=slide.shapes.add_shape(MSO_SHAPE.ROUNDED_RECTANGLE,Inches(.7),Inches(2.5),Inches(3.1),Inches(2.0));shape.fill.gradient();shape.fill.gradient_angle=25;shape.rotation=12
        shape.text='Gradient + rotation';shape.text_frame.paragraphs[0].font.size=Pt(20)
        group=slide.shapes.add_group_shape()
        a=group.shapes.add_shape(MSO_SHAPE.OVAL,Inches(4.5),Inches(2.6),Inches(1.5),Inches(1.5));a.fill.solid();a.fill.fore_color.rgb=RGBColor.from_string('2678D0')
        b=group.shapes.add_shape(MSO_SHAPE.ISOSCELES_TRIANGLE,Inches(5.4),Inches(3.3),Inches(1.5),Inches(1.5));b.fill.solid();b.fill.fore_color.rgb=RGBColor.from_string('F0B84C');group.rotation=15
        table=slide.shapes.add_table(3,3,Inches(7.5),Inches(2.6),Inches(4.8),Inches(2.1)).table
        for r in range(3):
            for c in range(3):table.cell(r,c).text=f'R{r+1} / C{c+1}'
        table.cell(0,0).merge(table.cell(0,1))
        text(slide,'Gradients, groups and tables use bounded per-object CPU tiles',.7,6.3,12,.7,17)
    # The visible master badge is real master XML, not a slide-local imitation.
master=p.slide_masters[0]
badge=OxmlElement('p:sp')
badge_xml='<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:nvSpPr><p:cNvPr id="901" name="Master badge"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="9144000" y="6400800"/><a:ext cx="2438400" cy="182880"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:schemeClr val="accent1"/></a:solidFill><a:ln><a:noFill/></a:ln></p:spPr></p:sp>'
from lxml import etree
master._element.spTree.append(etree.fromstring(badge_xml))
p.core_properties.title='JoyDeck R4/R5 independent corpus';p.core_properties.author='JoyDeck verification'
raw=BytesIO();p.save(raw)
out=folder/'independent-complex.pptx'
with zipfile.ZipFile(raw) as src,zipfile.ZipFile(out,'w',zipfile.ZIP_DEFLATED) as dest:
    for name in src.namelist():dest.writestr(name,src.read(name))
    dest.writestr('vendor/unchanged-r4-r5.xml',b'<future xmlns="urn:joydeck:r4-r5">preserve exact bytes</future>')
print(out)
