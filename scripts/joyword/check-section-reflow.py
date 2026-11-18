"""Edit a section with newspaper columns, nested tables and numbered footnotes in the native Word window."""
import argparse
import json
from pathlib import Path
import runpy
import secrets
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import app, digest, port, wait, word_request
from verify_save_changes import word_keys


def picture_anchor():
    wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
    a="http://schemas.openxmlformats.org/drawingml/2006/main"
    pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
    return f'''<w:r><w:drawing><wp:anchor xmlns:wp="{wp}" xmlns:a="{a}" xmlns:pic="{pic}" simplePos="0" relativeHeight="3" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1">
        <wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>184150</wp:posOffset></wp:positionH>
        <wp:positionV relativeFrom="paragraph"><wp:posOffset>95250</wp:posOffset></wp:positionV><wp:extent cx="685800" cy="342900"/>
        <wp:wrapSquare wrapText="bothSides"/><wp:docPr id="17" name="Anchor reference"/><wp:cNvGraphicFramePr/>
        <a:graphic><a:graphicData uri="{pic}"><pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="Reference image"/><pic:cNvPicPr/></pic:nvPicPr>
        <pic:blipFill><a:blip r:embed="img"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="685800" cy="342900"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic>
        </wp:anchor></w:drawing></w:r>'''


