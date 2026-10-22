#!/usr/bin/env python3
"""T111 gates: no-panic baseline, crate layers, and filesystem allowlist.

A counterexample is any extra argument that is a source file the checks must
reject. The committed baseline lists existing unwrap/expect/panic sites.

Hardening notes (T111 follow-up, 2026-10-22):

* The layer check parses every Cargo dependency spelling actually used in this
  repository -- ``name.workspace = true``, ``name = { ... }`` and the
  ``[dependencies.name]`` table -- including ``[target.'cfg(...)'.dependencies]``
  sections. Normal dependencies are checked strictly against ``LAYERS``;
  dev-dependencies are checked against ``LAYERS`` plus the separately reviewed
  ``DEV_LAYERS`` allowance.
* The no-panic scan suppresses a site only for a genuine ``#[allow(...)]`` /
  ``#[expect(...)]`` lint attribute that names a panic-family lint. The word
  "allow" in a comment no longer hides a site. Duplicates are counted, so N
  identical sites need N baseline entries.
* The filesystem scan only skips the body of a ``#[cfg(test)]`` module (brace
  matched with comments and literals masked), so production code below a test
  module is scanned, and it recognises process spawning and the remaining
  ``std::fs`` entry points.
"""

from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
APPS = ROOT / "apps"

# Direct dependencies that match ARCHITECTURE §1. Paths are crate directories.
LAYERS = {
    "joyword-cli": {
        "joyoffice-opc",
        "joyword-docx",
        "joyword-foundation",
        "joyword-layout",
        "joyword-model",
        "joyword-pdf",
        "joyword-render",
    },
    "joyword-foundation": set(),
    # T400: no document, engine, or network crate. Core crates must not depend
    # on it; shell may, only as an optional non-default feature.
    "joyword-agent": set(),
    "joyword-model": {"joyword-foundation"},
    "joyword-engine": {"joyword-model", "joyword-foundation"},
    "joyoffice-opc": set(),
    "joyword-docx": {"joyoffice-opc", "joyword-model", "joyword-foundation"},
    "joyword-layout": {"joyword-foundation", "joyword-model"},
    "joyword-render": {"joyword-foundation", "joyword-layout", "joyword-model"},
    "joyword-pdf": {"joyword-foundation", "joyword-layout", "joyword-model"},
    "joyword-corpus": {"joyoffice-opc"},
    "joyword-shell": {
        "joyoffice-opc",
        "joyword-agent",
        "joyword-docx",
        "joyword-engine",
        "joyword-model",
    },
    "joyword": {
        "joyword-engine",
        "joyword-foundation",
        "joyword-layout",
        "joyword-model",
        "joyword-pdf",
        "joyword-render",
        "joyword-shell",
    },
}

# Dev-dependency allowances on top of LAYERS.
#
# An integration test, a benchmark or a cross-check between two output crates
# legitimately needs a crate that the production layering forbids: the corpus
# crate exists only to hand reproducible originals to tests, and the raster and
# PDF writers are compared against each other. Those edges must stay visible and
# reviewed, so every one of them is registered here with the manifest line that
# needs it. A dev-dependency that is absent from both LAYERS and DEV_LAYERS is
# an error -- adding one is a deliberate architecture decision, not a test
# convenience. ARCHITECTURE §1 documents the layer table; this dict is the
# reviewed exception list for `[dev-dependencies]` only and never weakens the
# normal-dependency check.
DEV_LAYERS = {
    # apps/joyword-cli/Cargo.toml:22 -- CLI integration tests replay corpus cases.
    "joyword-cli": {"joyword-corpus"},
    # crates/joyword-docx/Cargo.toml:17 -- DOCX round-trip tests read corpus originals.
    "joyword-docx": {"joyword-corpus"},
    # crates/joyword-layout/Cargo.toml:20-21 -- layout tests drive a real engine
    # session and compare page scenes against the raster output.
    "joyword-layout": {"joyword-engine", "joyword-render"},
    # crates/joyword-pdf/Cargo.toml:21 -- PDF tests build documents through the engine.
    "joyword-pdf": {"joyword-engine"},
    # crates/joyword-render/Cargo.toml:22 -- render tests cross-check rasters
    # against the PDF writer over one shared page scene.
    "joyword-render": {"joyword-pdf"},
}

