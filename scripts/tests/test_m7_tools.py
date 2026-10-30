"""M7 evidence must reject incomplete, forged and changed release records."""
import copy
import hashlib
import json
import math
import struct
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_release
import m7_evidence
import package_preview


class ReleaseEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        docs = self.root / "docs/joyword"
        docs.mkdir(parents=True)
        original = (m7_evidence.ROOT / "docs/joyword/TASKS.md").read_text(encoding="utf-8")
        self.tasks = check_release.STATE.sub("- [x] 状态：`done`", original)
        (docs / "TASKS.md").write_text(self.tasks, encoding="utf-8")
        evidence = self.root / "evidence.json"
        evidence.write_text('{"synthetic": true}\n', encoding="utf-8")
        self.checklist = {"schema": 1, "observations": [
            {"id": name, "scope": "synthetic test only", "status": "passed",
             "evidence": [{"path": "evidence.json", "sha256": m7_evidence.digest(evidence)}]}
            for name in sorted(check_release.REQUIRED)]}

    def test_complete_synthetic_evidence_passes_and_hardware_pending_is_nonblocking(self):
        report = check_release.audit(self.root, self.checklist)
        self.assertTrue(report["release_ready"], report)
        hardware = next(item for item in self.checklist["observations"] if item["id"] == "xinchuang-hardware")
        hardware.update(status="pending", evidence=[], reason="hardware unavailable")
        report = check_release.audit(self.root, self.checklist)
        self.assertTrue(report["release_ready"], report)
        self.assertEqual(hardware["status"], "pending")

    def test_a_single_unfinished_task_or_observation_blocks_release(self):
        path = self.root / "docs/joyword/TASKS.md"
        path.write_text(self.tasks.replace("- [x] 状态：`done`", "- [ ] 状态：`in_progress`", 1), encoding="utf-8")
        self.assertFalse(check_release.audit(self.root, self.checklist)["release_ready"])
        path.write_text(self.tasks, encoding="utf-8")
        self.checklist["observations"][0].update(status="failed", reason="actual failure")
        self.assertFalse(check_release.audit(self.root, self.checklist)["release_ready"])

    def test_tampered_or_absent_evidence_cannot_close_a_gate(self):
        (self.root / "evidence.json").write_text("changed", encoding="utf-8")
        report = check_release.audit(self.root, self.checklist)
        self.assertFalse(report["release_ready"])
        self.assertTrue(report["invalid"])
        self.checklist["observations"][0]["evidence"] = []
        self.assertTrue(any("without evidence" in item for item in check_release.audit(self.root, self.checklist)["invalid"]))

    def test_duplicate_missing_or_outside_records_are_rejected(self):
        for edit in (lambda items: items.pop(), lambda items: items.append(copy.deepcopy(items[0]))):
            checklist = copy.deepcopy(self.checklist)
            edit(checklist["observations"])
            self.assertTrue(check_release.audit(self.root, checklist)["invalid"])
        self.checklist["observations"][0]["evidence"][0]["path"] = "../outside.json"
        self.assertTrue(any("escapes" in item for item in check_release.audit(self.root, self.checklist)["invalid"]))

    def test_missing_task_and_malformed_observations_are_not_a_release(self):
        path = self.root / "docs/joyword/TASKS.md"
        path.write_text(self.tasks.replace("### T700：", "### X700："), encoding="utf-8")
        self.assertTrue(check_release.audit(self.root, self.checklist)["invalid"])
        with self.assertRaises(ValueError):
            check_release.audit(self.root, {"schema": 1, "observations": [None]})


class ArtifactTests(unittest.TestCase):
    def test_percentiles_use_nearest_rank_and_reject_invalid_samples(self):
        stats = m7_evidence.distribution(list(range(1, 101)))
        self.assertEqual([stats[key] for key in ("p50", "p95", "p99", "max")], [50, 95, 99, 100])
        for samples in ([], [-1], [math.nan], [math.inf]):
            with self.assertRaises(ValueError):
                m7_evidence.distribution(samples)

    def test_architecture_mismatch_and_nonexecutables_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / "app.exe"
            binary.write_bytes(b"not an executable")
            with self.assertRaises(ValueError):
                package_preview.native_format(binary, "Windows", "AMD64")
            data = bytearray(256)
            data[:2] = b"MZ"
            struct.pack_into("<I", data, 60, 128)
            data[128:132] = b"PE\0\0"
            struct.pack_into("<H", data, 132, 0xAA64)
            binary.write_bytes(data)
            with self.assertRaises(ValueError):
                package_preview.native_format(binary, "Windows", "AMD64")
            package_preview.native_format(binary, "Windows", "ARM64")

    def test_preview_archives_are_reproducible_and_read_back_exactly(self):
        entries = {"joyword": b"synthetic executable", "LICENSE.txt": b"synthetic license"}
        with tempfile.TemporaryDirectory() as temp:
            first, second = Path(temp) / "a.zip", Path(temp) / "b.zip"
            package_preview.write_archive(first, entries)
            package_preview.write_archive(second, dict(reversed(list(entries.items()))))
            self.assertEqual(m7_evidence.digest(first), m7_evidence.digest(second))
            with zipfile.ZipFile(first) as archive:
                self.assertEqual(archive.read("JoyWord-preview/joyword"), entries["joyword"])
                self.assertEqual(archive.getinfo("JoyWord-preview/joyword").external_attr >> 16, 0o100755)

    def test_spdx_graph_checksums_and_local_path_redaction(self):
        local = "path+file:///private/user/work#app@0.0.1"
        remote = "registry+https://github.com/rust-lang/crates.io-index#dep@1.0.0"
        source = "registry+https://github.com/rust-lang/crates.io-index"
        metadata = {"packages": [
            {"id": local, "name": "app", "version": "0.0.1", "source": None, "license": "MIT"},
            {"id": remote, "name": "dep", "version": "1.0.0", "source": source, "license": "MIT OR Apache-2.0"}],
            "workspace_members": [local], "resolve": {"nodes": [
                {"id": local, "dependencies": [remote]}, {"id": remote, "dependencies": []}]}}
        checksum = hashlib.sha256(b"crate archive").hexdigest()
        output = package_preview.sbom(metadata, {"package": [{"name": "dep", "version": "1.0.0", "source": source, "checksum": checksum}]}, "2026-10-30T00:00:00Z")
        serialized = json.dumps(output)
        self.assertNotIn("/private/user", serialized)
        self.assertIn(checksum, serialized)
        self.assertEqual(len(output["relationships"]), 2)
        identifiers = {item["SPDXID"] for item in output["packages"]}
        for relationship in output["relationships"]:
            self.assertIn(relationship["relatedSpdxElement"], identifiers)
        changed = copy.deepcopy(metadata)
        changed["packages"][0]["id"] = local.replace("/private/user", "/other/user")
        changed_id = changed["packages"][0]["id"]
        changed["workspace_members"] = [changed_id]
        changed["resolve"]["nodes"][0]["id"] = changed_id
        self.assertEqual(output, package_preview.sbom(changed, {"package": [{"name": "dep", "version": "1.0.0", "source": source, "checksum": checksum}]}, "2026-10-30T00:00:00Z"))
        changed["packages"].reverse()
        changed["resolve"]["nodes"].reverse()
        self.assertEqual(output, package_preview.sbom(changed, {"package": [{"name": "dep", "version": "1.0.0", "source": source, "checksum": checksum}]}, "2026-10-30T00:00:00Z"))


if __name__ == "__main__":
    unittest.main()