def fixture(path, facing=False, stories=False, anchors=False):
    make = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))
    body = []
    for section in range(70):
        props = '<w:type w:val="continuous"/>' if section == 35 else ""
        margins = '<w:pgMar w:left="1760" w:right="1100" w:gutter="240" w:top="1440" w:bottom="1440" w:header="720" w:footer="720"/>' if facing else ""
        if facing and section in (34, 36, 55):
            kind = "evenPage" if section == 36 else "oddPage"
            props = f'<w:type w:val="{kind}"/>'
        props += margins
        if stories and section == 0:
            props += '<w:headerReference w:type="default" r:id="h1"/><w:headerReference w:type="even" r:id="h2"/><w:footerReference w:type="default" r:id="f1"/>'
        if stories and section == 36:
            props += '<w:titlePg/><w:headerReference w:type="first" r:id="h3"/>'
        if stories and section == 60:
            props += '<w:headerReference w:type="default" r:id="h4"/>'

        if facing and section == 38:
            props += '<w:pgNumType w:start="6"/>'
        if section == 34:
            props += '<w:cols w:num="2"/>'
        texts = [f"Section {section}", "Text after the heading"]
        if section == 34:
            texts += ["Alpha beta gamma. " * 6] * 8
        for i, text in enumerate(texts):
            marker = (f"<w:pPr><w:sectPr>{props}</w:sectPr></w:pPr>"
                      if i == len(texts) - 1 and section != 69 else "")
            note_id = {2: 1, 34: 2, 48: 3}.get(section) if i == 0 else None
            if anchors and section == 34:
                note_id = 2 if i == len(texts)-1 else None
            objects = ""
            if anchors and section == 34 and i == 3:
                objects = picture_anchor()
            if anchors and section == 34 and i == 4:
                objects = "".join(f'<w:r><w:t xml:space="preserve"> Body-{key}=</w:t></w:r><w:fldSimple w:instr="{key}"><w:r><w:t>12345</w:t></w:r></w:fldSimple>' for key in ("PAGE","NUMPAGES","SECTION","SECTIONPAGES"))
            note = f'<w:r><w:footnoteReference w:id="{note_id}"/></w:r>' if note_id else ""
            body.append(f"<w:p>{marker}<w:r><w:t>{text}</w:t></w:r>{objects}{note}</w:p>")
            if section == 34 and i == 1:
                wp = '<w:tblPr><w:tblW w:w="1950" w:type="dxa"/>{}</w:tblPr>'
                grid = '<w:tblGrid><w:gridCol w:w="1950"/></w:tblGrid>'
                value = '<w:p><w:r><w:t>Nested value</w:t></w:r></w:p>'
                inside = f'<w:tbl>{wp.format("")}{grid}<w:tr><w:tc>{value}</w:tc></w:tr></w:tbl>'
                floating = '<w:tblpPr w:horzAnchor="text" w:vertAnchor="text" w:tblpX="100" w:leftFromText="80"/>'
                body.append(f'<w:tbl>{wp.format(floating)}{grid}<w:tr><w:tc>{inside}<w:p/></w:tc></w:tr></w:tbl>')
    body.append(f"<w:sectPr>{margins}</w:sectPr>")
    notes = "".join(f'<w:footnote w:id="{i}"><w:p><w:r><w:t>Numbered note {i}</w:t></w:r></w:p></w:footnote>' for i in (1, 2, 3))
    if anchors:
        continuation="".join(f'<w:p><w:r><w:t>Continuation record {i:03}</w:t></w:r></w:p>' for i in range(1,55))
        notes=notes.replace("Numbered note 2</w:t></w:r></w:p>", "Numbered note 2</w:t></w:r></w:p>"+continuation)
    extra = {"word/footnotes.xml": f'<w:footnotes xmlns:w="{make["W"]}">{notes}</w:footnotes>'}
    rels = [("n", "footnotes", "footnotes.xml")]
    types = {}
    if facing:
        rels.append(("s", "settings", "settings.xml"))
        extra["word/settings.xml"] = f'<w:settings xmlns:w="{make["W"]}"><w:mirrorMargins/><w:evenAndOddHeaders/></w:settings>'
        types["word/settings.xml"] = "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"
    if anchors:
        import struct
        import zlib
        def chunk(tag,data):
            return struct.pack(">I",len(data))+tag+data+struct.pack(">I",zlib.crc32(tag+data)&0xffffffff)
        extra["word/media/ref.png"] = (b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",struct.pack(">2I5B",2,2,8,2,0,0,0))
            +chunk(b"IDAT",zlib.compress(bytes([0,166,40,20,35,76,160,0,35,76,160,166,40,20])))+chunk(b"IEND",b""))
        rels.append(("img","image","media/ref.png"))
    if stories:
        for part, kind, label in (("h1","header","Main"),("h2","header","Even"),("h3","header","Title"),("h4","header","Final"),("f1","footer","Foot")):
            tag = "hdr" if kind == "header" else "ftr"
            path_in_zip=f"word/{part}.xml"
            rels.append((part,kind,part+".xml"))
            types[path_in_zip]="application/vnd.openxmlformats-officedocument.wordprocessingml."+kind+"+xml"
            runs = f'<w:r><w:t xml:space="preserve">{label} </w:t></w:r>'
            for key in ("PAGE","NUMPAGES","SECTION","SECTIONPAGES"):
                runs += f'<w:r><w:t xml:space="preserve">{key}=</w:t></w:r><w:fldSimple w:instr="{key}"><w:r><w:t>12345</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve"> </w:t></w:r>'
            extra[path_in_zip]=f'<w:{tag} xmlns:w="{make["W"]}"><w:p>{runs}</w:p></w:{tag}>'
    make["package"](path, "".join(body), rels, extra, types)


def printed_pages(cli, docx, pdf):
    subprocess.run([cli, "convert", docx, pdf], capture_output=True, check=True, timeout=120)
    text = subprocess.check_output(["pdftotext", "-layout", pdf, "-"], timeout=120).decode()
    return text.rstrip("\f").split("\f")


def compare_rasters(first, second):
    with tempfile.TemporaryDirectory() as scratch:
        groups=[]
        for index,pdf in enumerate((first,second)):
            prefix=Path(scratch)/f"version{index}"
            subprocess.run(["pdftoppm","-r","48",pdf,prefix],capture_output=True,check=True,timeout=150)
            images=sorted(prefix.parent.glob(prefix.name+"-*.ppm"))
            groups.append([digest(image) for image in images])
        assert groups[0] and groups[0]==groups[1]
        return groups[0]


