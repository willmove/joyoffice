"""Original deterministic ZIP/XML cases, never fetch relationships."""
import hashlib
import io
import json
from pathlib import Path
import struct
import warnings
import zipfile

W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
R = "http://schemas.openxmlformats.org/package/2006/relationships"
CT = b'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="bin" ContentType="application/octet-stream"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>'
ROOT_RELS = ('<Relationships xmlns="'+R+'"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>').encode()
P = '<w:p><w:r><w:t xml:space="preserve">安全甲 👩‍💻e\u0301 &lt;&amp;&gt; </w:t></w:r></w:p>'

def archive(body=P, extras=(), raw=None, relations=None, compression=zipfile.ZIP_STORED):
    main = raw if raw is not None else ('<w:document xmlns:w="'+W+'"><w:body>'+body+'<w:sectPr/></w:body></w:document>').encode()
    parts = [("[Content_Types].xml", CT), ("_rels/.rels", ROOT_RELS), ("word/document.xml", main)]
    if relations is not None:
        parts.append(("word/_rels/document.xml.rels", relations))
    parts.extend(extras)
    out = io.BytesIO()
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', UserWarning)
        with zipfile.ZipFile(out, "w", compression=compression) as z:
            for name, data in parts:
                info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
                info.compress_type = compression
                z.writestr(info, data)
    return out.getvalue()

def rel(target, mode="Internal", kind="hyperlink"):
    return ('<Relationships xmlns="'+R+'"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/'+kind+'" Target="'+target+'" TargetMode="'+mode+'"/></Relationships>').encode()

def cases():
    base = archive()
    values = [("minimal", base), ("unknown-binary", archive(extras=[("custom/data.bin",b'\x00opaque\xff')])),
              ("external-http",archive(relations=rel('https://example.invalid/no-fetch','External'))),
              ("external-file",archive(relations=rel('file:///not-accessed','External'))),
              ("external-unc",archive(relations=rel('\\\\server\\share\\doc','External'))),
              ("relationship-cycle",archive(relations=rel('document.xml',kind='officeDocument'))),
              ("protected",archive(extras=[('word/settings.xml',('<w:settings xmlns:w="'+W+'"><w:documentProtection w:edit="readOnly" w:enforcement="1"/></w:settings>').encode())],relations=rel('settings.xml',kind='settings'))),
              ("tracked",archive(extras=[('word/settings.xml',('<w:settings xmlns:w="'+W+'"><w:trackRevisions/></w:settings>').encode())],relations=rel('settings.xml',kind='settings'))),
              ("revision",archive('<w:p><w:ins w:id="1"><w:r><w:t>revision</w:t></w:r></w:ins></w:p>')),
              ("signed",archive(extras=[('_xmlsignatures/sig1.xml',b'<Signature/>')])),
              ("unknown-namespace",archive('<x:future xmlns:x="urn:w09:unknown"/>'+P)),
              ("alternate-content",archive('<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="urn:w09"><mc:Choice Requires="x"><x:future/></mc:Choice><mc:Fallback>'+P+'</mc:Fallback></mc:AlternateContent>')),
              ("must-understand",archive(raw=('<w:document xmlns:w="'+W+'" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="urn:w09" mc:MustUnderstand="x"><w:body>'+P+'</w:body></w:document>').encode())),
              ("xml-entity",archive(raw=('<!DOCTYPE w:document [<!ENTITY x "entity">]><w:document xmlns:w="'+W+'"><w:body><w:p><w:r><w:t>&x;</w:t></w:r></w:p></w:body></w:document>').encode())),
              ("xml-depth",archive('<x:deep xmlns:x="urn:w09">'*40+P+'</x:deep>'*40)),
              ("xml-nodes",archive('<w:p/>'*11000)),
              ("duplicate-name",archive(extras=[('word/document.xml',b'<x/>')])),
              ("path-traversal",archive(extras=[('../escape.bin',b'x')])),
              ("case-duplicate",archive(extras=[('WORD/DOCUMENT.XML',b'<x/>')])),
              ("compression-ratio",archive(extras=[('custom/bomb.bin',b'0'*600000)],compression=zipfile.ZIP_DEFLATED)),
              ("truncated",base[:-7]), ("empty",b'')]
    central = base.index(b'PK\x01\x02')
    local = base.index(b'PK\x03\x04')
    for name, offsets, number, fmt in [
        ('crc',[(local+14),(central+16)],0x12345678,'<I'),
        ('declared-size',[(local+22),(central+24)],0x7fffffff,'<I'),
        ('encrypted',[(local+6),(central+8)],1,'<H'),
        ('unknown-compression',[(local+8),(central+10)],99,'<H'),
        ('overlap',[(central+42)],1,'<I'),
    ]:
        bad=bytearray(base)
        for offset in offsets:
            struct.pack_into(fmt,bad,offset,number)
        values.append((name,bytes(bad)))
    return values

def generate(directory):
    directory=Path(directory);directory.mkdir(parents=True,exist_ok=True)
    entries=[]
    for name,data in cases():
        path=directory/(name+'.docx');path.write_bytes(data)
        entries.append({'id':'W09-'+name,'file':path.name,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data),'source':'original inline ZIP/XML generator matrix.py'})
    (directory/'manifest.json').write_text(json.dumps({'schema':1,'cases':entries},indent=2)+'\n')
    return entries

if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser();p.add_argument('directory');generate(p.parse_args().directory)
