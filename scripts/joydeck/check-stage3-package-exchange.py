"""Check JoyDeck native projects and PresentationML exchange against the frozen binaries."""

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import posixpath
import subprocess
import sys
from urllib.parse import unquote, urlsplit
from xml.etree import ElementTree as ET
from zipfile import ZIP_DEFLATED, ZIP_STORED, ZipFile, ZipInfo


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests/fixtures/joydeck"
P = "http://schemas.openxmlformats.org/presentationml/2006/main"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
CX = "http://schemas.microsoft.com/office/drawing/2014/chartex"
TYPES = {
    "pptx": "presentation",
    "potx": "template",
    "ppsx": "slideshow",
}
MARKUP = b"<!-- JoyDeck phase 3 retained --><?phase3 keep?>"
LEGACY_REVIEW = "r9-ui2/legacy-review-media.joydeck"


def digest(value):
    return hashlib.sha256(value).hexdigest()


def invoke(exe, profile, *args):
    environment = os.environ.copy()
    environment.update(APPDATA=str(profile / "roaming"), LOCALAPPDATA=str(profile / "local"))
    return subprocess.run([exe, *args], env=environment, capture_output=True, text=True,
                          encoding="utf-8", timeout=90)


def success(exe, profile, *args):
    result = invoke(exe, profile, *args)
    assert result.returncode == 0, (exe.name, args, result.stderr)
    return result.stdout


def pack(path, parts):
    with ZipFile(path, "x", compression=ZIP_DEFLATED) as archive:
        for name, data in parts.items():
            archive.writestr(name, data)


def unpack(path):
    with ZipFile(path) as archive:
        entries = [info.filename for info in archive.infolist() if not info.is_dir()]
        assert len(entries) == len({entry.lower() for entry in entries}), path
        return {entry: archive.read(entry) for entry in entries}


def part_hashes(path):
    return {name: digest(data) for name, data in unpack(path).items()}


def local(name):
    return name.split("}")[-1]


def relationship_graph(path, extension):
    parts = unpack(path)
    inventory = {name.casefold(): name for name in parts}
    relationships = {}
    missing = []
    for name, data in parts.items():
        if not name.endswith(".rels"):
            continue
        root = ET.fromstring(data)
        folder, rels_name = posixpath.split(name)
        owner = posixpath.join(posixpath.dirname(folder), rels_name.removesuffix(".rels"))
        source = posixpath.dirname(owner)
        resolved = {}
        for rel in root:
            if local(rel.tag) != "Relationship":
                continue
            rid = rel.attrib["Id"]
            kind = rel.attrib["Type"]
            uri = rel.attrib["Target"]
            external = rel.get("TargetMode", "").casefold() == "external"
            if external:
                target = uri
            else:
                raw = unquote(urlsplit(uri).path).replace("\\", "/")
                absolute = raw.lstrip("/") if raw.startswith("/") else posixpath.join(source, raw)
                key = posixpath.normpath(absolute).casefold()
                target = inventory.get(key)
                if target is None:
                    missing.append((owner, kind.rsplit("/", 1)[-1], key))
            assert rid not in resolved, (name, rid)
            resolved[rid] = (kind, target, external)
        relationships[owner] = resolved
    roots = [target for kind, target, external in relationships.get("", {}).values()
             if kind.endswith("/officeDocument") and not external]
    assert len(roots) == 1 and roots[0] is not None, path
    main = roots[0]
    types = ET.fromstring(parts["[Content_Types].xml"])
    ct = next(item.attrib["ContentType"] for item in types
              if item.get("PartName", "").lstrip("/") == main)
    if extension is not None:
        assert ct.endswith(f"presentationml.{TYPES[extension]}.main+xml"), (path, ct)
    doc = ET.fromstring(parts[main])
    targets = relationships.get(main, {})
    def file_ids(part, rels, tag, root):
        result = []
        for node in root.findall(f"./{{{P}}}{tag}IdLst/{{{P}}}{tag}Id"):
            rid = node.attrib[f"{{{R}}}id"]
            kind, target, external = rels[rid]
            assert kind.endswith("/" + tag.replace("sld", "slide")) and target and not external
            result.append((int(node.attrib["id"]), target))
        assert len(result) == len({file_id for file_id, _ in result}), (path, part, tag)
        return result

    slides = file_ids(main, targets, "sld", doc)
    masters = file_ids(main, targets, "sldMaster", doc)
    layouts = {master: file_ids(master, relationships.get(master, {}), "sldLayout",
                               ET.fromstring(parts[master])) for _, master in masters}
    timing = {}
    for _, slide in slides:
        root = ET.fromstring(parts[slide])
        nodes = [node for node in root if local(node.tag) in {"timing", "transition", "AlternateContent"}]
        if nodes:
            value = b"".join(ET.tostring(node, encoding="utf-8") for node in nodes)
            timing[slide] = digest(value)
    media = {name: digest(data) for name, data in parts.items()
             if name.startswith(("ppt/media/", "ppt/embeddings/"))}
    parts_by_type = Counter(kind.rsplit("/", 1)[-1] for owner in relationships.values()
                            for kind, _, _ in owner.values())
    edges = digest(json.dumps(relationships, sort_keys=True).encode())
    return {"slides": slides, "masters": masters, "layouts": layouts, "contentType": ct,
            "relationshipKinds": dict(parts_by_type), "relationshipSha256": edges,
            "mediaSha256": media, "timingSha256": timing,
            "missingRelationships": sorted(missing)}


