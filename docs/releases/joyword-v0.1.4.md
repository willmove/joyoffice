# JoyWord v0.1.4 · Preview / 预览版

## English

JoyWord is a paginated word processor. The archives include JoyWord and `joyword-cli`.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-x86_64.zip.sha256) |
| Windows ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-aarch64.zip.sha256) |
| macOS Apple Silicon | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-aarch64.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-x86_64.zip.sha256) |
| Linux x64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-x86_64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-x86_64.tar.gz.sha256) |
| Linux ARM64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-aarch64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-aarch64.tar.gz.sha256) |

- **Windows** Extract the ZIP and run `joyword.exe`. Keep its companion files together.
- **macOS** Extract the ZIP and open `JoyWord.app`, or drag it to Applications.
- **Linux** Extract the archive and run `./joyword` in a desktop session.

Linux packages are built on Ubuntu 24.04 and require compatible Vulkan, font and X11/Wayland system libraries. The previews ship without publisher signatures or macOS notarization. Compare checksums with `Get-FileHash -Algorithm SHA256 filename` on Windows or run `shasum -a 256 -c filename.sha256` on macOS and Linux.

### What’s new

- The Windows ribbon follows Word more closely, with revised Home, Insert, Review and clipboard controls and working command menus.
- Shared light/dark theme presets and double-click ribbon collapse provide more room for documents.
- Pagination regression fixtures now select the exact bundled font instead of depending on the installed default font.

See the [product status](https://github.com/willmove/joyoffice/blob/joyword-v0.1.4/docs/joyword/r8/STATUS.md) for format coverage and platform validation. Find the other apps in the [JoyOffice download table](https://github.com/willmove/joyoffice/blob/joyword-v0.1.4/README.md#download-and-install).

### Validation scope

Windows x64 native desktop checks are recorded in the [release validation](https://github.com/willmove/joyoffice/blob/joyword-v0.1.4/docs/joyoffice/validation/20261113-v014/README.md). The six-platform CI packages do not establish manual acceptance on other hardware or operating systems. Physical IME, screen-reader, printer and multi-monitor coverage remains limited.

## 中文

JoyWord 是分页文字处理软件。压缩包内含 JoyWord 和 `joyword-cli`。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-x86_64.zip.sha256) |
| Windows ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-windows-aarch64.zip.sha256) |
| macOS Apple 芯片 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-aarch64.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-macos-x86_64.zip.sha256) |
| Linux x64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-x86_64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-x86_64.tar.gz.sha256) |
| Linux ARM64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-aarch64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.4/joyword-0.1.4-linux-aarch64.tar.gz.sha256) |

- **Windows** 解压 ZIP 后运行 `joyword.exe`，保留同目录的配套文件。
- **macOS** 解压 ZIP 后打开 `JoyWord.app`，也可将其拖入「应用程序」。
- **Linux** 解压后在图形会话中运行 `./joyword`。

Linux 构建基线为 Ubuntu 24.04，运行需要兼容的 Vulkan、字体和 X11／Wayland 系统库。预览包尚无发行者签名或 macOS 公证。Windows 可用 `Get-FileHash -Algorithm SHA256 文件名` 对照校验值，macOS 和 Linux 可运行 `shasum -a 256 -c 文件名.sha256`。

### 本版进展

- 按 Windows Word 调整开始、插入、审阅及剪贴板功能区，补齐对应命令菜单。
- 新增三产品共享的亮暗主题预设，双击当前标签可收起或展开功能区。
- 分页回归固定使用随仓库提供的字体，避免已安装默认字体改变测试边界。

格式覆盖与逐平台验收范围见[产品状态](https://github.com/willmove/joyoffice/blob/joyword-v0.1.4/docs/joyword/r8/STATUS.md)。其他产品可从 [JoyOffice 下载表](https://github.com/willmove/joyoffice/blob/joyword-v0.1.4/README.md#下载与安装)获取。


### 验证范围

Windows x64 本机验证范围见[发布验证](https://github.com/willmove/joyoffice/blob/joyword-v0.1.4/docs/joyoffice/validation/20261113-v014/README.md)。六个平台的 CI 构建不代替其它硬件或系统的人工验收；真实输入法、读屏、打印和多屏仍有未覆盖项。
