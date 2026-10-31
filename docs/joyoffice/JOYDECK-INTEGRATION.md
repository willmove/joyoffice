# JoyOffice / JoyDeck 融合准备

本文件保留依赖对齐阶段的历史分析。已批准的实施范围见 [迁移计划](MONOREPO-MIGRATION.md)，实际进度和新验证见 [迁移结果](MONOREPO-RESULT.md)。当前开发从 JoyOffice 开始，以下独立目录命令不是新的开发入口。

2026-10-31。基线：JoyOffice `ef8599d629b186efc0057e2b423ea648d8b36b55`；JoyDeck 来自本机独立目录，没有 `.git`。本次完成依赖与独立目录门禁准备，尚未导入 JoyDeck 或拆分其 crate。既有阶段完成记录不因此改写。

## 本次实现

| 项目 | JoyOffice 修改前 | 两边现在使用 |
| --- | --- | --- |
| UI 入口 | 直接依赖 gpui-pre =0.3.9 | gpui-kit =0.8.1，默认 component / assets |
| GPUI 快照 | gpui-pre 家族 0.3.9 | Cargo.lock 中快照家族 0.3.10 |
| image | =0.25.10 | =0.25.9，default-features = false；产品按需启用解码格式 |
| 工具链 | Rust 1.98.1 / edition 2024 | 保持不变 |

JoyWord 经 `gpui_kit::application` 和 `init` 初始化；保留 Kit 官方源码明确支持的 `extern crate gpui_kit as gpui` 兼容别名，现有适配器无需批量改路径。`gpui-pre-platform =0.3.10` 只为原有 `dev-runtime-shaders` 转发和平台 features 保留精确声明，原因是 Kit 0.8.1 未暴露 runtime_shaders；正常程序入口使用 Kit。相关测试的 Keystroke::parse 参数适配为 `&'static str`。

根锁文件采用 Cargo 的最小必要重新解析，没有运行无约束的 `cargo update`。GPUI 的 wgpu 29 → 30、taffy 0.13 → 0.15，以及 image 0.25.9 的 moxcms/tiff/ravif 等变化来自新版本的依赖约束。recovery-journal 与 docx-robustness 两个 CI 实验通过路径依赖使用根模型，独立锁也必须同步 image；历史 GPUI 实验和评估报告保留原版本。

