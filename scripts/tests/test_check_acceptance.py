"""Positive and negative fixtures for scripts/check_acceptance.py."""
import sys
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_acceptance  # noqa: E402

QUOTE_AUTO = "样式解析、编号重启、批量替换原子性和 Word 往返测试。"
QUOTE_HUMAN = "制作多级标题与列表，格式刷、查找替换和撤销。"
LEGEND = " ".join(check_acceptance.LEGEND)


def _write(root, relative, text):
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(textwrap.dedent(text).lstrip("\n"), encoding="utf-8")
    return path


def _fixture(root, evidence="`real_test_name`", status="automated", quote=True, link=True):
    _write(root, "crates/demo.rs", "fn real_test_name() {}\n")
    _write(root, "docs/sample.txt", "sample\n")
    _write(
        root,
        "docs/joyword/TASKS.md",
        f"""\
        ### T200：示例

        - 自动验证：{QUOTE_AUTO}
        - 人工验收：{QUOTE_HUMAN}
        """,
    )
    _write(root, "docs/joyword/acceptance/README.md", f"Legend: {LEGEND}\n")
    cited = ""
    if quote:
        cited = f"> 自动验证：{QUOTE_AUTO}\n> 人工验收：{QUOTE_HUMAN}\n"
    linked = "[sample](../../sample.txt)" if link else "[missing](../../missing.txt)"
    _write(
        root,
        "docs/joyword/acceptance/T200.md",
        f"""\
        # T200

        {cited}

        | 准则 | 状态 | 证据 | 备注 |
        | --- | --- | --- | --- |
        | 样式解析 | {status} | {evidence} | {linked} |

        ## GUI 抽查脚本

        1. 打开样例。
        """,
    )


class CheckAcceptanceTest(unittest.TestCase):
    def test_valid_fixture_passes(self):
        with self._temp() as root:
            _fixture(root)
            self.assertEqual(check_acceptance.problems(root), [])

    def test_missing_test_name_fails(self):
        with self._temp() as root:
            _fixture(root, evidence="`missing_test_name`")
            found = check_acceptance.problems(root)
            self.assertTrue(any("missing_test_name" in item for item in found))

    def test_note_column_does_not_require_a_test_name(self):
        with self._temp() as root:
            _fixture(root)
            path = root / "docs/joyword/acceptance/T200.md"
            text = path.read_text(encoding="utf-8").replace(
                "[sample](../../sample.txt)",
                "`not_checked_because_it_is_a_note`",
            )
            path.write_text(text, encoding="utf-8")
            self.assertEqual(check_acceptance.problems(root), [])

    def test_identifier_without_underscore_is_not_a_test_name(self):
        with self._temp() as root:
            _fixture(root, evidence="`build`")
            self.assertEqual(check_acceptance.problems(root), [])

    def test_bad_status_fails(self):
        with self._temp() as root:
            _fixture(root, status="done")
            found = check_acceptance.problems(root)
            self.assertTrue(any("done" in item for item in found))

    def test_missing_link_fails(self):
        with self._temp() as root:
            _fixture(root, link=False)
            found = check_acceptance.problems(root)
            self.assertTrue(any("missing linked file" in item for item in found))

    def test_missing_quoted_criterion_fails(self):
        with self._temp() as root:
            _fixture(root, quote=False)
            found = check_acceptance.problems(root)
            self.assertTrue(any("自动验证" in item for item in found))

    def test_legend_must_list_every_status(self):
        with self._temp() as root:
            _fixture(root)
            readme = root / "docs/joyword/acceptance/README.md"
            readme.write_text("Legend: automated partial\n", encoding="utf-8")
            found = check_acceptance.problems(root)
            self.assertTrue(any("needs-gui" in item for item in found))

    def test_real_repository_acceptance_docs(self):
        found = check_acceptance.problems(ROOT)
        self.assertEqual(found, [], "\n".join(found))

    def _temp(self):
        return tempfile_cm()


class tempfile_cm:
    def __enter__(self):
        import tempfile

        self._tmp = tempfile.TemporaryDirectory()
        return Path(self._tmp.name)

    def __exit__(self, *args):
        self._tmp.cleanup()


if __name__ == "__main__":
    unittest.main()
