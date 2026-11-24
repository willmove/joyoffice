"""Stage separately reviewed packs and the corresponding sources for a new release."""
import argparse
import json
from pathlib import Path
import runpy
import shutil
from urllib.parse import urlsplit
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo


TOOLS = runpy.run_path(str(Path(__file__).with_name("stage-hosted-language-packs.py")))
TAG = "joyword-language-packs-2"
ORIGIN = f"https://github.com/willmove/joyoffice/releases/download/{TAG}/"


def zip_bytes(bundle, name, data):
    member = ZipInfo(name)
    member.compress_type = ZIP_DEFLATED
    bundle.writestr(member, data)


def stage(archives, audit, cache, review, previous, out):
    verify = TOOLS["digest"]
    decision = json.loads(review.read_bytes())
    raw_audit = audit.read_bytes()
    if verify(raw_audit) != decision["previousAuditSha256"]:
        raise ValueError("The previous publication audit differs from the reviewed copy")
    original_bytes = (archives / "index.json").read_bytes()
    if verify(original_bytes) != TOOLS["INDEX_SHA"]:
        raise ValueError("The original pack index differs from the pinned copy")
    original = {p["locale"]: p for p in json.loads(original_bytes)["packs"]}
    records = {p["locale"]: p for p in json.loads(raw_audit)["packs"] if p["class"] == "Download-only"}
    decisions = decision["packs"]
    if set(decisions) != set(records) or len(records) != 34:
        raise ValueError("Each of the 34 outstanding locales requires a decision")

    previous = json.loads(previous.read_bytes())
    assets = out / "assets"
    assets.mkdir(parents=True)
    published = []
    inputs = set()
    with ZipFile(assets / "sources-and-notices.zip", "x") as bundle:
        for locale, record in records.items():
            entry = original[locale]
            archive = archives / f"{locale}.tar.zst"
            data = archive.read_bytes()
            if verify(data) != entry["sha256"] or len(data) != entry["size"] or entry["sha256"] != record["sha256"]:
                raise ValueError(f"{locale}: pack bytes differ from the original archive")
            content = TOOLS["files"](archive)
            for item in record["sources"]:
                sha = item["sha256"]
                if verify((cache / "pinned-source" / sha).read_bytes()) != sha or verify(content[item["file"]]) != sha:
                    raise ValueError(f"{locale}: source does not match {item['file']}")
                inputs.add(item["url"])
            current = dict(record, **decisions[locale])
            status = current["status"]
            if status == "hosted":
                if not current.get("licenceSelection") or not current.get("reason"):
                    raise ValueError(f"{locale}: missing source and licence assessment")
                for member in sorted(set(record["notices"]) | {item["file"] for item in record["sources"]}):
                    zip_bytes(bundle, f"{locale}/{member}", content[member])
                for item in record["sources"]:
                    # Preserve the source filename too, including LPPL .tex patterns.
                    path = Path(urlsplit(item["url"]).path).name
                    zip_bytes(bundle, f"{locale}/original/{item['file']}/{path}", content[item["file"]])
                for name in current.get("supplement", []):
                    descriptor = decision["supplement"][name]
                    path = cache / "supplement" / name
                    raw = path.read_bytes()
                    if path.name != name or verify(raw) != descriptor["sha256"]:
                        raise ValueError(f"{locale}: supplemental file differs from {name}")
                    zip_bytes(bundle, f"{locale}/upstream/{name}", raw)
                shutil.copyfile(archive, assets / archive.name)
            elif status != "upstream" or not current.get("reason"):
                raise ValueError(f"{locale}: invalid publication status")
            published.append(current)
        zip_bytes(bundle, "publication-review.json", review.read_bytes())

    migrated = {item["locale"] for item in published if item["status"] == "hosted"}
    index = []
    for item in previous["packs"]:
        locale = item["locale"]
        source = original[locale]
        base = {k: v for k, v in item.items() if k != "fallback_url"}
        base["url"] = item.get("fallback_url", item["url"])
        if base != source:
            raise ValueError(f"{locale}: previously bundled entry has changed its archive contract")
        index.append(dict(source, url=f"{ORIGIN}{locale}.tar.zst", fallback_url=source["url"]) if locale in migrated else item)
    if len(index) != 75 or {item["locale"] for item in index} != set(original) or not migrated:
        raise ValueError("Invalid complete index or empty release")
    # Keep the pack series identity shared by old and new stores.
    TOOLS["write"](assets / "index.json", dict(previous, packs=index))
    TOOLS["write"](assets / "publication-audit.json", dict(
        tag=TAG, reviewed=len(records), verifiedSourceUrls=len(inputs), newHosts=len(migrated),
        upstream=len(records) - len(migrated), previousAuditSha256=verify(raw_audit),
        sourceBundleSha256=verify((assets / "sources-and-notices.zip").read_bytes()),
        supplement=decision["supplement"], packs=published,
    ))
    print(f"{len(records)} reviewed, {len(inputs)} source URLs verified, {len(migrated)} archives staged")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ("archives", "audit", "cache", "review", "previous", "output"):
        parser.add_argument(argument, type=Path)
    stage(*vars(parser.parse_args()).values())
