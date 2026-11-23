"""Verify Word section-break splice, MCP DOCX/PDF exchange, and live desktop bounded reflow."""
import argparse
import json
from pathlib import Path
import runpy
import secrets
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import app, digest, port, wait, word_request

SECTIONS = runpy.run_path(str(Path(__file__).with_name("check-section-reflow.py")))
CORPUS = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))


def build_fixture(path: Path) -> None:
    def text(value: str) -> str:
        return f'<w:r><w:t xml:space="preserve">{value}</w:t></w:r>'

    paragraphs = []
    for section in range(68):
        end = "<w:sectPr/>" if section < 67 else ""
        paragraphs.append(f'<w:p>{text(f"Heading {section}")}</w:p>')
        paragraphs.append(f'<w:p>{text(f"Section {section} splice body alpha beta")}</w:p>')
        paragraphs.append(f'<w:p><w:pPr>{end}</w:pPr>{text("End")}</w:p>')
    paragraphs.append("<w:sectPr/>")
    styles = (
        f'<w:styles xmlns:w="{CORPUS["W"]}"><w:docDefaults><w:rPrDefault><w:rPr>'
        '<w:rFonts w:ascii="Source Sans 3" w:hAnsi="Source Sans 3"/></w:rPr>'
        "</w:rPrDefault></w:docDefaults></w:styles>"
    )
    prefix = "application/vnd.openxmlformats-officedocument.wordprocessingml."
    CORPUS["package"](
        path,
        "".join(paragraphs),
        [("s", "styles", "styles.xml")],
        {"word/styles.xml": styles},
        {"word/styles.xml": prefix + "styles+xml"},
    )


def section_count(docx_path: Path) -> int:
    ns = {"w": CORPUS["W"]}
    with ZipFile(docx_path) as archive:
        root = ET.fromstring(archive.read("word/document.xml"))
    return len(root.findall(".//w:sectPr", ns))


def mcp_run(cli: Path, requests: list[tuple[str, dict]], root: Path) -> list[dict]:
    lines = [
        json.dumps(
            {
                "jsonrpc": "2.0",
                "id": i,
                "method": "tools/call",
                "params": {"name": tool, "arguments": args},
            }
        )
        for i, (tool, args) in enumerate(requests, 1)
    ]
    res = subprocess.run(
        [
            str(cli),
            "mcp",
            "--automation-read-root",
            str(root),
            "--automation-write-root",
            str(root),
        ],
        input="\n".join(lines) + "\n",
        capture_output=True,
        text=True,
        check=True,
        timeout=120,
    )
    replies = [json.loads(line)["result"] for line in res.stdout.splitlines() if line.strip()]
    assert len(replies) == len(requests), (len(replies), len(requests))
    assert all(not r.get("isError") for r in replies), replies
    return replies