FS_ALLOW = {
    "apps/joyword-cli/src/main.rs",
    "apps/joyword-cli/src/visual.rs",
    "crates/joyoffice-opc/src/zip.rs",
    "crates/joyoffice-opc/src/package.rs",
    "crates/joyword-shell/src/print.rs",
    "crates/joyword-shell/src/storage.rs",
    "crates/joyword-shell/src/storage/source.rs",
    "crates/joyword-shell/src/storage/original.rs",
    "crates/joyword-shell/src/storage/locks.rs",
    "crates/joyword-shell/src/storage/recovery.rs",
    "crates/joyword-shell/src/storage/metadata.rs",
    "crates/joyword-shell/src/prefs.rs",
    "crates/joyword-shell/src/diag.rs",
    "crates/joyword-shell/src/docx.rs",
    "crates/joyword-shell/src/host.rs",
    "crates/joyword-shell/src/input.rs",
    "apps/joyword/src/main.rs",
    "apps/joyword/src/document_canvas.rs",
    "apps/joyword/src/input_ui.rs",
    # Linux file-dialog fallback: gpui-pre-linux 0.3.9 stores `set_menus` and its
    # XDG portal prompt can hang with no window on UKUI/X11, so this module asks
    # for a path in-window and spawns zenity, qarma or kdialog. Same category as
    # the already-listed `input_ui.rs`, which spawns a process to open a URL, and
    # it cannot move to joyword-shell because the shell crate must not depend on
    # GPUI. Allowlisted 2026-10-22 by the coordinator when the hardened scan
    # first saw it; see GATES-REPORT.md.
    "apps/joyword/src/linux_files.rs",
}

PANIC_RE = re.compile(
    r"(?:\.unwrap\s*\(|\.expect\s*\(|\bpanic!\s*\(|\bunreachable!\s*\("
    r"|\btodo!\s*\(|\bunimplemented!\s*\()"
)
# Filesystem and process entry points. Call and import forms only: prose in a
# doc comment must not trip the allowlist, and nothing here may be satisfied by
# a word appearing in a string.
FS_RE = re.compile(
    r"(?:"
    r"\bstd::fs::"
    r"|\bfs::(?:read|write|create_dir|remove_file|remove_dir|rename|copy|metadata"
    r"|symlink|hard_link|canonicalize|read_link|File|OpenOptions|DirBuilder)"
    r"|\bFile::(?:open|create|open_options)\s*\("
    r"|\bOpenOptions::new\s*\("
    r"|\bDirBuilder::new\s*\("
    r"|\b(?:read_dir|remove_dir(?:_all)?|hard_link|symlink_file|symlink_dir)\s*\("
    r"|\b(?:std::)?process::Command\b"
    r"|\bCommand::new\s*\("
    r"|\benv::(?:set_var|remove_var)\s*\("
    r")"
)
# A dependency key in any Cargo spelling: `name = {...}`, `name.workspace = true`
# and `name.version = "1"` / `name.path = "..."` split across lines.
DEP_KEY_RE = re.compile(r"^([A-Za-z0-9_-]+)(?:\s*\.\s*[A-Za-z0-9_-]+)*\s*=")
# A genuine lint allowance. Only panic-family lints suppress a site, so an
# unrelated `#[allow(dead_code)]` above an `unwrap` cannot hide it.
LINT_ALLOW_RE = re.compile(r"#\s*\[\s*(?:allow|expect)\s*\(([^)\]]*)\)")
PANIC_LINT_RE = re.compile(
    r"\b(?:unwrap_used|expect_used|panic|unreachable|todo|unimplemented|indexing_slicing)\b"
)
# T400: the agent crate must not grow a socket client. Comments should not
# name these crates either; the scan is textual on purpose.
AI_NET_RE = re.compile(
    r"\b(?:std::net::|TcpStream|UdpSocket|reqwest::|hyper::|ureq::|tokio::net)\b"
)
CORE_WITHOUT_AGENT = (
    "crates/joyword-foundation/Cargo.toml",
    "crates/joyword-model/Cargo.toml",
    "crates/joyword-engine/Cargo.toml",
    "crates/joyword-docx/Cargo.toml",
    "crates/joyword-layout/Cargo.toml",
    "crates/joyword-pdf/Cargo.toml",
    "crates/joyword-render/Cargo.toml",
    "crates/joyoffice-opc/Cargo.toml",
    "crates/joyword-corpus/Cargo.toml",
    "apps/joyword-cli/Cargo.toml",
    "apps/joyword/Cargo.toml",
)

