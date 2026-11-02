# JoyDeck 依赖与公共模块对齐

日期：2026-11-02。目标基线为 JoyOffice `a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d`。完整提取结果与文件 hash 见 [planning-baseline.json](evidence/planning-baseline.json)。

## 对齐规则

当前目录与未来 JoyOffice 都以 JoyOffice 的直接依赖版本、来源及 feature 为准。只迁移 JoyDeck 需要的依赖，不把 DeckCraft 根 manifest、patch、lock、构建环境整体复制过来。

从现有 JoyOffice 锁文件提取共同依赖的解析基线，在新 workspace 中解析实际成员并提交新锁文件；独立 workspace 和宿主成员不同，两个锁文件不要求逐字相同。比较应覆盖共同 package 的名称、版本、source、checksum 和实际 feature 闭包。

本次版本是已核对的快照。后续 JoyOffice 升级时，通过记录明确的新基线和使用方回归同步，不跟随“最新版本”自动升级。

## GPUI 与工具链

| 项目 | JoyOffice 当前值 | JoyDeck 处理 |
| --- | --- | --- |
| rust-toolchain.toml | `channel = "1.98.1"`，minimal，clippy/rustfmt | 完全一致 |
| workspace.package | edition `2024`，rust-version `1.98` | 保持相同；实际构建固定 1.98.1 |
| Cargo resolver | `3` | 完全一致 |
| gpui-kit | `=0.8.1` | 根 workspace 精确固定，产品用 workspace 依赖 |
| GPUI | Kit 依赖 package `gpui-pre =0.3.10` | 不再另加 crates.io `gpui =0.2.2` 或直接 Zed Git 版本 |
| gpui-base/component/kit-assets | 锁文件均为 `0.8.1` | 由 Kit 带入；锁定结果保持，避免各自更新 |
| gpui-pre-platform | `=0.3.10`，workspace alias `gpui_platform`，features font-kit/wayland/x11 | 需要平台桥接时沿用该精确声明；普通 API 经 Kit |
| Kit feature | 默认 component/assets；未启用 gpui-fast | 新项目相同，禁止为单产品暗中切换 backend |
| dev-runtime-shaders | JoyWord 用精确 platform 依赖转发开发特性 | mac 开发确有需要时同方式转发；正式构建不依赖此特性 |
| lints | unsafe_code forbid，Clippy 与产品门禁 | 保持；导入生产代码错误可恢复 |
| profile | dev 的依赖 opt-level=2/debug=false；release thin LTO/codegen-units=1 | 先沿用，性能比较使用相同构建配置 |

最小依赖形状：

```toml
[workspace.dependencies]
gpui-kit = "=0.8.1"
joyoffice-ui = { path = "crates/joyoffice-ui" }

# 仅当沿用宿主的平台桥接/开发 shader 特性时使用。
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.10", features = ["font-kit", "wayland", "x11"] }
```

