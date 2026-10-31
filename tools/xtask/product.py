#!/usr/bin/env python3
"""Product-scoped build/test/package and release-version routing for JoyOffice."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
PRODUCTS = {"joyword": ("joyword", "joyword-cli"), "joydeck": ("joydeck", "joydeck-cli"),
            "joysheet": ("joysheet", "joysheet-cli")}
GUI_PACKAGES = {"joyword": {"joyword"}, "joydeck": {"joydeck"},
                "joysheet": {"joysheet", "joysheet-ui-gpui", "joysheet-platform"}}
VALIDATION_FEATURES = {"joydeck": "joydeck/validation", "joysheet": "joysheet/automation"}

def manifest_version(product, root=ROOT):
    if product not in {name for pair in PRODUCTS.values() for name in pair}:
        raise ValueError("unknown application: " + product)
    text = (Path(root) / "apps" / product / "Cargo.toml").read_text(encoding="utf-8")
    section = re.search(r"(?ms)^\[package\]\s*(.*?)(?=^\[|\Z)", text)
    found = re.search(r'^version\s*=\s*"([^"]+)"', section[1] if section else "", re.M)
    if found:
        return found[1]
    if section and re.search(r"^version\.workspace\s*=\s*true", section[1], re.M):
        workspace = (Path(root) / "Cargo.toml").read_text(encoding="utf-8")
        section = re.search(r"(?ms)^\[workspace\.package\]\s*(.*?)(?=^\[|\Z)", workspace)
        found = re.search(r'^version\s*=\s*"([^"]+)"', section[1] if section else "", re.M)
        if found:
            return found[1]
    raise ValueError("missing product version: " + product)

def release_target(tag=None, product=None, version=None, root=ROOT):
    if tag:
        match = re.fullmatch(r"(?:(joyword|joydeck|joysheet)-)?v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)", tag)
        if not match:
            raise ValueError("unsupported product release tag: " + tag)
        routed, requested = match[1] or "joyword", match[2]
        if product and product != routed:
            raise ValueError("tag does not belong to requested product")
    else:
        if product not in PRODUCTS or not version:
            raise ValueError("manual release requires product and version")
        routed, requested = product, version
    if requested != manifest_version(routed, root):
        raise ValueError("release version does not match apps/" + routed + "/Cargo.toml")
    if manifest_version(PRODUCTS[routed][1], root) != requested:
        raise ValueError("CLI version does not match its product application")
    return {"product": routed, "version": requested,
            "tag": tag or routed + "-v" + requested}

def metadata():
    return json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT, text=True, encoding='utf-8'))

def product_packages(data, product, headless=False):
    if product not in PRODUCTS:
        raise ValueError("unknown product: " + product)
    members = set(data["workspace_members"])
    packages = {p["id"]: p for p in data["packages"]}
    chosen = {i for i in members if packages[i]["name"] == product or
              packages[i]["name"].startswith(product + "-")}
    if headless:
        chosen = {i for i in chosen if packages[i]["name"] not in GUI_PACKAGES[product]}
    # Include the shared libraries actually consumed by this product, not all
    # joyoffice-* packages and never another product's private crates.
    nodes = {p["id"]: p for p in data["resolve"]["nodes"]}
    queue = list(chosen)
    visited = set()
    while queue:
        key = queue.pop()
        if key in visited:
            continue
        visited.add(key)
        node = nodes.get(key, {})
        for dependency in node.get("deps", []):
            dep = dependency["pkg"]
            if dep not in members:
                continue
            name = packages[dep]["name"]
            if name.startswith("joyoffice-") and name != "joyoffice-xtask":
                chosen.add(dep)
                queue.append(dep)
            elif name == product or name.startswith(product + "-"):
                if headless and name in GUI_PACKAGES[product]:
                    raise ValueError("headless product depends on GUI package: " + name)
                chosen.add(dep)
                queue.append(dep)
            else:
                raise ValueError(product + " depends on another product: " + name)
    # Inspect the selected normal/build closure, including registry dependencies.
    # Removing a UI package from the test list must not conceal a transitive GUI edge.
    if headless:
        queue, seen = list(chosen), set()
        while queue:
            key = queue.pop()
            if key in seen:
                continue
            seen.add(key)
            name = packages[key]["name"]
            if name == "gpui" or name.startswith(("gpui-", "gpui_")):
                raise ValueError("headless product depends on GPUI: " + name)
            for dep in nodes.get(key, {}).get("deps", []):
                kinds = dep.get("dep_kinds", [{"kind": None}])
                if any(k.get("kind") != "dev" for k in kinds):
                    queue.append(dep["pkg"])
    if not chosen:
        raise ValueError("no workspace packages for " + product)
    return sorted(packages[i]["name"] for i in chosen)

def run(command):
    print("+ " + " ".join(str(v) for v in command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    version = sub.add_parser("release-version")
    version.add_argument("--tag")
    version.add_argument("--product", choices=PRODUCTS)
    version.add_argument("--version")
    version.add_argument("--github-output", type=Path)
    for name in ("ci", "smoke", "inventory", "bench", "wasm"):
        command = sub.add_parser(name)
        command.add_argument("product", choices=["joysheet"])
        command.add_argument("--stage", choices=["M" + str(n) for n in range(9)], default="M8")
        command.add_argument("--engine-only", action="store_true")
        command.add_argument("--headless", action="store_true")
    for name in ("check", "test", "build", "package", "packages", "stage"):
        command = sub.add_parser(name)
        command.add_argument("product", choices=PRODUCTS)
        command.add_argument("--headless", action="store_true")
        command.add_argument("--validation", action="store_true", help="Enable product native acceptance commands")
        command.add_argument("--release", action="store_true")
        command.add_argument("--target")
        if name == "stage":
            command.add_argument("stage", choices=["M" + str(n) for n in range(15)])
    args, extra = parser.parse_known_args(argv)
    if args.command in {"ci", "smoke", "inventory", "bench", "wasm"}:
        if extra:
            parser.error("unrecognized arguments: " + " ".join(extra))
        run([sys.executable, "scripts/joysheet/project.py", args.command,
             "--stage", args.stage, *(["--engine-only"] if args.engine_only or args.headless else [])])
        return
    if args.command == "release-version":
        if extra:
            parser.error("unrecognized arguments: " + " ".join(extra))
        result = release_target(args.tag, args.product, args.version)
        if args.github_output:
            with args.github_output.open("a", encoding="utf-8") as stream:
                for key, value in result.items():
                    stream.write(key + "=" + value + "\n")
        print(json.dumps(result))
        return
    if args.command == "package" and args.headless:
        parser.error("headless-only package output is not implemented; build --headless is available")
    if args.validation and (args.product not in VALIDATION_FEATURES or args.headless):
        parser.error("--validation requires a supported desktop product")
    data = metadata()
    packages = product_packages(data, args.product, args.headless)
    if args.command == "packages":
        print(json.dumps(packages))
        return
    selected = [item for name in packages for item in ("-p", name)]
    options = (["--release"] if args.release else []) + (["--target", args.target] if args.target else [])
    if args.validation:
        options += ["--features", VALIDATION_FEATURES[args.product]]
    if args.command in ("check", "test"):
        if extra:
            parser.error("unrecognized arguments: " + " ".join(extra))
        if args.command == "check":
            run(["cargo", "fmt", *selected, "--", "--check"])
            run(["cargo", "clippy", *selected, "--all-targets", "--locked", *options, "--", "-D", "warnings"])
        run(["cargo", "test", *selected, "--locked", *options])
    elif args.command in ("build", "stage"):
        applications = [PRODUCTS[args.product][1]] if args.headless else list(PRODUCTS[args.product])
        run(["cargo", "build", *[v for name in applications for v in ("-p", name)], "--locked", *options, *extra])
        if args.command == "stage":
            run(["cargo", "test", *selected, "--locked", *options])
            if args.product == "joysheet":
                run([sys.executable, "scripts/joysheet/project.py", "smoke", "--stage", args.stage,
                     *(["--engine-only"] if args.headless else [])])
    elif args.command == "package":
        if manifest_version(PRODUCTS[args.product][1]) != manifest_version(args.product):
            raise ValueError("CLI version does not match its product application")
        if args.target:
            parser.error("package currently runs natively; build --target is available separately")
        if args.product == "joydeck":
            run([sys.executable, "scripts/joydeck/package-native.py", "--profile", "release" if args.release else "debug", *(["--validation"] if args.validation else []), *extra])
        elif args.product == "joysheet":
            run([sys.executable, "scripts/joysheet/package.py", "--profile", "release" if args.release else "debug", *(["--validation"] if args.validation else []), *extra])
        else:
            rustc = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, text=True, encoding='utf-8')
            host = next(line.split(": ", 1)[1] for line in rustc.splitlines() if line.startswith("host:"))
            sys.path.insert(0, str(ROOT / "scripts/joydeck"))
            try:
                import sbom
            finally:
                sys.path.pop(0)
            baseline = sbom.snapshot(host, roots=PRODUCTS[args.product])
            # Explicit host target prevents CARGO_BUILD_TARGET or a Cargo config
            # from producing a cross binary labeled as a native package.
            output = subprocess.check_output(["cargo", "build", "-p", "joyword", "-p", "joyword-cli", "--locked",
                                              "--target", host, "--message-format=json-render-diagnostics", *options],
                                             cwd=ROOT, text=True, encoding='utf-8')
            artifacts = {}
            for line in output.splitlines():
                try:
                    value = json.loads(line)
                except ValueError:
                    print(line)
                    continue
                if value.get("reason") == "compiler-artifact" and value.get("executable"):
                    artifacts[value["target"]["name"]] = value["executable"]
            if not all(name in artifacts for name in PRODUCTS[args.product]):
                raise ValueError("Cargo did not report both product executable artifacts")
            system = {"Darwin": "macos", "Windows": "windows", "Linux": "linux"}[platform.system()]
            architecture = {"arm64": "aarch64", "aarch64": "aarch64", "AMD64": "x86_64", "x86_64": "x86_64"}[platform.machine()]
            binary = Path(artifacts["joyword"])
            sbom.require_unchanged_sources(baseline, sbom.snapshot(host, roots=PRODUCTS[args.product]))
            with tempfile.TemporaryDirectory(prefix="word-source-snapshot-", dir=data["target_directory"]) as temporary:
                snapshot_file = Path(temporary) / "sources.json"
                snapshot_file.write_text(json.dumps(baseline), encoding='utf-8')
                run([sys.executable, "scripts/package_release.py", "--binary", str(binary), "--cli", artifacts["joyword-cli"], "--version", manifest_version("joyword"), "--profile", "release" if args.release else "debug", "--source-snapshot", str(snapshot_file), "--platform", system, "--arch", architecture, "--out", "dist/joyword", *extra])

if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print("error: " + str(error), file=sys.stderr)
        sys.exit(1)
