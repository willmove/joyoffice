#!/usr/bin/env python3
"""Build an unsigned portable preview archive and SPDX 2.3 source inventory.

No installer, association, signing or upgrade claim is made. Inputs are existing
native binaries; no system fonts, documents or reference-product assets enter
the archive. SPDX fields follow spdx.github.io/spdx-spec/v2.3.1 clauses 6-7.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import struct
import subprocess
import tomllib
import zipfile
from pathlib import Path

from m7_evidence import ROOT, command, digest, source_identity


def native_format(path: Path, system: str, machine: str) -> None:
    with path.open("rb") as handle:
        header = handle.read(4096)
    if system == "Windows":
        if len(header) < 64 or header[:2] != b"MZ":
            raise ValueError("expected a Windows PE executable")
        offset = struct.unpack_from("<I", header, 60)[0]
        with path.open("rb") as handle:
            handle.seek(offset)
            signature = handle.read(6)
        expected = 0xAA64 if machine.lower() in ("arm64", "aarch64") else 0x8664
        if len(signature) != 6 or signature[:4] != b"PE\0\0" or struct.unpack_from("<H", signature, 4)[0] != expected:
            raise ValueError("PE architecture does not match the packaging host")
    elif system == "Linux":
        expected = {"x86_64": 62, "aarch64": 183, "loongarch64": 258}.get(machine.lower())
        if expected is None or len(header) < 20 or header[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", header, 18)[0] != expected:
            raise ValueError("ELF architecture does not match the packaging host")
    elif system == "Darwin":
        expected = 0x0100000C if machine.lower() in ("arm64", "aarch64") else 0x01000007
        if len(header) < 8 or header[:4] != b"\xcf\xfa\xed\xfe" or struct.unpack_from("<I", header, 4)[0] != expected:
            raise ValueError("expected a native thin 64-bit Mach-O executable")
    else:
        raise ValueError("unsupported packaging host")


def sbom(metadata: dict, lock: dict, created: str) -> dict:
    """Target-filtered workspace source graph, including build/dev dependencies.

    This deliberately overapproximates linked dependencies; it is not a claim
    that every package is present in each binary or a completed license audit.
    """
    checksums = {(item["name"], item["version"], item.get("source")): item.get("checksum")
                 for item in lock["package"]}
    packages = sorted(metadata["packages"], key=lambda item: (item["name"], item["version"], item.get("source") or "workspace"))
    identifiers = {item["id"]: "SPDXRef-Package-" + hashlib.sha256(item["id"].encode()).hexdigest()[:24]
                   for item in packages}
    # Cargo path IDs contain the user's absolute workspace path. Use a stable
    # local identity for generated SPDX IDs and never serialize those paths.
    for item in packages:
        if item.get("source") is None:
            local_key = f"{item['name']}@{item['version']}".encode()
            identifiers[item["id"]] = "SPDXRef-Local-" + hashlib.sha256(local_key).hexdigest()[:24]
    if len(set(identifiers.values())) != len(identifiers):
        raise ValueError("ambiguous package identities in SPDX inventory")
    records = []
    for item in packages:
        source = item.get("source")
        record = {"SPDXID": identifiers[item["id"]], "name": item["name"],
                  "versionInfo": item["version"], "downloadLocation": "NOASSERTION",
                  "filesAnalyzed": False, "licenseConcluded": "NOASSERTION",
                  "licenseDeclared": item.get("license") or "NOASSERTION",
                  "copyrightText": "NOASSERTION"}
        if source and source.startswith("registry+https://github.com/rust-lang/crates.io-index"):
            record["downloadLocation"] = f"https://crates.io/api/v1/crates/{item['name']}/{item['version']}/download"
        checksum = checksums.get((item["name"], item["version"], source))
        if checksum:
            record["checksums"] = [{"algorithm": "SHA256", "checksumValue": checksum}]
        records.append(record)
    relationships = [{"spdxElementId": "SPDXRef-DOCUMENT", "relationshipType": "DESCRIBES",
                      "relatedSpdxElement": identifiers[item]} for item in metadata["workspace_members"]]
    for node in metadata["resolve"]["nodes"]:
        for dependency in node["dependencies"]:
            relationships.append({"spdxElementId": identifiers[node["id"]],
                                  "relationshipType": "DEPENDS_ON", "relatedSpdxElement": identifiers[dependency]})
    relationships.sort(key=lambda item: (item["spdxElementId"], item["relationshipType"], item["relatedSpdxElement"]))
    identity = hashlib.sha256(json.dumps([records, relationships, created], sort_keys=True).encode()).hexdigest()
    return {"spdxVersion": "SPDX-2.3", "dataLicense": "CC0-1.0", "SPDXID": "SPDXRef-DOCUMENT",
            "name": "JoyWord portable preview source dependency inventory",
            "documentNamespace": f"https://github.com/willmove/joyoffice/spdx/preview-{identity}",
            "creationInfo": {"creators": ["Tool: JoyWord-package-preview-1"], "created": created},
            "comment": "Target-filtered workspace source graph, including build/dev dependencies; not a linked-binary or license-compliance assertion.",
            "packages": records, "relationships": relationships}


def write_archive(path: Path, entries: dict[str, bytes]) -> None:
    # Stored members avoid zlib-dependent output; timestamps and modes are fixed.
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_STORED, allowZip64=False) as archive:
        for name, content in sorted(entries.items()):
            info = zipfile.ZipInfo("JoyWord-preview/" + name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (0o100755 if name in ("joyword", "joyword-cli") else 0o100644) << 16
            archive.writestr(info, content)
    with zipfile.ZipFile(path) as archive:
        if archive.testzip() is not None:
            raise ValueError("archive readback failed")
        for name, content in entries.items():
            if archive.read("JoyWord-preview/" + name) != content:
                raise ValueError("archive content readback mismatch")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary-dir", type=Path, default=ROOT / "target/release")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    system, machine = platform.system(), platform.machine()
    extension = ".exe" if system == "Windows" else ""
    names = ["joyword" + extension, "joyword-cli" + extension]
    for name in names:
        native_format(args.binary_dir / name, system, machine)
    host = next(line.split(": ", 1)[1] for line in command("rustc", "-vV").splitlines() if line.startswith("host: "))
    metadata = json.loads(command("cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", host))
    created = command("git", "show", "-s", "--format=%cI", "HEAD")
    from datetime import datetime, timezone
    created = datetime.fromisoformat(created).astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    inventory = sbom(metadata, lock, created)
    entries = {name: (args.binary_dir / name).read_bytes() for name in names}
    entries["LICENSE.txt"] = (ROOT / "LICENSE").read_bytes()
    entries["source-inventory.spdx.json"] = (json.dumps(inventory, sort_keys=True, indent=2) + "\n").encode()
    entries["README.txt"] = (
        "JoyWord unsigned portable preview\nExtract into a new folder and run joyword.\n"
        "No installer, file associations, automatic updates or signatures.\n"
        "Use document copies; see the compatibility and known-limits documentation.\n"
        "AI is disabled by default. No system fonts or reference Office assets are bundled.\n"
        "The SPDX source inventory includes build/dev dependencies; it is not a completed distribution license audit.\n"
    ).encode()
    # Include source-provided notices for every target-filtered dependency.
    # Missing notice files are recorded, so a preview cannot silently become a
    # signed-distribution license approval.
    missing_notices = []
    for item in metadata["packages"]:
        if not item.get("source"):
            continue
        directory = Path(item["manifest_path"]).parent
        notices = sorted(path for path in directory.iterdir() if path.is_file() and
                         path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")))
        if not notices:
            missing_notices.append(f"{item['name']}@{item['version']}")
        key = hashlib.sha256(item["id"].encode()).hexdigest()[:12]
        for notice in notices:
            entries[f"notices/{item['name']}-{item['version']}-{key}/{notice.name}"] = notice.read_bytes()
    manifest = {"schema": 1, "channel": "unsigned-portable-preview", "target": host,
                "source": source_identity(), "rustc": command("rustc", "--version"),
                "cargo_lock_sha256": digest(ROOT / "Cargo.lock"),
                "files": {name: hashlib.sha256(blob).hexdigest() for name, blob in sorted(entries.items())},
                "missing_dependency_notices": missing_notices, "stable_release_approved": False}
    entries["manifest.json"] = (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    if args.out.exists():
        parser.error("output already exists; choose a new artifact path")
    write_archive(args.out, entries)
    print(json.dumps({"archive": str(args.out), "sha256": digest(args.out), "bytes": args.out.stat().st_size,
                      "source_packages": len(inventory["packages"]), "missing_notices": len(missing_notices)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
