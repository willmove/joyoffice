# JoyDeck v0.1.4 · Preview / 预览版

## English

JoyDeck is a presentation editor. This preview includes the R9 desktop application and `joydeck-cli`.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-aarch64-release.zip.sha256) |
| macOS Apple Silicon | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-aarch64-release.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-x86_64-release.zip.sha256) |
| Linux x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-aarch64-release.zip.sha256) |

- **Windows** Extract the ZIP and run `joydeck.exe`. Keep its companion files together.
- **macOS** Extract the ZIP and open `JoyDeck.app`, or drag it to Applications.
- **Linux** Extract the archive and run `./joydeck` in a desktop session.

Linux packages are built on Ubuntu 24.04 and require compatible Vulkan, font and X11/Wayland system libraries. The previews ship without publisher signatures or macOS notarization. Compare checksums with `Get-FileHash -Algorithm SHA256 filename` on Windows or run `shasum -a 256 -c filename.sha256` on macOS and Linux.

### What’s new

- The ribbon follows the observed PowerPoint grouping, and the File backstage now offers a clearer home and recent-file workflow.
- Shared theme presets, double-click ribbon collapse and Up/Down slide navigation work in the editor and slide show.
- Slide-show frames use the window pixel size and DPI, with cache invalidation and background rendering improvements to avoid enlarged blurry frames.

See the [product status](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.4/docs/joydeck/STATUS.md) for format coverage and platform validation. Find the other apps in the [JoyOffice download table](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.4/README.md#download-and-install).

### Known format limit

Adding notes to an imported PPTX with no reusable notes part is refused without changing the source; save as native `.joydeck` instead. Native save/reopen was verified in this release.

### Validation scope

Windows x64 native desktop checks are recorded in the [release validation](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.4/docs/joyoffice/validation/20261113-v014/README.md). The six-platform CI packages do not establish manual acceptance on other hardware or operating systems. Physical IME, screen-reader, printer and multi-monitor coverage remains limited.

## 中文

JoyDeck 是演示文稿编辑器。本预览包包含 R9 桌面应用和 `joydeck-cli`。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-windows-aarch64-release.zip.sha256) |
| macOS Apple 芯片 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-aarch64-release.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-macos-x86_64-release.zip.sha256) |
| Linux x64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-x86_64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-aarch64-release.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.1.4/joydeck-0.1.4-linux-aarch64-release.zip.sha256) |

- **Windows** 解压 ZIP 后运行 `joydeck.exe`，保留同目录的配套文件。
- **macOS** 解压 ZIP 后打开 `JoyDeck.app`，也可将其拖入「应用程序」。
- **Linux** 解压后在图形会话中运行 `./joydeck`。

Linux 构建基线为 Ubuntu 24.04，运行需要兼容的 Vulkan、字体和 X11／Wayland 系统库。预览包尚无发行者签名或 macOS 公证。Windows 可用 `Get-FileHash -Algorithm SHA256 文件名` 对照校验值，macOS 和 Linux 可运行 `shasum -a 256 -c 文件名.sha256`。

### 本版进展

- 按实测 PowerPoint 分组重排功能区，恢复文件标签并重做首页与最近文稿流程。
- 新增共享主题、双击收展功能区，以及编辑和放映中的上下键翻页。
- 放映画面按窗口实际像素与 DPI 渲染，完善缓存失效与后台渲染，修复低分辨率底图放大导致的模糊。

格式覆盖与逐平台验收范围见[产品状态](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.4/docs/joydeck/STATUS.md)。其他产品可从 [JoyOffice 下载表](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.4/README.md#下载与安装)获取。


### 验证范围

Windows x64 本机验证范围见[发布验证](https://github.com/willmove/joyoffice/blob/joydeck-v0.1.4/docs/joyoffice/validation/20261113-v014/README.md)。六个平台的 CI 构建不代替其它硬件或系统的人工验收；真实输入法、读屏、打印和多屏仍有未覆盖项。


### 已知格式边界

导入的 PPTX 若没有可复用的备注部件，新增备注后会拒绝回写并保留原件，应另存 `.joydeck` 原生格式。本轮已验证原生保存与读回。