def mismatch(left, right, key="$"):
    if type(left) is type(right) and isinstance(left, dict) and left.keys() == right.keys():
        for name in left:
            if left[name] != right[name]:
                return mismatch(left[name], right[name], f"{key}.{name}")
    elif type(left) is type(right) and isinstance(left, list) and len(left) == len(right):
        for index, (a, b) in enumerate(zip(left, right)):
            if a != b:
                return mismatch(a, b, f"{key}[{index}]")
    return key, str(left)[:210], str(right)[:210]


def assert_equal(left, right, label):
    assert left == right, (label, mismatch(left, right))


def compare_legacy_review(frozen, candidate):
    # Stage 2 began preserving the picture fallback of this unattached legacy audio object.
    # The frozen file has no poster, and neither output can invent the missing audio bytes.
    before = frozen["slides"][0]["shapes"][1]
    after = candidate["slides"][0]["shapes"][1]
    assert before["kind"] == {"type": "shape"}
    assert after["kind"]["type"] == "picture"
    assert after["kind"]["fill"]["media"] == 13
    assert frozen["media"] == []
    assert candidate["media"] == [{
        "id": 13, "name": "joydeck-1-image1.png", "contentType": "image/png", "link": None,
    }]
    assert candidate["nextId"] == frozen["nextId"] + 1 == 13
    after["kind"] = before["kind"]
    candidate["media"] = []
    candidate["nextId"] = frozen["nextId"]
    assert_equal(frozen, candidate, LEGACY_REVIEW)


def make_extended_chart(path):
    parts = unpack(FIXTURES / "r4-r5/independent-complex.pptx")
    chart = "http://schemas.openxmlformats.org/drawingml/2006/chart"
    slide = "ppt/slides/slide2.xml"
    old = f'<a:graphicData uri="{chart}"><c:chart xmlns:c="{chart}" r:id="rId4"/></a:graphicData>'
    new = f'<a:graphicData uri="{CX}"><cx:chart xmlns:cx="{CX}" r:id="rId4"/></a:graphicData>'
    assert parts[slide].count(old.encode()) == 1
    parts[slide] = parts[slide].replace(old.encode(), new.encode())
    rels = "ppt/slides/_rels/slide2.xml.rels"
    old_type = f'{R}/chart"'
    new_type = 'http://schemas.microsoft.com/office/2014/relationships/chartEx"'
    assert parts[rels].count(old_type.encode()) == 1
    parts[rels] = parts[rels].replace(old_type.encode(), new_type.encode())
    content_types = "[Content_Types].xml"
    parts[content_types] = parts[content_types].replace(
        b"application/vnd.openxmlformats-officedocument.drawingml.chart+xml",
        b"application/vnd.ms-office.chartex+xml",
    )
    parts["ppt/charts/chart1.xml"] = f"""<cx:chartSpace xmlns:cx="{CX}" xmlns:r="{R}">
      <cx:chartData><cx:data id="7">
        <cx:strDim type="cat"><cx:lvl ptCount="3">
          <cx:pt idx="2">Delta</cx:pt><cx:pt idx="0">Alpha</cx:pt>
        </cx:lvl><cx:lvl ptCount="3"><cx:pt idx="2">East</cx:pt><cx:pt idx="0">West</cx:pt></cx:lvl></cx:strDim>
        <cx:numDim type="colorVal"><cx:lvl ptCount="1"><cx:pt idx="0">100</cx:pt></cx:lvl></cx:numDim>
        <cx:numDim type="size"><cx:lvl ptCount="3" formatCode="0.00">
          <cx:pt idx="2">31</cx:pt><cx:pt idx="0">11</cx:pt>
        </cx:lvl></cx:numDim>
      </cx:data></cx:chartData>
      <cx:chart><cx:plotArea><cx:plotAreaRegion>
        <cx:series layoutId="treemap"><cx:tx><cx:txData><cx:v>Value</cx:v></cx:txData></cx:tx>
          <cx:dataId val="7"/>
        </cx:series>
      </cx:plotAreaRegion></cx:plotArea></cx:chart>
    </cx:chartSpace>""".encode()
    pack(path, parts)


