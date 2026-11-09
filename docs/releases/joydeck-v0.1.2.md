# JoyDeck v0.1.2 · Preview / 预览版

## English

JoyDeck is a presentation editor. This preview includes the R9 desktop application and `joydeck-cli`.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-windows-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-windows-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-windows-aarch64-release.zip.sha256) |
| macOS Apple Silicon | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-macos-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-macos-aarch64-release.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-macos-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-macos-x86_64-release.zip.sha256) |
| Linux x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-linux-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-linux-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-linux-aarch64-release.zip.sha256) |

- **Windows** Extract the ZIP and run `joydeck.exe`. Keep its companion files together.
- **macOS** Extract the ZIP and open `JoyDeck.app`, or drag it to Applications.
- **Linux** Extract the archive and run `./joydeck` in a desktop session.

Linux packages are built on Ubuntu 24.04 and require compatible Vulkan, font and X11/Wayland system libraries. The previews ship without publisher signatures or macOS notarization. Compare checksums with `Get-FileHash -Algorithm SHA256 filename` on Windows or run `shasum -a 256 -c filename.sha256` on macOS and Linux.

### What’s new

- Edit PPTX and native `.joydeck` v2 documents, use slide and presenter views, and export PDF, images and other supported formats.
- Fill, update or clear slide footer text and undo the changes. Windows testing covered rich text pasted from Word and reopening a saved PPTX in PowerPoint.
- PPTX input supports files up to 500 MiB within the existing ZIP and XML budgets. The original v1 file is preserved when explicitly migrating a legacy `.joydeck` document.

See the [product status](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.2/docs/joydeck/STATUS.md) for format coverage and platform validation. Find the other apps in the [JoyOffice download table](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.2/README.md#download-and-install).

## 中文

JoyDeck 是演示文稿编辑器。本预览包包含 R9 桌面应用和 `joydeck-cli`。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-windows-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-windows-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-windows-aarch64-release.zip.sha256) |
| macOS Apple 芯片 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-macos-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-macos-aarch64-release.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-macos-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-macos-x86_64-release.zip.sha256) |
| Linux x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-linux-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-linux-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.2/joydeck-0.1.2-linux-aarch64-release.zip.sha256) |

- **Windows** 解压 ZIP 后运行 `joydeck.exe`，保留同目录的配套文件。
- **macOS** 解压 ZIP 后打开 `JoyDeck.app`，也可将其拖入「应用程序」。
- **Linux** 解压后在图形会话中运行 `./joydeck`。

Linux 构建基线为 Ubuntu 24.04，运行需要兼容的 Vulkan、字体和 X11／Wayland 系统库。预览包尚无发行者签名或 macOS 公证。Windows 可用 `Get-FileHash -Algorithm SHA256 文件名` 对照校验值，macOS 和 Linux 可运行 `shasum -a 256 -c 文件名.sha256`。

### 本版进展

- 编辑 PPTX 和原生 `.joydeck` v2 文稿，使用放映与演示者视图，导出 PDF、图片等受支持格式。
- 补建、更新或清空页脚文字，可撤销。Windows 已验证从 Word 粘贴富文本，并在 PowerPoint 打开另存的 PPTX。
- PPTX 文件读取上限提高至 500 MiB，仍受 ZIP 和 XML 安全预算约束。旧 `.joydeck` v1 文稿可显式迁移并保留原件。

格式覆盖与逐平台验收范围见[产品状态](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.2/docs/joydeck/STATUS.md)。其他产品可从 [JoyOffice 下载表](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.2/README.md#下载与安装)获取。
