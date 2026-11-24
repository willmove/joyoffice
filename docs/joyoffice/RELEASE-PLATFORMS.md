# 发布规则与平台扩展

2026-11-16。本文区分当前产物、可增加的安装格式和需要移植的新平台，供后续发布排期使用。

## 从 v0.2.0 开始的发布规则

- 三产品、CLI、第一方库和工具统一使用根 `Cargo.toml` 的 `0.2.0`；内部精确版本约束和 Cargo.lock 同步更新。
- 后续按正常 GitHub Release 发布，标题为 `JoyWord v0.2.0`、`JoySheet v0.2.0`、`JoyDeck v0.2.0`，不再增加 Preview／预览版，也不设置 prerelease。版本入口拒绝带预发布或构建后缀的版本。
- 产品标签仍分别为 `joyword-v0.2.0`、`joysheet-v0.2.0`、`joydeck-v0.2.0`。历史 `v*` 标签仅映射 Word，不代表三个产品一起发布。
- 先生成普通 release 的临时草稿，全部平台产物、图标、归档结构、SHA-256 和 GitHub 上传摘要校验通过后再公开；最后设置为 Latest。同仓库只有一个 Latest，三个产品以各自标签定位下载。
- 继续使用双语说明、原生六平台矩阵和既有打包／许可门禁。正常 Release 不自动代表未验收功能、签名、公证或所有硬件已经完成验收。
- v0.1.4 等历史预览保留原标签、资产和说明；在 v0.2.0 实际发布前，README 下载表继续指向已存在的 v0.1.4。

实现入口：[发布工作流](../../.github/workflows/release.yml)、[版本路由](../../tools/xtask/product.py)。准备版本与说明不触发公开发布；本次没有创建产品标签或执行发布工作流。

## v0.3.0 起拉齐的发布矩阵与 UOS 兼容基线

2026-11-24 起，JoyWord、JoySheet、JoyDeck 在六组系统／架构上拉齐为完全一致的 14 项产物（加 14 份 SHA-256 校验文件，共 28 项资产）：

| 系统／架构 | JoyWord | JoySheet | JoyDeck |
| --- | --- | --- | --- |
| Windows x86_64 / ARM64 | Setup EXE、ZIP | Setup EXE、ZIP | Setup EXE、ZIP |
| macOS Intel / Apple 芯片 | DMG、ZIP 内含 `.app` | DMG、ZIP 内含 `.app` | DMG、ZIP 内含 `.app` |
| Linux x86_64 / ARM64 | DEB、TAR.GZ、ZIP | DEB、TAR.GZ、ZIP | DEB、TAR.GZ、ZIP |

- Linux x86_64 与 ARM64 发布构建切换至 **Debian 10 Buster（glibc 2.28）** 容器基线，覆盖 **UOS 20（glibc 2.28）** 与 **UOS 25（glibc 2.38）**（含 x86_64 与 ARM64）以及 Debian 10–13 / Ubuntu 20.04–24.04。
- 三产品共用 [scripts/package_installers.py](../../scripts/package_installers.py) 生成 macOS DMG 与 xz 压缩（`dpkg-deb -Zxz`）的 Linux DEB，DEB 依赖使用 `libc6 (>= 2.28)` 与 `libgcc-s1 | libgcc1` 等跨发行版通用包名，移除 `libssl3t64` 和 `libc6 (>= 2.39)` 限制。
- [scripts/verify_release_assets.py](../../scripts/verify_release_assets.py) 自动校验全部 28 项资产、Linux 二进制 ELF 最高 GLIBC 符号版本（`<= GLIBC_2.28`）、DEB 非 zstd 压缩与 UOS 兼容依赖字段。UOS ARM64 真机仍按根 AGENTS.md 保留未实机验证说明。

## 历史基线（v0.1.4 / v0.2.0）

截至本文日期，三个 v0.1.4 发布均覆盖下面六组系统／架构，每个产物均有 SHA-256 sidecar。

