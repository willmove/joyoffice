"""Deterministic, original reading-order/image/table DOCX for T806."""
from pathlib import Path
import argparse
from make_feature_fixtures import _package, _pack, _document, _inline, _sect, png_rgb, R
ROOT=Path(__file__).resolve().parents[1]
def fixture():
 heading=lambda t: '<w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val="28"/></w:rPr><w:t>'+t+'</w:t></w:r></w:p>'
 para=lambda t: '<w:p><w:r><w:t>'+t+'</w:t></w:r></w:p>'
 cell=lambda t:'<w:tc><w:tcPr/><w:p><w:r><w:t>'+t+'</w:t></w:r></w:p></w:tc>'
 body=heading('第一章 阅读顺序')+'<w:p><w:r><w:t>中文 é 🙂。</w:t><w:br/><w:t>العربية محفوظة。</w:t></w:r></w:p>'
 body+='<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2400"/><w:gridCol w:w="2400"/></w:tblGrid><w:tr>'+cell('项目')+cell('值')+'</w:tr><w:tr>'+cell('编号')+cell('001')+'</w:tr></w:tbl>'
 body+=heading('第二章 图片')+'<w:p>'+_inline('rImage').replace('9525', '914400')+'</w:p>'+_sect('')
 return _pack(_package('DOCX-EPUB-T806',_document(body,True),[('word/media/image.png',png_rgb(25,110,210),'image/png')],[('rImage',R+'/image','media/image.png')],png=True))
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');args=parser.parse_args();path=ROOT/'tests/fixtures/epub/book.docx';data=fixture()
 if args.check:
  if path.read_bytes()!=data:raise SystemExit('EPUB DOCX fixture differs')
 else:path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
 print('EPUB original fixture: byte-identical' if args.check else 'EPUB original fixture: written')
if __name__=='__main__':main()
