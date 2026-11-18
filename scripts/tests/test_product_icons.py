"""Native icon, development bundle and real desktop-launcher regression checks."""
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import product_icons as icons
import install_linux_desktop as desktop


class ProductIcons(unittest.TestCase):
    def test_development_bundles_use_the_selected_product_artwork_and_executable(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / 'build/desktop'
            binary.parent.mkdir()
            binary.write_bytes(b'development executable')
            binary.chmod(0o755)
            for product, (name, _) in desktop.PRODUCTS.items():
                with self.subTest(product=product):
                    executable = icons.development_bundle(binary, product, '0.2.0', root / 'dev')
                    self.assertEqual(executable, root / 'dev' / f'{name}.app/Contents/MacOS' / product)
                    self.assertEqual(executable.read_bytes(), binary.read_bytes())
                    self.assertTrue(os.access(executable, os.X_OK))
                    contents = executable.parents[1]
                    info = plistlib.loads((contents / 'Info.plist').read_bytes())
                    self.assertEqual(info['CFBundleExecutable'], product)
                    self.assertEqual(info['CFBundleIconFile'], product + '.icns')
                    self.assertEqual(info['CFBundleShortVersionString'], '0.2.0')
                    self.assertEqual(info['CFBundleDocumentTypes'][0]['CFBundleTypeRole'], 'Editor')
                    self.assertEqual((contents / 'Resources' / info['CFBundleIconFile']).read_bytes(),
                                     (icons.BRAND / (product + '.icns')).read_bytes())

    def test_portable_installer_is_self_contained_and_does_not_require_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / '中文 portable directory'
            data = Path(temporary) / 'isolated data'
            root.mkdir()
            for product in desktop.PRODUCTS:
                binary = root / product
                binary.write_text('#!/bin/sh\nexit 0\n', encoding='utf-8')
                binary.chmod(0o755)
                icons.copy_logo(root, product)
                icons.copy_linux_installer(root)
                subprocess.run([sys.executable, root / 'install-desktop.py', '--product', product],
                               cwd='/', env={**os.environ, 'XDG_DATA_HOME': str(data)}, check=True,
                               stdout=subprocess.PIPE)
                entry = data / 'applications' / (product + '.desktop')
                self.assertIn(f'Exec="{binary}" %f', entry.read_text())
                self.assertIn(f'StartupWMClass={product}', entry.read_text())
                self.assertEqual((data / 'icons/hicolor/scalable/apps' / (product + '.svg')).read_bytes(),
                                 (icons.BRAND / (product + '.svg')).read_bytes())

    @unittest.skipUnless(shutil.which('gio') and sys.platform == 'linux', 'requires Linux GIO')
    def test_desktop_exec_round_trips_reserved_characters_through_gio(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            # None of these characters may become a shell expansion or field code.
            binary = root / '中文 " $HOME `test` %f back\\slash'
            marker = root / 'argv.json'
            binary.write_text(f'#!{sys.executable}\nimport json,sys\n'
                              f'from pathlib import Path\nPath({str(marker)!r}).write_text(json.dumps(sys.argv[1:]))\n')
            binary.chmod(0o755)
            entry = desktop.install('joyword', binary, icons.BRAND / 'joyword.svg', root / 'data')
            document = root / 'sample document.docx'
            document.touch()
            result = subprocess.run(['gio', 'launch', entry, document], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            self.assertEqual(result.returncode, 0, result.stderr + entry.read_text())
            for _ in range(100):
                if marker.exists():
                    break
                time.sleep(.05)
            self.assertEqual(json.loads(marker.read_text()), [str(document)])

    def test_invalid_executable_does_not_write_desktop_resources(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / 'not-executable'
            binary.touch()
            with self.assertRaises(ValueError):
                desktop.install('joyword', binary, icons.BRAND / 'joyword.svg', root / 'data')
            self.assertFalse((root / 'data').exists())


if __name__ == '__main__':
    unittest.main()
