"""XLSX cell editing regressions run against the production GPUI inputs."""
import re
from xml.sax.saxutils import escape
from xml.etree import ElementTree as ET
from zipfile import ZipFile, ZIP_DEFLATED


def exercise(control, output, wait, key, capture, checks):
    source = output / "multiline-source.xlsx"
    values = {"A1": "首行😀\n第二行", "A2": "CRLF 第一行\r\n第二行",
              "A3": "自动换行格式的普通长文本", "A4": "第一行\n\n第三行"}
    ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    cells = "".join(
        f'<row r="{i}" ht="64" customHeight="1"><c r="{at}" s="1" t="inlineStr">'
        f'<is><t xml:space="preserve">{escape(text).replace(chr(13), "&#13;")}</t></is></c></row>'
        for i, (at, text) in enumerate(values.items(), 1))
    parts = {
        "[Content_Types].xml": '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
            '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
            '<Default Extension="xml" ContentType="application/xml"/>'
            '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
            '<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
            '<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>',
        "_rels/.rels": f'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            f'<Relationship Id="rId1" Type="{rel}/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        "xl/workbook.xml": f'<workbook xmlns="{ns}" xmlns:r="{rel}">'
            '<sheets><sheet name="Multiline" sheetId="1" r:id="rId1"/></sheets></workbook>',
        "xl/_rels/workbook.xml.rels": '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            f'<Relationship Id="rId1" Type="{rel}/worksheet" Target="worksheets/sheet1.xml"/>'
            f'<Relationship Id="rId2" Type="{rel}/styles" Target="styles.xml"/></Relationships>',
        "xl/styles.xml": f'<styleSheet xmlns="{ns}"><fonts count="2"><font/><font><b/><sz val="14"/>'
            '<color rgb="FF336699"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill>'
            '<fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid">'
            '<fgColor rgb="FFFFFFCC"/></patternFill></fill></fills><borders count="1"><border/></borders>'
            '<cellStyleXfs count="1"><xf/></cellStyleXfs><cellXfs count="2"><xf/>'
            '<xf numFmtId="49" fontId="1" fillId="2" borderId="0" xfId="0" applyAlignment="1">'
            '<alignment wrapText="1" vertical="top"/></xf></cellXfs></styleSheet>',
        "xl/worksheets/sheet1.xml": f'<worksheet xmlns="{ns}"><cols><col min="1" max="1" width="20" customWidth="1"/>'
            f'</cols><sheetData>{cells}</sheetData></worksheet>',
    }
    with ZipFile(source, "w", ZIP_DEFLATED) as archive:
        for name, xml in parts.items():
            archive.writestr(name, xml)
    control.call("app.open", path=str(source))
    wait(lambda v: v["worksheet"]["name"] == "Multiline", "multiline XLSX import")
    originals = {at: control.call("cell.get", cell=at) for at in values}
    assert all(cell["style"]["align"]["wrap"] for cell in originals.values()), "fixture lost wrap style"

    def assert_cell(at, expected):
        actual = control.call("cell.get", cell=at)
        assert actual["value"] == expected, (at, actual["value"], expected)
        assert actual["style"] == originals[at]["style"], (at, "style changed")

    def assert_inputs(expected):
        view = control.call("ui.inspect")
        assert view["editor"]["text"] == expected, view["editor"]
        if view["editor"]["inline"]:
            assert view["cellEditorText"] == expected, view["cellEditorText"]
        assert view["formulaText"] == expected, view["formulaText"]

    for at, text in values.items():
        assert_cell(at, text)
        before = control.call("ui.inspect")
        original_window = next(w for w in before["windows"] if w["window"] == before["window"])
        control.call("ui.cell.click", cell=at)
        control.call("ui.cell.click", cell=at, count=2)
        wait(lambda v: v["editor"] is not None, "double-click editing")
        assert_inputs(text)
        if at == "A1":
            capture("04-multiline-editor.png")
        control.call("ui.key", key="tab" if at in ["A2", "A3"] else "enter", shift=at in ["A3", "A4"])
        after = wait(lambda v: v["editor"] is None, "unchanged multiline commit")
        current_window = next(w for w in after["windows"] if w["window"] == after["window"])
        assert current_window["revision"] == original_window["revision"], "unchanged edit changed revision"
        assert not current_window["dirty"], "unchanged edit dirtied imported workbook"
        assert_cell(at, text)
        control.call("ui.cell.click", cell=at)
        key("f2")
        assert_inputs(text)
        control.call("ui.text", text="取消的内容")
        key("escape")
        wait(lambda v: v["editor"] is None, "cancel multiline edit")
        assert_cell(at, text)
        key("f6")
        assert_inputs(text)
        key("enter")
        after = wait(lambda v: v["editor"] is None, "unchanged formula-bar commit")
        current_window = next(w for w in after["windows"] if w["window"] == after["window"])
        assert not current_window["dirty"], "cancelled or unchanged edit dirtied imported workbook"
        assert_cell(at, text)
    checks.append("XLSX LF/CRLF/blank lines and wrap style survive double-click/Enter/Tab/Shift navigation and F2/Escape without dirtying")

    for at in ["A1", "A2"]:
        control.call("ui.cell.click", cell=at)
        if at == "A1":
            key("f2")
        else:
            key("f6")
        assert_inputs(values[at])
        control.call("ui.text", text="追加😀")
        assert_inputs(values[at] + "追加😀")
        key("enter" if at == "A1" else "tab")
        wait(lambda v: v["editor"] is None, "changed multiline commit")
        values[at] += "追加😀"
        assert_cell(at, values[at])
        key("z", True)
        control.call("ui.awaitIdle")
        assert_cell(at, originals[at]["value"])
        key("y", True)
        control.call("ui.awaitIdle")
        assert_cell(at, values[at])
    checks.append("inline and formula-bar edits append at the caret and preserve line breaks and full style")

    saved = output / "multiline-saved.xlsx"
    control.call("app.save", path=str(saved))
    # Independently inspect the serialized OOXML as well as reopening it below.
    q = {"s": ns}
    with ZipFile(saved) as archive:
        shared = []
        if "xl/sharedStrings.xml" in archive.namelist():
            strings = ET.fromstring(archive.read("xl/sharedStrings.xml"))
            shared = ["".join(si.itertext()) for si in strings]
        sheet = ET.fromstring(archive.read("xl/worksheets/sheet1.xml"))
        for cell in sheet.findall("s:sheetData/s:row/s:c", q):
            at = cell.attrib["r"]
            if at in values:
                text = (shared[int(cell.findtext("s:v", namespaces=q))] if cell.attrib.get("t") == "s"
                        else "".join(t.text or "" for t in cell.findall("s:is//s:t", q)))
                # OOXML text escapes CR as _x000D_; decode escapes once so a
                # literal escaped underscore is never interpreted a second time.
                text = re.sub(r"_x([0-9A-Fa-f]{4})_", lambda match: chr(int(match[1], 16)), text)
                assert text == values[at], (at, "saved OOXML changed text", text)
    control.call("engine.execute", command="file.close", params={"force": True, "document": True})
    control.call("app.open", path=str(saved))
    wait(lambda v: v["worksheet"]["name"] == "Multiline", "multiline XLSX reopen")
    for at, text in values.items():
        assert_cell(at, text)
    capture("05-multiline-roundtrip.png")
    checks.append("multiline text and imported cell styles survive XLSX save/close/reopen")
