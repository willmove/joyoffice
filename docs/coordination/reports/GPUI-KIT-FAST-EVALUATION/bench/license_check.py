#!/usr/bin/env python3
"""Approximate the repository's cargo-deny license allow-list against each
variant's full resolved graph (all targets). Not a cargo-deny substitute:
it only parses the SPDX `license` field (no license-file clarification)."""
import json
import re
import subprocess
import sys
import os

ALLOW = {
    "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
    "CC0-1.0", "ISC", "MIT", "MPL-2.0", "NCSA", "Unicode-3.0", "Zlib",
}


def tokens(expr):
    return re.findall(r"\(|\)|[A-Za-z0-9.+\-]+(?: WITH [A-Za-z0-9.+\-]+)?", expr.replace("/", " OR "))


def ok(expr):
    toks = tokens(expr)
    pos = 0

    def parse_or():
        nonlocal pos
        v = parse_and()
        while pos < len(toks) and toks[pos] == "OR":
            pos += 1
            r = parse_and()
            v = v or r
        return v

    def parse_and():
        nonlocal pos
        v = parse_atom()
        while pos < len(toks) and toks[pos] == "AND":
            pos += 1
            r = parse_atom()
            v = v and r
        return v

    def parse_atom():
        nonlocal pos
        t = toks[pos]
        pos += 1
        if t == "(":
            v = parse_or()
            pos += 1
            return v
        return t in ALLOW

    return parse_or()


for v in sys.argv[1:]:
    meta = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        cwd=v, capture_output=True, text=True, check=True).stdout)
    resolved = {n["id"] for n in meta["resolve"]["nodes"]}
    bad, nolic = [], []
    by_license = {}
    for p in meta["packages"]:
        if p["id"] not in resolved or p["source"] is None:
            continue
        lic = p.get("license")
        if not lic:
            nolic.append(f'{p["name"]} {p["version"]}')
            continue
        by_license[lic] = by_license.get(lic, 0) + 1
        if not ok(lic):
            bad.append(f'{p["name"]} {p["version"]}: {lic}')
    print(f"== {os.path.basename(v)}: packages={len(resolved)} not_allowed={len(bad)} no_license_field={len(nolic)}")
    for b in sorted(bad):
        print("   DENY?", b)
    for b in sorted(nolic):
        print("   NOLICENSE", b)