Kit 的 GPUI 类型、组件、基础能力和资源通过 `gpui_kit::*`、`gpui_kit::component`、`gpui_kit::base`、`gpui_kit::assets` 获取。官方说明见 [GPUI Kit 安装文档](https://gpui-kit.com/docs/installation/)。

JoyOffice ADR-010 对应 Zed 快照 `08cbb88a152c3751a43e853425a70e9f631a5918`，并登记已审阅的发布差异。沿用 `scripts/verify_gpui_pre_provenance.py` 及 `scripts/gpui_pre_reviewed_deviations.json` 的核对方法；crate 版本相同不能替代来源验证。

## 已发现的版本和来源冲突

下表区分上游锁文件中的实际值与目标工作区的直接依赖。目标锁中已有的合理传递多版本无需为“版本唯一”强行全局覆盖。

| 依赖 | DeckCraft 本机情况 | JoyOffice 目标 | 处理和验收 |
| --- | --- | --- | --- |
| serde | lock `1.0.229` | 直接 `=1.0.228` | 迁移时编译与序列化语义回归，不带动整个宿主升级 |
| serde_json | lock `1.0.154`，上游开启 float_roundtrip | 直接 `=1.0.154` | 版本一致；评估需要的 float_roundtrip feature 对共同解析的影响 |
| image | lock `0.25.10`，较多格式 feature | 直接 `=0.25.9` | 先适配宿主版本；PNG/JPEG/GIF 等按用途声明；核对统一后 feature 扩张 |
| zip | 直接要求 3，lock 同时有 `3.0.0`/`9.1.0` | `=9.0.0`，default-features=false，deflate-flate2 | 适配 API；保持 Rust flate2 后端、重复项检查及所有包预算 |
| harfrust | 直接 0.13，lock 有 `0.13.3`/`0.12.0` | 产品直接 `=0.8.4`；锁另有传递 `0.5.2` | JD-003 验证 shaping API；适配到宿主直接版本；cluster/metrics/字体回归通过后采用 |
| skrifa | 直接 0.47，lock 另有 `0.42.1`/`0.44.0` | 产品直接 `=0.42.1`；锁也有其它传递版本 | 适配读取/字形 API，不能将 Word/GPUI 字体链一起升级来绕过冲突 |
| kurbo | lock `0.13.1` | 直接 `=0.13.1` | 直接复用；核对 serde feature，合并已有预设几何来源 |
| quick-xml | 直接 0.41，lock 还出现 `0.42.0` | 直接 `=0.41.0` | 采用宿主直接版本；XML 保留和边界解析回归 |
| rfd / arboard | lock `0.17.2` / `3.7.0` | `=0.17.2` / `=3.7.0` | 版本可对齐；优先沿用已有平台/GPUI 对话框；富剪贴板单独适配 |
| vello_cpu/common | lock `0.2.0` / `0.2.0` | 当前主锁无此依赖 | 作为新增项精确固定并审阅闭包，先用于 CPU 参考/fallback，不引入第二套窗口循环 |
| krilla | `0.8.2`，source 是 craft-libs Git fork，根 patch 生效 | `=0.8.2`，source 为 crates.io | 版本相同而来源不同；禁止照搬全局 patch 影响 Word PDF；单独做官方 backend 适配实验 |
| craft-pdf/media | 固定 craft-libs rev `c5a1cbfb5f9049f9270955caffb5bee5997aebf7` | 当前主 manifest 无这两个依赖 | PDF 优先用宿主官方 Krilla 适配；media 只在确有需要时保留固定来源并隔离 feature/许可 |
| epaint_default_fonts | fonts 的生产 fallback 和测试均使用，且来自 egui Git patch | 新程序无 egui/epaint 来源依赖 | 替换为受控字体输入或许可明确的独立字体资源，重建相应测试 |
| egui/eframe/winit | 上游 UI 和桌面入口使用，含 egui Git fork patches | GPUI/Kit 统一桌面栈 | 不迁入运行时、开发测试闭包或根 patches；迁移行为测试到新适配层 |
| craft-ui / craft-update | 前者为 egui UI，后者开启 egui feature | 公共 JoyOffice UI、现有维护流程 | 不直接复用 egui 部分；更新/诊断产品行为单独迁移 |

字体链和 PDF 不是纯修改版本号的工作。R0 先用真实字体与样例编译最小调用，识别 API/效果差异；必要适配进入明确任务。没有验证兼容前，不宣称可以无损替换。

对 `craft-pdf` 若不能直接改用宿主的官方 Krilla，先把所需差异和调用点列清，再选择本地适配或受控的独立后端。任何临时独立后端须用明确别名隔离、固定来源、不得给整个 workspace 添加同名全局 patch，并在最终整合前处理。不能以未经验证的“同为 0.8.2”关闭冲突任务。

## 公共模块与同步方式

| 模块 | 使用范围 | 当前目录的方式 | 最终整合方式 |
| --- | --- | --- | --- |
| joyoffice-ui | 明暗 Palette、Deck 品牌、metrics、控件/图标原语 | 固定来源的仓库内快照，保持产品中立 | 回接宿主唯一 crate，合并必要增量，验证 Word/Sheet |
| joyoffice-opc | ZIP/OPC 原始结构、成员/路径/预算及关系能力 | 复制真实需要的 crate 与实际依赖闭包，保持契约 | 回接宿主；新增行为由其所有使用方测试 |
| joyoffice-drawingml | 可复用的 DrawingML 几何/样式语义 | 先比较 API、默认值、单位和未知字段保留，再按切片接入 | 只共享真正中立的实现，Deck 场景语义留在产品 |
| joyoffice-omml/cfb 等 | 当前主要迁移范围无直接需求 | 不为潜在需求批量引入 | 后续有真实调用方再评估 |
| 旧 JoyDeck 产品模块 | 窗口、输入、平台、存储、偏好、放映、验证 | 提取代码与测试断言，改为新模型适配；保留出处 | 同名产品目录内成为唯一实现 |
| tools/xtask / scripts | 产品选择、门禁、构建/打包/原生验证 | 保持命令形状，依实际成员调整登记 | 使用宿主统一工具，移除重复 workspace 管理 |

共享快照至少记录：宿主 commit、原路径、目标路径、原文件 SHA-256、本地修改说明、待回流补丁。每阶段比较新增漂移。构建不能要求开发机恰好存在 `D:\Coding\EditorProjects\joyoffice` 或 `deckcraft-main`。

现有 JoyDeck 的代码也不能整体作为第二套 engine 挂在新应用里。仅 legacy 读取器可以保留旧 DTO；可编辑状态必须统一，否则 undo、资源 ID、保存和自动化会产生双重真值。

## Feature 和依赖闭包检查

R0 建立或复用自动检查，至少包括：

1. `joydeck` 默认启用桌面；CLI/核心选择不带 GPUI、窗口库或另一个产品应用。
2. 单独 desktop、validation、媒体能力和开发 shader feature 的构建可追溯，validation 不进入正常用户流程。
3. `gpui-fast` 未被任一新依赖意外启用；共同 GPUI 类型来源一致。
4. `cargo tree -e features -p joydeck` 与产品 metadata 核对真实 feature，并对比宿主联合构建。
5. 查清 `image`、serde、字体、ZIP、压缩/PDF 的 feature 合并是否改变默认行为与包大小。
6. 新运行时与测试闭包不再含 egui、eframe、epaint、egui_kittest、egui-winit、egui-wgpu、craft-ui。
7. 第三方 codec/fork 的 version、rev、许可证与实际分发方式有完整登记。

禁止无依据地要求整个 Monorepo 所有传递依赖只出现一个版本；需要保证的是共同 ABI/类型边界与直接依赖基线一致，以及额外多版本有明确来源和用途。

## 平台构建基线

- Windows：MSVC、Windows SDK、CMake 与可用图形驱动；项目脚本用 `pwsh.exe`。依赖安装参考 [GPUI Kit 安装说明](https://gpui-kit.com/docs/installation/)和 [GPUI 上游 Windows 构建说明](https://zed.dev/docs/development/windows)。
- macOS：保持宿主 CI 的 Xcode/Metal 环境；开发 runtime_shaders 选项与正常发布 shader 构建分别验收。参考 [上游 macOS 构建说明](https://zed.dev/docs/development/macos)。
- Linux：沿宿主已验证的系统库与 Wayland/X11 配置，按目标图形驱动和字体验证；不可仅安装用户态 Vulkan loader 就视为窗口可用。参考 [Kit 安装说明](https://gpui-kit.com/docs/installation/)和 [上游 Linux 构建说明](https://zed.dev/docs/development/linux)。

三平台文档只提供构建前提；最终支持范围由本项目当次构建和实际运行证据决定。