def verify_extended_chart(model):
    [chart] = [shape["kind"] for shape in model["slides"][1]["shapes"]
               if shape["kind"]["type"] == "chart"]
    assert chart["kind"] == "treemap"
    assert chart["categories"] == ["Alpha", "", "Delta"]
    assert chart["categoryLevels"] == [["West", "", "East"]]
    assert chart["series"][0]["values"] == [11.0, None, 31.0]
    assert chart["series"][0]["numberFormat"] == "0.00"


def compile_inventory(exes, work, report):
    seed = work / "no changes.json"
    seed.write_text("[]", encoding="utf-8")
    native_source = work / "native-with-extension.joydeck"
    template = work / "native-seed.joydeck"
    success(exes["frozen"], work / "frozen", "edit", FIXTURES / "independent.pptx",
            "--script", seed, "--output", template)
    parts = unpack(template)
    manifest = json.loads(parts["manifest.json"])
    manifest["phase3_extension"] = {"v": 1, "label": "保留原始附加部件"}
    parts["manifest.json"] = json.dumps(manifest).encode()
    parts["extension/opaque.bin"] = b"opaque native extension\x00\x83\xff"
    pack(native_source, parts)
    chart_source = work / "extended-chart-hierarchy.pptx"
    make_extended_chart(chart_source)
    cases = [path for path in sorted(FIXTURES.rglob("*"))
             if path.suffix in {".pptx", ".joydeck", ".deckcraft"} and path.name != "broken.pptx"]
    cases.extend((native_source, chart_source))
    sources = {}
    for i, source in enumerate(cases):
        label = source.name if source.parent == work else source.relative_to(FIXTURES).as_posix()
        target = work / f"sample-{i:02}"
        target.mkdir()
        row = {"name": label, "inputSha256": digest(source.read_bytes()), "reads": 0, "outputs": {}}
        models = {}
        for version, exe in exes.items():
            profile = work / version
            info = json.loads(success(exe, profile, "inspect", source))
            model = json.loads(success(exe, profile, "content", source))
            models[version] = model
            row["reads"] += 1
            sources[label] = info["format"]
        assert_equal(models["frozen"], models["candidate"], label)
        if source == chart_source:
            verify_extended_chart(models["candidate"])

        def reopen(file):
            views = []
            for version, exe in exes.items():
                views.append(json.loads(success(exe, work / version, "content", file)))
                row["reads"] += 1
            assert_equal(*views, file.name)
            if source == chart_source:
                verify_extended_chart(views[1])
            return views[0]

        for mode, ext in (("native", "joydeck"), ("preserve", "pptx"), ("regenerate", "pptx")):
            if mode == "preserve" and sources[label] not in {"pptx", "joydeck", "joydeck-v1"}:
                continue
            writer_models = {}
            record = {}
            for version, exe in exes.items():
                profile = work / version
                dest = target / f"{mode}-{version}.{ext}"
                if mode == "native":
                    command = (["migrate", source, "--output", dest]
                               if sources[label] == "joydeck-v1"
                               else ["edit", source, "--script", seed, "--output", dest])
                else:
                    command = ["exchange", source, "--mode", mode, "--output", dest]
                result = invoke(exe, profile, *command)
                record[version] = {"status": result.returncode}
                if result.returncode:
                    assert not dest.exists()
                    record[version]["diagnostic"] = result.stderr.strip()
                    continue
                writer_models[version] = reopen(dest)
                if mode == "native":
                    written = unpack(dest)
                    if sources[label] == "joydeck-v1":
                        assert written["migration/legacy-original.joydeck"] == source.read_bytes()
                    elif source == native_source:
                        assert written["extension/opaque.bin"] == parts["extension/opaque.bin"]
                        assert written["retained-original-model.deckcraft"] == parts["model.deckcraft"]
                        assert json.loads(written["manifest.json"])["phase3_extension"] == manifest["phase3_extension"]
                        assert written["source.package"] == parts["source.package"]
                    else:
                        assert written["source.package"] == source.read_bytes()
                elif mode == "preserve" and sources[label] == "pptx":
                    assert_equal(part_hashes(source), part_hashes(dest), (label, version, mode))
                    record[version]["exactParts"] = len(unpack(dest))
                elif mode == "regenerate":
                    record[version]["graph"] = relationship_graph(dest, "pptx")
                record[version]["sha256"] = digest(dest.read_bytes())
            assert_equal(record["frozen"]["status"], record["candidate"]["status"], (label, mode))
            if record["frozen"]["status"]:
                assert_equal(record["frozen"]["diagnostic"], record["candidate"]["diagnostic"], (label, mode))
            else:
                if (label, mode) == (LEGACY_REVIEW, "preserve"):
                    compare_legacy_review(writer_models["frozen"], writer_models["candidate"])
                    row["existingDifference"] = "picture placeholder for legacy audio, introduced in phase 2"
                else:
                    assert_equal(writer_models["frozen"], writer_models["candidate"], (label, mode))
                if mode == "regenerate":
                    assert_equal(record["frozen"]["graph"], record["candidate"]["graph"], (label, mode))
            row["outputs"][mode] = record
        assert row["inputSha256"] == digest(source.read_bytes())
        report["samples"].append(row)
        print(f"{label} passed {row['reads']} reads", flush=True)


