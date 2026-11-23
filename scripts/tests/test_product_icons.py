"""Native icon, development bundle and real desktop-launcher regression checks."""
import json
import ctypes
import ctypes.util
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

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
            base = Path(temporary).resolve()
            root = base / '中文 portable directory'
            data = base / 'isolated data'
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
                self.assertIn(f'Icon={data}/icons/hicolor/scalable/apps/{product}.svg\n', entry.read_text())
                self.assertEqual((data / 'icons/hicolor/scalable/apps' / (product + '.svg')).read_bytes(),
                                 (icons.BRAND / (product + '.svg')).read_bytes())

    @unittest.skipUnless(sys.platform == 'linux' and ctypes.util.find_library('gio-2.0'), 'requires Linux GIO')
    def test_gio_reads_an_absolute_file_icon_without_an_icon_theme_cache(self):
        gio = ctypes.CDLL(ctypes.util.find_library('gio-2.0'))
        for name in ('g_desktop_app_info_new_from_filename', 'g_app_info_get_icon', 'g_file_icon_get_file'):
            function = getattr(gio, name)
            function.argtypes = [ctypes.c_char_p if name.endswith('_filename') else ctypes.c_void_p]
            function.restype = ctypes.c_void_p
        gio.g_file_get_path.argtypes = [ctypes.c_void_p]
        gio.g_file_get_path.restype = ctypes.c_void_p
        gio.g_object_unref.argtypes = [ctypes.c_void_p]
        gio.g_free.argtypes = [ctypes.c_void_p]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / 'app'
            binary.write_text('#!/bin/sh\nexit 0\n')
            binary.chmod(0o755)
            data = root / '中文 " $HOME `test` %f back\\slash'
            for product in desktop.PRODUCTS:
                with self.subTest(product=product):
                    entry = desktop.install(product, binary, icons.BRAND / (product + '.svg'), data)
                    app = gio.g_desktop_app_info_new_from_filename(os.fsencode(entry))
                    self.assertTrue(app)
                    path = None
                    try:
                        file = gio.g_file_icon_get_file(gio.g_app_info_get_icon(app))
                        self.assertTrue(file, 'GIO must use a file icon, not theme-name lookup')
                        path = gio.g_file_get_path(file)
                        self.assertEqual(Path(os.fsdecode(ctypes.string_at(path))),
                                         data / f'icons/hicolor/scalable/apps/{product}.svg')
                    finally:
                        if path:
                            gio.g_free(path)
                        gio.g_object_unref(app)

    def test_refresh_repairs_only_the_main_icon_preserving_custom_launch_commands(self):
        with tempfile.TemporaryDirectory() as temporary:
            data = Path(temporary).resolve()
            for product in desktop.PRODUCTS:
                entry = data / 'applications' / (product + '.desktop')
                entry.parent.mkdir(exist_ok=True)
                original = (f'[Desktop Entry]\r\n# keep custom runtime wrapper\r\nIcon={product}\r\n'
                            f'Exec=/custom/glibc-wrapper {product} %f\r\nName[zh_CN]=测试\r\n'
                            '[Desktop Action New]\r\nIcon=other\r\nExec=other --new\r\n')
                entry.write_bytes(original.encode())
                desktop.refresh_icon(product, icons.BRAND / (product + '.svg'), data)
                expected = original.replace(f'Icon={product}\r\n',
                                            f'Icon={data}/icons/hicolor/scalable/apps/{product}.svg\r\n')
                self.assertEqual(entry.read_bytes(), expected.encode())
                entry.write_bytes(original.replace(f'Icon={product}', f'  Icon = {product}').encode())
                desktop.refresh_icon(product, icons.BRAND / (product + '.svg'), data)
                self.assertEqual(entry.read_bytes(), expected.encode())

    def test_system_launcher_references_the_installed_icon_not_the_staging_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            entry, _ = icons.linux_launcher(Path(temporary) / 'debian/usr', 'joysheet', system_install=True)
            self.assertIn('Icon=/usr/share/icons/hicolor/scalable/apps/joysheet.svg\n', entry.read_text())
            self.assertNotIn(temporary, entry.read_text())

    def test_refresh_adds_a_missing_icon_and_rejects_invalid_desktop_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            data = Path(temporary)
            entry = data / 'applications/joyword.desktop'
            entry.parent.mkdir()
            entry.write_bytes(b'[Desktop Entry]')
            desktop.refresh_icon('joyword', icons.BRAND / 'joyword.svg', data)
            self.assertTrue(entry.read_text().startswith('[Desktop Entry]\nIcon=/'))
            entry.write_bytes(b'[Desktop Action New]\nIcon=other\n')
            with self.assertRaises(ValueError):
                desktop.refresh_icon('joyword', icons.BRAND / 'joyword.svg', data)
            self.assertEqual(entry.read_bytes(), b'[Desktop Action New]\nIcon=other\n')

    def test_desktop_reload_is_optional_and_does_not_start_a_service(self):
        with patch.object(desktop.shutil, 'which', return_value='/usr/bin/busctl'), \
                patch.object(desktop.subprocess, 'run', side_effect=subprocess.TimeoutExpired('busctl', 3)) as run:
            desktop.notify_desktop()
        self.assertIn('--auto-start=no', run.call_args.args[0])
        self.assertEqual(run.call_args.kwargs['timeout'], 3)

    @unittest.skipUnless(shutil.which('gio') and sys.platform == 'linux', 'requires Linux GIO')
    def test_desktop_exec_round_trips_reserved_characters_through_gio(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            # None of these characters may become a shell expansion or field code.
            binary = root / '中文 " $HOME `test` %f back\\slash'
            wrapper = root / 'shared-wrapper'
            marker = root / 'argv.json'
            wrapper.write_text(f'#!{sys.executable}\nimport json,sys\n'
                              f'assert sys.argv[0] == {str(binary)!r}\n'
                              f'from pathlib import Path\nPath({str(marker)!r}).write_text(json.dumps(sys.argv[1:]))\n')
            wrapper.chmod(0o755)
            binary.symlink_to(wrapper)
            entry = desktop.install('joyword', binary, icons.BRAND / 'joyword.svg', root / 'data')
            document = root / 'sample document.docx'
            document.touch()
            launch_help = subprocess.run(['gio', 'help', 'launch'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if launch_help.returncode == 0:
                command = ['gio', 'launch', entry, document]
            elif shutil.which('gtk-launch'):
                command = ['gtk-launch', 'joyword', document]
            else:
                self.skipTest('requires GIO desktop launch support')
            environment = dict(os.environ, XDG_DATA_HOME=str(root / 'data'))
            result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=environment)
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
