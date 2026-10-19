#!/usr/bin/env python3
"""Deterministic original DOCX feature fixtures.

The registered M0/M1 corpus is produced by ``crates/joyword-corpus``. This
script does not call that crate. It writes a separate, byte-reproducible set
under ``tests/fixtures/feature/`` with the Python standard library only:
``zipfile`` entries in a fixed order, ZIP date 1980-01-01 00:00:00, and
deflate level 6. Text is fictional and contains no personal data.

ECMA-376 Part 1 (Transitional) element and attribute names used here are the
public vocabulary of WordprocessingML, not copied from another implementation.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import sys
import tempfile
import zipfile
import zlib
import struct
from pathlib import Path

ORIGIN = "JoyWord 原创特性样例，由 scripts/make_feature_fixtures.py 确定性生成，2026-10-20"
ZIP_DATE = (1980, 1, 1, 0, 0, 0)
COMPRESS_LEVEL = 6

W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
WP = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
A = "http://schemas.openxmlformats.org/drawingml/2006/main"
PIC = "http://schemas.openxmlformats.org/drawingml/2006/picture"
REL_NS = "http://schemas.openxmlformats.org/package/2006/relationships"
CT_NS = "http://schemas.openxmlformats.org/package/2006/content-types"
OFFICE_DOCUMENT = (
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
)
REL_STYLES = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
REL_NUMBERING = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering"
REL_SETTINGS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings"
REL_HEADER = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header"
REL_IMAGE = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"

CT_DOCUMENT = "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
CT_STYLES = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"
CT_NUMBERING = "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"
CT_SETTINGS = "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"
CT_HEADER = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"
CT_XML = "application/xml"
CT_RELS = "application/vnd.openxmlformats-package.relationships+xml"
CT_PNG = "image/png"

PAGE = (
    '<w:pgSz w:w="11906" w:h="16838"/>'
    '<w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" '
    'w:header="851" w:footer="992" w:gutter="0"/>'
)
EDITABLE = "<w:p><w:r><w:t>可编辑句。</w:t></w:r></w:p>"

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DIR = ROOT / "tests" / "fixtures" / "feature"


def png_rgb(red: int, green: int, blue: int) -> bytes:
    """1×1 8-bit truecolor PNG. Filter 0, one RGB sample, fixed zlib level."""

    def chunk(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    ihdr = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)
    scanline = bytes((0, red, green, blue))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(scanline, 9))
        + chunk(b"IEND", b"")
    )


def _xml(body: str) -> bytes:
    return ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' + body).encode("utf-8")


def _rels(items: list[tuple[str, str, str]]) -> bytes:
    body = "".join(
        f'<Relationship Id="{rel_id}" Type="{kind}" Target="{target}"/>'
        for rel_id, kind, target in items
    )
    text = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        f'<Relationships xmlns="{REL_NS}">{body}</Relationships>'
    )
    return text.encode("utf-8")


def _content_types(overrides: list[tuple[str, str]], png: bool) -> bytes:
    defaults = [
        f'<Default Extension="rels" ContentType="{CT_RELS}"/>',
        f'<Default Extension="xml" ContentType="{CT_XML}"/>',
    ]
    if png:
        defaults.append(f'<Default Extension="png" ContentType="{CT_PNG}"/>')
    parts = "".join(
        f'<Override PartName="{name}" ContentType="{kind}"/>' for name, kind in overrides
    )
    text = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        f'<Types xmlns="{CT_NS}">{"".join(defaults)}{parts}</Types>'
    )
    return text.encode("utf-8")


def _document(inner: str, drawing: bool) -> bytes:
    namespaces = f'xmlns:w="{W}" xmlns:r="{R}"'
    if drawing:
        namespaces += f' xmlns:wp="{WP}" xmlns:a="{A}" xmlns:pic="{PIC}"'
    return _xml(f"<w:document {namespaces}><w:body>{inner}</w:body></w:document>")


def _header(inner: str) -> bytes:
    return _xml(f'<w:hdr xmlns:w="{W}">{inner}</w:hdr>')


def _settings(inner: str) -> bytes:
    return _xml(f'<w:settings xmlns:w="{W}">{inner}</w:settings>')


def _note(case_id: str) -> bytes:
    text = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        f"<note>JoyWord fictional fixture {case_id}. Not a personal record.</note>"
    )
    return text.encode("utf-8")


def _pack(parts: list[tuple[str, bytes]]) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(
        buffer,
        "w",
        compression=zipfile.ZIP_DEFLATED,
        compresslevel=COMPRESS_LEVEL,
        allowZip64=False,
    ) as archive:
        for name, data in parts:
            info = zipfile.ZipInfo(filename=name, date_time=ZIP_DATE)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 0
            info.external_attr = 0o644 << 16
            info.extra = b""
            archive.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED, compresslevel=COMPRESS_LEVEL)
    return buffer.getvalue()


def _package(
    case_id: str,
    document: bytes,
    extras: list[tuple[str, bytes, str]],
    rels: list[tuple[str, str, str]],
    png: bool,
) -> list[tuple[str, bytes]]:
    """ZIP members in the order they are stored.

    ``extras`` is ``(item name, bytes, content type)``. Content types and the
    package relationship part always lead. ``word/custom/note.xml`` is an
    unreferenced part so a later text patch can show its bytes survive.
    """
    overrides = [("/word/document.xml", CT_DOCUMENT)]
    overrides.extend((f"/{name}", kind) for name, _, kind in extras)
    overrides.append(("/word/custom/note.xml", CT_XML))
    parts = [
        ("[Content_Types].xml", _content_types(overrides, png)),
        (
            "_rels/.rels",
            _rels([("rId1", OFFICE_DOCUMENT, "word/document.xml")]),
        ),
        ("word/document.xml", document),
    ]
    if rels:
        parts.append(("word/_rels/document.xml.rels", _rels(rels)))
    parts.extend((name, data) for name, data, _kind in extras)
    parts.append(("word/custom/note.xml", _note(case_id)))
    return parts


def _sect(inner: str) -> str:
    return f"<w:sectPr>{inner}{PAGE}</w:sectPr>"


def case_t200() -> list[tuple[str, bytes]]:
    styles = _xml(
        f'<w:styles xmlns:w="{W}">'
        "<w:docDefaults>"
        "<w:rPrDefault><w:rPr>"
        '<w:rFonts w:ascii="Times New Roman" w:eastAsia="宋体"/>'
        '<w:sz w:val="21"/>'
        "</w:rPr></w:rPrDefault>"
        "<w:pPrDefault><w:pPr>"
        '<w:spacing w:after="160" w:line="360" w:lineRule="auto"/>'
        "</w:pPr></w:pPrDefault>"
        "</w:docDefaults>"
        '<w:style w:type="paragraph" w:default="1" w:styleId="Body">'
        '<w:name w:val="Normal"/>'
        "<w:rPr><w:sz w:val=\"21\"/></w:rPr>"
        "</w:style>"
        '<w:style w:type="paragraph" w:customStyle="1" w:styleId="Clause">'
        '<w:name w:val="Clause"/>'
        '<w:basedOn w:val="Body"/>'
        "<w:pPr><w:spacing w:before=\"120\" w:after=\"80\"/></w:pPr>"
        "</w:style>"
        '<w:style w:type="paragraph" w:customStyle="1" w:styleId="Article">'
        '<w:name w:val="Article"/>'
        '<w:basedOn w:val="Clause"/>'
        "<w:rPr><w:b/></w:rPr>"
        "</w:style>"
        "</w:styles>"
    )
    numbering = _xml(
        f'<w:numbering xmlns:w="{W}">'
        '<w:abstractNum w:abstractNumId="0">'
        '<w:lvl w:ilvl="0">'
        '<w:start w:val="1"/>'
        '<w:numFmt w:val="decimal"/>'
        '<w:lvlText w:val="%1"/>'
        '<w:lvlRestart w:val="0"/>'
        '<w:suff w:val="space"/>'
        "</w:lvl>"
        '<w:lvl w:ilvl="1">'
        '<w:start w:val="1"/>'
        '<w:numFmt w:val="chineseCounting"/>'
        '<w:lvlText w:val="%1.%2"/>'
        '<w:lvlRestart w:val="1"/>'
        '<w:suff w:val="tab"/>'
        "</w:lvl>"
        "</w:abstractNum>"
        '<w:num w:numId="1">'
        '<w:abstractNumId w:val="0"/>'
        '<w:lvlOverride w:ilvl="0"><w:startOverride w:val="3"/></w:lvlOverride>'
        "</w:num>"
        "</w:numbering>"
    )
    settings = _settings('<w:defaultTabStop w:val="720"/>')
    numbered = (
        '<w:p><w:pPr><w:pStyle w:val="Clause"/>'
        '<w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="1"/></w:numPr>'
        "</w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"
    )
    body = (
        '<w:p><w:pPr><w:pStyle w:val="Article"/></w:pPr>'
        "<w:r><w:t>第一条 虚构条款</w:t></w:r></w:p>"
        + numbered.format(level="0", text="总则")
        + numbered.format(level="1", text="定义")
        + numbered.format(level="0", text="价款")
        + numbered.format(level="1", text="支付")
        + "<w:p><w:pPr><w:tabs>"
        '<w:tab w:val="left" w:leader="dot" w:pos="2880"/>'
        "</w:tabs></w:pPr>"
        "<w:r><w:t>金额</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>壹仟</w:t></w:r></w:p>"
        "<w:p><w:r><w:t>备忘：Order-014 已登记。</w:t></w:r></w:p>"
        + EDITABLE
        + _sect("")
    )
    return _package(
        "DOCX-FEAT-T200-NUM",
        _document(body, False),
        [
            ("word/styles.xml", styles, CT_STYLES),
            ("word/numbering.xml", numbering, CT_NUMBERING),
            ("word/settings.xml", settings, CT_SETTINGS),
        ],
        [
            ("rId1", REL_STYLES, "styles.xml"),
            ("rId2", REL_NUMBERING, "numbering.xml"),
            ("rId3", REL_SETTINGS, "settings.xml"),
        ],
        png=False,
    )


def _cell(props: str, blocks: str) -> str:
    return f"<w:tc><w:tcPr>{props}</w:tcPr>{blocks}</w:tc>"


def _p(text: str, bold: bool = False) -> str:
    run = "<w:rPr><w:b/></w:rPr>" if bold else ""
    return f"<w:p><w:r>{run}<w:t>{text}</w:t></w:r></w:p>"


def case_t201() -> list[tuple[str, bytes]]:
    borders = (
        "<w:tcBorders>"
        '<w:top w:val="single" w:sz="4" w:color="auto"/>'
        '<w:left w:val="single" w:sz="8" w:color="1F4E79"/>'
        '<w:bottom w:val="double" w:sz="6" w:color="1F4E79"/>'
        '<w:right w:val="nil"/>'
        "</w:tcBorders>"
    )
    shade = '<w:shd w:val="clear" w:color="auto" w:fill="FFF2CC"/>'
    nested = (
        "<w:tbl><w:tblPr><w:tblW w:w=\"2000\" w:type=\"dxa\"/></w:tblPr>"
        '<w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>'
        "<w:tr><w:tc><w:tcPr><w:tcW w:w=\"2000\" w:type=\"dxa\"/></w:tcPr>"
        f"{_p('附注：虚构', bold=True)}</w:tc></w:tr></w:tbl>"
    )
    header = (
        "<w:tr><w:trPr><w:tblHeader/><w:cantSplit/></w:trPr>"
        + _cell(
            f'<w:tcW w:w="1800" w:type="dxa"/>{shade}{borders}',
            _p("条款", bold=True),
        )
        + _cell('<w:tcW w:w="2200" w:type="dxa"/>' + borders, _p("甲方"))
        + _cell('<w:tcW w:w="2200" w:type="dxa"/>' + borders, _p("乙方"))
        + "</w:tr>"
    )
    merged = (
        "<w:tr>"
        + _cell(
            '<w:gridSpan w:val="2"/><w:vMerge w:val="restart"/>',
            _p("价款与交付"),
        )
        + _cell('<w:tcW w:w="2200" w:type="dxa"/>', _p("见下栏"))
        + "</w:tr>"
        + "<w:tr>"
        + _cell('<w:gridSpan w:val="2"/><w:vMerge w:val="continue"/>', "<w:p/>")
        + _cell('<w:tcW w:w="2200" w:type="dxa"/>', _p("交付附注") + nested)
        + "</w:tr>"
    )
    table = (
        "<w:tbl><w:tblPr>"
        '<w:tblW w:w="0" w:type="auto"/>'
        '<w:tblLook w:val="01A0" w:firstRow="1" w:lastRow="0" '
        'w:firstColumn="1" w:lastColumn="1" w:noHBand="0" w:noVBand="0"/>'
        "</w:tblPr>"
        '<w:tblGrid><w:gridCol w:w="1800"/><w:gridCol w:w="2200"/><w:gridCol w:w="2200"/></w:tblGrid>'
        + header
        + merged
        + "</w:tbl>"
    )
    body = _p("合作备忘录（虚构）") + table + EDITABLE + _sect("")
    return _package(
        "DOCX-FEAT-T201-TABLE",
        _document(body, False),
        [],
        [],
        png=False,
    )


def _field(instr: str, cached: str) -> str:
    return f'<w:fldSimple w:instr="{instr}"><w:r><w:t>{cached}</w:t></w:r></w:fldSimple>'


def case_t202() -> list[tuple[str, bytes]]:
    first = _header(f'<w:p><w:r><w:t>首页</w:t></w:r>{_field(" PAGE ", "1")}</w:p>')
    even = _header(f'<w:p><w:r><w:t>偶数页</w:t></w:r>{_field(" NUMPAGES ", "2")}</w:p>')
    default = _header(
        "<w:p><w:r><w:t>正文页眉</w:t></w:r>"
        f'{_field(" PAGE ", "1")}{_field(" NUMPAGES ", "2")}</w:p>'
    )
    columns = (
        '<w:cols w:equalWidth="0" w:num="2">'
        '<w:col w:w="2800" w:space="200"/>'
        '<w:col w:w="5306"/>'
        "</w:cols>"
    )
    end_section = (
        "<w:p><w:pPr><w:sectPr>"
        '<w:type w:val="nextPage"/>'
        "<w:titlePg/>"
        + PAGE
        + '<w:pgNumType w:fmt="decimal" w:start="1"/>'
        + columns
        + '<w:headerReference w:type="first" r:id="rId2"/>'
        + '<w:headerReference w:type="even" r:id="rId3"/>'
        + '<w:headerReference w:type="default" r:id="rId4"/>'
        + "</w:sectPr></w:pPr><w:r><w:t>第一节收束</w:t></w:r></w:p>"
    )
    body = (
        "<w:p><w:pPr><w:keepNext/><w:widowControl/></w:pPr>"
        "<w:r><w:t>第一节标题</w:t></w:r></w:p>"
        "<w:p><w:pPr><w:keepLines/></w:pPr>"
        "<w:r><w:t>左栏虚构文字</w:t></w:r>"
        '<w:r><w:br w:type="column"/></w:r>'
        "<w:r><w:t>右栏虚构文字</w:t></w:r></w:p>"
        + end_section
        + "<w:p><w:r><w:t>连续节正文</w:t></w:r></w:p>"
        + EDITABLE
        + "<w:sectPr><w:type w:val=\"continuous\"/>"
        + PAGE
        + '<w:pgNumType w:fmt="lowerRoman" w:start="4"/>'
        + "</w:sectPr>"
    )
    return _package(
        "DOCX-FEAT-T202-PAGES",
        _document(body, False),
        [
            ("word/settings.xml", _settings("<w:evenAndOddHeaders/>"), CT_SETTINGS),
            ("word/header-first.xml", first, CT_HEADER),
            ("word/header-even.xml", even, CT_HEADER),
            ("word/header-default.xml", default, CT_HEADER),
        ],
        [
            ("rId1", REL_SETTINGS, "settings.xml"),
            ("rId2", REL_HEADER, "header-first.xml"),
            ("rId3", REL_HEADER, "header-even.xml"),
            ("rId4", REL_HEADER, "header-default.xml"),
        ],
        png=False,
    )


def _anchor(rel_id: str, x: int, y: int, wrap: str, distances: str) -> str:
    return (
        f'<w:r><w:drawing><wp:anchor behindDoc="0" {distances}simplePos="0">'
        '<wp:simplePos x="0" y="0"/>'
        f'<wp:positionH relativeFrom="page"><wp:posOffset>{x}</wp:posOffset></wp:positionH>'
        f'<wp:positionV relativeFrom="page"><wp:posOffset>{y}</wp:posOffset></wp:positionV>'
        '<wp:extent cx="457200" cy="228600"/>'
        f"{wrap}"
        f'<a:graphic><a:graphicData uri="{PIC}">'
        "<pic:pic><pic:blipFill>"
        f'<a:blip r:embed="{rel_id}"/>'
        "</pic:blipFill></pic:pic></a:graphicData></a:graphic>"
        "</wp:anchor></w:drawing></w:r>"
    )


def _inline(rel_id: str) -> str:
    return (
        "<w:r><w:drawing><wp:inline>"
        '<wp:extent cx="9525" cy="9525"/>'
        f'<a:graphic><a:graphicData uri="{PIC}">'
        "<pic:pic><pic:blipFill>"
        f'<a:blip r:embed="{rel_id}"/>'
        "</pic:blipFill></pic:pic></a:graphicData></a:graphic>"
        "</wp:inline></w:drawing></w:r>"
    )


def case_t203() -> list[tuple[str, bytes]]:
    pictures = [
        ("word/media/inline.png", png_rgb(220, 40, 40)),
        ("word/media/wrap-none.png", png_rgb(40, 160, 60)),
        ("word/media/wrap-square.png", png_rgb(40, 80, 200)),
        ("word/media/wrap-tight.png", png_rgb(230, 180, 20)),
    ]
    body = (
        f"<w:p>{_inline('rId1')}</w:p>"
        f"<w:p>{_anchor('rId2', 914400, 1828800, '<wp:wrapNone/>', '')}</w:p>"
        "<w:p>"
        + _anchor(
            "rId3",
            457200,
            914400,
            '<wp:wrapSquare wrapText="bothSides"/>',
            'distT="0" distB="0" distL="114300" distR="114300" ',
        )
        + "</w:p>"
        f"<w:p>{_anchor('rId4', 1828800, 457200, '<wp:wrapTight/>', '')}</w:p>"
        + _p("图注：虚构示意图")
        + EDITABLE
        + _sect("")
    )
    extras = [(name, data, CT_PNG) for name, data in pictures]
    rels = [
        ("rId1", REL_IMAGE, "media/inline.png"),
        ("rId2", REL_IMAGE, "media/wrap-none.png"),
        ("rId3", REL_IMAGE, "media/wrap-square.png"),
        ("rId4", REL_IMAGE, "media/wrap-tight.png"),
    ]
    return _package("DOCX-FEAT-T203-IMAGES", _document(body, True), extras, rels, png=True)


def _revision(kind: str, rel_id: str, text_tag: str, text: str) -> str:
    return (
        f'<w:{kind} w:id="{rel_id}" w:author="青禾审阅" w:date="2026-10-20T00:00:00Z">'
        f"<w:r><w:{text_tag}>{text}</w:{text_tag}></w:r>"
        f"</w:{kind}>"
    )


def _marker(local: str, rel_id: str) -> str:
    return (
        f'<w:{local} w:id="{rel_id}" w:author="青禾审阅" w:name="move-a" '
        'w:date="2026-10-20T00:00:00Z"/>'
    )


def case_t300() -> list[tuple[str, bytes]]:
    header = _header(
        "<w:p>"
        + _revision("ins", "21", "t", "页眉新稿")
        + _revision("del", "22", "delText", "页眉旧稿")
        + "</w:p>"
    )
    move = (
        "<w:p>"
        + _marker("moveFromRangeStart", "11")
        + _revision("moveFrom", "12", "delText", "旧址")
        + _marker("moveFromRangeEnd", "13")
        + _marker("moveToRangeStart", "14")
        + _revision("moveTo", "15", "t", "新址")
        + _marker("moveToRangeEnd", "16")
        + "</w:p>"
    )
    body = (
        "<w:p>"
        + _revision("ins", "1", "t", "新增一句虚构说明")
        + _revision("del", "2", "delText", "删去的旧句")
        + "</w:p>"
        + move
        + EDITABLE
        + "<w:sectPr>"
        + PAGE
        + '<w:headerReference w:type="default" r:id="rId2"/>'
        + "</w:sectPr>"
    )
    return _package(
        "DOCX-FEAT-T300-REVIEW",
        _document(body, False),
        [
            ("word/settings.xml", _settings("<w:trackRevisions/>"), CT_SETTINGS),
            ("word/header1.xml", header, CT_HEADER),
        ],
        [
            ("rId1", REL_SETTINGS, "settings.xml"),
            ("rId2", REL_HEADER, "header1.xml"),
        ],
        png=False,
    )


CASES: dict[str, callable] = {
    "DOCX-FEAT-T200-NUM": case_t200,
    "DOCX-FEAT-T201-TABLE": case_t201,
    "DOCX-FEAT-T202-PAGES": case_t202,
    "DOCX-FEAT-T203-IMAGES": case_t203,
    "DOCX-FEAT-T300-REVIEW": case_t300,
}


def case_parts(case_id: str) -> list[tuple[str, bytes]]:
    try:
        builder = CASES[case_id]
    except KeyError as error:
        raise KeyError(case_id) from error
    return builder()


def documents() -> dict[str, bytes]:
    built = {case_id: _pack(case_parts(case_id)) for case_id in CASES}
    again = {case_id: _pack(case_parts(case_id)) for case_id in CASES}
    if built != again:
        raise RuntimeError("feature fixture generator is not deterministic")
    return built


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_documents(directory: Path) -> dict[str, str]:
    directory.mkdir(parents=True, exist_ok=True)
    digests = {}
    for case_id, blob in documents().items():
        path = directory / f"{case_id}.docx"
        path.write_bytes(blob)
        digests[case_id] = sha256_hex(blob)
    return digests


def check_committed(directory: Path = DEFAULT_DIR) -> list[str]:
    problems = []
    fresh = documents()
    for case_id, blob in fresh.items():
        path = directory / f"{case_id}.docx"
        if not path.is_file():
            problems.append(f"missing {path}")
            continue
        committed = path.read_bytes()
        if committed != blob:
            problems.append(
                f"{case_id}: committed bytes differ from a fresh generation "
                f"({sha256_hex(committed)} != {sha256_hex(blob)})"
            )
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        type=Path,
        default=DEFAULT_DIR,
        help="directory that receives the DOCX packages",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="regenerate in a temp directory and compare with --out (default: committed fixtures)",
    )
    args = parser.parse_args(argv)
    if args.check:
        with tempfile.TemporaryDirectory(prefix="joyword-feature-fixtures-") as temp:
            write_documents(Path(temp))
            problems = check_committed(args.out)
        if problems:
            for problem in problems:
                print(problem, file=sys.stderr)
            return 1
        print(f"OK: {len(CASES)} feature fixtures match {args.out}")
        return 0
    digests = write_documents(args.out)
    for case_id, digest in digests.items():
        print(f"{case_id} {digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