def run(desktop: Path, baseline_cli: Path, candidate_cli: Path, folder: Path) -> dict:
    folder.mkdir(parents=True, exist_ok=True)
    original = folder / "原始分节.docx"
    working = folder / "分节 变更.docx"
    build_fixture(original)
    source_sha = digest(original)
    shutil.copyfile(original, working)
    assert section_count(original) == 68

    # 1. Exercise all 4 section-break kinds + undo/redo + save via candidate MCP CLI
    mcp_steps = [
        ("open_document", {"path": str(working)}),
        ("execute", {"command": "edit.goto", "params": {"page": 35}}),
    ]
    for kind in ("nextPage", "continuous", "oddPage", "evenPage"):
        mcp_steps.append(("execute", {"command": "layout.break", "params": {"kind": kind}}))
        mcp_steps.append(("execute", {"command": "edit.undo", "params": {}}))
        mcp_steps.append(("execute", {"command": "edit.redo", "params": {}}))
        if kind != "evenPage":
            mcp_steps.append(("execute", {"command": "edit.undo", "params": {}}))
    mcp_steps.append(("execute", {"command": "file.save", "params": {"path": str(working)}}))
    mcp_run(candidate_cli, mcp_steps, folder)
    assert section_count(working) == 69

    # 2. Cross-version PDF export & page-by-page raster comparison (baseline vs candidate)
    old_pdf = folder / "baseline.pdf"
    new_pdf = folder / "candidate.pdf"
    old_pages = SECTIONS["printed_pages"](baseline_cli, working, old_pdf)
    new_pages = SECTIONS["printed_pages"](candidate_cli, working, new_pdf)
    assert len(old_pages) == len(new_pages) and old_pages == new_pages
    rasters = SECTIONS["compare_rasters"](old_pdf, new_pdf)
    assert len(rasters) == len(new_pages)

    # 3. Cross-version DOCX resave check
    cross = {}
    for label, cli in (("baseline", baseline_cli), ("candidate", candidate_cli)):
        resaved = folder / f"{label}-resaved.docx"
        mcp_run(
            cli,
            [
                ("open_document", {"path": str(working)}),
                ("execute", {"command": "file.saveAs", "params": {"path": str(resaved)}}),
            ],
            folder,
        )
        assert section_count(resaved) == 69
        cross[label] = {"sections": 69, "sha256": digest(resaved)}

    # 4. Native desktop JoyWord session: open 69-section doc, paragraph-splice insert/undo/redo
    endpoint = port()
    key = secrets.token_hex(32)
    args = ["--control", str(endpoint), "--", str(working)]
    env = {
        "JOYWORD_CONTROL_KEY": key,
        "JOYWORD_CONFIG_DIR": str(folder / "config"),
        "JOYWORD_PERF": "1",
    }
    log = folder / "desktop.log"

    def inspect():
        return word_request(endpoint, "document.inspect", key=key).get("result")

    def execute(proc, action: str, **params):
        state = wait(inspect, "current document revision", proc)
        answer = word_request(
            endpoint,
            "engine.execute",
            {"doc": state["doc"], "version": state["version"], "command": action, "params": params},
            key,
        )
        assert answer.get("ok"), (action, answer)
        return answer

    def events(offset: int):
        with log.open(encoding="utf-8", errors="replace") as stream:
            stream.seek(offset)
            return [
                json.loads(line.removeprefix("joyword-perf "))
                for line in stream
                if line.startswith("joyword-perf ") and line.endswith("}\n")
            ]

    patches = []
    with app(desktop, args, folder, env) as proc:
        opened = wait(inspect, "opened 69-section document", proc)
        assert len(opened["summary"]["pages"]) == len(new_pages)
        execute(proc, "edit.goto", page=35)
        wait(
            lambda: any(e.get("stage") == "raster_installed" and e.get("page") == 35 for e in events(0)),
            "raster of section 35",
            proc,
        )
        for action, params in [
            ("text.insert", {"text": "SpliceParaA\nSpliceParaB\n", "paragraphs": True}),
            ("edit.undo", {}),
            ("edit.redo", {}),
        ]:
            offset = log.stat().st_size
            execute(proc, action, **params)
            patch = wait(
                lambda: next((e for e in events(offset) if e.get("stage") == "layout_patch"), None),
                f"bounded reflow on {action}",
                proc,
            )
            patches.append(
                {
                    "action": action,
                    "firstPage": patch["firstPage"],
                    "lastPage": patch["lastPage"],
                    "pages": len(inspect()["summary"]["pages"]),
                }
            )

    assert digest(original) == source_sha
    return {
        "sourceSha256": source_sha,
        "desktopSha256": digest(desktop),
        "cliSha256": [digest(baseline_cli), digest(candidate_cli)],
        "initialSections": 68,
        "savedSections": 69,
        "finalPages": len(new_pages),
        "mcpBreaksVerified": ["nextPage", "continuous", "oddPage", "evenPage"],
        "twoVersionPageRastersCount": len(rasters),
        "twoVersionRastersMatch": True,
        "crossVersion": cross,
        "nativePatches": patches,
        "originalsUnchanged": True,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("desktop", "baseline", "candidate", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    report = run(args.desktop.resolve(), args.baseline.resolve(), args.candidate.resolve(), out)
    (out / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
