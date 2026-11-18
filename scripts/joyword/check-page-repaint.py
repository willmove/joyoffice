"""Verify a changed page is repainted and its unchanged neighbor is retained in a native window."""

import argparse
import json
from pathlib import Path
import runpy
import secrets
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import app, digest, port, wait, word_request


def check(binary, out):
    make = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))
    doc = out / "columns-and-note.docx"
    body = []
    for n in range(1, 7):
        props = "" if n == 1 else "<w:pPr><w:pageBreakBefore/></w:pPr>"
        note = '<w:r><w:footnoteReference w:id="1"/></w:r>' if n == 1 else ""
        body.append(f"<w:p>{props}<w:r><w:t>Section {n} first line</w:t></w:r>{note}</w:p>")
        body.append(f"<w:p><w:r><w:t>Unchanged line {n}</w:t></w:r></w:p>")
    body.append('<w:sectPr><w:cols w:num="2"/></w:sectPr>')
    notes = (f'<w:footnotes xmlns:w="{make["W"]}"><w:footnote w:id="1"><w:p><w:r>'
             '<w:t>Footnote on the page being edited</w:t></w:r></w:p></w:footnote></w:footnotes>')
    make["package"](doc, "".join(body), [("n", "footnotes", "footnotes.xml")],
                    {"word/footnotes.xml": notes})
    original_sha = digest(doc)
    number, key = port(), secrets.token_hex(32)
    config = {"JOYWORD_CONFIG_DIR": str(out / "profile"), "JOYWORD_CONTROL_KEY": key, "JOYWORD_PERF": "1"}
    log = out / "desktop.log"

    def rows(offset=0):
        with log.open() as file:
            file.seek(offset)
            return [json.loads(line.removeprefix("joyword-perf ")) for line in file
                    if line.startswith("joyword-perf ") and line.endswith("}\n")]

    def inspect():
        return word_request(number, "document.inspect", key=key).get("result")

    def command(name, **params):
        state = inspect()
        response = word_request(number, "engine.execute", {
            "command": name, "params": params, "doc": state["doc"], "version": state["version"],
        }, key=key)
        if not response["ok"]:
            raise RuntimeError(response["error"])

    def observed(offset, stage, number):
        return next((item for item in rows(offset) if item["stage"] == stage and item["page"] == number), None)

    results = []
    with app(binary, ["--control", str(number), "--", str(doc)], out, config) as process:
        initial = wait(inspect, "opened DOCX", process)
        assert len(initial["summary"]["pages"]) == 6
        wait(lambda: observed(0, "raster_installed", 2), "the neighboring page's first raster", process)
        initial_text = initial["text"]
        for action in ("text.insert", "edit.undo"):
            offset = log.stat().st_size
            command(action, **({"text": "Two column note test. " * 8} if action == "text.insert" else {}))
            after = inspect()
            assert len(after["summary"]["pages"]) == 6
            if action == "edit.undo":
                assert after["text"] == initial_text
            else:
                assert after["text"].replace("Two column note test. " * 8, "", 1) == initial_text
                assert after["text"] != initial_text
            changed = wait(lambda: observed(offset, "raster_installed", 1), "fresh edited page", process)
            neighbor = wait(lambda: observed(offset, "raster_reused", 2), "unchanged neighboring page", process)
            assert not observed(offset, "raster_installed", 2)
            results.append({"action": action, "repaintedPage": changed["page"], "reusedPage": neighbor["page"]})
    assert digest(doc) == original_sha
    return {"binarySha256": digest(binary), "fixtureSha256": original_sha, "unchangedSource": True,
            "pageCount": 6, "columns": 2, "footnotes": 1, "actions": results}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("desktop", "out"):
        parser.add_argument(arg, type=Path)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    (out / "report.json").write_text(json.dumps(check(args.desktop.resolve(), out), indent=2) + "\n")
