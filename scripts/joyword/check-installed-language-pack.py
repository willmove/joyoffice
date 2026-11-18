"""Exercise a local language pack with the shipping CLI and a fresh desktop process."""

import argparse
import io
import json
import os
from pathlib import Path
import runpy
import secrets
import subprocess
import sys
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import app, digest, port, wait, word_request


def fixture(path):
    fmt = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))
    text = ("en lettergrepen de lettergrepen of lettergrepen " * 48).strip()
    para = f'<w:p><w:r><w:rPr><w:lang w:val="nl-NL"/></w:rPr><w:t>{text}</w:t></w:r></w:p>'
    section = ('<w:sectPr><w:pgSz w:w="3000" w:h="3000"/>'
               '<w:pgMar w:top="240" w:bottom="240" w:left="250" w:right="250"/></w:sectPr>')
    fmt["package"](path, para + section, [("settings", "settings", "settings.xml")],
                   {"word/settings.xml": f'<w:settings xmlns:w="{fmt["W"]}"><w:autoHyphenation/>'
                                         '<w:hyphenationZone w:val="0"/></w:settings>'},
                   {"word/settings.xml": "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"})
    return text


def check(cli, gui, out, published=False):
    doc = out / "hyphenation.docx"
    text = fixture(doc)
    source_sha = digest(doc)
    store = out / "store"
    store.mkdir()
    profile = out / "profile"
    index = out / "index.json"
    archive = out / "nl.tar.zst"
    env = {"JOYWORD_CONFIG_DIR": str(profile), "CRAFT_FONT_STORE": str(store),
           "CRAFT_LANGUAGE_PACK_INDEX": str(index)}

    def command(*args, ok=True):
        result = subprocess.run([cli, *args], env={**os.environ, **env}, capture_output=True, timeout=120)
        if (result.returncode == 0) != ok:
            raise RuntimeError(f"unexpected exit {result.returncode}: {result.stderr.decode()}")
        return result.stdout

    def pages(label):
        state = json.loads(command("inspect", "--session", doc))
        assert state["text"] == text
        number, key = port(), secrets.token_hex(32)
        with app(gui, ["--control", str(number), "--", str(doc)], out,
                 {**env, "JOYWORD_CONTROL_KEY": key}, label + ".log") as process:
            state_ui = wait(lambda: word_request(number, "document.inspect", key=key).get("result"),
                            "opened document", process)
            assert state_ui["text"] == text
            assert len(state_ui["summary"]["pages"]) == len(state["pages"])
        return len(state["pages"])

    def pack_with(exception):
        members = {"pack.toml": 'locale="nl"\nname="Nederlands"\nlicence="CC0-1.0"\n',
                   "hyphenation.pat": "l e t r g p n\n\\hyphenation{" + exception + "}\n",
                   "LICENCES/NOTICE.txt": "Original synthetic test patterns, CC0-1.0.\n"}
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode="w") as tar:
            for name, content in members.items():
                raw = content.encode()
                header = tarfile.TarInfo(name)
                header.size = len(raw)
                tar.addfile(header, io.BytesIO(raw))
        archive.write_bytes(subprocess.run(["zstd", "-q", "-c"], input=stream.getvalue(),
                                           capture_output=True, check=True).stdout)
        record = {"locale": "nl", "name": "Nederlands", "url": str(archive), "sha256": digest(archive),
                  "size": archive.stat().st_size, "licence": "CC0-1.0", "class": "Embeddable",
                  "spelling": False, "hyphenation": True, "source": "https://example.org/joyoffice-fixture"}
        index.write_text(json.dumps({"format": 1, "release": "local-test", "packs": [record]}))
        return record["sha256"]

    refresh = 'proof.packs={"refresh":true}'
    install = 'proof.installPack={"locale":"nl"}'
    remove = 'proof.removePack={"locale":"nl"}'
    def run(*commands, ok=True):
        return command("run", doc, *(item for name in commands for item in ("--cmd", name)), ok=ok)

    original_pages = pages("before")
    first_hash = pack_with("lettergrepen")
    run(refresh, ok=False)
    assert not (store / "indexes/language-packs.json").exists()
    # Represent an already-consenting user in this newly created, isolated profile.
    (store / "settings.json").write_text('{"network":"on"}')
    run(refresh, install)
    first = pages("unbroken")
    installed = store / "language-packs/nl"
    assert "CC0-1.0" in (installed / "LICENCES/NOTICE.txt").read_text()
    run(remove)
    second_hash = pack_with("let-ter-gre-pen")
    run(refresh)
    archive_bytes = archive.read_bytes()
    archive.write_bytes(archive_bytes[:-1] + bytes([archive_bytes[-1] ^ 1]))
    run(install, ok=False)
    assert not installed.exists()
    archive.write_bytes(archive_bytes)
    run(install)
    second = pages("segmented")
    assert second < first, (first, second)
    installed_list = json.loads((store / "language-packs.json").read_text())
    assert [(p["locale"], p["sha256"]) for p in installed_list["packs"]] == [("nl", second_hash)]
    run(remove)
    final = pages("restored")
    assert original_pages == final and not installed.exists() and digest(doc) == source_sha
    public_pack = {}
    if published:
        env["CRAFT_LANGUAGE_PACK_INDEX"] = ""
        run(refresh)
        cached = store / "indexes/language-packs.json"
        expected_index = "b35c7aef0591177f18233b551da16035dcd615549ff874e169ffd19f16fea74b"
        assert digest(cached) == expected_index
        entries = json.loads(cached.read_bytes())["packs"]
        entry = next(row for row in entries if row["locale"] == "nl")
        run(install)
        record = next(row for row in json.loads((store / "language-packs.json").read_bytes())["packs"]
                      if row["locale"] == "nl")
        assert (record["sha256"], record["size"], record["licence"]) == (entry["sha256"], entry["size"], entry["licence"])
        assert all((installed / name).stat().st_size > 0 for name in ("spelling.aff", "spelling.dic", "hyphenation.pat"))
        assert any(f.stat().st_size for f in (installed / "LICENCES").iterdir() if f.is_file())
        public_pack = {"releaseIndexSha256": expected_index, "indexEntries": len(entries),
                       "locale": entry["locale"], "archiveSha256": entry["sha256"], "archiveBytes": entry["size"],
                       "source": entry["source"], "license": entry["licence"],
                       "checkedFiles": ["spelling.aff", "spelling.dic", "hyphenation.pat", "LICENCES/"],
                       "desktopAndCliPages": pages("published")}
        run(remove)
        assert not installed.exists() and pages("removed-published") == original_pages
    return {"cliSha256": digest(cli), "desktopSha256": digest(gui), "fixtureSha256": source_sha,
            "archiveSha256": [first_hash, second_hash], "desktopAndCliPages": {
                "before": original_pages, "wholeWordException": first,
                "syllableExceptions": second, "afterRemoval": final},
            "refusedConsentOff": True, "refusedChangedArchive": True,
            "licenseAndInstallRecordVerified": True, "sourceDocumentUnchanged": digest(doc) == source_sha, **({"published": public_pack} if published else {})}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("cli", "desktop", "out"):
        parser.add_argument(arg, type=Path)
    parser.add_argument("--published", action="store_true", help="also fetch and verify the pinned Dutch release in the isolated store")
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    (out / "report.json").write_text(json.dumps(check(args.cli.resolve(), args.desktop.resolve(), out, args.published), indent=2) + "\n")
