"""Pixel QA must establish foreground conditions rather than relax tolerances."""
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import foreground


class ForegroundPrecondition(unittest.TestCase):
    def test_activation_requires_observed_foreground_process(self):
        with patch.object(foreground.subprocess, 'check_output', side_effect=[
                '{"activated":true,"frontmostPid":99}',
                '{"activated":true,"frontmostPid":123}']) as process, \
                patch.object(foreground.time, 'sleep'):
            report = foreground.focus_window(123)
        self.assertEqual(report['frontmostPid'], 123)
        self.assertTrue(report['verified'])
        self.assertEqual(report['attempts'], 2)
        self.assertEqual(process.call_count, 2)

    def test_inactive_window_cannot_be_accepted(self):
        with patch.object(foreground.subprocess, 'check_output', return_value=
                          '{"activated":false,"frontmostPid":99}'), \
                patch.object(foreground.time, 'monotonic', side_effect=[0.0, 0.1, 4.0]), \
                patch.object(foreground.time, 'sleep'):
            with self.assertRaisesRegex(RuntimeError, 'did not become foreground'):
                foreground.focus_window(123)

    def test_invalid_process_is_rejected_before_os_call(self):
        with patch.object(foreground.subprocess, 'check_output') as process:
            for pid in [0, -1, '123', True]:
                with self.assertRaises(ValueError):
                    foreground.focus_window(pid)
        process.assert_not_called()


if __name__ == '__main__':
    unittest.main()
