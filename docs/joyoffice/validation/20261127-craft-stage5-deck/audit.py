"""Audit the JoyDeck build inputs and legal material of a local qualification package."""

import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import re
import subprocess
import zipfile


ROOT = Path(__file__).resolve().parents[4]
TARGETS = (
    "x86_64-pc-windows-msvc",
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
)
ENTRIES = ("joydeck", "joydeck-cli")
CRAFT_PACKAGES = ("deckcraft", "wordcraft", "gridcraft", "craft-libs", "craft-fonts", "craft-i18n")
NOTICE_FILES = (
    "THIRD_PARTY-JOYDECK.md",
    "licenses/deckcraft/LICENSE-MIT",
    "licenses/deckcraft/LICENSE-APACHE",
    "licenses/deckcraft/NOTICE",
    "licenses/deckcraft/ATTRIBUTION-upstream.md",
    "licenses/deckcraft/file-map.json",
    "licenses/deckcraft/source-inventory.json",
    "licenses/joyoffice/LICENSE-MIT",
    "licenses/joyoffice/source-map.json",
    "licenses/fonts/sources.json",
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def cargo_graph(root, target):
    tree = subprocess.check_output(
        ["cargo", "tree", "--locked", "--offline", "--target", target, "-p", root,
         "--edges", "normal,build", "--prefix", "none", "--format", "{p}"],
        cwd=ROOT, encoding="utf-8",
    )
    assert not re.search(r"third-party[/\\]craft-libs", tree, re.IGNORECASE), (root, target)
    return set(re.findall(r"^(\S+) v(\S+)", tree, re.MULTILINE))


def rust_inputs(dep_info):
    rules = dep_info.read_text(encoding="utf-8").splitlines()[0]
    paths = re.split(r"(?<!\\)\s+", rules.split(": ", 1)[1])
    files = {Path(p.replace("\\ ", " ")).resolve() for p in paths if p}
    return {path.relative_to(ROOT).as_posix() for path in files if path.is_relative_to(ROOT)}


def source_audit(dep_info_dir):
    records = read_json(ROOT / "licenses/deckcraft/file-map.json")["files"]
    retired_paths = {row["target_path"] for row in records if row.get("retired")}
    by_entry = {name: rust_inputs(dep_info_dir / f"{name}.d") for name in ENTRIES}
    by_crate = defaultdict(list)
    build_scripts = []
    retired = []
    unselected = []
    origins = defaultdict(list)
    for record in records:
        origins[record["target_path"]].append(record["source_path"])
    for path, source_paths in sorted(origins.items()):
        entries = [name for name, files in by_entry.items() if path in files]
        record = {"path": path, "entries": entries, "originPaths": sorted(source_paths)}
        if path in retired_paths:
            retired.append(path)
        elif entries and len(path.split("/")) == 3 and path.endswith("/build.rs"):
            build_scripts.append(record)
        elif entries and path.endswith(".rs"):
            by_crate[path.split("/")[1]].append(record)
        else:
            unselected.append(path)
    return {
        "ledgerRecords": len(records),
        "mappedCompiledRust": dict(sorted(by_crate.items())),
        "mappedBuildScripts": build_scripts,
        "retired": sorted(retired),
        "otherActiveLedgerPaths": sorted(unselected),
    }


def package_audit(archive_path):
    with zipfile.ZipFile(archive_path) as archive:
        names = archive.namelist()
        prefix = names[0].split("/", 1)[0] + "/"
        assert all(name.startswith(prefix) for name in names)
        assert len(names) == len(set(names))

        def payload(name):
            return archive.read(prefix + name)

        members = dict(line.split("  ", 1)[::-1]
                       for line in payload("SHA256SUMS").decode("utf-8").splitlines())
        assert set(members) | {"SHA256SUMS"} == {name[len(prefix):] for name in names}
        for name, sha in members.items():
            assert digest(payload(name)) == sha, name
        for name in NOTICE_FILES:
            assert members[name] == digest((ROOT / name).read_bytes()), name
        native = json.loads(payload("native-helpers.json"))
        for name, sha in native["binaries"].items():
            assert members[name] == sha, name
        for name, sha in native["sources"].items():
            assert digest((ROOT / "crates/joydeck-shell/native" / name).read_bytes()) == sha, name
        sbom = json.loads(payload("SBOM.spdx.json"))
        working_list = json.loads(payload("sbom.json"))["packages"]
        license_index = json.loads(payload("dependency-licenses/index.json"))
        [krilla] = [row for row in working_list if row["name"] == "krilla"]
        assert krilla["version"] == "0.8.2" and krilla["source"].startswith("registry+")
        assert {(row["name"], row["versionInfo"]) for row in sbom["packages"]} == {
            (row["name"], row["version"]) for row in working_list
        }
        assert all(text in members for row in license_index for text in row["texts"])
        missing = sorted(f"{row['name']}@{row['version']}" for row in license_index if not any(
            Path(text).name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE"))
            for text in row["texts"]
        ))
        standard_texts = []
        for row in license_index:
            for name in row["texts"]:
                if Path(name).name != "SOURCE.json":
                    continue
                source = json.loads(payload(name))
                if source.get("originalStandaloneTextAvailable") is not False:
                    continue
                parent = Path(name).parent
                for file, sha in source["files"].items():
                    assert members[(parent / file).as_posix()] == sha, (name, file)
                standard_texts.append({
                    "package": f"{row['name']}@{row['version']}",
                    "declared": row["declared"], "standardText": source["standardText"],
                    "originalStandaloneTextAvailable": False,
                    "recordHashesVerified": True,
                })
        build_info = json.loads(payload("BUILD-INFO.json"))
        return {
            "target": build_info["target"],
            "profile": build_info["profile"],
            "signed": build_info["officialSignatureVerified"],
            "buildGraphSha256": build_info["sourceGraphSha256"],
            "archiveSha256": digest(archive_path.read_bytes()),
            "verifiedMembers": len(members),
            "spdxSha256": members["SBOM.spdx.json"],
            "packages": len(sbom["packages"]),
            "missingLicenseTexts": missing,
            "standardTextFallbacks": standard_texts,
            "executables": {name: sha for name, sha in members.items() if name.endswith(".exe")},
            "noticeHashes": {name: members[name] for name in NOTICE_FILES},
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("dep_info_dir", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    graphs = {}
    for target in TARGETS:
        graphs[target] = {}
        for root in ENTRIES:
            graph = cargo_graph(root, target)
            craft = sorted(name for name, _ in graph if any(
                name == prefix or name.startswith(prefix + "-") for prefix in CRAFT_PACKAGES
            ))
            assert not craft, (target, root, craft)
            graphs[target][root] = {
                "packages": len(graph),
                "craftCrates": craft,
                "selectedVersions": dict(sorted(
                    (name, version) for name, version in graph
                    if name in {"gpui-kit", "gpui-pre", "gpui-pre-linux", "krilla", "mac", "seahash"}
                )),
            }
    report = {
        "targets": graphs,
        "sourceLedger": source_audit(args.dep_info_dir),
        "package": package_audit(args.package),
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(TARGETS) * len(ENTRIES)} dependency graphs audited, "
          f"{report['package']['verifiedMembers']} package files verified")


if __name__ == "__main__":
    main()
