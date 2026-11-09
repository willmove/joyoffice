# JoySheet v0.1.2 · Preview / 预览版

## English

This is the first public JoySheet preview in JoyOffice. Each installer and portable archive includes the spreadsheet editor and `joysheet-cli`.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-x86_64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-aarch64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-aarch64-release.zip.sha256) |
| macOS Apple Silicon | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-aarch64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-aarch64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-aarch64-release.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-x86_64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-x86_64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-x86_64-release.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-x86_64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-x86_64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-x86_64-release.deb.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-aarch64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-aarch64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-aarch64-release.deb.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-aarch64-release.zip.sha256) |

- **Windows** Run the Setup EXE, or extract the ZIP and run `joysheet.exe`.
- **macOS** Mount the DMG or extract the ZIP and drag `Joysheet.app` to Applications. Requires macOS 15 or later.
- **Linux** On Ubuntu 24.04, run `sudo apt install ./filename.deb`. You can also extract the ZIP and run `./joysheet` in a desktop session. PDF preview uses Poppler.

Linux packages are built on Ubuntu 24.04 and require compatible Vulkan, font and X11/Wayland system libraries. The previews ship without publisher signatures or macOS notarization. Compare checksums with `Get-FileHash -Algorithm SHA256 filename` on Windows or run `shasum -a 256 -c filename.sha256` on macOS and Linux.

### What’s new

- Edit XLSX workbooks with a virtualized grid, formulas, tables, filters, conditional formatting, charts and scoped analysis tools.
- Open password-protected XLSX with a retryable prompt. Save As writes a separate unencrypted copy and protects the source.
- Windows testing covered copying integers, decimals and text both ways with Excel and checking the values saved to XLSX.

See the [product status](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.2/docs/joysheet/STATUS.md) for format coverage and platform validation. Find the other apps in the [JoyOffice download table](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.2/README.md#download-and-install).

## 中文

这是 JoySheet 在 JoyOffice 中首次公开的预览版。安装器与便携包均包含电子表格应用和 `joysheet-cli`。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-x86_64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-aarch64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-windows-aarch64-release.zip.sha256) |
| macOS Apple 芯片 | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-aarch64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-aarch64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-aarch64-release.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-x86_64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-x86_64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-macos-x86_64-release.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-x86_64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-x86_64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-x86_64-release.deb.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-aarch64-release.deb) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-aarch64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-aarch64-release.deb.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.1.2/joysheet-0.1.2-linux-aarch64-release.zip.sha256) |

- **Windows** 运行 Setup EXE，或解压 ZIP 后运行 `joysheet.exe`。
- **macOS** 挂载 DMG 或解压 ZIP，将 `Joysheet.app` 拖入「应用程序」。需要 macOS 15 或更新版本。
- **Linux** 在 Ubuntu 24.04 上可用 `sudo apt install ./文件名.deb` 安装。也可解压 ZIP 并在桌面会话中运行 `./joysheet`。PDF 预览需要 Poppler。

Linux 构建基线为 Ubuntu 24.04，运行需要兼容的 Vulkan、字体和 X11／Wayland 系统库。预览包尚无发行者签名或 macOS 公证。Windows 可用 `Get-FileHash -Algorithm SHA256 文件名` 对照校验值，macOS 和 Linux 可运行 `shasum -a 256 -c 文件名.sha256`。

### 本版进展

- 支持 XLSX 编辑，提供虚拟化网格、公式、表格、筛选、条件格式、图表和部分数据分析工具。
- 打开受密码保护的 XLSX 时可重试密码。「另存为」写出独立的未加密副本，保留源文件。
- Windows 已验证与 Excel 双向复制整数、小数和文本，并核对保存到 XLSX 的值。

格式覆盖与逐平台验收范围见[产品状态](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.2/docs/joysheet/STATUS.md)。其他产品可从 [JoyOffice 下载表](https://github.com/willmove/joyoffice/blob/joysheet-v0.1.2/README.md#下载与安装)获取。