# Cargo dependency tables, mapped to the tier they belong to. Build scripts run
# at build time with full host access, so they are held to the normal tier.
DEPENDENCY_TABLES = {
    "dependencies": "normal",
    "dev-dependencies": "dev",
    "build-dependencies": "normal",
}


def rel(path: Path, root: Path = ROOT) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()


def crate_name(path: Path) -> str | None:
    text = path.read_text(encoding="utf-8")
    match = re.search(r'(?m)^name\s*=\s*"([^"]+)"', text)
    return match.group(1) if match else None


def _split_dotted(header: str) -> list[str]:
    """Split a TOML table header on dots that are not inside quotes."""
    parts: list[str] = []
    current: list[str] = []
    quote = ""
    for char in header:
        if quote:
            current.append(char)
            if char == quote:
                quote = ""
            continue
        if char in "'\"":
            quote = char
            current.append(char)
            continue
        if char == ".":
            parts.append("".join(current))
            current = []
            continue
        current.append(char)
    parts.append("".join(current))
    return [part.strip() for part in parts if part.strip()]


def dependency_table(header: str) -> tuple[str | None, str | None]:
    """Classify a manifest table header.

    Returns ``(tier, inline_name)`` where tier is ``"normal"``, ``"dev"`` or
    ``None`` for a table that holds no dependencies. ``inline_name`` is the
    package named by a ``[dependencies.name]`` style header.
    """
    if not (header.startswith("[") and header.endswith("]")):
        return None, None
    parts = _split_dotted(header[1:-1])
    if parts and parts[0] == "target":
        # [target.'cfg(unix)'.dependencies] and
        # [target.x86_64-unknown-linux-gnu.dev-dependencies]: the second part is
        # the target spec, quoted or not, and carries no dependency meaning.
        if len(parts) < 3:
            return None, None
        parts = parts[2:]
    if not parts:
        return None, None
    tier = DEPENDENCY_TABLES.get(parts[0].strip("'\""))
    if tier is None:
        return None, None
    name = parts[1].strip("'\"") if len(parts) > 1 else None
    return tier, name


def dependencies(manifest: Path) -> set[str]:
    """Normal-tier dependencies of a manifest, in every Cargo spelling."""
    return dependency_tiers(manifest)["normal"]


def dependency_tiers(manifest: Path) -> dict[str, set[str]]:
    """Parse `[dependencies]`, `[dev-dependencies]` and target-specific variants.

    Understands `name = {...}`, the dotted `name.workspace = true` form used
    throughout this repository, and the `[dependencies.name]` table form.
    """
    tiers: dict[str, set[str]] = {"normal": set(), "dev": set()}
    tier: str | None = None
    inline: str | None = None
    for raw in manifest.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("["):
            tier, inline = dependency_table(line)
            if tier and inline:
                tiers[tier].add(inline)
            continue
        if tier is None or inline is not None:
            # `inline` means the following keys describe one package already
            # registered above (`path`, `version`, `features`, ...).
            continue
        found = DEP_KEY_RE.match(line)
        if found:
            tiers[tier].add(found.group(1))
    return tiers


def layer_violations(crate: str, deps: set[str]) -> list[str]:
    allowed = LAYERS.get(crate)
    if allowed is None:
        return [f"unregistered crate {crate}"]
    return [
        f"{crate} depends on {dep}, which its layer does not allow"
        for dep in sorted(deps)
        if dep in LAYERS and dep not in allowed
    ]


def dev_layer_violations(crate: str, deps: set[str]) -> list[str]:
    allowed = LAYERS.get(crate)
    if allowed is None:
        return [f"unregistered crate {crate}"]
    allowed = allowed | DEV_LAYERS.get(crate, set())
    return [
        f"{crate} dev-depends on {dep}, which is registered in neither LAYERS nor DEV_LAYERS"
        for dep in sorted(deps)
        if dep in LAYERS and dep not in allowed
    ]