依据：[Kit 安装说明](https://gpui-kit.com/docs/installation/)、本机校验和对应的 gpui-kit 0.8.1 发布源码 `src/lib.rs`、Cargo manifest 和 Cargo.lock。未引入 gpui-fast。Kit、Base、Component、Component Macros、Assets 均解析为 0.8.1。

## 来源及依赖策略

现用 Zed 提交为 `08cbb88a152c3751a43e853425a70e9f631a5918`。provenance 先不带旧偏差运行，核对新差异后更新 CI 提交号与 `scripts/gpui_pre_reviewed_deviations.json`，未取消前缀来源检查。

- 20 个 Zed workspace 快照包：16 个逐文件相同，4 个包共 12 处发布偏差。原有 9 处 actions 宏、Metal 源文件副本、宏门面重写保留，实施代码仅更新来源注释。新增 3 个 wgpu 文件按内容 hash 和上游 blob 固定。
- wgpu 适配包含 adapter 参数、DeviceDescriptor 队列字段、SurfaceColorSpace、queue.present 和 fallible readback；同时 **path alpha blending 的目标因子由 One 改为 OneMinusSrcAlpha**，不是纯 API 拼写变化。Linux 驱动和像素行为未实测，后续对照应覆盖半透明路径叠加、剪裁与 GPU readback。
- `gpui-pre-reqwest 0.12.15` 不是 Zed workspace 快照，没有 gpui-pre metadata。它的 78 个非打包文件与公开 `zed-reqwest 0.12.15-zed` 发布包逐字节相同。验证器只对这个精确名称／版本登记独立参照，仍验证两份 archive 的 SHA-256、构建目标及全部非打包文件；其它名称／版本不能免检。参照包 SHA-256：`ac2d05756ff48539950c3282ad7acf3817ad3f08797c205ad1c34a2ce03b9970`，其 VCS 提交为 `c15662463bda39148ba154100dd44d3fba5873a4`。不能把它说成未经修改的官方 reqwest 0.12.15；Zed 已有网络接口及逐请求 redirect 等 fork 改动。
- Cargo.toml / Cargo.lock / Cargo.toml.orig / VCS note 属于既有打包排除范围；manifest 依赖与 feature 不由源码比对证明，由锁校验和、构建及 cargo-deny 覆盖。此检查也不等于上游实现安全审计。
- Kit 引入 `instant 0.1.13` 和 `rustls-pemfile 2.2.0` 的停止维护公告（RUSTSEC-2024-0384 / RUSTSEC-2025-0134）。没有可用安全升级，两项仅因 Kit 间接依赖登记理由，下一次 Kit 更新复查；没有忽略已知漏洞。`enum-iterator 2.3.0` 与 `enum-iterator-derive 1.5.0` 的 0BSD 采用精确包级许可例外。发布仍须生成包含新增依赖和图标许可的 notices / SBOM。

## 两边门禁如何接入

JoyOffice 的 `scripts/check_gates.py` 对 `apps/*/Cargo.toml` 和 `crates/*/Cargo.toml` 做显式 `LAYERS` 登记；normal/build 和 dev 依赖分别受 `LAYERS` / `DEV_LAYERS` 约束。未登记 crate 直接失败。它还检查文件系统白名单和生产 panic baseline。`scripts/check_docs.py` 要求 workflow 的外部 `uses:` 使用完整 40 位 SHA。

将现有 JoyDeck 直接放进 apps 后的实际预扫描结果：

- `joydeck` 未登记，架构门禁拒绝。
- 84 个文件系统／进程访问命中，分布在 build.rs、document/edit、storage、媒体／打印／控制及 GUI smoke 等文件。
- 生产扫描范围内 407 个 panic-family 命中。包括应用内 smoke / bench；这是静态扫描命中数，不是运行时发生过 panic 的数量。
- 根 `xtask` 不在当前 JoyOffice 扫描目录内，不能以此绕过工具 crate 登记。

因此，新增一条 `joydeck` LAYERS 或给整个目录加文件系统豁免并不足以融合。本次没有预登记不存在的 crate、自动接纳全部 panic baseline，或给 JoyDeck 目录加通配白名单。

JoyDeck 独立目录本次新增 `scripts/check-repository.py`，明确登记当前两个包 `joydeck` / `xtask`、禁止未经审阅的跨产品／path 依赖，要求 workspace `unsafe_code = forbid` 和完整 Action SHA；CI 的独立 repository 作业执行该检查及反例测试，native 作业依赖它。它是导入前检查，**不能替代 JoyOffice 的分层、文件系统及 panic 检查**。工具使用 Python 3.11+，CI 固定 Python 3.12。导入后迁入统一门禁，删除重复登记源。

现有 Action 主版本保持不变，通过 `git ls-remote` 解析实际提交（annotated tag 取解引用提交）：

| Action | 固定提交 |
| --- | --- |
| checkout v4 | `11d5960a326750d5838078e36cf38b85af677262` |
| rust-cache v2 | `d1527c8f67ef9af357290475d4af4b0fab8f12e0` |
| upload-artifact v4 | `ea165f8d65b6e75b540449e92b4886f43607fa02` |

`dtolnay/rust-toolchain@master` 替换为固定 Rust 1.98.1 的 rustup 命令，与 JoyOffice 相同。新增 setup-python 沿用 JoyOffice 已固定的完整 SHA。

## 建议的融合顺序

建议沿目标架构拆分，先完成 workspace 和可运行入口的融合，再用有行为回归的小步收敛领域边界。初期保留 JoyDeck 的产品模型和布局，跨产品优先共享格式中立基础设施。

1. **导入前整理**：在 JoyDeck 内将完整无界面 library 提取为一个有实际实现的过渡 `joydeck-core`；GUI 留在 `apps/joydeck`，CLI 单列 `apps/joydeck-cli`。先保持现有 module 名称和重导出，122 项既有测试继续验证行为。把 `_smoke` / bench 从默认生产路径转入明确的工具 feature 或独立验证工具；即使有 feature，仍须审阅并登记其 IO，不用文件名规避检查。
2. **一份 workspace / lock / 工具链**：将上述实际包加入根 members；default-members 保持无 GUI 核心与 CLI。直接依赖集中于 workspace.dependencies，图像和 GPUI 版本沿本次结果，不复制独立 profile、嵌套 workspace 或锁文件。把 xtask 改为有产品前缀的 `joydeck-xtask`，若保留在 tools 下，门禁改为按 Cargo workspace metadata 枚举全部成员并为工具声明明确边界；同步审阅层表、逐文件 IO 和可追踪 panic 修复／遗留项。根 CI 组合两个产品的 headless 和桌面检查，GUI 与 CLI 功能集显式验证。
3. **先断模型循环，再拆核心 crate**：当前 `document -> pptx -> document`、`document -> preview -> edit -> document`、`table -> edit -> table` 均有源码证据。先把类型／Serde schema／不变量从文件读取、编辑命令、排版与导出代码分开。目标边界见下表；过渡 core 随边界迁出逐步消失，不作为永久杂物层。
4. **共享格式能力**：先对齐 OPC 的预算、原始成员与关系保留契约，再逐切片替换 JoyDeck 自有 ZIP / OPC 代码。已有 `joyoffice-opc` / DrawingML / OMML / CFB 均为格式中立层，不能让它们反向依赖 Word 或 Deck 模型。跨产品通用 shell / fonts / UI 只在两个真实调用方契约稳定后抽取。

| 目标 crate | 从现有模块迁移的实际内容 | 主要依赖与约束 |
| --- | --- | --- |
| joydeck-model | document 结构，design/table/chart/diagram/motion/review/resource 的纯数据部分 | 无 GPUI、文件、进程；不引用 engine/formats/scene |
| joydeck-engine | edit、design_commands、geometry 等命令／历史部分 | model；文件写入和恢复调度移交 shell |
| joydeck-formats | document 的字节读取、pptx/pptx_write、各 *_package、interop | model + joyoffice-opc；内存字节输入输出，未知 XML 保留或明确拒绝 |
| joydeck-scene | text 的布局部分、preview、scene、motion 采样 | 只读 model；统一场景，屏幕／PDF 共用，不修改文档 |
| joydeck-export | PDF／图像／视频内容生成 | scene/model；输出字节／计划，落盘和进程在 shell/platform |
| joydeck-shell | storage、workspace、control/automation、preferences、媒体／打印服务及 jobs | engine/formats/scene/export；逐文件 IO 登记，无 GPUI 类型 |
| apps/joydeck / joydeck-cli | viewer/editor/chrome/slideshow 等 UI，CLI 协议和入口 | 组合上层服务；UI 用 Kit，CLI 不引入 GPUI |

`joyword-model` 和 `joydeck-model` 不合并：流式文字与固定画布具有不同不变量。文字整形、字体身份和几何共享另作真实 API 设计，不把 Word 分页器套到演示文稿，也不重做两个产品核心。若首轮就要求完成全部目标 crate，模型循环和副作用迁移会扩大首个导入变更；过渡 core 的价值是保持每一步可运行、可回归，同时让剩余分层债务明确可见。

## 验证记录

本机 macOS ARM64、Rust 1.98.1。可复核的命令、计数、输入与本地日志 SHA-256 见[自动验证记录](validation/20261031-dependency-alignment.json)；原始日志在各工作目录 `target/`，未提交。

| 自动验证 | 结果 |
| --- | --- |
| JoyOffice `cargo test --locked`（默认成员） | 1180 项通过 |
| JoyWord `cargo test -p joyword --locked --offline` | 92 项通过 |
| workspace fmt / clippy all-targets `-D warnings` / 正常默认功能 build | 通过，未使用 dev-runtime-shaders |
| check_docs / check_gates / 门禁反例 | 通过；反例按预期被拒绝 |
| Python 工具 unittest | 121 项通过，含新增改名包来源、源码修改／缺失／新增、参照 checksum、未知来源及打包目标反例 |
| cargo-deny all-features | advisories / bans / licenses / sources 全部通过 |
| 两边锁文件 provenance | 各 21 包通过，见前文分类 |
| recovery-journal / docx-robustness 独立锁实验 | 15 / 3 项通过，两边 fmt 和 clippy 通过 |
| JoyDeck `cargo xtask check` | fmt / clippy / 122 项 Rust 测试通过 |
| JoyDeck 无默认功能测试 | 119 项通过 |
| JoyDeck repository 检查与 Python 反例测试 | 通过，4 项测试 |

另执行 JoyWord 本机图形 smoke：正常构建产物启动、1100×882 窗口与原有示例页绘制成功，注入键盘文字后画面可见；截图保留于本地 `target/dependency-alignment-gui*.png`。系统拼音当时启用，尾词仍显示组合态；未将这次注入冒充完整 IME 提交／撤销验收。仅终止本次启动的进程，没有保存或覆盖文稿。

未执行远端 CI、Windows/Linux GUI、信创真机、完整 IME／打印／无障碍或 wgpu 新 alpha 混合的像素对照。上游 `block 0.1.6` 仍有 Rust future-incompat 提示，未顺带升级。统一 workspace 与目标 crate 拆分仍是下一步实施事项。
