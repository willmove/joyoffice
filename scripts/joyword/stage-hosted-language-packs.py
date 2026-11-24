"""Audit the pinned release per file and stage the packs cleared for JoyOffice hosting."""
import argparse
from collections import Counter
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tarfile
import tomllib

try:
    from compression import zstd
except ImportError:
    zstd = None

INDEX_SHA = "b35c7aef0591177f18233b551da16035dcd615549ff874e169ffd19f16fea74b"
MANIFEST_SHA = "2f4e88908dc85a030e789465b2631d70409e5e4474f7044ab87cf182f8c055b9"
TAG = "joyword-language-packs-1"
OLD_ORIGIN = "https://github.com/storytold/craft-i18n/releases/download/language-packs-2/"
NEW_ORIGIN = f"https://github.com/willmove/joyoffice/releases/download/{TAG}/"
SOURCE_REV = "20467b63558f04fe075e83aaea501bd10f7fa684"
GENERATED = {
    "de": "igerman98/frami preferred source", "de-at": "igerman98/frami preferred source",
    "de-ch": "igerman98/frami preferred source", "he": "Hspell 1.4 inputs and Myspell conversion",
    "ko": "generated Hunspell affixes and their build input",
    "nb": "generated Norwegian Hunspell affixes and their build input",
    "nn": "generated Norwegian Hunspell affixes and their build input",
    "ta": "licence evidence for the spelling files beyond the generic MPL notice",
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


@contextmanager
def decompress(path):
    if zstd is not None:
        with zstd.open(path, "rb") as stream:
            yield stream
    else:
        with subprocess.Popen(["zstd", "-dq", "-c", path], stdout=subprocess.PIPE) as proc:
            yield proc.stdout
            while proc.stdout.read(65536):
                pass
            if proc.wait():
                raise ValueError(f"Could not decode {path.name}")


def files(path):
    with decompress(path) as stream, tarfile.open(fileobj=stream, mode="r|") as archive:
        result = {}
        for member in archive:
            name = member.name.removeprefix("./")
            relative = PurePosixPath(name)
            if relative.is_absolute() or ".." in relative.parts or "\\" in name:
                raise ValueError(f"{path.name}: unsafe member {name}")
            if member.isdir():
                continue
            if not member.isfile() or name in result or not name:
                raise ValueError(f"{path.name}: unexpected member {name}")
            result[name] = archive.extractfile(member).read()
        return result


def write(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def stage(source, manifest, out):
    old = (source / "index.json").read_bytes()
    pinned_manifest = manifest.read_bytes()
    if (digest(old), digest(pinned_manifest)) != (INDEX_SHA, MANIFEST_SHA):
        raise ValueError("The pinned release data has changed")
    original = json.loads(old)
    declared = {
        (r["locale"], r["url"], r.get("sha256"), r.get("licence"), r["class"])
        for r in tomllib.loads(pinned_manifest.decode())["file"]
    }
    approvals = []
    index = []
    selected = []
    for entry in original["packs"]:
        locale = entry["locale"]
        filename = f"{locale}.tar.zst"
        path = source / filename
        raw = path.read_bytes()
        if len(raw) != entry["size"] or digest(raw) != entry["sha256"] or entry["url"] != OLD_ORIGIN + filename:
            raise ValueError(f"{locale}: archive differs from the pinned release")
        content = files(path)
        pack = tomllib.loads(content["pack.toml"].decode())
        for k in ("locale", "licence", "class", "version"):
            if pack[k] != entry[k]:
                raise ValueError(f"{locale}: archive metadata differs in {k}")
        covered = set()
        for row in pack["source"]:
            key = (locale, row["url"], row["sha256"], row["licence"], row["class"])
            if (key not in declared or row["commit"] not in row["url"]
                    or digest(content[row["file"]]) != row["sha256"]):
                raise ValueError(f"{locale}: source or bytes differ for {row['file']}")
            covered.add(row["file"])
        names = {n for n in content if n.startswith("LICENCES/")}
        texts = {f"LICENCES/{name}" for name in pack["licence_texts"]}
        if not names or not texts <= names or set(content) - {"pack.toml"} - names - covered:
            raise ValueError(f"{locale}: missing licence text or source mapping")
        if any(not content[n].strip() for n in names):
            raise ValueError(f"{locale}: empty licence file")
        eligible = entry["class"] == "Embeddable"
        if eligible and any(row["class"] != "Embeddable" or
                            re.search(r"\b(?:A?GPL|LGPL)-|CC-BY-SA-", row["licence"])
                            for row in pack["source"]):
            raise ValueError(f"{locale}: source requires further distribution review")
        if eligible:
            index.append(dict(entry, url=NEW_ORIGIN + filename, fallback_url=entry["url"]))
            selected.append(path)
            decision = "JoyOffice host, exact archived source data and notices"
        else:
            index.append(entry)
            decision = "Original distribution pending " + GENERATED.get(locale, "preferred-source and notice review for separate redistribution")
        approvals.append({
            "locale": locale, "licence": entry["licence"], "class": entry["class"], "decision": decision,
            "sha256": entry["sha256"], "size": entry["size"], "origin": entry["url"],
            "notices": sorted(names), "sources": pack["source"],
        })
    assert len(index) == len({row["locale"] for row in index}) == 75
    out.mkdir(parents=True)
    assets = out / "assets"
    assets.mkdir()
    for path in selected:
        shutil.copyfile(path, assets / path.name)
    write(out / "index.json", {"format": 1, "release": TAG, "packs": index})
    write(assets / "publication-audit.json", {
        "upstreamCommit": SOURCE_REV, "upstreamIndexSha256": INDEX_SHA, "upstreamManifestSha256": MANIFEST_SHA,
        "hosted": len(selected), "originalDistribution": len(index) - len(selected),
        "classes": Counter(entry["class"] for entry in original["packs"]), "packs": approvals,
    })
    print(f"{len(index)} audited, {len(selected)} staged, {sum(len(p['sources']) for p in approvals)} source records")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source", "manifest", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    stage(args.source.resolve(), args.manifest.resolve(), args.output.resolve())
