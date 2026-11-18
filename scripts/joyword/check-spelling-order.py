"""Compare JoyWord's word acceptance, issue ranges and ordered spelling suggestions in two builds."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import secrets
import shutil
import subprocess
import sys
import tarfile
import xml.etree.ElementTree as ET
from xml.sax.saxutils import escape
from zipfile import ZipFile

PUBLISHED_INDEX = "b35c7aef0591177f18233b551da16035dcd615549ff874e169ffd19f16fea74b"
VOCABULARY = ["test/S", "toast", "tosty", "text", "tens", "tent", "the", "word", "worded", "house",
              "work", "case", "base", "space", "km", "tést", "best", "east", "Testy"]
CASES = ["teh", "tset", "tost", "wrod", "tést", "tests", "testing", "testinng", "tsets", "ßpase", "houseword", "km"]
EXPECTED = {"teh": ["the", "tens", "tent", "test", "text"], "tset": ["test"],
            "tost": ["toast", "test", "tést", "tosty", "tests"], "wrod": ["word"], "testing": [],
            "testinng": [], "tsets": ["tests"], "ßpase": [], "houseword": ["house word"]}
SAMPLE_AFFIX = b"SET UTF-8\nTRY estorawdinlmphbc\nMAP 1\nMAP e\xc3\xa9\nREP 1\nREP eh he\nSFX S Y 1\nSFX S 0 s .\n"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def make_document(document, locale, words):
    make = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))
    body = "".join(f'<w:p><w:r><w:rPr><w:lang w:val="{locale}"/></w:rPr><w:t>{escape(word)}</w:t></w:r></w:p>'
                   for word in words)
    make["package"](document, body+"<w:sectPr/>")


def results(binary, root, locale, words, kind="spelling"):
    document = root / "sample.docx"
    if not document.exists():
        make_document(document, locale, words)
    calls = [("open_document", {"path": str(document)}), ("execute", {"command": "review.issues", "params": {}})]
    stdin = "".join(json.dumps({"jsonrpc":"2.0", "id":i+1, "method":"tools/call",
                    "params":{"name":name, "arguments":args}})+"\n" for i, (name,args) in enumerate(calls))
    response = subprocess.run([binary, "mcp", "--automation-read-root", root], input=stdin,
                              env={**os.environ, "JOYWORD_CONFIG_DIR": str(root/"profile"),
                                   "WORDCRAFT_DICTIONARIES": str(root)}, text=True, capture_output=True,
                              check=True, timeout=180)
    messages = [json.loads(line)["result"] for line in response.stdout.splitlines()]
    assert len(messages) == 2 and not any(m["isError"] for m in messages)
    issues = json.loads(messages[1]["content"][0]["text"])
    fields = ("start","end","text","suggestions","lang")
    if kind == "grammar":
        fields += ("message",)
    return [{k:i[k] for k in fields} for i in issues if i["kind"] == kind]


def write_dictionary(folder, locale, affix, words):
    folder.mkdir()
    full = locale.split("-")
    canonical = [full[0], *(s.title() if len(s) == 4 and s.isalpha() else s.upper() for s in full[1:])]
    names = {locale, locale.replace("-", "_"), "-".join(canonical), "_".join(canonical)}
    if not any(len(s) == 4 and s.isalpha() for s in full[1:]):
        names.add(full[0])
    for suffix, data in [("aff",affix),("dic",words)]:
        file = folder/(locale+"."+suffix)
        file.write_bytes(data)
        for name in names:
            alias = folder/(name+"."+suffix)
            if not alias.exists():
                os.link(file, alias)


def published_data(archive):
    found = {}
    with subprocess.Popen(["zstd","-dq","-c",archive],stdout=subprocess.PIPE) as decoder:
        with tarfile.open(fileobj=decoder.stdout,mode="r|") as stream:
            for member in stream:
                if member.name in ("spelling.aff","spelling.dic"):
                    assert member.isfile() and 0 < member.size <= 64 << 20 and member.name not in found
                    found[member.name] = stream.extractfile(member).read(member.size)
        for _ in iter(lambda: decoder.stdout.read(65536), b""):
            pass
        decoder.stdout.close()
        assert decoder.wait() == 0
    return found["spelling.aff"],found["spelling.dic"]


def sample_words(affix, dictionary):
    charset = re.search(rb"(?mi)^SET[ \t]+([^\s]+)",affix)
    codec = charset.group(1).decode("ascii") if charset else "utf-8"
    rows = dictionary.decode(codec,errors="replace").splitlines()[1:]
    words = []
    for begin in (len(rows)//3, len(rows)*2//3):
        for line in rows[begin:]:
            stem = line.split("/",1)[0].split("\t",1)[0].strip()
            if 4 <= len(stem) <= 14 and stem.isalpha() and stem not in words and stem[0] != stem[1]:
                words.extend((stem, stem[1]+stem[0]+stem[2:], stem[:1]+stem[2:]))
                break
    assert words
    return words


def edit_probes(affix, dictionary):
    words = sample_words(affix, dictionary)
    charset = re.search(rb"(?mi)^SET[ \t]+([^\s]+)", affix)
    tables = affix.decode(charset.group(1).decode("ascii") if charset else "utf-8", errors="replace")
    try_table = re.search(r"(?mi)^TRY[ \t]+(\S+)", tables)
    keyboard = re.search(r"(?mi)^KEY[ \t]+(\S+)", tables)
    groups = re.findall(r"(?mi)^MAP[ \t]+([^\s#]+)", tables)
    substitutions = re.findall(r"(?mi)^REP[ \t]+([^\s#]+)[ \t]+([^\s#]+)", tables)
    alphabet = [c for c in (try_table.group(1) if try_table else "") if c.isalpha()]
    for stem in list(words[::3]):
        middle = len(stem) // 2
        words.extend([stem[:middle] + stem[middle - 1:], stem + stem, stem.upper(),
                      stem[:middle] + stem[middle + 1:] + stem[middle]])
        if alphabet:
            words.extend((stem[:middle] + alphabet[0] + stem[middle:],
                          stem[:middle] + alphabet[-1] + stem[middle + 1:]))
        for group in groups + (keyboard.group(1).split("|") if keyboard else []):
            if len(group) < 2 or not group.isalpha():
                continue
            index = next((i for i, c in enumerate(stem) if c in group), None)
            if index is not None:
                c = group[(group.index(stem[index]) + 1) % len(group)]
                words.append(stem[:index] + c + stem[index + 1:])
                break
        for wrong, right in substitutions:
            if wrong.isalpha() and right.isalpha() and right in stem:
                words.append(stem.replace(right, wrong, 1))
                break
    return list(dict.fromkeys(words))


def compare(binaries, directory, locale, probes):
    before,after = [results(cli,directory,locale,probes) for cli in binaries]
    assert before == after, f"different issue or suggestion order for {locale}"
    signature = hashlib.sha256(json.dumps(after,sort_keys=True,ensure_ascii=False).encode()).hexdigest()
    return after, {"probes":len(probes), "spellingIssues":len(after),
                   "issuesWithSuggestions":sum(bool(i["suggestions"]) for i in after),
                   "orderedIssueSha256":signature}


def verify_desktop(binary, dictionary, out, words=("teh", "tests", "tset"),
                   ordered=EXPECTED["teh"], replacement="the", locale="nl"):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from native_pilot_support import app, port, wait, word_request
    from verify_save_changes import word_keys
    from verify_shared_ribbon import NativeWindow

    out.mkdir()
    source = out / "原始拼写.docx"
    working = out / "校对 副本.docx"
    make_document(source, locale, words)
    original = sha(source)
    shutil.copyfile(source, working)
    number, token = port(), secrets.token_hex(32)
    env = {"JOYWORD_CONFIG_DIR": str(out / "profile"), "JOYWORD_CONTROL_KEY": token,
           "WORDCRAFT_DICTIONARIES": str(dictionary)}
    args = ["--control", str(number), "--", str(working)]

    def inspect():
        return word_request(number, "document.inspect", key=token).get("result")

    def edit(command, **params):
        state = inspect()
        reply = word_request(number, "engine.execute", {"command": command, "doc": state["doc"],
                             "version": state["version"], "params": params}, token)
        assert reply["ok"], reply

    with app(binary, args, out, env) as proc:
        initial = wait(inspect, "document opened", proc)["text"]
        surface = NativeWindow(proc, out)
        surface.size(1180)
        surface.press(wait(lambda: surface.find({"Review", "审阅"}), "Review tab", proc))
        spell = wait(lambda: surface.find({"review-spelling", "拼写和语法", "Spelling & Grammar"}, "button"),
                     "Spelling & Grammar", proc)
        surface.press(spell)
        suggestions = ", ".join(ordered)

        def displayed():
            texts = []
            for node in surface.nodes():
                texts.extend(str(node.get(k) or "") for k in ("title", "label", "value"))
                if not surface.mac and node["role"] in ("label", "text", "static"):
                    interface = node["object"].get_text_iface()
                    if interface:
                        texts.append(interface.get_text(0, -1))
            issues = {f"Issue {words[0]}", f"发现的问题 {words[0]}"}
            choices = {f"Suggestions {suggestions}", f"建议 {suggestions}"}
            return bool(issues.intersection(texts) and choices.intersection(texts))

        wait(displayed, "issue and ordered recommendations displayed", proc)
        surface.key("escape")
        wait(lambda: not displayed(), "spelling dialog closed", proc)
        corrected = initial.replace(words[0], replacement, 1)
        edit("text.insert", text=replacement)
        assert inspect()["text"] == corrected
        edit("edit.undo")
        assert inspect()["text"] == initial
        edit("edit.redo")
        assert inspect()["text"] == corrected
        key = word_keys(proc)
        key("new")
        key("enter")
        key("enter")

        def saved():
            with ZipFile(working) as archive:
                body = ET.fromstring(archive.read("word/document.xml"))
            return [t.text for t in body.findall(".//{*}t")] == [replacement, *words[1:]]

        wait(saved, "corrected DOCX written", proc)
    with app(binary, args, out, env, "reopen.log") as proc:
        assert wait(inspect, "corrected copy reopened", proc)["text"] == corrected
    assert sha(source) == original
    return {"binarySha256": sha(binary), "sourceSha256": original, "selectedWord": words[0],
            "displayedSuggestions": ordered, "replacement": replacement, "undoRedo": True,
            "savedAndReopened": True, "originalUnchanged": True}


def verify_circumfix(binaries, desktop, out):
    aff = b"SET UTF-8\nTRY worksablen\nCIRCUMFIX c\nPFX P Y 1\nPFX P 0 ne/c .\nSFX A Y 1\nSFX A 0 able/Bc .\nSFX B Y 1\nSFX B 0 s .\n"
    folder = out/"paired-affixes"
    write_dictionary(folder, "nl", aff, b"1\nwork/AP\n")
    words = ["neworkabless", "neworkables", "neworkable", "workables", "nework"]
    old, new = [results(binary, folder, "nl", words) for binary in binaries]
    assert [r["text"] for r in old] == [words[i] for i in (0, 1, 3, 4)]
    assert [r["text"] for r in new] == [words[i] for i in (0, 3, 4)]
    assert new[0]["suggestions"] == [words[1]]
    result = {"words": words, "baseline": old, "candidate": new}
    if desktop:
        result["desktop"] = verify_desktop(desktop, folder, out/"circumfix-window",
                                         words[:3], [words[1]], words[1])
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("baseline","candidate","output"):
        parser.add_argument(arg,type=Path)
    parser.add_argument("--release-packs",type=Path,help="verified index.json and its locale.tar.zst archives")
    parser.add_argument("--desktop",type=Path,help="also exercise the native Review ribbon and its spelling dialog")
    parser.add_argument("--candidate-edits", action="store_true",
                        help="also compare insertion, substitution, duplication, case and dictionary-directed edits")
    parser.add_argument("--paired-affixes", action="store_true", help="verify corrected prefix and double-suffix recognition")
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True,exist_ok=False)
    binaries = [args.baseline.resolve(),args.candidate.resolve()]
    report = {"binarySha256":[sha(b) for b in binaries],"synthetic":{},"published":{}}
    for locale in ["nl","sv","da","fi"]:
        folder = out/("small-"+locale)
        write_dictionary(folder,locale,SAMPLE_AFFIX,(str(len(VOCABULARY))+"\n"+"\n".join(VOCABULARY)+"\n").encode())
        issues,counts = compare(binaries,folder,locale,CASES)
        assert {x["text"]:x["suggestions"] for x in issues} == EXPECTED
        report["synthetic"][locale] = counts
    if args.desktop:
        report["desktop"] = verify_desktop(args.desktop.resolve(), out/"small-nl", out/"desktop")
    if args.paired_affixes:
        report["pairedAffixes"] = verify_circumfix(binaries, args.desktop.resolve() if args.desktop else None, out)
    if args.release_packs:
        index = args.release_packs/"index.json"
        assert sha(index) == PUBLISHED_INDEX
        report["indexSha256"] = PUBLISHED_INDEX
        for row in json.loads(index.read_text())["packs"]:
            locale = row["locale"]
            archive = args.release_packs/(locale+".tar.zst")
            assert archive.stat().st_size == row["size"] and sha(archive) == row["sha256"]
            affix,dictionary = published_data(archive)
            folder = out/locale
            write_dictionary(folder,locale,affix,dictionary)
            probes = (edit_probes if args.candidate_edits else sample_words)(affix, dictionary)
            _, counts = compare(binaries, folder, locale, probes)
            report["published"][locale] = {**counts,"archiveSha256":row["sha256"]}
    (out/"report.json").write_text(json.dumps(report,indent=2)+"\n")
