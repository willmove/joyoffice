# JoySheet v0.3.0

## English

JoySheet is a spreadsheet application in JoyOffice. Packages include the desktop application and `joysheet-cli`.

### Release policy & package parity

All JoyOffice products (JoyWord, JoySheet, JoyDeck) share unified version **0.3.0** and now ship the exact same package set across all six platform/architecture targets:
- **Windows (x64, ARM64):** Setup EXE + ZIP
- **macOS (Apple Silicon, Intel):** DMG + ZIP
- **Linux (x64, ARM64):** DEB + TAR.GZ + ZIP

Linux x64 and ARM64 packages are now built on a **glibc 2.28** baseline (Debian 10 Buster) with xz-compressed `.deb` packages and universal runtime dependencies (`libc6 >= 2.28`, no Ubuntu-only `libssl3t64`), ensuring compatibility with **UOS 20 (glibc 2.28)** and **UOS 25 (glibc 2.38)** across both x86_64 and ARM64 as well as Debian 10–13 and Ubuntu 20.04–24.04.

### What’s new

- **Aligned installers & UOS compatibility:** Added Linux TAR.GZ archives alongside DEB and ZIP; fixed DEB dependencies and xz compression for UOS 20/25 (`amd64` and `arm64`) and lowered the Linux ABI baseline from glibc 2.39 to glibc 2.28 with automated ELF verification.
- **First-party formula kernels:** Rewrote conditional aggregates, criteria matching, matrix operations, text and calendar/workday date functions, cash-flow/depreciation, complex/Bessel/special-math kernels, multivariate regression, dynamic references, and in-place lookups and array spills.
- **Unified Excel format import:** Unified pane selections/scroll origins, display options/zoom, worksheet dimensions/hidden lines/outlines, print setup/margins/headers/page breaks, and sheet protection/hyperlinks across XLS, XLSB, and XLSX with bounded pivot reads.
- **Desktop & UOS fixes:** Fixed backstage recent-file truncation/alignment, native save-panel target safety, UOS DDE desktop icons, X11 Chinese IME initialization, and Linux software Vulkan fallback startup.

### Download and install

| OS and processor | Download | SHA-256 files |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-x86_64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-aarch64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-aarch64-release.zip.sha256) |
| macOS Apple Silicon | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-aarch64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-aarch64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-aarch64-release.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-x86_64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-x86_64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-x86_64-release.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.zip.sha256) |

- **Windows:** Run the per-user Setup EXE (bundles app-local Visual C++ runtime DLLs), or extract the ZIP.
- **macOS:** Mount the DMG and drag `Joysheet.app` to Applications, or extract the ZIP (requires macOS 15 or later; PDF preview uses Poppler).
- **Linux (UOS 20/25, Debian, Ubuntu):** Install the `.deb` with `sudo apt install ./joysheet-0.3.0-linux-<arch>-release.deb` (uses xz compression and `libc6 >= 2.28`), or extract the `.tar.gz`/`.zip` archive and run `python3 install-desktop.py --product joysheet`.

Packages remain unsigned and macOS notarization is pending. Each download includes a SHA-256 sidecar.

### Capabilities and validation