def verify(binary, out, grow=False, facing=False, reference_cli=None, stories=False, anchors=False):
    original, working = out / "原始分节.docx", out / "分节 编辑.docx"
    fixture(original, facing, stories, anchors)
    sha = digest(original)
    shutil.copyfile(original, working)
    key, number = secrets.token_hex(32), port()
    env = {"JOYWORD_CONTROL_KEY": key, "JOYWORD_CONFIG_DIR": str(out / "profile"), "JOYWORD_PERF": "1"}
    args = ["--control", str(number), "--", str(working)]
    log = out / "desktop.log"

    def inspect():
        return word_request(number, "document.inspect", key=key).get("result")

    def command(action, **params):
        state = inspect()
        reply = word_request(number, "engine.execute", {"doc": state["doc"], "version": state["version"],
                             "command": action, "params": params}, key)
        if not reply["ok"]:
            raise RuntimeError(reply["error"])

    def events(offset):
        with log.open() as f:
            f.seek(offset)
            return [json.loads(line.removeprefix("joyword-perf ")) for line in f
                    if line.startswith("joyword-perf ") and line.endswith("}\n")]

    insert = "Edited clause. " * (740 if grow else 18)
    patches = []
    with app(binary, args, out, env) as proc:
        first = wait(inspect, "opened reference", proc)
        total = len(first["summary"]["pages"])
        first_page = 35
        if facing:
            reference = printed_pages(reference_cli or binary.with_name("joyword-cli"), original, out / "original.pdf")
            assert len(reference) == total
            first_page = next(i for i, text in enumerate(reference, 1) if "Section 34" in text)
            next_side = next(i for i, text in enumerate(reference, 1) if "Section 36" in text)
            assert first_page % 2 == 1 and next_side % 2 == 0
        else:
            assert total == 69
        previous_pages = total
        command("edit.goto", page=first_page)
        wait(lambda: any(e.get("stage") == "raster_installed" and e.get("page") == first_page for e in events(0)),
             "chapter displayed", proc)
        expected = first["text"].replace("Section 34", insert + "Section 34", 1)
        for action in ("text.insert", "edit.undo", "edit.redo"):
            offset = log.stat().st_size
            command(action, **({"text": insert} if action == "text.insert" else {}))
            after = inspect()
            assert after["text"] == (first["text"] if action == "edit.undo" else expected)
            current_pages = len(after["summary"]["pages"])
            if grow and action != "edit.undo":
                assert current_pages > total
            else:
                assert current_pages == total
            patch = wait(lambda: next((e for e in events(offset) if e.get("stage") == "layout_patch"), None),
                         "affected pages calculated by the editor", proc)
            last_dirty = patch["lastPage"]
            assert patch["firstPage"] == (1 if stories and previous_pages != current_pages else first_page), patch
            if previous_pages != current_pages:
                assert last_dirty == max(previous_pages, current_pages), patch
            else:
                assert first_page <= last_dirty <= first_page + (8 if facing else 0), patch
            wait(lambda: any(e.get("stage") == "raster_installed" and first_page <= e.get("page", 0) <= last_dirty for e in events(offset)),
                 "edited page raster", proc)
            patches.append({"action": action, "firstPage": patch["firstPage"], "lastPage": last_dirty, "pageCount": current_pages})
            previous_pages = current_pages
        keypress = word_keys(proc)
        keypress("new")
        keypress("enter")
        keypress("enter")
        def saved():
            with ZipFile(working) as z:
                doc = ET.fromstring(z.read("word/document.xml"))
                notes = ET.fromstring(z.read("word/footnotes.xml"))
            if insert not in "".join(doc.itertext()):
                return False
            assert doc.find(".//{*}tbl/{*}tr/{*}tc/{*}tbl") is not None
            assert doc.find(".//{*}tblpPr") is not None
            if facing:
                with ZipFile(working) as z:
                    settings = ET.fromstring(z.read("word/settings.xml"))
                assert settings.find("{*}mirrorMargins") is not None and settings.find("{*}evenAndOddHeaders") is not None
                assert sum(next((v for k,v in t.attrib.items() if k.endswith('}val')),None) in ("evenPage","oddPage") for t in doc.findall(".//{*}sectPr/{*}type")) == 3
            assert sum("Numbered note" in "".join(n.itertext()) for n in notes) == 3
            return True
        wait(saved, "saved document and numbered notes", proc)
    with app(binary, args, out, env, "reopen.log") as proc:
        reopened = wait(inspect, "saved copy reopened", proc)
        assert reopened["text"] == expected and reopened["summary"]["pages"] == after["summary"]["pages"]
    assert digest(original) == sha
    return {"binarySha256": digest(binary), "sourceSha256": sha, "pages": total, "sections": 70, "numberedNotes": 3,
            "nativePatches": patches, "reopenedPageCount": previous_pages, "savedAndReopened": True, "sourceUnchanged": True,
            "savedNestedAndFloatingTable": True}