def probes(exes, work, report):
    profile = work / "candidate"
    candidate = exes["candidate"]
    public = FIXTURES / "independent.pptx"
    for ext in ("potx", "ppsx"):
        file = work / f"generated.{ext}"
        success(candidate, profile, "exchange", public, "--mode", "regenerate", "--output", file)
        graph = relationship_graph(file, ext)
        assert not graph["missingRelationships"], graph["missingRelationships"]
        a = json.loads(success(candidate, profile, "content", file))
        b = json.loads(success(exes["frozen"], work / "frozen", "content", file))
        assert_equal(a, b, f"read back {ext}")
        report["templates"][ext] = graph

    parts = unpack(public)
    slide = "ppt/slides/slide2.xml"
    case = "ppt/slides/Slide2.xml"
    original = parts.pop(slide)
    end = original.rindex(b"</p:sld>")
    parts[case] = original[:end] + MARKUP + original[end:]
    source = work / "case-spelling-and-markup.pptx"
    pack(source, parts)
    commands = work / "preserve markup.json"
    commands.write_text(json.dumps([
        {"command": "select", "page": 2, "shapes": []},
        {"command": "content", "action": {"kind": "background", "rgb": 0x265DA2}},
    ]), encoding="utf-8")
    dest = work / "preserve markup.pptx"
    success(candidate, profile, "edit", source, "--script", commands, "--output", dest)
    output = unpack(dest)
    assert case in output and slide not in output and MARKUP in output[case]
    changed = {name for name, data in output.items() if data != parts.get(name)}
    assert changed == {case}, changed
    report["retainedMarkup"] = sorted(changed)

    notes_source = FIXTURES / "r4-r5/independent-complex.pptx"
    original = part_hashes(notes_source)
    commands = work / "edit notes.json"
    marker = "Phase 3 preserves all other parts"
    commands.write_text(json.dumps([
        {"command": "select", "page": 2, "shapes": []},
        {"command": "set-text", "target": {"kind": "notes"},
         "body": {"paragraphs": [{"runs": [{"text": marker}]}]}},
    ]), encoding="utf-8")
    dest = work / "edit notes.pptx"
    success(candidate, profile, "edit", notes_source, "--script", commands, "--output", dest)
    edited = part_hashes(dest)
    assert original.keys() == edited.keys()
    changed = [part for part in original if original[part] != edited[part]]
    assert changed == ["ppt/notesSlides/notesSlide2.xml"], changed
    for version, exe in exes.items():
        view = json.loads(success(exe, work / version, "inspect", dest))
        assert view["slides"][1]["notes"] == marker
    report["notesOnlyChangedParts"] = changed

    native = unpack(work / "native-with-extension.joydeck")
    manifest = json.loads(native["manifest.json"])
    bad_source = unpack(public)
    refusals = {
        "broken-pptx": FIXTURES / "broken.pptx",
    }
    for name, edit in (
        ("future-native", {"version": 99}),
        ("unknown-capability", {"required_features": ["future-required-v99"]}),
        ("unbound-source", {"source_format": "not-the-source-format"}),
    ):
        path = work / f"{name}.joydeck"
        pack(path, native | {"manifest.json": json.dumps(manifest | edit).encode()})
        refusals[name] = path
    dtd = work / "utf16-dtd.pptx"
    pack(dtd, bad_source | {"extra/utf16.xml": "<?xml version='1.0' encoding='UTF-16'?><!DOCTYPE x><x/>".encode("utf-16")})
    refusals["utf16-dtd"] = dtd
    linked = work / "symlink-metadata.pptx"
    metadata = ZipInfo("extra/link.txt")
    metadata.create_system = 3
    metadata.external_attr = 0o120777 << 16
    pack(linked, bad_source | {metadata: b"part name with Unix link metadata"})
    refusals["symlink-metadata"] = linked
    crc = work / "corrupt-member.pptx"
    marker = b"phase-3-test-of-payload-integrity"
    pack(crc, bad_source)
    with ZipFile(crc, "a", compression=ZIP_STORED) as archive:
        archive.writestr("extra/check.bin", marker)
    crc.write_bytes(crc.read_bytes().replace(marker, bytes([marker[0] ^ 1]) + marker[1:], 1))
    refusals["bad-member-crc"] = crc
    for label, source in refusals.items():
        sha = digest(source.read_bytes())
        errors = {}
        for version, exe in exes.items():
            result = invoke(exe, work / version, "inspect", source)
            if label != "utf16-dtd" or version == "candidate":
                assert result.returncode != 0, (label, version)
            errors[version] = {"status": result.returncode, "diagnostic": result.stderr.strip()}
        assert sha == digest(source.read_bytes())
        report["invalidInputs"][label] = errors

    for label, member in (
        ("signed", "_xmlsignatures/origin.sigs"),
        ("macro-enabled", "ppt/vbaProject.bin"),
    ):
        source = work / f"{label}-source.pptx"
        pack(source, bad_source | {member: b"preservation-refusal fixture"})
        sha = digest(source.read_bytes())
        for version, exe in exes.items():
            dest = work / f"{label}-{version}.pptx"
            result = invoke(exe, work / version, "exchange", source, "--mode", "preserve", "--output", dest)
            assert result.returncode != 0 and not dest.exists(), (label, version)
            assert label in result.stderr.lower(), (label, version, result.stderr)
        assert sha == digest(source.read_bytes())
        report["writeRefusals"].append(label)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--frozen", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    args = parser.parse_args()
    work = args.folder.resolve()
    work.mkdir(parents=True)
    exes = {"frozen": args.frozen.resolve(), "candidate": args.candidate.resolve()}
    report = {"binarySha256": {name: digest(path.read_bytes()) for name, path in exes.items()},
              "samples": [], "invalidInputs": {}, "writeRefusals": [], "templates": {}}
    compile_inventory(exes, work, report)
    probes(exes, work, report)
    report["reads"] = sum(row["reads"] for row in report["samples"])
    report["successfulFiles"] = sum(
        item["status"] == 0 for row in report["samples"]
        for modes in row["outputs"].values() for item in modes.values()
    )
    report["expectedRefusals"] = sum(
        item["status"] != 0 for row in report["samples"]
        for modes in row["outputs"].values() for item in modes.values()
    )
    (work / "verification.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n",
                                            encoding="utf-8")
    print(f"{len(report['samples'])} samples, {report['reads']} reads, "
          f"{report['successfulFiles']} files, {report['expectedRefusals']} expected exchange refusals")


if __name__ == "__main__":
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    main()
