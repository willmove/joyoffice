"""Compare JoyWord's language-pack installer, rollback and original release files in an isolated store."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess
import tarfile


def sha(data):
    return hashlib.sha256(data).hexdigest()


def manifest(archive):
    files = {}
    with subprocess.Popen(["zstd", "-dq", "-c", archive], stdout=subprocess.PIPE) as process:
        with tarfile.open(fileobj=process.stdout, mode="r|") as tar:
            for member in tar:
                if member.isfile():
                    name = member.name.removeprefix("./")
                    assert name not in files
                    files[name] = sha(tar.extractfile(member).read())
        for _ in iter(lambda: process.stdout.read(65536), b""):
            pass
        process.stdout.close()
        assert process.wait() == 0
    return files


def verify(old, new, released, out):
    proof = runpy.run_path(str(Path(__file__).with_name("check-spelling-order.py")))
    pin = proof["PUBLISHED_INDEX"]
    index = json.loads((released / "index.json").read_bytes())
    assert sha((released / "index.json").read_bytes()) == pin
    folder = out / "store"
    folder.mkdir()
    pools = folder / "language-packs"
    cache = folder / "language-packs.json"
    local_index = out / "mirrored-index.json"
    fixture = out / "source.docx"
    proof["make_document"](fixture, "nl", ["teh"])
    original = sha(fixture.read_bytes())
    expected = {}
    for entry in index["packs"]:
        locale = entry["locale"]
        archive = released / (locale + ".tar.zst")
        assert archive.stat().st_size == entry["size"] and sha(archive.read_bytes()) == entry["sha256"]
        expected[locale] = manifest(archive)
        entry["url"] = str(archive)
    local_index.write_text(json.dumps(index))
    env = dict(os.environ, JOYWORD_CONFIG_DIR=str(out/"settings"), CRAFT_FONT_STORE=str(folder),
               CRAFT_LANGUAGE_PACK_INDEX=str(local_index))

    def run(binary, commands, success=True):
        args = [binary, "run", fixture, *(item for c in commands for item in ["--cmd", c])]
        result = subprocess.run(args, env=env, capture_output=True, timeout=300)
        if (result.returncode == 0) != success:
            raise RuntimeError(f"unexpected status {result.returncode}: {result.stderr.decode(errors='replace')}")

    def operation(name, locale):
        return "proof."+name+'='+json.dumps({"locale":locale}, separators=(",", ":"))

    def installed(binary):
        run(binary, ["proof.packs"])
        # The CLI's run command prints the document summary. Read the public
        # on-disk inventory independently after the normal command has returned.
        inventory = json.loads(cache.read_text())["packs"]
        tags = {row["locale"] for row in inventory}
        assert len(tags) == len(inventory)
        assert tags == {d.name for d in pools.iterdir() if d.is_dir()}
        return tags

    def actual(locale):
        root = pools / locale
        return {p.relative_to(root).as_posix():sha(p.read_bytes()) for p in root.rglob("*") if p.is_file()}

    refresh = 'proof.packs={"refresh":true}'
    run(new, [refresh], False)
    assert not (folder/"indexes/language-packs.json").exists()
    (folder/"settings.json").write_text('{"network":"on"}')
    run(new, [refresh])
    locales = sorted(expected)
    split = len(locales)//2
    for binary, subset in [(old,locales[:split]), (new,locales[split:])]:
        for i in range(0,len(subset),8):
            run(binary,[operation("installPack",tag) for tag in subset[i:i+8]])
    assert installed(old)==installed(new)==set(locales)
    records = {r["locale"]:r for r in json.loads(cache.read_bytes())["packs"]}
    for entry in index["packs"]:
        tag = entry["locale"]
        assert records[tag]["sha256"]==entry["sha256"] and records[tag]["licence"]==entry["licence"]
        assert actual(tag)==expected[tag]
    # In a new process, opening the store must recover the interrupted change
    # before the dictionary is handed to the proofing engine.
    journal = folder/".language-pack-change"
    crash_points = []
    for action in ("replace","remove"):
        for step in ("journal","old-moved","candidate-published","inventory-written"):
            locale = "nl"
            dest = pools/locale
            journal.mkdir()
            (journal/"previous.json").write_text(json.dumps({"locale":locale,"had_folder":True,"entry":records[locale]}))
            if step != "journal":
                dest.rename(journal/"previous")
            if action=="replace" and step in ("candidate-published","inventory-written"):
                dest.mkdir()
                (dest/"partial.dic").write_text("unpublished")
            if step=="inventory-written":
                rows = json.loads(cache.read_text())
                rows["packs"] = [r for r in rows["packs"] if r["locale"]!=locale]
                cache.write_text(json.dumps(rows))
            assert installed(new)==set(locales) and not journal.exists()
            assert actual(locale)==expected[locale] and installed(old)==set(locales)
            crash_points.append([action,step])
    # Reinstall with the other version to verify publication over an older
    # directory and its own inventory, then remove with both program versions.
    for binary, group in [(new,locales[:split]),(old,locales[split:])]:
        for i in range(0,len(group),8):
            run(binary,[operation("installPack",tag) for tag in group[i:i+8]])
    for tag in locales:
        assert actual(tag)==expected[tag]
    # Separate CLI processes share the inventory. Exercise two simultaneous
    # replacements of one locale along with writes to unrelated entries.
    parallel = [locales[0], locales[1], locales[0], locales[2]]
    with ThreadPoolExecutor(max_workers=len(parallel)) as workers:
        list(workers.map(lambda tag: run(new, [operation("installPack", tag)]), parallel))
    assert installed(new)==installed(old)==set(locales)
    for tag in set(parallel):
        assert actual(tag)==expected[tag]
    for binary, group in [(new,locales[:split]),(old,locales[split:])]:
        for i in range(0,len(group),8):
            run(binary,[operation("removePack",tag) for tag in group[i:i+8]])
    assert not list(pools.iterdir()) and installed(new)==installed(old)==set()
    assert not journal.exists() and sha(fixture.read_bytes())==original
    return {"baselineSha256":sha(old.read_bytes()),"candidateSha256":sha(new.read_bytes()),"indexSha256":pin,
            "archives":len(locales),"regularFiles":sum(len(x) for x in expected.values()),
            "licenseFiles":sum(sum(k.startswith("LICENCES/") for k in x) for x in expected.values()),
            "archiveBytes":sum(e["size"] for e in index["packs"]),"firstInstalledWithOld":split,
            "firstInstalledWithNew":len(locales)-split,"crossVersionReplacements":len(locales),
            "crossVersionInventoryVerified":True,"verifiedCrashStates":crash_points,
            "concurrentReplacements":len(parallel),"allRemoved":True,
            "originalUnchanged":True,"consentOffRefused":True}


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    for arg in ("baseline","candidate","release_packs","output"):
        parser.add_argument(arg,type=Path)
    args=parser.parse_args()
    out=args.output.resolve()
    out.mkdir(parents=True,exist_ok=False)
    data=verify(args.baseline.resolve(),args.candidate.resolve(),args.release_packs.resolve(),out)
    (out/"report.json").write_text(json.dumps(data,indent=2)+"\n")