def check_layers(crates: Path = CRATES, apps: Path = APPS) -> list[str]:
    errors = []
    manifests = sorted(crates.glob("*/Cargo.toml")) + sorted(apps.glob("*/Cargo.toml"))
    for manifest in manifests:
        name = crate_name(manifest)
        if name not in LAYERS:
            errors.append(f"unregistered crate {name} at {rel(manifest)}")
            continue
        tiers = dependency_tiers(manifest)
        errors.extend(layer_violations(name, tiers["normal"]))
        errors.extend(dev_layer_violations(name, tiers["dev"]))
    for crate, deps in sorted(DEV_LAYERS.items()):
        if crate not in LAYERS:
            errors.append(f"DEV_LAYERS names unregistered crate {crate}")
        for dep in sorted(deps):
            if dep not in LAYERS:
                errors.append(f"DEV_LAYERS entry {crate} -> {dep} names an unregistered crate")
    return errors


def rust_files(crates: Path = CRATES, apps: Path = APPS) -> list[Path]:
    files = []
    for root in (crates, apps):
        if not root.exists():
            continue
        for path in root.rglob("*.rs"):
            if "/target/" in path.as_posix():
                continue
            files.append(path)
    return sorted(files, key=lambda path: path.as_posix())


def is_test_path(shown: str) -> bool:
    """True only for real test locations.

    A ``tests`` path *component* or a ``tests.rs`` module body. A directory that
    merely contains the substring (``src/contests/``, ``src/latest/``) is
    production code and stays in scope.
    """
    parts = shown.split("/")
    return "tests" in parts[:-1] or parts[-1] == "tests.rs"


def is_example_or_bin(shown: str) -> bool:
    parts = shown.split("/")
    if "examples" in parts[:-1]:
        return True
    return len(parts) >= 2 and parts[-2] == "bin" and "src" in parts[:-2]


def panic_scope(shown: str) -> bool:
    """Files the no-panic gate covers.

    Shared by ``check_panic`` and ``write_baseline`` so regenerating the
    baseline can never change what is gated. ``#[cfg(test)]`` modules inside a
    production file stay in scope: T111 inventoried their sites too.
    """
    return not is_test_path(shown)


def filesystem_scope(shown: str) -> bool:
    """Files the filesystem allowlist covers."""
    return not is_test_path(shown) and not is_example_or_bin(shown)


def mask_rust(text: str) -> str:
    """Blank out comments and literal contents, preserving offsets and newlines.

    Used only to locate ``#[cfg(test)]`` module bodies; the gate scans still run
    on the original text so that a commented-out call remains visible.
    """
    out: list[str] = []
    index = 0
    size = len(text)
    while index < size:
        char = text[index]
        pair = text[index:index + 2]
        if pair == "//":
            while index < size and text[index] != "\n":
                out.append(" ")
                index += 1
        elif pair == "/*":
            depth = 0
            while index < size:
                if text.startswith("/*", index):
                    depth += 1
                    out.append("  ")
                    index += 2
                elif text.startswith("*/", index):
                    out.append("  ")
                    index += 2
                    depth -= 1
                    if depth == 0:
                        break
                else:
                    out.append("\n" if text[index] == "\n" else " ")
                    index += 1
        elif (
            char in "br"
            and text[index + 1:index + 2] in ('"', "r", "#")
            # A `b`/`r` that ends an identifier is not a literal prefix.
            and not (index and (text[index - 1].isalnum() or text[index - 1] == "_"))
        ):
            cursor = index
            while cursor < size and text[cursor] in "br":
                cursor += 1
            hashes = 0
            while cursor < size and text[cursor] == "#":
                hashes += 1
                cursor += 1
            if cursor < size and text[cursor] == '"':
                cursor += 1
                terminator = '"' + "#" * hashes
                while cursor < size:
                    if hashes == 0 and text[cursor] == "\\":
                        cursor += 2
                        continue
                    if text.startswith(terminator, cursor):
                        cursor += len(terminator)
                        break
                    cursor += 1
                segment = text[index:cursor]
                inner = "".join("\n" if c == "\n" else " " for c in segment[1:-1])
                out.append(segment[0] + inner + (segment[-1:] if len(segment) > 1 else ""))
                index = cursor
            else:
                out.append(char)
                index += 1
        elif char == '"':
            cursor = index + 1
            while cursor < size:
                if text[cursor] == "\\":
                    cursor += 2
                    continue
                if text[cursor] == '"':
                    cursor += 1
                    break
                cursor += 1
            segment = text[index:cursor]
            inner = "".join("\n" if c == "\n" else " " for c in segment[1:-1])
            out.append('"' + inner + ('"' if len(segment) > 1 else ""))
            index = cursor
        elif char == "'":
            found = re.match(r"'(?:\\.|[^'\\])'", text[index:])
            if found:
                segment = found.group(0)
                out.append(" " * (len(segment) - 1) + segment[-1])
                index += len(segment)
            else:
                out.append(char)
                index += 1
        else:
            out.append(char)
            index += 1
    return "".join(out)


