# JoyDeck v0.3.0

## English

JoyDeck is a presentation editor in JoyOffice. Packages include the desktop application and `joydeck-cli`.

### Release policy & package parity

All JoyOffice products (JoyWord, JoySheet, JoyDeck) share unified version **0.3.0** and now ship the exact same package set across all six platform/architecture targets:
- **Windows (x64, ARM64):** Setup EXE + ZIP
- **macOS (Apple Silicon, Intel):** DMG + ZIP
- **Linux (x64, ARM64):** DEB + TAR.GZ + ZIP

Linux x64 and ARM64 packages are now built on a **glibc 2.28** baseline (Debian 10 Buster) with xz-compressed `.deb` packages and portable runtime dependencies, ensuring compatibility with **UOS 20 (glibc 2.28)** and **UOS 25 (glibc 2.38)** across both x86_64 and ARM64 as well as Debian 10–13 and Ubuntu 20.04–24.04.

### What’s new

- **Aligned installers & UOS compatibility:** Added macOS DMG, Linux DEB (`amd64` / `arm64`), and Linux TAR.GZ packages alongside ZIP; lowered Linux ABI baseline from glibc 2.39 to glibc 2.28 with automated ELF symbol-version and Debian package gates.
- **First-party slide transitions & GPU compositing:** Replaced all 49 PresentationML slide transition kinds with first-party deterministic generators and bounded Metal/Vulkan slide-texture compositing (including prism, paper, cloth, ripple, glitter, and vortex transitions).
- **DrawingML preset shapes & connectors:** Independently implemented all 183 ECMA DrawingML preset shapes, connectors, flowcharts, callouts, and layered action buttons with guide-preserving geometry editing.
- **Desktop & UOS fixes:** Preserved slideshow keyboard shortcuts with Chinese IME active, fixed native save-panel target safety, UOS DDE desktop icons, X11 input-context initialization, and Linux software Vulkan fallback startup.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-x86_64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-aarch64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-aarch64-release.zip.sha256) |
| macOS Apple Silicon | [DMG](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-aarch64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-aarch64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-aarch64-release.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-x86_64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-x86_64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-x86_64-release.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.zip.sha256) |

- **Windows:** Run the per-user Setup EXE (bundles app-local Visual C++ runtime DLLs), or extract the ZIP.
- **macOS:** Mount the DMG and drag `JoyDeck.app` to Applications, or extract the ZIP.
- **Linux (UOS 20/25, Debian, Ubuntu):** Install the `.deb` with `sudo apt install ./joydeck-0.3.0-linux-<arch>-release.deb` (uses xz compression and `libc6 >= 2.28`), or extract the `.tar.gz`/`.zip` archive and run `python3 install-desktop.py --product joydeck`.

Packages remain unsigned and macOS notarization is pending. Each download includes a SHA-256 sidecar.

### Capabilities and validation

See [product status](https://github.com/willmove/joyoffice/blob/joydeck-v0.3.0/docs/joydeck/STATUS.md) and the [2026-11-23 UOS refactor validation](https://github.com/willmove/joyoffice/blob/joydeck-v0.3.0/docs/joyoffice/validation/20261123-uos-refactor/README.md) for verified scope and remaining work. Six-platform builds, SHA-256 checksums, ELF `GLIBC <= 2.28` checks, and Debian metadata/compression gates run before publication. Imported PPTX files without a reusable notes part still refuse saving newly added notes; save a native `.joydeck` copy instead. UOS ARM64, Wayland, accessibility, printing, and publisher signing remain unverified on physical hardware where noted.

## 中文

JoyDeck 是 JoyOffice 的演示文稿应用。软件包包含桌面应用和 `joydeck-cli`。

### 发布规则与软件包拉齐

JoyOffice 三款产品（JoyWord、JoySheet、JoyDeck）统一采用 **0.3.0** 版本，并在全部六组平台与架构上拉齐了完全一致的软件包格式：
- **Windows（x64、ARM64）：** Setup EXE + ZIP
- **macOS（Apple 芯片、Intel）：** DMG + ZIP
- **Linux（x64、ARM64）：** DEB + TAR.GZ + ZIP

Linux x64 与 ARM64 软件包现统一在 **glibc 2.28**（Debian 10 Buster）基线上构建，`.deb` 使用 xz 压缩并采用通用依赖声明（移除 `libssl3t64` 与 `libc6 >= 2.39` 限制），支持 **统信 UOS 20（glibc 2.28）** 与 **统信 UOS 25（glibc 2.38）**（含 x86_64 与 ARM64 架构），同时兼容 Debian 10–13 与 Ubuntu 20.04–24.04。

### 本版更新

- **安装包拉齐与 UOS 兼容：** 新增 macOS DMG、Linux DEB（`amd64` / `arm64`）和 Linux TAR.GZ 包；Linux 构建基线降至 glibc 2.28，发布门禁自动校验 ELF 符号版本（`<= GLIBC_2.28`）和 DEB 元数据与压缩格式。
- **第一方幻灯片转场与 GPU 合成：** 全部 49 种 PresentationML 幻灯片转场切换为第一方确定性生成器，并在 Metal／Vulkan 纹理上合成放映（含棱镜、纸张折叠、布料、波纹、闪光与漩涡等效果）。
- **DrawingML 预设图形与连接线：** 独立实现 183 项 ECMA DrawingML 预设图形、连接线、流程图、标注及分层动作按钮，编辑顶点与控制点时保持原有几何引导。
- **桌面与 UOS 修复：** 中文输入法激活时保留放映快捷键，修复原生另存目标校验、UOS DDE 桌面图标、X11 输入法上下文初始化与 Linux 软件 Vulkan 回退启动。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-x86_64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-aarch64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-windows-aarch64-release.zip.sha256) |
| macOS Apple 芯片 | [DMG](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-aarch64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-aarch64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-aarch64-release.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-x86_64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-x86_64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-macos-x86_64-release.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joydeck-v0.3.0/joydeck-0.3.0-linux-aarch64-release.zip.sha256) |

- **Windows：** 运行当前用户级 Setup EXE（携带 app-local Visual C++ 运行库），或解压便携 ZIP。
- **macOS：** 挂载 DMG 将 `JoyDeck.app` 拖入「应用程序」，或解压 ZIP 打开。
- **Linux（UOS 20/25、Debian、Ubuntu）：** 运行 `sudo apt install ./joydeck-0.3.0-linux-<架构>-release.deb` 安装（采用 xz 压缩与 `libc6 >= 2.28`），或解压 `.tar.gz` / `.zip` 后执行 `python3 install-desktop.py --product joydeck`。

软件包仍未作发行者签名，macOS 公证待完成。每项下载均附 SHA-256 校验文件。

### 能力与验证范围

详见[当前产品状态](https://github.com/willmove/joyoffice/blob/joydeck-v0.3.0/docs/joydeck/STATUS.md)与[2026-11-23 UOS 重构实机复验](https://github.com/willmove/joyoffice/blob/joydeck-v0.3.0/docs/joyoffice/validation/20261123-uos-refactor/README.md)。发布前自动执行六平台构建、SHA-256 校验、ELF `GLIBC <= 2.28` 门禁及 DEB 元数据／压缩检查；导入 PPTX 若没有可复用的备注部件，新增备注仍会拒绝回写，可另存 `.joydeck` 原生副本。UOS ARM64 真机、Wayland、读屏、打印与发行签名保留为未验证项，待具备条件后补验。
