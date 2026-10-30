#!/usr/bin/env python3
"""Reproduce original local-mail-merge fixtures using only the standard library."""
from pathlib import Path
import argparse
import csv
import io
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def fixtures():
    w = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
    body = (f'<w:document xmlns:w="{w}"><w:body>'
            '<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val="28"/></w:rPr><w:t>项目通知</w:t></w:r></w:p>'
            '<w:p><w:r><w:t xml:space="preserve">致 </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>{{姓名}}</w:t></w:r><w:r><w:t>：</w:t></w:r></w:p>'
            '<w:p><w:r><w:t>欢迎{{单位}}参加本地协作项目。</w:t></w:r></w:p>'
            '<w:p><w:r><w:t>编号：{{编号}}。请保留本通知。</w:t></w:r></w:p>'
            '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:bottom="1440" w:left="1440" w:right="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr>'
            '</w:body></w:document>')
    content = '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'
    rels = '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'
    out = io.BytesIO()
    with zipfile.ZipFile(out, 'w', compression=zipfile.ZIP_STORED) as archive:
        for name, text in [('[Content_Types].xml', content), ('_rels/.rels', rels), ('word/document.xml', body)]:
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, text.encode('utf-8'))
    data = io.StringIO(newline='')
    writer = csv.writer(data, lineterminator='\r\n')
    writer.writerows([['单位', '姓名', '编号'], ['星河,实验室', '张三', '001'], ['远山 "研发"', '李四', '002'], ['南山\n研究组', '王五', '003']])
    return {'template.docx': out.getvalue(), 'recipients.csv': b'\xef\xbb\xbf' + data.getvalue().encode('utf-8')}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    directory = ROOT / 'tests/fixtures/mail-merge'
    if not args.check:
        directory.mkdir(parents=True, exist_ok=True)
    for name, data in fixtures().items():
        path = directory / name
        if args.check:
            if path.read_bytes() != data:
                raise SystemExit(f'mail merge fixture differs: {name}')
        else:
            path.write_bytes(data)
    print('mail merge fixtures: byte-identical' if args.check else 'mail merge fixtures: written')


if __name__ == '__main__':
    main()
