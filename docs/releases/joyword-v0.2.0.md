# JoyWord v0.2.0

## English

**Shenzhen welcomes you as we celebrate APEC.**

JoyWord is a document editor in JoyOffice. Packages include the desktop application and `joyword-cli`.

### Release policy

Starting with v0.2.0, JoyOffice publishes normal GitHub releases. All first-party workspace packages share version 0.2.0; products retain separate tags and downloads.

### What’s new

- The template gallery offers Chinese and English content and thumbnails, with examples for project plans, meeting minutes, weekly reports, announcements and study notes.
- Page redraws retain the current image while changed pages render. Ribbon updates cover editing, review and page layout, and the save-changes dialog uses the shared centered layout.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-x86_64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-x86_64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-x86_64-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-x86_64.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-aarch64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-aarch64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-aarch64-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-aarch64.zip.sha256) |
| macOS Apple Silicon | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-macos-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-macos-aarch64.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-macos-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-macos-x86_64.zip.sha256) |
| Linux x64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-linux-x86_64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-linux-x86_64.tar.gz.sha256) |
| Linux ARM64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-linux-aarch64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-linux-aarch64.tar.gz.sha256) |

- **Windows** Run the per-user Setup EXE, or extract the ZIP. Installers include app-local Visual C++ runtime DLLs; Word/Deck portable ZIPs require a system-provided Visual C++ v14 runtime.
- **macOS** Extract the ZIP and open or move the included `.app` to Applications. JoySheet also offers a DMG.
- **Linux** Extract the archive and run the app in a desktop session. JoySheet also offers a DEB for Ubuntu 24.04.

Linux packages use Ubuntu 24.04 and need compatible Vulkan, font and X11/Wayland libraries. Packages remain unsigned and macOS notarization is pending. Each download has a SHA-256 sidecar.

### Capabilities and validation

See the [current product status](https://github.com/willmove/joyoffice/blob/joyword-v0.2.0/docs/joyword/r8/STATUS.md) for implementation changes and remaining work. Publishing as a normal release does not extend the recorded hardware or feature acceptance. Prior [v0.1.4 Windows evidence](https://github.com/willmove/joyoffice/blob/joyword-v0.2.0/docs/joyoffice/validation/20261113-v014/README.md) remains evidence for that version. Six-platform build and asset checks run before publication; full system IME, accessibility, printing, multi-monitor and upgrade/rollback coverage remains scenario-specific.

On UOS 25 x86_64 with X11/DDE, the tested JoyWord build accepted ordinary saves, while native Save As did not produce a file. System Pinyin composition is still open. [UOS desktop validation](https://github.com/willmove/joyoffice/blob/joyword-v0.2.0/docs/joyoffice/validation/20261117-uos-desktop/README.md)

## 中文

**喜迎APEC，深圳欢迎您**

JoyWord 是 JoyOffice 的文字处理应用。软件包包含桌面应用和 `joyword-cli`。

### 发布规则

从 v0.2.0 开始，JoyOffice 按正常 GitHub Release 发布。所有第一方工作区包统一使用 0.2.0，产品继续保留独立标签和下载。

### 本版更新

- 模板库按界面语言展示中英文正文及缩略图，包含项目计划书、会议纪要、工作周报、通知公告和学习笔记。
- 编辑时保留当前页面图像，待有变化的页面完成重绘后再替换。更新编辑、审阅和页面布局功能区，未保存更改使用居中对话框。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-x86_64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-x86_64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-x86_64-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-x86_64.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-aarch64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-aarch64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-aarch64-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-windows-aarch64.zip.sha256) |
| macOS Apple 芯片 | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-macos-aarch64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-macos-aarch64.zip.sha256) |
| macOS Intel | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-macos-x86_64.zip) | [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-macos-x86_64.zip.sha256) |
| Linux x64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-linux-x86_64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-linux-x86_64.tar.gz.sha256) |
| Linux ARM64 | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-linux-aarch64.tar.gz) | [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.2.0/joyword-0.2.0-linux-aarch64.tar.gz.sha256) |

- **Windows** 运行当前用户级 Setup EXE，或解压 ZIP。安装器携带 app-local Visual C++ 运行库；Word／Deck 便携 ZIP 需系统提供 Visual C++ v14 运行库。
- **macOS** 解压 ZIP 后打开随附 `.app`，或移入「应用程序」。JoySheet 另提供 DMG。
- **Linux** 解压后在图形会话中启动。JoySheet 另提供面向 Ubuntu 24.04 的 DEB。

Linux 构建基线为 Ubuntu 24.04，需要兼容的 Vulkan、字体和 X11／Wayland 系统库。软件包仍未作发行者签名，macOS 公证待完成。每项下载均附 SHA-256 校验文件。

### 能力与验证范围

实现进展与剩余工作见[当前产品状态](https://github.com/willmove/joyoffice/blob/joyword-v0.2.0/docs/joyword/r8/STATUS.md)。正常 Release 的发布类型不扩大已有硬件或功能验收范围。[v0.1.4 Windows 历史证据](https://github.com/willmove/joyoffice/blob/joyword-v0.2.0/docs/joyoffice/validation/20261113-v014/README.md)仍仅对应当时版本。发布前执行六平台构建和资产校验；完整系统输入法、无障碍、打印、多显示器及升级回退继续按场景验收。

UOS 25 x86_64 的 X11／DDE 实测中，普通保存通过，原生「另存为」未生成目标文件，系统拼音组合输入仍待验证通过。 [UOS 桌面实测记录](https://github.com/willmove/joyoffice/blob/joyword-v0.2.0/docs/joyoffice/validation/20261117-uos-desktop/README.md)
