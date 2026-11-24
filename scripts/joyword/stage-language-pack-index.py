"""Assemble the JoyWord index and data provenance from a verified release snapshot."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy

ROOT = Path(__file__).resolve().parents[2]
RELEASE = "joyword-language-packs-1"
SOURCE_INDEX_SHA256 = "b35c7aef0591177f18233b551da16035dcd615549ff874e169ffd19f16fea74b"
INDEX = ROOT / "crates/joyword-engine/data/language-pack-index.json"
UPSTREAM = "https://github.com/storytold/craft-i18n/releases/download/language-packs-2/"


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def stage(source, output, *, check):
    input_index = source / "index.json"
    if sha(input_index) != SOURCE_INDEX_SHA256:
        raise ValueError("The original release index has an unexpected SHA-256")
    previous = json.loads(input_index.read_bytes())
    inspect = runpy.run_path(str(Path(__file__).with_name("check-pack-publication.py")))["manifest"]
    locales = set()
    all_packs = []
    for entry in previous["packs"]:
        locale = entry["locale"]
        filename = f"{locale}.tar.zst"
        if locale in locales or entry["url"] != UPSTREAM + filename:
            raise ValueError(f"Unexpected locale or archive URL for {locale}")
        locales.add(locale)
        archive = source / filename
        if archive.stat().st_size != entry["size"] or sha(archive) != entry["sha256"]:
            raise ValueError(f"Archive size or SHA-256 mismatch for {locale}")
        files = inspect(archive)
        notices = sorted(path for path in files if path.startswith("LICENCES/"))
        if not notices or "pack.toml" not in files:
            raise ValueError(f"Missing pack metadata or license for {locale}")
        all_packs.append(dict(locale=locale, archive=filename, size=entry["size"], sha256=entry["sha256"],
                              source=entry.get("source"), licence=entry["licence"], distributionClass=entry["class"],
                              licenceFiles=notices, files=files))
    index = encoded({"format": 1, "release": RELEASE, "packs": previous["packs"]})
    if check and INDEX.read_bytes() != index:
        raise ValueError("Bundled JoyWord index differs from the assembled index")
    embedded = []
    for folder in ("crates/joyword-proof/data", "third-party/craft-i18n/language-data"):
        for path in sorted((ROOT / folder).rglob("*")):
            if path.is_file():
                embedded.append({"path": path.relative_to(ROOT).as_posix(), "size": path.stat().st_size,
                                 "sha256": sha(path), "notice": ("crates/joyword-proof/README.md"
                                    if folder.startswith("crates/") else "third-party/craft-i18n/language-data/README.md")})
    evidence = {
        "release": RELEASE, "format": 1, "indexSha256": hashlib.sha256(index).hexdigest(),
        "sourceIndex": UPSTREAM + "index.json", "sourceIndexSha256": SOURCE_INDEX_SHA256,
        "packs": all_packs, "embeddedInputs": embedded,
    }
    output.mkdir(parents=True)
    (output / "index.json").write_bytes(index)
    (output / "manifest.json").write_bytes(encoded(evidence))
    print(f"{len(all_packs)} packs, {len(embedded)} embedded inputs, index {evidence['indexSha256']}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--check", action="store_true", help="compare with the bundled JoyWord index")
    args = parser.parse_args()
    stage(args.source.resolve(), args.output.resolve(), check=args.check)
