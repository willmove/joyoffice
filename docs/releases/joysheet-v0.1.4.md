# JoySheet v0.1.4 · Preview / 预览版

## English

JoySheet is the spreadsheet application in JoyOffice. Each installer and portable archive includes the spreadsheet editor and `joysheet-cli`.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release.zip.sha256) |
| macOS Apple Silicon | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.deb.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.deb.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.zip.sha256) |

- **Windows** Run the Setup EXE, or extract the ZIP and run `joysheet.exe`.
- **macOS** Mount the DMG or extract the ZIP and drag `Joysheet.app` to Applications. Requires macOS 15 or later.
- **Linux** On Ubuntu 24.04, run `sudo apt install ./filename.deb`. You can also extract the ZIP and run `./joysheet` in a desktop session. PDF preview uses Poppler.

Linux packages are built on Ubuntu 24.04 and require compatible Vulkan, font and X11/Wayland system libraries. The previews ship without publisher signatures or macOS notarization. Compare checksums with `Get-FileHash -Algorithm SHA256 filename` on Windows or run `shasum -a 256 -c filename.sha256` on macOS and Linux.

### What’s new

- The ribbon follows Windows Excel grouping with Drawing, Help, File backstage and thinner icons.
- Supported SmartArt, WordArt, calendar, embedded-file and equation workflows now include editable objects, page-layout cell/header/footer editing, draggable page breaks and formula trace arrows.
- Shared theme presets and double-click ribbon collapse retain an editable grid and keyboard focus navigation. Complex object and full Excel parity limits remain documented.

See the [product status](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.4/docs/joysheet/STATUS.md) for format coverage and platform validation. Find the other apps in the [JoyOffice download table](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.4/README.md#download-and-install).

### Validation scope

Windows x64 native desktop checks are recorded in the [release validation](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.4/docs/joyoffice/validation/20261113-v014/README.md). The six-platform CI packages do not establish manual acceptance on other hardware or operating systems. Physical IME, screen-reader, printer and multi-monitor coverage remains limited.

## 中文

JoySheet 是 JoyOffice 的电子表格应用。安装器与便携包均包含电子表格应用和 `joysheet-cli`。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-windows-aarch64-release.zip.sha256) |
| macOS Apple 芯片 | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-aarch64-release.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-macos-x86_64-release.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.deb.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.deb.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.4/joysheet-0.1.4-linux-aarch64-release.zip.sha256) |

- **Windows** 运行 Setup EXE，或解压 ZIP 后运行 `joysheet.exe`。
- **macOS** 挂载 DMG 或解压 ZIP，将 `Joysheet.app` 拖入「应用程序」。需要 macOS 15 或更新版本。
- **Linux** 在 Ubuntu 24.04 上可用 `sudo apt install ./文件名.deb` 安装。也可解压 ZIP 并在桌面会话中运行 `./joysheet`。PDF 预览需要 Poppler。

Linux 构建基线为 Ubuntu 24.04，运行需要兼容的 Vulkan、字体和 X11／Wayland 系统库。预览包尚无发行者签名或 macOS 公证。Windows 可用 `Get-FileHash -Algorithm SHA256 文件名` 对照校验值，macOS 和 Linux 可运行 `shasum -a 256 -c 文件名.sha256`。

### 本版进展

- 按 Windows Excel 分组重排功能区，新增绘图、帮助、文件后台并减细图标。
- 补齐受支持的 SmartArt、WordArt、日程表、嵌入文件和公式对象编辑子集，接入页面布局单元格/页眉页脚编辑、分页线拖动和公式追踪箭头。
- 新增共享主题和双击收展功能区，收起后仍可编辑网格并使用键盘焦点导航。复杂对象及完整 Excel 等价范围继续按文档跟踪。

格式覆盖与逐平台验收范围见[产品状态](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.4/docs/joysheet/STATUS.md)。其他产品可从 [JoyOffice 下载表](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.4/README.md#下载与安装)获取。


### 验证范围

Windows x64 本机验证范围见[发布验证](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.4/docs/joyoffice/validation/20261113-v014/README.md)。六个平台的 CI 构建不代替其它硬件或系统的人工验收；真实输入法、读屏、打印和多屏仍有未覆盖项。
