"""Edit a section with newspaper columns, nested tables and numbered footnotes in the native Word window."""
import argparse
import json
from pathlib import Path
import runpy
import secrets
import shutil
import sys
import xml.etree.ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import app, digest, port, wait, word_request
from verify_save_changes import word_keys


def fixture(path):
    make = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))
    body = []
    for section in range(70):
        props = '<w:type w:val="continuous"/>' if section == 35 else ""
        if section == 34:
            props += '<w:cols w:num="2"/>'
        texts = [f"Section {section}", "Text after the heading"]
        if section == 34:
            texts += ["Alpha beta gamma. " * 6] * 8
        for i, text in enumerate(texts):
            marker = (f"<w:pPr><w:sectPr>{props}</w:sectPr></w:pPr>"
                      if i == len(texts) - 1 and section != 69 else "")
            note_id = {2: 1, 34: 2, 48: 3}.get(section) if i == 0 else None
            note = f'<w:r><w:footnoteReference w:id="{note_id}"/></w:r>' if note_id else ""
            body.append(f"<w:p>{marker}<w:r><w:t>{text}</w:t></w:r>{note}</w:p>")
            if section == 34 and i == 1:
                wp = '<w:tblPr><w:tblW w:w="1950" w:type="dxa"/>{}</w:tblPr>'
                grid = '<w:tblGrid><w:gridCol w:w="1950"/></w:tblGrid>'
                value = '<w:p><w:r><w:t>Nested value</w:t></w:r></w:p>'
                inside = f'<w:tbl>{wp.format("")}{grid}<w:tr><w:tc>{value}</w:tc></w:tr></w:tbl>'
                floating = '<w:tblpPr w:horzAnchor="text" w:vertAnchor="text" w:tblpX="100" w:leftFromText="80"/>'
                body.append(f'<w:tbl>{wp.format(floating)}{grid}<w:tr><w:tc>{inside}<w:p/></w:tc></w:tr></w:tbl>')
    body.append("<w:sectPr/>")
    notes = "".join(f'<w:footnote w:id="{i}"><w:p><w:r><w:t>Numbered note {i}</w:t></w:r></w:p></w:footnote>' for i in (1, 2, 3))
    make["package"](path, "".join(body), [("n", "footnotes", "footnotes.xml")],
                    {"word/footnotes.xml": f'<w:footnotes xmlns:w="{make["W"]}">{notes}</w:footnotes>'})


def verify(binary, out):
    original, working = out / "原始分节.docx", out / "分节 编辑.docx"
    fixture(original)
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

    insert = "Edited clause. " * 18
    patches = []
    with app(binary, args, out, env) as proc:
        first = wait(inspect, "opened reference", proc)
        total = len(first["summary"]["pages"])
        assert total == 69
        command("edit.goto", page=35)
        wait(lambda: any(e.get("stage") == "raster_installed" and e.get("page") == 35 for e in events(0)),
             "chapter displayed", proc)
        expected = first["text"].replace("Section 34", insert+"Section 34", 1)
        for action in ("text.insert", "edit.undo", "edit.redo"):
            offset = log.stat().st_size
            command(action, **({"text": insert} if action == "text.insert" else {}))
            after = inspect()
            assert after["text"] == (first["text"] if action == "edit.undo" else expected)
            assert len(after["summary"]["pages"]) == total
            patch = wait(lambda: next((e for e in events(offset) if e.get("stage") == "layout_patch"), None),
                         "affected pages calculated by the editor", proc)
            assert (patch["firstPage"], patch["lastPage"]) == (35, 35), patch
            wait(lambda: any(e.get("stage") == "raster_installed" and e.get("page") == 35 for e in events(offset)),
                 "edited page raster", proc)
            patches.append({"action": action, "firstPage": 35, "lastPage": 35})
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
            assert sum("Numbered note" in "".join(n.itertext()) for n in notes) == 3
            return True
        wait(saved, "saved document and numbered notes", proc)
    with app(binary, args, out, env, "reopen.log") as proc:
        reopened = wait(inspect, "saved copy reopened", proc)
        assert reopened["text"] == expected and len(reopened["summary"]["pages"]) == total
    assert digest(original) == sha
    return {"binarySha256": digest(binary), "sourceSha256": sha, "pages": total, "sections": 70, "numberedNotes": 3,
            "nativePatches": patches, "savedAndReopened": True, "sourceUnchanged": True,
            "savedNestedAndFloatingTable": True}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("desktop", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    dest = args.output.resolve()
    dest.mkdir(parents=True, exist_ok=False)
    (dest / "report.json").write_text(json.dumps(verify(args.desktop.resolve(), dest), indent=2) + "\n")
