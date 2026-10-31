import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("repository", SOURCE / "scripts/check-repository.py")
repository = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(repository)


class RepositoryGates(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        for name in ("Cargo.toml", "apps/joydeck/Cargo.toml", "xtask/Cargo.toml", ".github/workflows/ci.yml"):
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(SOURCE / name, target)

    def test_current_configuration(self):
        self.assertEqual(repository.check(self.root), [])

    def test_unregistered_crate(self):
        target = self.root / "crates/extra/Cargo.toml"
        target.parent.mkdir(parents=True)
        target.write_text('[package]\nname = "extra"\n')
        self.assertTrue(any("unregistered crate extra" in x for x in repository.check(self.root)))

    def test_cross_product_target_build_dependency(self):
        target = self.root / "apps/joydeck/Cargo.toml"
        with target.open("a") as out:
            out.write('\n[target.\'cfg(unix)\'.build-dependencies]\nalias = { package = "joyword-model", path = "../../../crates/joyword-model" }\n')
        self.assertTrue(any("internal dependency: joyword-model" in x for x in repository.check(self.root)))

    def test_action_tags_fail(self):
        target = self.root / ".github/workflows/ci.yml"
        with target.open("a") as out:
            out.write("\n      - uses: actions/checkout@v4\n")
        self.assertTrue(any("full commit SHA" in x for x in repository.check(self.root)))


if __name__ == "__main__":
    unittest.main()
