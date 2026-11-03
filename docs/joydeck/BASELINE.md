# JoyDeck 重构基线记录

日期：2026-11-02，时区 Asia/Shanghai。本轮交付为分析与规划；当前目录尚无 Rust 应用源码或 Cargo workspace。

## 来源

| 用途 | 本机位置 | 固定信息 |
| --- | --- | --- |
| 待重构来源 | `D:\Coding\GithubProjects\deckcraft-main` | workspace 版本 0.14.1；本目录无 `.git`，不推断它等于某个上游提交 |
| 对齐与复用来源 | `D:\Coding\EditorProjects\joyoffice` | main，HEAD `a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d`；分析开始时 git status 干净 |
| 规划与后续实施 | `D:\Coding\EditorProjects\joydeck-new` | 开始时为空；应用/package/module 命名仍使用 joydeck |

DeckCraft 源码目录没有可用于证明完整提交身份的 Git 元数据，故 [机器基线](evidence/planning-baseline.json)记录本地文件指纹。JoyOffice 历史预设几何来源 `82573331...` 不能据此当成本次 DeckCraft 整个目录的提交身份。

## 本机静态结果

| 检查 | 结果 | 含义 |
| --- | --- | --- |
| rustc/cargo | 本机均为 1.98.1，默认 MSVC | 与 JoyOffice 工具链一致；不证明所有原生依赖可用 |
| DeckCraft metadata | `cargo metadata --locked --offline --no-deps --format-version 1` 成功，28 个成员 | 可读取 workspace manifest；不包含编译或完整依赖可用性验证 |
| JoyOffice metadata | 相同命令成功，50 个成员 | 已确认目标 workspace 与现有 JoyDeck 包布局 |
| DeckCraft 核心 | 24 个库，3 个应用及 xtask | 保留核心、重建桌面适配具备结构基础 |
| DeckCraft egui | 172 个 Rust 文件 / 45,553 行 | 含测试、注释、空行；是迁移规模参考 |
| DeckCraft engine | 101 个 Rust 文件 / 42,693 行 | 实际还承担文件与导出服务，需要分层迁移 |
| DeckCraft render | 32 个 Rust 文件 / 14,480 行 | CPU renderer 作为迁移初期正确性参考 |
| 现有 JoyDeck GUI | 40 个 Rust 文件 / 13,535 行 | 已有大量 GPUI 交互/平台实现可供适配复用 |
| 命令静态提取 | `crates/engine/src/cmd/**/*.rs` 的 `cmd!` 宏得到 329 个不同 ID | [命令清单](COMMANDS.csv)是待验证初表，不能替代运行时注册/GUI 覆盖验收 |

文件数量和行数由本机文本扫描得到，统计范围为所列目录所有 `.rs`；不据此推算完成百分比或照搬上游工时。

## 关键源码依据

路径均相对于各自来源根目录，关键文件 SHA-256 见机器基线。

| 来源 | 路径 | 核对内容 |
| --- | --- | --- |
| DeckCraft | `Cargo.toml`、`Cargo.lock`、各 crate manifest | Rust/依赖/patch、模型/UI 分层、版本和来源冲突 |
| DeckCraft | `crates/model/src/lib.rs` | Presentation、typed IDs、文本/主题/对象模型 |
| DeckCraft | `crates/engine/src/lib.rs`、`cmd.rs`、`io.rs` | Session、命令入口、历史、UI 请求与实际 IO 职责 |
| DeckCraft | `crates/render/src/lib.rs`、`docs/architecture.md` | vello_cpu、布局与图像/转场输出，屏幕纹理方案 |
| DeckCraft | `crates/fonts/{Cargo.toml,build.rs,src/fontdb.rs}` | harfrust/skrifa、craft-fonts、本地字体与 epaint_default_fonts |
| DeckCraft | `crates/format/src/lib.rs`、`crates/pdf/Cargo.toml` | 原生容器、预算、版本和 craft-pdf/Krilla fork |
| JoyOffice | `AGENTS.md`、根 Cargo 与工具链 | 产品边界、固定依赖、无界面构建、门禁与验证规则 |
| JoyOffice | `docs/joyoffice/{ARCHITECTURE,MONOREPO-MIGRATION,MONOREPO-RESULT}.md` | 统一 workspace/锁、产品工程命令与整合方式 |
| JoyOffice | `docs/joydeck/{STATUS,DEVELOPMENT}.md` | 现有能力、开放缺口和真实开发入口 |
| JoyOffice | `crates/joyoffice-ui/src/lib.rs` | Palette、Deck 品牌、metrics、control/card/field/icon |
| JoyOffice | `apps/joydeck/src/{editor,scene_canvas,canvas_text,chrome}.rs` | 旧模型绑定、GPUI 绘制/缓存、输入和对话框路由 |
| JoyOffice | `crates/joydeck-{model,engine,formats,shell}` | 旧 Deck/Object、命令/会话、文件/source 保留与恢复职责 |
| JoyOffice | `docs/joydeck/41-preset-geometry.md`、`01-clean-room.md` | 已引入 DeckCraft 几何的来源及旧净室说明范围 |
| JoyOffice | `docs/joydeck/42-windows-startup-stack.md`、`43-design-notes-a11y.md` | 默认 Windows 栈及重复输入实体的已知回归 |
| JoyOffice | `docs/joyword/ARCHITECTURE.md` 的 ADR-010 | Kit/GPUI 快照、字体映射限制和 provenance |
| JoyOffice | `scripts/joydeck`、`tools/xtask`、产品工作流 | 分产品构建/检查/包装/原生验证的可复用入口 |

现有 JoyDeck 已有若干真实窗口与文件回归，但其文档仍列有排版、格式、媒体、多屏与完整平台资格化缺口。复用实现时保留这些边界，并对新应用重新验证。

## 官方资料核对

- [GPUI Kit 安装与依赖入口](https://gpui-kit.com/docs/installation/)：Kit 重导出 GPUI/组件/资源，默认特性和平台构建前提。
- [GPUI Kit 上游仓库](https://github.com/longbridge/gpui-kit)：项目与组件栈来源。
- [Windows](https://zed.dev/docs/development/windows)、[macOS](https://zed.dev/docs/development/macos)、[Linux](https://zed.dev/docs/development/linux)上游构建说明：作为原生依赖准备参考，具体以固定版本和宿主工程为准。

版本结论同时核对了本机 `gpui-kit-0.8.1` 已缓存 manifest，确认其依赖 `gpui-pre =0.3.10`。没有使用滚动文档覆盖 JoyOffice 锁文件。

## 本轮验证范围

已执行源码/manifest/锁文件读取、两侧离线 Cargo 元数据检查、工具链检查、静态命令提取、规划文档结构和引用校验。未执行源码迁移、Rust 编译/测试、新 GUI 启动或产品打包。

机器基线与命令表由本机源码生成，不包含新的应用运行结果。后续 R0–R9 必须生成各自构建和桌面验收证据。
