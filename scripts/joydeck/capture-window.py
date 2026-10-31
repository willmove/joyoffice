"""Capture only the visible window of a supplied local acceptance-process PID."""
import ctypes
from ctypes import wintypes
from pathlib import Path
import sys

ctypes.windll.user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
from PIL import ImageGrab

pid = int(sys.argv[1])
output = Path(sys.argv[2]).resolve()
user32 = ctypes.windll.user32
windows = []
@ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)
def inspect(hwnd, _):
    owner = wintypes.DWORD()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
    title = ctypes.create_unicode_buffer(512)
    user32.GetWindowTextW(hwnd, title, 512)
    if owner.value == pid and user32.IsWindowVisible(hwnd) and title.value.startswith('JoyDeck —'):
        windows.append(hwnd)
    return True
user32.EnumWindows(inspect, 0)
if len(windows) != 1:
    raise RuntimeError(f'Expected one visible acceptance window; found {len(windows)}')
rect = wintypes.RECT()
ctypes.windll.dwmapi.DwmGetWindowAttribute(windows[0], 9, ctypes.byref(rect), ctypes.sizeof(rect))
if rect.right <= rect.left or rect.bottom <= rect.top:
    raise RuntimeError('Window bounds unavailable')
ImageGrab.grab(bbox=(rect.left, rect.top, rect.right, rect.bottom), all_screens=True).save(output)
print(output)
