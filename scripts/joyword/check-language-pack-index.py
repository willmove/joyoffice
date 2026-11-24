"""Check the bundled index and upgrade/rollback cache behavior through JoyWord CLI."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess


def verify(old, candidate, legacy, output):
    output.mkdir(parents=True)
    fixture = output / "source.docx"
    helpers = runpy.run_path(str(Path(__file__).with_name("check-spelling-order.py")))
    helpers["make_document"](fixture, "nl", ["lettergrepen"])
    sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    original = sha(fixture)
    assert sha(legacy) == helpers["PUBLISHED_INDEX"]
    root = Path(__file__).resolve().parents[2]
    bundled = root / "crates/joyword-engine/data/language-pack-index.json"
    store = output / "assets"
    (store / "indexes").mkdir(parents=True)
    env = dict(os.environ, CRAFT_FONT_STORE=str(store), CRAFT_LANGUAGE_PACK_INDEX="",
               JOYWORD_CONFIG_DIR=str(output / "profile"))
    first_party = store / "indexes/joyword-language-packs.json"
    old_cache = store / "indexes/language-packs.json"

    def run(binary, command, success=False):
        done = subprocess.run([binary, "run", fixture, "--cmd", command],
                              capture_output=True, encoding="utf-8", env=env, timeout=30)
        assert (done.returncode == 0) == success, done.stderr
        return done.stderr.strip()

    refresh = 'proof.packs={"refresh":true}'
    install = 'proof.installPack={"locale":"nl"}'
    remove = 'proof.removePack={"locale":"nl"}'
    reference = json.loads(legacy.read_bytes())["packs"]
    dutch = next(item for item in reference if item["locale"] == "nl")

    def readback(binary):
        run(binary, install, True)
        inventory = json.loads((store / "language-packs.json").read_bytes())
        record, = inventory["packs"]
        assert (record["locale"], record["sha256"], record["size"]) == ("nl", dutch["sha256"], dutch["size"])
        run(binary, remove, True)

    assert run(old, refresh) == run(candidate, refresh)
    assert not first_party.exists() and not old_cache.exists()
    settings = store / "settings.json"
    consent = b'{"network":"on","vendorSetting":true}'
    settings.write_bytes(consent)
    absent = run(old, install)
    assert run(candidate, install) == absent
    old_cache.write_bytes(legacy.read_bytes())
    readback(old)
    readback(candidate)
    assert not first_party.exists()
    published = json.loads(bundled.read_bytes())["packs"]
    remapped = []
    for item in published:
        original_item = item.copy()
        fallback = original_item.pop("fallback_url", None)
        if fallback is not None:
            original_item["url"] = fallback
        remapped.append(original_item)
    assert len(reference) == 75 and remapped == reference
    assert sum("fallback_url" in item for item in published) == 41
    run(candidate, refresh, True)
    assert first_party.read_bytes() == bundled.read_bytes()
    readback(candidate)
    readback(old)
    assert old_cache.read_bytes() == legacy.read_bytes()
    first_party.write_bytes(b'{"format":1,"packs":[]}')
    readback(candidate)
    old_cache.write_bytes(b'{}')
    assert run(candidate, install) == absent
    run(candidate, refresh, True)
    readback(candidate)
    assert run(old, install) == absent
    assert first_party.read_bytes() == bundled.read_bytes()
    assert settings.read_bytes() == consent and sha(fixture) == original
    hosted = next(item for item in published if item["locale"] == "nl").copy()
    override = output / "host-probe.json"
    env["CRAFT_LANGUAGE_PACK_INDEX"] = str(override)

    def refresh_override(item):
        override.write_text(json.dumps({"format": 1, "release": "joyword-language-packs-1", "packs": [item]}), encoding="utf-8")
        run(candidate, refresh, True)

    fallback = hosted.pop("fallback_url")
    refresh_override(hosted)
    readback(candidate)
    refresh_override(dict(hosted, url=hosted["url"].replace("nl.tar.zst", "missing-nl.tar.zst"), fallback_url=fallback))
    readback(candidate)
    refresh_override(dict(hosted, sha256="a" * 64, fallback_url=fallback))
    run(candidate, install)
    assert json.loads((store / "language-packs.json").read_bytes())["packs"] == []
    evidence = dict(packs=len(reference), consentRefused=True, bundledRefresh=True,
                    verifiedLegacyCacheMigration=True, rollbackReadsOriginalCache=True,
                    hostedPrimary=True, upstreamFallback=True, invalidDigestRejectedOnBothOrigins=True,
                    corruptCacheRejected=True, indexSha256=sha(bundled), originalSourceSha256=original)
    (output / "report.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "legacy_index", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    verify(*(getattr(args, name).resolve() for name in ("baseline", "candidate", "legacy_index", "output")))
