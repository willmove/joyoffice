# JoyWord v0.3.0

## English

JoyWord is a document editor in JoyOffice. Packages include the desktop application and `joyword-cli`.

### Release policy & package parity

All JoyOffice products (JoyWord, JoySheet, JoyDeck) share unified version **0.3.0** and now ship the exact same package set across all six platform/architecture targets:
- **Windows (x64, ARM64):** Setup EXE + ZIP
- **macOS (Apple Silicon, Intel):** DMG + ZIP
- **Linux (x64, ARM64):** DEB + TAR.GZ + ZIP

Linux x64 and ARM64 packages are now built on a **glibc 2.28** baseline (Debian 10 Buster) with xz-compressed `.deb` packages and portable runtime dependencies, ensuring compatibility with **UOS 20 (glibc 2.28)** and **UOS 25 (glibc 2.38)** across both x86_64 and ARM64 as well as Debian 10–13 and Ubuntu 20.04–24.04.

### What’s new

- **Aligned installers & UOS compatibility:** Added macOS DMG, Linux DEB (`amd64` / `arm64`), and Linux ZIP packages alongside TAR.GZ; lowered Linux ABI baseline from glibc 2.39 to glibc 2.28 with automated ELF symbol-version and Debian package gates.
- **First-party proofing & language packs:** Retired legacy `craft-libs` runtime crates in favor of first-party JoyWord proofing (`joyword-proof`, `joyword-proof-rules`), compact dictionary indexing, weighted spelling ranking, localized lexical/editorial rules, and verified language pack installation/uninstallation.
- **Bounded incremental section reflow:** Added section-boundary splice reflow across facing/rotated/unequal-column sections, line numbering, footnotes/endnotes, anchored drawings, comment balloons, and distant page fields, falling back to full pagination whenever budgets are exceeded.
- **Desktop & UOS fixes:** Fixed native product icons and UOS DDE desktop launcher registration, X11 input-context initialization for Chinese IME, Linux software Vulkan fallback startup, and shared save-changes dialogs and metafiles.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-x86_64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-x86_64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-x86_64-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-x86_64.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-aarch64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-aarch64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-aarch64-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-aarch64.zip.sha256) |
| macOS Apple Silicon | [DMG](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-aarch64.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-aarch64.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-aarch64.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-aarch64.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-x86_64.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-x86_64.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-x86_64.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-x86_64.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.zip.sha256) |

- **Windows:** Run the per-user Setup EXE (bundles app-local Visual C++ runtime DLLs), or extract the ZIP.
- **macOS:** Mount the DMG and drag `JoyWord.app` to Applications, or extract the ZIP.
- **Linux (UOS 20/25, Debian, Ubuntu):** Install the `.deb` with `sudo apt install ./joyword-0.3.0-linux-<arch>.deb` (uses xz compression and `libc6 >= 2.28`), or extract the `.tar.gz`/`.zip` archive and run `python3 install-desktop.py --product joyword`.

Packages remain unsigned and macOS notarization is pending. Each download includes a SHA-256 sidecar.

### Capabilities and validation

See [product status](https://github.com/willmove/joyoffice/blob/joyword-v0.3.0/docs/joyword/r8/STATUS.md) and the [2026-11-23 UOS refactor validation](https://github.com/willmove/joyoffice/blob/joyword-v0.3.0/docs/joyoffice/validation/20261123-uos-refactor/README.md) for verified scope and remaining work. Six-platform builds, SHA-256 checksums, ELF `GLIBC <= 2.28` checks, and Debian metadata/compression gates run before publication. UOS ARM64, Wayland, accessibility, printing, and publisher signing remain unverified on physical hardware where noted.

## 中文

JoyWord 是 JoyOffice 的文字处理应用。软件包包含桌面应用和 `joyword-cli`。

### 发布规则与软件包拉齐

JoyOffice 三款产品（JoyWord、JoySheet、JoyDeck）统一采用 **0.3.0** 版本，并在全部六组平台与架构上拉齐了完全一致的软件包格式：
- **Windows（x64、ARM64）：** Setup EXE + ZIP
- **macOS（Apple 芯片、Intel）：** DMG + ZIP
- **Linux（x64、ARM64）：** DEB + TAR.GZ + ZIP

Linux x64 与 ARM64 软件包现统一在 **glibc 2.28**（Debian 10 Buster）基线上构建，`.deb` 使用 xz 压缩并采用通用依赖声明（移除 `libssl3t64` 与 `libc6 >= 2.39` 限制），支持 **统信 UOS 20（glibc 2.28）** 与 **统信 UOS 25（glibc 2.38）**（含 x86_64 与 ARM64 架构），同时兼容 Debian 10–13 与 Ubuntu 20.04–24.04。

### 本版更新

- **安装包拉齐与 UOS 兼容：** 新增 macOS DMG、Linux DEB（`amd64` / `arm64`）和 Linux ZIP 包；Linux 构建基线降至 glibc 2.28，发布门禁自动校验 ELF 符号版本（`<= GLIBC_2.28`）和 DEB 元数据与压缩格式。
- **第一方校对引擎与语言包：** 退役旧 `craft-libs` 运行时依赖，切换至第一方 `joyword-proof` / `joyword-proof-rules`，支持紧凑词典索引、加权拼写排序、本地化词法／编辑规则以及语言包安装与卸载校验。
- **有界增量分节重排：** 支持分节边界局部重排（含奇偶页、旋转页、不等宽分栏、行号、脚注／尾注、浮动绘图、批注气泡与跨节页码字段），超预算时安全回退整篇分页。
- **桌面与 UOS 修复：** 修复原生产品图标与 UOS DDE 桌面启动器注册、X11 输入法上下文初始化、Linux 软件 Vulkan 回退启动，以及共享保存提示框与元文件读取。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-x86_64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-x86_64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-x86_64-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-x86_64.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-aarch64-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-aarch64.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-aarch64-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-windows-aarch64.zip.sha256) |
| macOS Apple 芯片 | [DMG](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-aarch64.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-aarch64.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-aarch64.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-aarch64.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-x86_64.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-x86_64.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-x86_64.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-macos-x86_64.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-x86_64.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joyword-v0.3.0/joyword-0.3.0-linux-aarch64.zip.sha256) |

- **Windows：** 运行当前用户级 Setup EXE（携带 app-local Visual C++ 运行库），或解压便携 ZIP。
- **macOS：** 挂载 DMG 将 `JoyWord.app` 拖入「应用程序」，或解压 ZIP 打开。
- **Linux（UOS 20/25、Debian、Ubuntu）：** 运行 `sudo apt install ./joyword-0.3.0-linux-<架构>.deb` 安装（采用 xz 压缩与 `libc6 >= 2.28`），或解压 `.tar.gz` / `.zip` 后执行 `python3 install-desktop.py --product joyword`。

软件包仍未作发行者签名，macOS 公证待完成。每项下载均附 SHA-256 校验文件。

### 能力与验证范围

详见[当前产品状态](https://github.com/willmove/joyoffice/blob/joyword-v0.3.0/docs/joyword/r8/STATUS.md)与[2026-11-23 UOS 重构实机复验](https://github.com/willmove/joyoffice/blob/joyword-v0.3.0/docs/joyoffice/validation/20261123-uos-refactor/README.md)。发布前自动执行六平台构建、SHA-256 校验、ELF `GLIBC <= 2.28` 门禁及 DEB 元数据／压缩检查；UOS ARM64 真机、Wayland、读屏、打印与发行签名保留为未验证项，待具备条件后补验。