CFG_TEST_RE = re.compile(r"^\s*#\s*\[\s*cfg\s*\(.*\btest\b.*\)\s*\]\s*$")
SKIPPABLE_RE = re.compile(r"^\s*#\s*!?\[|^\s*//|^\s*$")
MODULE_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*([{;])"
)


def test_module_ranges(text: str) -> list[tuple[int, int]]:
    """1-based inclusive line ranges of ``#[cfg(test)]`` module declarations.

    Inline modules are brace-matched on masked text so that braces inside
    strings, chars and comments cannot move the boundary. An out-of-line
    ``#[cfg(test)] #[path = "tests.rs"] mod name;`` covers only its declaration.
    A ``#[cfg(test)]`` on a `use`, field or fn is not a module and hides nothing.
    """
    lines = mask_rust(text).split("\n")
    ranges: list[tuple[int, int]] = []
    index = 0
    while index < len(lines):
        if not CFG_TEST_RE.match(lines[index]):
            index += 1
            continue
        cursor = index + 1
        while cursor < len(lines) and SKIPPABLE_RE.match(lines[cursor]):
            cursor += 1
        found = MODULE_RE.match(lines[cursor]) if cursor < len(lines) else None
        if found is None:
            index += 1
            continue
        if found.group(2) == ";":
            ranges.append((index + 1, cursor + 1))
            index = cursor + 1
            continue
        depth = 0
        end = len(lines) - 1
        for offset in range(cursor, len(lines)):
            depth += lines[offset].count("{") - lines[offset].count("}")
            if depth <= 0:
                end = offset
                break
        ranges.append((index + 1, end + 1))
        index = end + 1
    return ranges


def production_lines(text: str) -> list[tuple[int, str]]:
    """Numbered lines of ``text`` outside any ``#[cfg(test)]`` module body."""
    ranges = test_module_ranges(text)
    return [
        (number, line)
        for number, line in enumerate(text.split("\n"), 1)
        if not any(start <= number <= end for start, end in ranges)
    ]


def check_filesystem(crates: Path = CRATES, apps: Path = APPS, root: Path = ROOT) -> list[str]:
    errors = []
    for path in rust_files(crates, apps):
        shown = rel(path, root)
        if not filesystem_scope(shown) or shown in FS_ALLOW:
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for number, line in production_lines(text):
            if FS_RE.search(line):
                errors.append(
                    f"filesystem access outside the allowlist: {shown}:{number}: {line.strip()}"
                )
    return errors


def lint_suppressed(lines: list[str], index: int) -> bool:
    """True when a panic-family lint attribute genuinely covers ``lines[index]``."""
    for candidate in (lines[index], lines[index - 1] if index else ""):
        found = LINT_ALLOW_RE.search(candidate)
        if found and PANIC_LINT_RE.search(found.group(1)):
            return True
    return False


def panic_sites(path: Path, root: Path = ROOT) -> list[str]:
    lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
    sites = []
    for index, line in enumerate(lines):
        if not PANIC_RE.search(line) or lint_suppressed(lines, index):
            continue
        sites.append(f"{rel(path, root)}:{index + 1}:{line.strip()}")
    return sites


def panic_payload(site: str) -> str:
    """``path:code`` -- line numbers drift, the code text is the identity."""
    parts = site.split(":", 2)
    return parts[0] + ":" + (parts[2] if len(parts) == 3 else site)


