# JoyWord v0.1.3 · Preview / 预览版

## English

JoyWord is a paginated word processor. The archives include JoyWord and `joyword-cli`.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-windows-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-windows-x86_64.zip.sha256) |
| Windows ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-windows-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-windows-aarch64.zip.sha256) |
| macOS Apple Silicon | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-macos-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-macos-aarch64.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-macos-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-macos-x86_64.zip.sha256) |
| Linux x64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-linux-x86_64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-linux-x86_64.tar.gz.sha256) |
| Linux ARM64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-linux-aarch64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-linux-aarch64.tar.gz.sha256) |

- **Windows** Extract the ZIP and run `joyword.exe`. Keep its companion files together.
- **macOS** Extract the ZIP and open `JoyWord.app`, or drag it to Applications.
- **Linux** Extract the archive and run `./joyword` in a desktop session.

Linux packages are built on Ubuntu 24.04 and require compatible Vulkan, font and X11/Wayland system libraries. The previews ship without publisher signatures or macOS notarization. Compare checksums with `Get-FileHash -Algorithm SHA256 filename` on Windows or run `shasum -a 256 -c filename.sha256` on macOS and Linux.

### What’s new

- The File workspace provides a dedicated Export page, including PDF, PNG, Word 2003 XML, Flat OPC, EPUB and PDF/A-2b.
- ODT, OTT and FODT exchange preserves more of the supported layout, including paragraph spacing, image anchors and table borders. Windows testing included round trips with LibreOffice Writer.
- Improved import of legacy DOC files, retained theme bindings and unknown XML in supported DOCX scenarios, and tested portable upgrade and rollback on Windows.

See the [product status](https://github.com/willmove/joyoffice/blob/joyword-v0.1.3/docs/joyword/r8/STATUS.md) for format coverage and platform validation. Find the other apps in the [JoyOffice download table](https://github.com/willmove/joyoffice/blob/joyword-v0.1.3/README.md#download-and-install).

## 中文

JoyWord 是分页文字处理软件。压缩包内含 JoyWord 和 `joyword-cli`。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-windows-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-windows-x86_64.zip.sha256) |
| Windows ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-windows-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-windows-aarch64.zip.sha256) |
| macOS Apple 芯片 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-macos-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-macos-aarch64.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-macos-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-macos-x86_64.zip.sha256) |
| Linux x64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-linux-x86_64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-linux-x86_64.tar.gz.sha256) |
| Linux ARM64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-linux-aarch64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.1.3/joyword-0.1.3-linux-aarch64.tar.gz.sha256) |

- **Windows** 解压 ZIP 后运行 `joyword.exe`，保留同目录的配套文件。
- **macOS** 解压 ZIP 后打开 `JoyWord.app`，也可将其拖入「应用程序」。
- **Linux** 解压后在图形会话中运行 `./joyword`。

Linux 构建基线为 Ubuntu 24.04，运行需要兼容的 Vulkan、字体和 X11／Wayland 系统库。预览包尚无发行者签名或 macOS 公证。Windows 可用 `Get-FileHash -Algorithm SHA256 文件名` 对照校验值，macOS 和 Linux 可运行 `shasum -a 256 -c 文件名.sha256`。

### 本版进展

- 「文件」工作区新增独立导出页，支持 PDF、PNG、Word 2003 XML、Flat OPC、EPUB 和 PDF/A-2b 等格式。
- ODT、OTT 和 FODT 读写改善了段落间距、图片锚点、表格边框等版式保留。Windows 已与 LibreOffice Writer 验证样例往返。
- 改善旧版 DOC 导入，并加强受支持 DOCX 的主题绑定与未知 XML 保留。Windows 已验证便携包换版和回退。

格式覆盖与逐平台验收范围见[产品状态](https://github.com/willmove/joyoffice/blob/joyword-v0.1.3/docs/joyword/r8/STATUS.md)。其他产品可从 [JoyOffice 下载表](https://github.com/willmove/joyoffice/blob/joyword-v0.1.3/README.md#下载与安装)获取。
