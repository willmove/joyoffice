"""Exercise isolation and refusal paths against a disposable local Git remote."""

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "prepare_agent_workspace.py"
REF = "codex/multi-agent-coordination"


class WorkspacePreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "control"
        self.source.mkdir()
        self.run_git("init", cwd=self.source)
        self.run_git("config", "user.name", "Workspace Test", cwd=self.source)
        self.run_git("config", "user.email", "workspace-test@example.invalid", cwd=self.source)
        (self.source / "baseline.txt").write_text("baseline\n", encoding="utf-8")
        self.run_git("add", "baseline.txt", cwd=self.source)
        self.run_git("commit", "-m", "baseline", cwd=self.source)
        baseline = self.run_git("rev-parse", "HEAD", cwd=self.source)
        self.run_git("switch", "-c", REF, cwd=self.source)
        (self.source / "scripts").mkdir()
        shutil.copyfile(SCRIPT, self.source / "scripts/prepare_agent_workspace.py")
        docs = self.source / "docs/coordination"
        docs.mkdir(parents=True)
        (docs / "task.md").write_text("task\n", encoding="utf-8")
        (docs / "plan.json").write_text(json.dumps({
            "code_base": baseline, "coordination_ref": "refs/heads/" + REF,
            "tasks": [{"id": "W01", "branch": "codex/w01-test", "directory": "task",
                       "card": "docs/coordination/task.md"}],
        }), encoding="utf-8")
        self.run_git("add", "scripts", "docs", cwd=self.source)
        self.run_git("commit", "-m", "coordination", cwd=self.source)
        self.start = self.run_git("rev-parse", "HEAD", cwd=self.source)
        self.remote = self.root / "remote.git"
        self.run_git("init", "--bare", str(self.remote), cwd=self.root)
        self.run_git("remote", "add", "origin", str(self.remote), cwd=self.source)
        self.run_git("push", "origin", REF, cwd=self.source)

    def run_git(self, *args, cwd):
        return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True,
                              text=True).stdout.strip()

    def invoke(self, *args):
        return subprocess.run([sys.executable, str(self.source / "scripts/prepare_agent_workspace.py"),
                               "--task", "W01", *args], cwd=self.source,
                              capture_output=True, text=True)

    def test_worktree_is_pinned_and_source_is_untouched(self):
        (self.source / "baseline.txt").write_text("uncommitted\n", encoding="utf-8")
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        task = self.root / "task"
        self.assertEqual(self.run_git("rev-parse", "HEAD", cwd=task), self.start)
        self.assertEqual(self.run_git("branch", "--show-current", cwd=task), "codex/w01-test")
        self.assertEqual((task / "baseline.txt").read_text(), "baseline\n")
        self.assertEqual((self.source / "baseline.txt").read_text(), "uncommitted\n")
        self.assertEqual(self.run_git("branch", "--show-current", cwd=self.source), REF)

    def test_clone_has_independent_git_metadata(self):
        result = self.invoke("--mode", "clone")
        self.assertEqual(result.returncode, 0, result.stderr)
        task = self.root / "task"
        self.assertTrue((task / ".git").is_dir())
        self.assertEqual(self.run_git("rev-parse", "HEAD", cwd=task), self.start)
        self.assertEqual(self.run_git("remote", "get-url", "origin", cwd=task), str(self.remote))

    def test_existing_directory_is_never_overwritten(self):
        destination = self.root / "task"
        destination.mkdir()
        marker = destination / "keep.txt"
        marker.write_text("keep", encoding="utf-8")
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(marker.read_text(), "keep")

    def test_dry_run_creates_nothing(self):
        result = self.invoke("--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.root / "task").exists())
        self.assertEqual(self.run_git("branch", "--show-current", cwd=self.source), REF)

    def test_remote_task_branch_prevents_duplicate_start(self):
        self.run_git("push", "origin", "HEAD:refs/heads/codex/w01-test", cwd=self.source)
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("already exists on origin", result.stderr)
        self.assertFalse((self.root / "task").exists())


if __name__ == "__main__":
    unittest.main()