| 系统／架构 | JoyWord | JoySheet | JoyDeck |
| --- | --- | --- | --- |
| Windows x86_64 / ARM64 | Setup EXE、ZIP | Setup EXE、ZIP | Setup EXE、ZIP |
| macOS Intel / Apple 芯片 | ZIP 内含 `.app` | DMG、ZIP 内含 `.app` | ZIP 内含 `.app` |
| Linux x86_64 / ARM64 | TAR.GZ | DEB、ZIP | ZIP |

核对来源：[Word v0.1.4](https://github.com/willmove/joyoffice/releases/tag/joyword-v0.1.4)、[Sheet v0.1.4](https://github.com/willmove/joyoffice/releases/tag/joysheet-v0.1.4)、[Deck v0.1.4](https://github.com/willmove/joyoffice/releases/tag/joydeck-v0.1.4) 和当前 workflow。Linux 发布构建使用 Ubuntu 24.04；包格式、CPU 架构和动态库最低要求是三个不同维度。

## 建议优先扩展的安装格式

以下均为候选，尚未接入发布矩阵。

| 优先级 | 候选产物 | 可扩展的安装渠道 | 主要实施条件 |
| --- | --- | --- | --- |
| P1 | Word／Deck 的 DEB，三产品的 RPM；x86_64 / ARM64 | Debian／Ubuntu／Deepin／部分 UOS 与麒麟版本；Fedora／openEuler／兼容 RPM 系统 | 共用 Linux 安装布局、桌面项、图标、MIME、动态依赖和卸载清单；按具体发行版及版本构建／验证，不能仅改扩展名 |
| P1 | 三产品 AppImage；x86_64 / ARM64 | 更便捷的跨发行版便携分发 | 确定较低系统构建基线、收集允许携带的依赖、核对 Vulkan／图形驱动和运行时加载；不保证运行于所有 Linux |
| P1 | 三产品 Flatpak；x86_64 / ARM64 | 有 Flatpak 环境的发行版、后续 Flathub 渠道 | 选择 runtime，接入文件／打印等 portal、中文字体、输入法、GPU、剪贴板和外部 helper；分别验证沙箱内 CLI／自动化能力 |
| P2 | Word／Deck 的 DMG，按需增加 macOS PKG | macOS 安装体验与管理部署 | 复用现有 app bundle；继续跟踪签名、公证、系统打开文档和卸载。DMG 本身不增加系统／架构覆盖 |
| P2 | Windows MSI／MSIX | 企业部署、包管理和可选 Store 渠道 | 现有 EXE 已覆盖两种架构；额外格式按部署需要做，处理签名、注册身份、升级／回退及 CLI／外部文件访问 |
| P3 | Homebrew、WinGet、Scoop、APT／RPM 仓库 | 自动发现、安装和升级 | 建立稳定下载、校验、元数据和升级策略；属于分发渠道，不增加 CPU 架构 |
| P3 | 玲珑等国产桌面分发格式 | 支持对应运行时的 Deepin／UOS 桌面 | 按运行时／架构另做打包与集成验证，不从通用 Linux 构建推断兼容 |

兼容较老发行版应单独立项：先评估更低的 glibc／libstdc++／OpenSSL 等构建基线，再验证目标发行版的实际动态依赖及图形环境。把 Ubuntu 24.04 二进制装入 DEB、RPM 或 AppImage，并不会自动降低其 ABI 要求。[AppImage 官方建议](https://docs.appimage.org/introduction/concepts.html#build-on-old-systems-run-on-newer-systems)是在目标范围内较老的系统上构建；[Flatpak runtime](https://docs.flatpak.org/en/latest/basic-concepts.html#runtimes)提供跨发行版依赖基础，但仍需应用适配与沙箱验证。

## 新系统／架构的可行性顺序

| 顺序 | 候选 | 仓库已有基础与缺口 | 建议交付方式 |
| --- | --- | --- | --- |
| 1 | Linux LoongArch64（龙芯新架构，常见包架构名 `loong64`） | Word 的 CI 已配置 headless 交叉／QEMU 检查；Deck／Sheet 需补对应验证，三产品均无该架构公开包 | 先补独立 CLI 包与三产品依赖审计，再处理 GPUI／图形、字体、输入法和安装包；QEMU 与真机证据分别记录 |
| 2 | Linux RISC-V 64（`riscv64gc`） | Rust 有对应 Linux 目标；当前发布 workflow 没有该架构 | 先做核心／CLI 构建与测试、依赖和工具链验证，再判断桌面后端与 GPU 驱动条件 |
| 3 | Alpine／musl Linux，先 x86_64 / ARM64 | 当前 Linux 包使用 glibc 系统构建，不能直接假定兼容 musl | 优先验证独立 CLI 的依赖与链接方式；静态链接范围和桌面动态库需求以实际产物为准 |
| 3 | Linux ARMv7、PowerPC64LE、s390x | 当前产品没有相应桌面包和完整依赖验证；32 位还有地址空间限制 | 有明确用户需求时优先交付 CLI／文档处理工具，桌面另作可行性验证 |
| 4 | FreeBSD 等 BSD 桌面 | Rust 目标存在不代表现有 GPUI 后端、系统 API 或打包脚本可直接使用 | 需要平台移植；先核对 CLI、字体和系统调用，再评估桌面，不能只添加矩阵项 |
| 另立项目 | Android、iOS／iPadOS、鸿蒙、浏览器 | 当前是原生桌面应用；UI、输入、窗口／文件权限和分发模型均需重新适配 | 复用核心引擎并开发对应前端；不纳入本轮桌面安装格式扩展 |

[Rust 目标支持表](https://doc.rust-lang.org/rustc/platform-support.html)只能证明编译器目标的支持级别，不能替代 JoyOffice 的依赖与运行验收。现有 `cargo xtask package --headless` 尚未实现，LoongArch／RISC-V 的 CLI 单独分发也需要补打包入口、资产校验和发布说明。飞腾／鲲鹏等 ARM64 和海光／兆芯等 x86_64 设备通常优先沿用已有 CPU 目标，按具体系统、驱动及 CPU 指令基线补兼容验证；旧 MIPS 龙芯不能使用 LoongArch 包。

建议推进次序：先补 DEB／RPM 和较低 Linux ABI 基线，再扩 AppImage／Flatpak；新架构先 LoongArch64、后 RISC-V64，先 CLI、再桌面。保留六组现有包作为每次 release 的必需矩阵，新增平台单独定义产物与验证条件。信创真机暂不可用时继续开发及自动验证，按根 AGENTS.md 保留“真机未验证”，不阻塞现有平台发布。

## 本次验证

- `python3 -m unittest discover -s tools/xtask -p 'test_*.py'`：17 项通过，含产品版本路由及带后缀／非规范版本拒绝。
- `PYTHONPATH=scripts/tests python3 -m unittest test_release_workflow test_verify_release_assets test_package_archive test_package_windows test_release_archive_members`：29 项通过。工作流测试执行实际 Bash 步骤并使用本地 GitHub CLI 替身，覆盖三产品普通草稿创建／续跑／公开，以及已公开、预发布草稿和说明不匹配时拒绝修改。
- `cargo metadata --locked --offline --no-deps --format-version 1`：61 个第一方 workspace 包均为 0.2.0；逐条核对 Cargo.lock，第三方版本、来源、checksum 及依赖记录不变。
- `cargo check --locked --offline -p joyoffice-xtask`、Word／Deck 文档门禁、10 个发布 Bash 块语法检查和本次文档本地链接检查通过。

本次验证针对版本与发布策略，没有构建六平台新发布包、执行真实 GitHub 发布或增加 GUI／信创真机验收。新增格式和架构仍为上文候选。