def gated_panic_sites(crates: Path = CRATES, apps: Path = APPS, root: Path = ROOT) -> list[str]:
    sites = []
    for path in rust_files(crates, apps):
        shown = rel(path, root)
        if not panic_scope(shown):
            continue
        sites.extend(panic_sites(path, root))
    return sites


def read_baseline(baseline: Path) -> list[str]:
    if not baseline.exists():
        return []
    return [
        line
        for line in baseline.read_text(encoding="utf-8").splitlines()
        if line and not line.startswith("#")
    ]


def check_panic(
    baseline: Path, crates: Path = CRATES, apps: Path = APPS, root: Path = ROOT
) -> list[str]:
    current = gated_panic_sites(crates, apps, root)
    expected = read_baseline(baseline)
    current_counts = Counter(panic_payload(site) for site in current)
    expected_counts = Counter(panic_payload(site) for site in expected)
    errors = []
    # Line numbers drift, so the comparison is per (file, code text). It is a
    # multiset comparison: N identical sites in one file need N baseline entries.
    for key in sorted(current_counts):
        found = current_counts[key]
        listed = expected_counts.get(key, 0)
        if found <= listed:
            continue
        where = ", ".join(
            site.split(":", 2)[1] for site in current if panic_payload(site) == key
        )
        errors.append(
            f"new panic site: {key} occurs {found}x but the baseline lists it {listed}x "
            f"(lines {where}); append the missing entries to scripts/panic_baseline.txt"
        )
    return errors


def write_baseline(
    baseline: Path, crates: Path = CRATES, apps: Path = APPS, root: Path = ROOT
) -> int:
    sites = gated_panic_sites(crates, apps, root)
    baseline.parent.mkdir(parents=True, exist_ok=True)
    baseline.write_text("\n".join(sites) + "\n", encoding="utf-8")
    return len(sites)


def missing_baseline_entries(
    baseline: Path, crates: Path = CRATES, apps: Path = APPS, root: Path = ROOT
) -> list[str]:
    """Baseline lines to append so the multiplicity gate passes.

    Existing lines are kept byte-for-byte (their line numbers are allowed to
    drift), so top-ups stay a small reviewable diff instead of a rewrite. Every
    occurrence in the tree consumes one baseline entry for its payload, which
    is what makes the appended count exact.
    """
    current = gated_panic_sites(crates, apps, root)
    listed = read_baseline(baseline)
    budget = Counter(panic_payload(line) for line in listed)
    exact = set(listed)
    unlisted = []
    for site in current:
        key = panic_payload(site)
        # Prefer the entries whose line number still matches, so the appended
        # lines never duplicate a line the file already spells exactly.
        if site in exact and budget[key] > 0:
            budget[key] -= 1
            continue
        unlisted.append(site)
    appended = []
    for site in unlisted:
        key = panic_payload(site)
        if budget[key] > 0:
            budget[key] -= 1
            continue
        appended.append(site)
    return appended


def manifest_layer_problems(text: str) -> list[str]:
    """Same allowlist as ``check_layers``, over a manifest fragment."""
    def uncomment(line: str) -> str:
        stripped = line.strip()
        if stripped.startswith("//"):
            stripped = stripped[2:].strip()
        return stripped.lstrip("#").strip()

    name = None
    for line in text.splitlines():
        stripped = uncomment(line)
        if stripped.startswith("name ") or stripped.startswith("name="):
            match = re.search(r'name\s*=\s*"([^"]+)"', stripped)
            if match:
                name = match.group(1)
    deps: set[str] = set()
    dev: set[str] = set()
    tier: str | None = None
    inline: str | None = None
    for line in text.splitlines():
        stripped = uncomment(line)
        if stripped.startswith("["):
            tier, inline = dependency_table(stripped)
            if tier and inline:
                (deps if tier == "normal" else dev).add(inline)
            continue
        if tier is None or inline is not None:
            continue
        found = DEP_KEY_RE.match(stripped)
        if found:
            (deps if tier == "normal" else dev).add(found.group(1))
    if name is None or not (deps or dev):
        return []
    return layer_violations(name, deps) + dev_layer_violations(name, dev)