def verify_readers(out, baseline, candidate, pages):
    run = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))["run_cli"]
    saved = out / "分节 编辑.docx"
    expected = run(candidate, "inspect", "--session", saved)
    assert len(expected["pages"]) == pages
    original = digest(saved)
    builds = (("baseline", baseline), ("candidate", candidate))
    checked = []
    for writer, program in builds:
        copy = out / f"{writer}-resaved.docx"
        run(program, "convert", saved, copy)
        for reader, other in builds:
            opened = run(other, "inspect", "--session", copy)
            assert (opened["text"], opened["pages"]) == (expected["text"], expected["pages"])
            checked.append([writer, reader])
    assert digest(saved) == original
    return {"cliSha256": {label: digest(binary) for label, binary in builds},
            "writerReaderPairs": checked, "pageCount": pages, "savedCopyUnchanged": True}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("desktop", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--facing", action="store_true", help="include odd/even starts, mirrored margins and a numbered restart")
    parser.add_argument("--anchors", action="store_true", help="include a wrapped drawing, body page fields and a continued note")
    parser.add_argument("--stories", action="store_true", help="exercise linked first/even/default headers and global page fields")
    parser.add_argument("--grow", action="store_true", help="expand the edited section across fresh pages")
    parser.add_argument("--compare-cli", nargs=2, type=Path, metavar=("BASELINE", "CANDIDATE"))
    args = parser.parse_args()
    dest = args.output.resolve()
    dest.mkdir(parents=True, exist_ok=False)
    report = verify(args.desktop.resolve(), dest, args.grow, args.facing, args.compare_cli[0].resolve() if args.compare_cli else None, args.stories, args.anchors)
    if args.facing and args.compare_cli:
        saved = dest / "分节 编辑.docx"
        printed = [printed_pages(program.resolve(), saved, dest / f"edited-{i}.pdf") for i,program in enumerate(args.compare_cli)]
        assert printed[0] == printed[1] and len(printed[0]) == report["reopenedPageCount"]
        report["matchingPrintedPages"] = len(printed[0])
        if args.anchors:
            pages=printed[0]
            continuation=[i+1 for i,p in enumerate(pages) if "Continuation record" in p]
            assert len(continuation)>=2 and sum(p.count("Continuation record") for p in pages)==54
            raw=subprocess.check_output(["pdftotext","-raw",dest/"edited-1.pdf","-"],timeout=120).decode()
            words="".join(raw.split())
            assert f"Body-NUMPAGES={len(pages)}" in words and "Body-SECTION=35" in words
            with ZipFile(saved) as z:
                document=ET.fromstring(z.read("word/document.xml"))
                assert document.find(".//{*}anchor/{*}wrapSquare") is not None
            report["continuedNotePages"]=continuation
            report["continuedParagraphs"]=54
            report["anchorSurvivedSave"]=True
            report["rasterSha256At48Dpi"]=compare_rasters(dest/"edited-0.pdf",dest/"edited-1.pdf")
            assert len(report["rasterSha256At48Dpi"])==len(pages)
        if args.stories:
            assert all(f"NUMPAGES={len(printed[0])} " in page for page in printed[0])
            assert all(any(label+" PAGE=" in p for p in printed[0]) for label in ("Main","Even","Title","Final","Foot"))
            report["everyPageHasCorrectTotal"] = True
            report["runningStories"] = ["default", "even", "first", "inherited", "newDefault", "footer"]
    if args.compare_cli:
        report["crossVersion"] = verify_readers(dest, *(p.resolve() for p in args.compare_cli),
                                                report["reopenedPageCount"])
    (dest / "report.json").write_text(json.dumps(report, indent=2) + "\n")