See [product status](https://github.com/willmove/joyoffice/blob/joysheet-v0.3.0/docs/joysheet/STATUS.md) and the [2026-11-23 UOS refactor validation](https://github.com/willmove/joyoffice/blob/joysheet-v0.3.0/docs/joyoffice/validation/20261123-uos-refactor/README.md) for verified scope and remaining work. Six-platform builds, SHA-256 checksums, ELF `GLIBC <= 2.28` checks, and Debian metadata/compression gates run before publication. UOS ARM64, Wayland, accessibility, printing, and publisher signing remain unverified on physical hardware where noted.

## 中文

JoySheet 是 JoyOffice 的电子表格应用。软件包包含桌面应用和 `joysheet-cli`。

### 发布规则与软件包拉齐

JoyOffice 三款产品（JoyWord、JoySheet、JoyDeck）统一采用 **0.3.0** 版本，并在全部六组平台与架构上拉齐了完全一致的软件包格式：
- **Windows（x64、ARM64）：** Setup EXE + ZIP
- **macOS（Apple 芯片、Intel）：** DMG + ZIP
- **Linux（x64、ARM64）：** DEB + TAR.GZ + ZIP

Linux x64 与 ARM64 软件包现统一在 **glibc 2.28**（Debian 10 Buster）基线上构建，`.deb` 使用 xz 压缩并采用通用依赖声明（`libc6 >= 2.28`，移除仅限 Ubuntu 24.04 的 `libssl3t64`），支持 **统信 UOS 20（glibc 2.28）** 与 **统信 UOS 25（glibc 2.38）**（含 x86_64 与 ARM64 架构），同时兼容 Debian 10–13 与 Ubuntu 20.04–24.04。

### 本版更新

- **安装包拉齐与 UOS 兼容：** 新增 Linux TAR.GZ 归档，修复 DEB 依赖与 xz 压缩以支持 UOS 20/25（`amd64` / `arm64`），Linux 构建基线降至 glibc 2.28 并接入自动化 ELF 门禁校验。
- **第一方公式内核：** 重写条件聚合、条件匹配、矩阵运算、文本与日历／工作日函数、现金流与折旧、复数／贝塞尔／特殊数学函数、多元回归、动态引用以及原地查找与数组溢出。
- **统一 Excel 格式导入：** 在 XLS、XLSB、XLSX 之间统一冻结窗格与滚动原点、显示选项与缩放、行列默认尺寸／隐藏与分级大纲、打印设置／页边距／页眉页脚／分页符以及工作表保护与超链接，并约束透视表读取预算。
- **桌面与 UOS 修复：** 修复文件后台最近文件截断与对齐、原生另存目标校验、UOS DDE 桌面图标、X11 中文输入法初始化与 Linux 软件 Vulkan 回退启动。

### 下载与安装

| 系统与架构 | 下载 | SHA-256 校验文件 |
| --- | --- | --- |
| Windows x64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-x86_64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-x86_64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-x86_64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-x86_64-release.zip.sha256) |
| Windows ARM64 | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-aarch64-release-setup.exe) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-aarch64-release.zip) | [Setup EXE](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-aarch64-release-setup.exe.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-windows-aarch64-release.zip.sha256) |
| macOS Apple 芯片 | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-aarch64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-aarch64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-aarch64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-aarch64-release.zip.sha256) |
| macOS Intel | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-x86_64-release.dmg) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-x86_64-release.zip) | [DMG](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-x86_64-release.dmg.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-macos-x86_64-release.zip.sha256) |
| Linux x64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-x86_64-release.zip.sha256) |
| Linux ARM64 | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.deb) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.tar.gz) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.zip) | [DEB](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.deb.sha256) · [TAR.GZ](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.tar.gz.sha256) · [ZIP](https://github.com/willmove/joyoffice/releases/download/joysheet-v0.3.0/joysheet-0.3.0-linux-aarch64-release.zip.sha256) |

- **Windows：** 运行当前用户级 Setup EXE（携带 app-local Visual C++ 运行库），或解压便携 ZIP。
- **macOS：** 挂载 DMG 将 `Joysheet.app` 拖入「应用程序」，或解压 ZIP 打开（需 macOS 15 或更新版本；PDF 预览使用 Poppler）。
- **Linux（UOS 20/25、Debian、Ubuntu）：** 运行 `sudo apt install ./joysheet-0.3.0-linux-<架构>-release.deb` 安装（采用 xz 压缩与 `libc6 >= 2.28`），或解压 `.tar.gz` / `.zip` 后执行 `python3 install-desktop.py --product joysheet`。

软件包仍未作发行者签名，macOS 公证待完成。每项下载均附 SHA-256 校验文件。

### 能力与验证范围

详见[当前产品状态](https://github.com/willmove/joyoffice/blob/joysheet-v0.3.0/docs/joysheet/STATUS.md)与[2026-11-23 UOS 重构实机复验](https://github.com/willmove/joyoffice/blob/joysheet-v0.3.0/docs/joyoffice/validation/20261123-uos-refactor/README.md)。发布前自动执行六平台构建、SHA-256 校验、ELF `GLIBC <= 2.28` 门禁及 DEB 元数据／压缩检查；UOS ARM64 真机、Wayland、读屏、打印与发行签名保留为未验证项，待具备条件后补验。