def check_ai_isolation(root: Path = ROOT) -> list[str]:
    """Core crates stay free of the agent, and the agent stays free of sockets."""
    errors = []
    for relative in CORE_WITHOUT_AGENT:
        text = (root / relative).read_text(encoding="utf-8")
        if "joyword-agent" in text:
            errors.append(f"{relative} must not reference joyword-agent")
    shell = (root / "crates/joyword-shell/Cargo.toml").read_text(encoding="utf-8")
    if "optional = true" not in shell or "joyword-agent" not in shell:
        errors.append("joyword-shell must depend on joyword-agent only as an optional crate")
    default = re.search(r"(?m)^default\s*=\s*\[([^\]]*)\]", shell)
    if default is None or "ai" in default.group(1):
        errors.append("joyword-shell must not enable the ai feature by default")
    app = (root / "apps/joyword/Cargo.toml").read_text(encoding="utf-8")
    app_default = re.search(r"(?m)^default\s*=\s*\[([^\]]*)\]", app)
    if app_default is None or "ai" in app_default.group(1):
        errors.append("joyword must not enable the ai feature by default")
    agent_src = root / "crates/joyword-agent"
    if not agent_src.is_dir():
        errors.append("joyword-agent crate is missing")
        return errors
    manifest = (agent_src / "Cargo.toml").read_text(encoding="utf-8")
    if AI_NET_RE.search(manifest):
        errors.append("joyword-agent Cargo.toml names a network client")
    for path in (agent_src / "src").rglob("*.rs"):
        text = path.read_text(encoding="utf-8", errors="replace")
        if AI_NET_RE.search(text):
            errors.append(f"network client reference in {rel(path, root)}")
    return errors


def counterexample_tree(root: Path, source: str, manifest: str | None = None) -> None:
    """Materialise a throwaway crate that every gate must reject."""
    crates = root / "crates"
    apps = root / "apps"
    model = crates / "joyword-model"
    (model / "src").mkdir(parents=True, exist_ok=True)
    apps.mkdir(exist_ok=True)
    (model / "Cargo.toml").write_text(
        manifest
        if manifest is not None
        else '[package]\nname = "joyword-model"\n\n[dependencies]\n'
        'joyword-shell = { path = "../joyword-shell" }\n',
        encoding="utf-8",
    )
    (model / "src" / "lib.rs").write_text(source, encoding="utf-8")
    (root / "panic_baseline.txt").write_text("", encoding="utf-8")


def counterexample(path: Path) -> list[str]:
    """Run ``check_layers``, ``check_filesystem`` and ``check_panic`` on a
    throwaway crate. A stubbed guard returns no findings and this command
    then fails.
    """
    import tempfile

    path = path if path.is_absolute() else (ROOT / path)
    source = path.read_text(encoding="utf-8", errors="replace")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        counterexample_tree(root, source)
        crates = root / "crates"
        apps = root / "apps"
        layers = check_layers(crates, apps)
        files = check_filesystem(crates, apps, root)
        panics = check_panic(root / "panic_baseline.txt", crates, apps, root)
    if not layers or not files or not panics:
        return []
    return layers + files + panics


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--write-baseline":
        baseline = ROOT / "scripts" / "panic_baseline.txt"
        print(f"wrote {write_baseline(baseline)} sites")
        return 0
    if len(sys.argv) > 1 and sys.argv[1] == "--update-baseline":
        baseline = ROOT / "scripts" / "panic_baseline.txt"
        appended = missing_baseline_entries(baseline)
        if appended:
            existing = baseline.read_text(encoding="utf-8").splitlines()
            baseline.write_text(
                "\n".join(existing + appended) + "\n", encoding="utf-8"
            )
        print(f"appended {len(appended)} sites")
        return 0
    if len(sys.argv) > 1 and sys.argv[1] == "--counterexample":
        problems = []
        for arg in sys.argv[2:]:
            problems.extend(counterexample(Path(arg)))
        if not problems:
            print("counterexample was not rejected")
            return 1
        print("\n".join(problems))
        return 0
    errors = []
    errors.extend(check_layers())
    errors.extend(check_filesystem())
    errors.extend(check_ai_isolation())
    errors.extend(check_panic(ROOT / "scripts" / "panic_baseline.txt"))
    if errors:
        print("\n".join(errors))
        return 1
    print(f"gates ok ({len(rust_files())} rust files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
