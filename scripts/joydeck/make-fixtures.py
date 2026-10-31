"""Self-authored, deterministic public-format fixtures; no external assets."""
from pathlib import Path
import json
from zipfile import ZipFile, ZIP_DEFLATED
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[1] / 'tests' / 'fixtures'
ROOT.mkdir(parents=True, exist_ok=True)

def object_(kind, x, y, w, h, fill, text='', size=24):
    return dict(kind=kind, x=x, y=y, width=w, height=h, fill=fill, text=text, fontSize=size)

slides = []
for i in range(1, 21):
    slides.append(dict(id=f'page-{i}', title=f'独立文稿 · {i:02}', background='#f7f9fc', objects=[
        object_('rect', 0, 0, 16, 540, '#27b6a7'),
        object_('text', 64, 52, 820, 68, '#182c45', f'JoyDeck · 第 {i:02} 页', 38),
        object_('text', 66, 142, 810, 130, '#52667d', '原生只读文稿 / Native presentation\n页面浏览 · 缩放 · 保持源文件不变', 23),
        object_('rect', 66, 318, 260, 120, '#27b6a7'),
        object_('ellipse', 366, 318, 120, 120, '#f5bd58'),
        object_('text', 558, 350, 290, 60, '#182c45', f'{i} / 20', 34),
    ]))
with ZipFile(ROOT / 'native-20.joydeck', 'w', ZIP_DEFLATED) as z:
    z.writestr('mimetype', 'application/vnd.joydeck.presentation+zip')
    z.writestr('manifest.json', json.dumps(dict(format='joydeck', schemaMajor=1, schemaMinor=0, minReader=1, requiredFeatures=[], document='document.json')))
    z.writestr('document.json', json.dumps(dict(title='JoyDeck 原生样例', width=960, height=540, slides=slides), ensure_ascii=False))

P = 'http://schemas.openxmlformats.org/presentationml/2006/main'
A = 'http://schemas.openxmlformats.org/drawingml/2006/main'
R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
PKG = 'http://schemas.openxmlformats.org/package/2006/relationships'
CT = 'http://schemas.openxmlformats.org/package/2006/content-types'
def shape(num, kind, x, y, w, h, fill, text='', size=24):
    body = ''
    if text:
        paras = ''.join(f'<a:p><a:r><a:rPr sz="{size*100}"><a:solidFill><a:srgbClr val="{fill}"/></a:solidFill></a:rPr><a:t>{escape(t)}</a:t></a:r><a:endParaRPr/></a:p>' for t in text.split('\n'))
        body = f'<p:txBody><a:bodyPr/><a:lstStyle/>{paras}</p:txBody>'
    paint = '<a:noFill/>' if text else f'<a:solidFill><a:srgbClr val="{fill}"/></a:solidFill>'
    return f'<p:sp><p:nvSpPr><p:cNvPr id="{num}" name="Object {num}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x*12700}" y="{y*12700}"/><a:ext cx="{w*12700}" cy="{h*12700}"/></a:xfrm><a:prstGeom prst="{kind}"><a:avLst/></a:prstGeom>{paint}</p:spPr>{body}</p:sp>'

with ZipFile(ROOT / 'public-format-20.pptx', 'w', ZIP_DEFLATED) as z:
    types = ['<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>', '<Default Extension="xml" ContentType="application/xml"/>', '<Override PartName="/slideset/main.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>']
    types += [f'<Override PartName="/slideset/pages/p{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>' for i in range(1,21)]
    z.writestr('[Content_Types].xml', f'<Types xmlns="{CT}">'+''.join(types)+'</Types>')
    z.writestr('_rels/.rels', f'<Relationships xmlns="{PKG}"><Relationship Id="main" Type="{R}/officeDocument" Target="slideset/main.xml"/></Relationships>')
    # Reverse file numbering makes presentation order distinguishable from ZIP/name order.
    order = list(range(20, 0, -1))
    ids = ''.join(f'<p:sldId id="{256+i}" r:id="r{i}"/>' for i in order)
    z.writestr('slideset/main.xml', f'<p:presentation xmlns:p="{P}" xmlns:r="{R}"><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/></p:presentation>')
    z.writestr('slideset/_rels/main.xml.rels', f'<Relationships xmlns="{PKG}">'+''.join(f'<Relationship Id="r{i}" Type="{R}/slide" Target="pages/p{i}.xml"/>' for i in range(1,21))+'</Relationships>')
    for i in range(1,21):
        content = shape(2, 'rect', 0,0,16,540,'27b6a7')
        content += shape(3,'rect',64,52,820,68,'182c45',f'PPTX · 页面标识 {i:02}',38)
        content += shape(4,'rect',66,142,810,130,'52667d','公开格式合成文稿 / Public-format fixture\n按 presentation 关系顺序浏览',23)
        content += shape(5,'rect',66,318,260,120,'27b6a7')
        content += shape(6,'ellipse',366,318,120,120,'f5bd58')
        if i == 19:
            content += '<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="7" name="Unsupported chart"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><a:graphic><a:graphicData uri="urn:joydeck:test-unsupported"/></a:graphic></p:graphicFrame>'
        z.writestr(f'slideset/pages/p{i}.xml', f'<p:sld xmlns:p="{P}" xmlns:a="{A}"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="f7f9fc"/></a:solidFill></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{content}</p:spTree></p:cSld></p:sld>')
(ROOT / 'broken.pptx').write_bytes(b'PK\x03\x04broken input')
(ROOT / 'README.md').write_text('''# 自建读取样例

由 `python scripts/make-fixtures.py` 确定性生成；作者为本项目，不含外部模板、字体、图片或第三方产品实现数据。

- native-20.joydeck：20 页、中文/英文、换行文字、矩形、椭圆。
- public-format-20.pptx：20 页公开格式合成文稿；非固定 presentation 路径，sldIdLst 顺序为 20→1，第二页含未知对象。测试 OPC 关系和预览子集，不作为 Office 互操作已通过的证据。
- broken.pptx：截断 ZIP；错误时保留当前文稿。
''', encoding='utf-8')
print(f'Generated independent fixtures: {ROOT}')
