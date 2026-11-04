# JoyDeck R9 交接与继续开发

2026-11-03。用户已明确要求备份并用 DeckCraft/GPUI 重构替换 JoyOffice 的旧 JoyDeck。**以后只在 `D:\Coding\EditorProjects\joyoffice` 开发；原 `joydeck-new` 退出开发。不要重新导入旧目录，也不要把 R9 整合当成 UI/功能已经与 DeckCraft 或 PowerPoint 完全一致。**

当前依据依次为根 AGENTS.md、本文、[Deck 状态](../joydeck/STATUS.md)、[98 项任务](../joydeck/TASKS.md)、[UI 差距](../joydeck/UI-PARITY.md)、[329 命令范围](../joydeck/COMMANDS.csv) 与 [R9 验证](../joydeck/evidence/R9/README.md)。旧 M0–M14 状态见 [替换前状态](../joydeck/STATUS-pre-r9.md)，仅作历史基线。

[PR #120](https://github.com/willmove/joyoffice/pull/120) 已按用户明确授权于 2026-11-03 合入 main，合并提交 `3691d4dea11df473c95f94af9718cc699cd984f8`，合入前 21 项检查全部通过。源仓库 9 个提交继续保留为宿主祖先。后续从最新 main 开切片，不重新迁移。[PR #121](https://github.com/willmove/joyoffice/pull/121) 已合入 main `6689c6e`；R9-UI1 是历史切片，见 [R9-UI1 验证](../joydeck/evidence/R9-UI1/README.md)。当前分支与 R9-UI2 证据见下节。

R9 整合基线联合回归为 3,573 通过、0 失败、1 项原计时测试忽略，Deck check 为 873 通过；R9-UI1 最新检查为 **联合 3,575 通过、0 失败、1 忽略，Deck 875 通过**。新 Release 包、Word/Sheet 开发包和旧软件回退包均已实际构建，且在仓库外中文/空格路径运行。多平台 PR 检查和 JD-906 完整旧功能验收以任务表/实时 PR 为准，当前工程替换不代表整个 R9 已关闭。

## R9-UI2 当前接续

PR #121 已按用户要求合入 main `6689c6e`。PR #119 已整理为 QEMU 子进程 runner 修复，并在 20 项 CI 全绿后合入 `8002f27`；没有回退 main 实验锁或升级 registry。当前任务分支为 `codex/joydeck-r9-ui-compat`，非草稿 [PR #122](https://github.com/willmove/joyoffice/pull/122) 已提交，CI 见 PR 实时状态，开发、自动检查、实际包和剩余差距统一见 [R9-UI2 范围账本](../joydeck/evidence/R9-UI2/README.md)。本轮已推进混合格式／段落表格、日常工作区、旧审阅媒体、比较接受、旧参数严格子集和 opt-in 桌面 stdio；完整旧 16 领域／JD-906／UI 一致仍开放。最终 Deck 包的格式保存、审阅／最近文件重开、跨窗口主题和子窗口关闭已现场验证，额外模板／UIA HWND 竞态修复含自动回归；联合 3,596 通过、Deck 894 通过、291 条来源指纹通过。Word emoji 的最终包 Windows GUI 与普通 PDF 已独立验证；UOS 输入法按用户 2026-11-03 最新安排留待之后进入该环境验证，本轮不再等待接入。下述兼容表已同步本轮新增能力；完整范围与实际验证以上述账本为准。

## 备份与恢复

本机完整备份位于 `D:\Coding\EditorProjects\a-backups\joyoffice-before-joydeck-r9-20261103-100321`。

- 原 JoyOffice 基线为 `a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d`，main 与 origin/main 一致、工作区干净。
- `repository/` 是整个目录（含 .git、支持文档及忽略文件）的副本；3,979 个文件、177,075,313 字节，源与副本复制前后逐文件 SHA-256 一致，清单为 `manifest.json`。
- `joyoffice-all.bundle` 包含全部可达 refs，SHA-256 为 `363a16ba246e59de116bb98cc3da1e8c71f5287dd565a91f962a918c4605c6bc`；已独立 clone 到 `restore-verification/`，HEAD/干净状态和 git fsck --full 通过。
- `state.json` 保留原 HEAD、分支、refs、工作区及校验结果，`RESTORE.md` 给出恢复步骤。恢复到新空目录，保留当前后续开发，不直接覆盖新工作区。
- `joydeck-import.bundle` 与 `joydeck-import-working.patch` 保留原重构仓库的历史及两份未提交证据日志；导入代码基线为 `526ebf02372ca3c21491fd9922ee84041b7608d8`，所有阶段证据已迁入本仓库。
- R8 历史开发包 `joydeck-0.8.0-r8-windows-x86_64-release-97c23f9b0a.zip` 及校验文件也位于备份根目录。它是导入前重构包，不是旧 JoyOffice 产品，也不是 R9 宿主构建。

完整备份不参与编译或运行。回退旧软件可从 bundle 克隆旧基线另建目录并使用其工程入口。v2 文稿不能直接交给旧 reader；使用保留的原件，或显式导出公共 PPTX。迁移生成的包中还保留 `migration/legacy-original.joydeck` 原始字节与迁移报告，不能静默降版覆盖用户文件。

旧软件已有可运行回退包：备份下 `rollback-runtime/joydeck-pre-r9-a973f75-windows-amd64-debug.zip`，SHA-256 `f01230240370144a0dbc58cfde40b7b357b38ec02c490be968a151ab065d7453`。它由额外 clone 的旧基线构建，未改 repository/ 备份；965 项 payload 校验、CLI 及旧桌面 20 页文稿通过。恢复源码依然按 RESTORE.md 到新空目录，不覆盖继续开发的工作区。

## 当前模块和依赖

- GUI / CLI 为唯一的 `apps/joydeck`、`apps/joydeck-cli`，应用名称均为 joydeck，版本 0.8.0。
- 唯一可编辑模型是 `joydeck-model`，命令在 `joydeck-engine`；纯格式/呈现为 `format/pptx/geom/color/text/anim/metafile/render/pdf`；`fonts` 隔离系统字体发现；`shell` 负责文稿、恢复、文件/媒体/helper、输出与自动化；`ui-gpui` 负责原生界面。
- 旧 `joydeck-formats/scene/export/platform` 已移除。`joydeck-legacy` 只是私有、只读 DTO/OOXML 转换桥，不能作为第二个编辑引擎。
- 公共 `joyoffice-ui` 和 `joyoffice-opc` 只有宿主一套实现；UI 内容没有变化，OPC 仅保留错误返回改进，接口与预算不变。
- 共用根 Cargo.toml / Cargo.lock / Rust 1.98.1、edition 2024、resolver 3；GPUI Kit 0.8.1 / pre 0.3.10、官方 Krilla 0.8.2 不变，没有 egui。新增十个 registry 包来自原重构渲染器，既有包版本/来源/checksum 无变动，见 [锁差异](../joydeck/evidence/R9/lock-delta.json)。
- Word 0.1.1、Sheet 0.1.0 和宿主内部库元数据保持原值；本次没有发布三产品新版本。产品仍使用各自的版本、标签与打包入口。

## 在这里构建和验证

Windows 默认使用 PowerShell 7：

```powershell
cd D:\Coding\EditorProjects\joyoffice
cargo fetch --locked # fresh checkout: populate the locked graph before offline gates
cargo xtask check joydeck --headless
cargo xtask check joydeck
cargo xtask run joydeck
cargo xtask package joydeck --release
python scripts/joydeck/check_sources.py
python scripts/joydeck/check_dependencies.py
python scripts/check_gates.py
python scripts/check_docs.py
```

`pwsh.exe -File scripts/joydeck/dev.ps1 run` 同样可用。工具会定位 Visual Studio 的 CMake。Windows 原生 helper 在开发运行前准备，包内已包含。包使用实际 Cargo 返回的可执行文件和原生 host target；开发包名带 R9 和源码指纹，源码在构建中改变会拒绝出包。`--release-layout` 接入宿主正式归档命名/元数据检查，但不代表已取得签名或多平台发行资格。

修改根锁、feature、公共库或工程工具后，须验证 Word/Sheet 及联合组合：

```powershell
cargo xtask check joyword
cargo xtask check joysheet
cargo clippy --workspace --locked --all-targets --features joydeck/validation,joysheet/automation -- -D warnings
cargo test --workspace --locked --features joydeck/validation,joysheet/automation
cargo test --locked -p joyword-shell --features ai --test ai_toggle
```

新 CLI 的行为回归为 scripts/joydeck/verify-r1.py、verify-r2.py、verify-r3.py、verify-r4-r5.py、verify-r6-r7.py。旧 M* 工具针对旧模型/UI，见 [工具边界](../joydeck/LEGACY-TOOLS.md)。不能仅运行应用入口的零项测试代替整个产品。

最新 Windows Release 开发包为 `dist/joydeck/joydeck-0.8.0-r9-windows-x86_64-release-8e70ec1350.zip`，SHA-256 `110db0a349561c989d8e2ea38b34ba6b3f6a090ffcf8c2d38e57acafe420f99d`，源码快照 `8e70ec13500373fe9196f85732bd1d3a4b4f4be8e30f42958c343428b0d72bfe`。R9-UI1 最终解压包的字号快捷键、Undo/Redo、字体/颜色控件、原生 PPTX 另存/重启重开、独立读回及 R1–R7/Control/MCP 验证均通过，见 [切片记录](../joydeck/evidence/R9-UI1/README.md)。本机 Deck 875 通过、三产品联合 3,575 通过/0 失败/1 忽略。原 R9 dd8add7cf4 包与全部历史证据保留。

本轮还修复了旧公共 PPTX 的自定义路径：沿根 officeDocument 关系定位并保留 `slideset/main.xml` 等部件、处理合法无 .rels/未变母版的最简文稿、克隆时重定位内部关系。原宿主五个公共/PowerPoint 文稿的读取和首页渲染通过，native-20 原始附件逐字节保留。完整命令与旧媒体/审阅语义仍按下面缺项继续。

Word CI 还暴露出原基线也存在的两份独立实验锁漂移，已仅补齐 RTF/textfmt 路径包，未改 registry pins；恢复与 DOCX 健壮性实验的测试、fmt/Clippy、固定语料和 200 次变异验证通过，见 R9 记录。该修复不改变当前 Deck 包构建输入。

## 旧文件与自动化边界

| 旧行为 | 当前接入与未完成项 |
| --- | --- |
| 旧 .joydeck v1 | 只读识别/转换；显式 `migrate OLD --output NEW.joydeck`，保留原件与原始 ID/报告；未知内容不伪装为完整可编辑 |
| PPTX | 公共读取、基础编辑、保存重开，preserve/regenerate 交换与输出；长尾 OOXML/复杂主题、组、媒体/动画仍有保真差项 |
| 控制 envelope | `joydeck-control-1` 名称保留，但命令 schema 是 2；engine.commands 明确返回 commandSchemaVersion=2、modelSchema=joydeck-v2、legacyCommandSchemaSupported=false。旧 schema 1 包装仍拒绝；R9-UI2 增加严格字符串命令子集和 legacyCommandSubset 能力列表，未支持参数原子拒绝 |
| 旧 page.add/copy/delete | R9-UI2 严格映射旧 pages 参数，新增排序／隐藏／尺寸；保留 schema 2 命令，不宣称完整旧 schema |
| 旧 document.undo/redo | R9-UI1 恢复同名方法，真实历史/重放/过期修订检查；仍须 target 与顶层 expectedRevision；command.execute 的 undo/redo 同样可用 |
| 旧 document.export | 新 export.start + jobs.get/cancel；输出只允许新路径。静态 MP4 使用 Windows 或 macOS 本机编码器，macOS ARM64 已从移位 Release 包导出并独立解码。嵌入媒体探测与播放的 macOS 移植继续开放。[本机证据](validation/20261105-macos-video/README.md) |
| command.validate | R9-UI1 恢复：params 接受 schema 2 或已广告的旧字符串子集；快照执行真实引擎与预算/历史检查，返回 valid/wouldChange/sideEffects=false；不修改活文稿，完整旧 16 领域未实现 |
| 旧 document.check/compare/acceptObject 等 | R9-UI2 恢复实际兼容报告、按页序比较与双摘要保护的基础对象接受／单步撤销；复杂组／媒体／跨页匹配仍缺 |
| 旧桌面远程绑定/多窗口 | CLI/MCP 默认独立 headless；R9-UI2 显式 --control-stdio=AUTHORIZED_ROOT 绑定该窗口唯一 Editor，attachedDesktop=true，限制修订／重放／组合输入／拖动／保存和文稿代次；新窗口独立生命周期。桌面 MCP 分发与完整旧 16 领域仍缺 |
| 旧偏好、最近文件、比较接受/图片依赖、完整设计与审阅 | 新增有界最近文件、原创模板、基础比较接受、旧批注／回复／解决／媒体／链接／放映配置迁移及审阅 GUI；完整设计／母版／偏好与复杂内容仍缺，不能沿用旧 done |

使用机器接口时先查询能力，不只看 envelope 字符串。新旧未覆盖功能属于实际产品差距，不是仅等待硬件验收。完整“旧功能无回退”验收（JD-906）仍开放，不能因导入和构建成功而关闭。

## 后续任务及建议顺序

1. **最高优先：继续 UI 与日常编辑对齐。** 用户的 DeckCraft/PowerPoint 完整一致目标仍未实现。按 JD-208、JD-301–308、JD-501–508、JD-804，继续原生字体/颜色选择器、完整 Ribbon/图库/表格控件、旋转与多选、最近文件/模板、多窗口、母版/审阅等。主参照仍为本机 DeckCraft，PowerPoint 辅助核对中文流程。每个切片都构建并实际操作窗口。
2. **旧功能与自动化缺项：** JD-506、JD-706、JD-906，恢复比较/逐对象接受、原有设计/审阅行为与旧命令契约；先补有语义的回归和能力协商，再接 UI/MCP，不能建立第二个长期可编辑模型。
3. **渲染与性能：** JD-408、JD-806，固定真实 release 文稿/字体/屏幕条件，记录实际呈现、缓存、RSS/显存与 p95/p99；不能用命令耗时代替显示延迟。
4. **平台资格：** JD-800–804。macOS ARM64 普通 Metal Release、移位启动、首屏和 PPTX 键鼠编辑往返已验证。PPTX 系统冷启动和运行中打开、文件／编辑菜单、原生打开与另存为面板的取消及确认均有实机证据。继续核对扩展名切换、Linux/ARM、原生 IME、读屏、DPI、设备与多屏。信创真机按条件补验。[原生对话框补验](validation/20261105-macos-desktop/README.md#原生文件对话框)
5. **分发：** JD-807/808，正式安装/关联/升级回退、签名/公证、缺失许可全文与完整 SBOM 再分发审阅。macOS ARM64 开发包按实际 SPDX 索引核对，补齐 8 个版本后仍有 5 个缺口。[复核与包指纹](validation/20261105-macos-video/README.md#开发包许可复核)

R4–R8 未完成项和 329 条命令的原始范围继续保留。任务状态、实际测试数、包指纹、PR/CI 状态以 [R9 验证](../joydeck/evidence/R9/README.md) 为准，不重复使用 R8/旧 M* 包作新验证。

## 新会话接续提示词

> 请在 D:\Coding\EditorProjects\joyoffice 中继续 JoyDeck。先读 AGENTS.md、docs/joyoffice/JOYDECK-R9-HANDOFF.md、docs/joydeck/STATUS.md、TASKS.md、UI-PARITY.md，检查分支与工作区。从最高优先的 UI/功能差距选一个可构建、可实际操作的切片实施，并保留未完成项。依赖与公共库保持宿主一致，修改共享内容时运行三产品与联合回归。不要回到 joydeck-new 或重新导入旧快照。
