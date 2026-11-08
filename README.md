# JoyOffice

[English](#english) · [简体中文](#chinese)

<a id="english"></a>

JoyOffice is a desktop office suite built with **Rust and GPUI Kit**. It brings documents, spreadsheets, and presentations into one repository through three independently versioned applications: **JoyWord**, **JoySheet**, and **JoyDeck**. Each product includes a native desktop app and a headless CLI, with shared UI, file-container infrastructure, build tooling, and quality checks.

This overview reflects the repository as of **2026-11-08**. All three products have runnable development builds. Feature compatibility, platform qualification, and release readiness are still being completed; linked status and validation records describe the scope actually verified.

## Products and current capabilities

| Product | App / CLI version | Primary files | Current development baseline |
| --- | --- | --- | --- |
| **JoyWord** | **0.1.1** | DOCX | R8 implementation integrated into JoyOffice; [handoff and compatibility](docs/joyoffice/JOYWORD-R8-HANDOFF.md) · [status](docs/joyword/r8/STATUS.md) |
| **JoySheet** | **0.1.0** | XLSX | Integrated M8-preview6 feature baseline with subsequent fixes; [developer guide](docs/joysheet/README.md) · [status](docs/joysheet/STATUS.md) |
| **JoyDeck** | **0.8.0** | PPTX, `.joydeck` v2 | R9 integration of the DeckCraft/GPUI refactor; [handoff](docs/joyoffice/JOYDECK-R9-HANDOFF.md) · [status](docs/joydeck/STATUS.md) |

These are application manifest versions, not a claim that every milestone or release gate is complete.

### JoyWord — documents

- Paginated editing with character/paragraph formatting, styles, tables, images, sections, columns, headers, footers, and page fields.
- Find and replace, outline navigation, comments, tracked changes, comparison, and mail merge.
- DOCX open/save, PNG/PDF output, explicit-font PDF/A-2b export, EPUB export, and format exchange with loss reporting. Individual formats support defined subsets.
- Shared desktop/CLI editing sessions, recovery drafts, legacy journal recovery, and authenticated access to selected active-document operations through a local control bridge and MCP.
- Insert and Layout ribbons follow the installed Word for Mac grouping, with real command menus, paragraph numeric controls, and explicit disabled states for unavailable features. See the [Insert](docs/joyword/r8/INSERT-UI.md) and [Layout](docs/joyword/r8/LAYOUT-UI.md) implementation and validation records.
- Recent work strengthens imported theme/font/color bindings, unknown XML retention, recovery and preference safety, and complex-object compatibility regression coverage.

The optional `agent` build feature provides local selection proposals with preview and undoable acceptance. Provider, credential, and broader desktop integration remain open. See [R8 validation](docs/joyword/r8/VALIDATION.md) for tested workflows and remaining conditions.

### JoySheet — spreadsheets

- A virtualized grid with frozen panes, hidden rows/columns, zoom, range selection, cell editing, and a shared formula-bar draft.
- Formula evaluation, function completion, reference highlighting, number formats, cell styles, sorting, filtering, tables, conditional formatting, and data validation.
- Charts, pictures, shapes, sparklines, pivot-table workflows, analysis/solver forms, comments, protection, and named ranges.
- XLSX workflows, additional spreadsheet readers and text/data exchange, PDF export/preview, multi-window workbook views, recovery, scripts, CLI, and MCP.
- Recent work improves edge-drag selection and formula references, external HTML clipboard values, and editing/analysis/review regressions.

Format and UI coverage varies by workflow. The [migration matrix](docs/joysheet/MIGRATION.md) distinguishes implemented features, automated checks, and remaining native acceptance work.

### JoyDeck — presentations

- A slide workspace with Ribbon controls, thumbnails, canvas selection and transforms, notes, slide browsing, zoom, and multiple views sharing one editable document.
- Text boxes, character/paragraph formatting, rotated/vertical text, Unicode/IME input, tables, images, hyperlinks, and slide/date fields.
- PPTX and native v2 save/reopen, undo/redo, recovery, basic comparison/review, and explicit migration of legacy `.joydeck` v1 files.
- Slide-show and presenter windows; PDF, image, GIF, outline, and other scoped exports; static MP4 output on verified Windows/macOS paths; CLI, JSON control, and MCP.
- Recent work strengthens hidden-content clipboard handling, embedded pasted media, footer metadata and conservative footer commands, and export/print task handling.

Full master/theme editing, complex content and media, the complete command/UI matrix, and old automation parity remain in progress. See the [task list](docs/joydeck/TASKS.md) and [UI parity record](docs/joydeck/UI-PARITY.md).

## Architecture and repository layout

The suite shares one Cargo workspace, lockfile, and toolchain while keeping each product's document model and editing engine independent. Product applications do not depend on one another. GPUI stays at the desktop boundary so CLI and core tests can run without the GUI dependency.

```text
apps/
  joyword/           joyword-cli/
  joysheet/          joysheet-cli/
  joydeck/           joydeck-cli/
crates/
  joyword-*          Document model, editing, layout, formats, rendering, shell, UI
  joysheet-*         Workbook model, formulas, calculation, formats, charts, UI
  joydeck-*          Slide model, editing, text, animation, formats, rendering, UI
  joyoffice-*        Shared UI, OPC, CFB, DrawingML, and OMML infrastructure
tools/xtask/         Product-scoped engineering commands
scripts/            Validation, compatibility, packaging, and platform tools
docs/               Architecture, product guides, tasks, and validation evidence
assets/             Application assets and sample documents
tests/fixtures/     Registered format and regression inputs
licenses/           Third-party notices, source records, and license texts
```

- **Word:** the engine owns edits; layout reads document state; screen and PDF consumers share page-scene data. Desktop and CLI use the same `Session` through `joyword-shell`.
- **Sheet:** one engine worker owns the session; UI views consume immutable snapshots through bounded queues and revision checks.
- **Deck:** `joydeck-model`/`joydeck-engine` own the editable model and commands; format/rendering modules are separated from shell IO and GPUI. `joydeck-legacy` is a private read-only migration bridge.
- **Shared code:** public modules stay product-neutral. Further format/font consolidation proceeds incrementally, with consumer regression checks.

See [module boundaries](docs/joyoffice/ARCHITECTURE.md). Engineering rules require bounded untrusted inputs, preservation or explicit refusal of unsupported content, and no silent loss of unknown XML.

## Build and run

### Prerequisites

The repository pins **Rust 1.98.1**, **edition 2024**, **GPUI Kit 0.8.1**, and **GPUI Pre 0.3.10**. Engineering commands require **Python 3.11 or newer**; set `JOYOFFICE_PYTHON` to select an interpreter if needed. Use the committed `Cargo.lock`.

| Platform | Native development requirements |
| --- | --- |
| Windows | PowerShell 7, Visual Studio C++ build tools and Windows SDK, CMake. Product wrappers help locate the Visual Studio CMake installation. |
| macOS | Xcode with the Metal compiler available through `xcrun --find metal`. Distribution builds use the normal Metal path. |
| Linux | Clang, CMake, pkg-config, font and X11/Wayland development libraries, Vulkan, OpenSSL, and ALSA development packages. GUI execution needs a working display and graphics driver. |

Exact Linux package lists are maintained in [Word CI](.github/workflows/ci.yml), [Sheet CI](.github/workflows/ci-joysheet.yml), and [Deck CI](.github/workflows/ci-joydeck.yml). Install appropriate CJK fonts for Chinese text. Deck's external HTML/RTF clipboard adapter uses `xclip` on X11 or `wl-paste` from `wl-clipboard` on Wayland; the relevant command must be on `PATH`. Sheet's macOS/Linux PDF preview uses Poppler's `pdftoppm`.

### Get the source and launch a product

```sh
git clone https://github.com/willmove/joyoffice.git
cd joyoffice
cargo fetch --locked

cargo xtask build joyword
cargo xtask build joysheet
cargo xtask build joydeck

cargo xtask run joyword -- assets/samples/mixed-media.docx
cargo xtask run joysheet -- --sample sales
cargo xtask run joydeck -- tests/fixtures/joydeck/r1-preview.pptx
```

Choose the product you need; building all three is optional. Add `--release` to a product build or run command for the optimized profile. Use the `xtask` run entry point so required Deck platform helpers are prepared.

## CLI and automation

`--headless` selects the product CLI for build/run and excludes desktop packages from product checks. It also checks that the selected normal/build dependency closure does not include GPUI.

```sh
cargo xtask build joyword --headless
cargo xtask build joysheet --headless
cargo xtask build joydeck --headless

cargo xtask run joyword --headless -- inspect --session assets/samples/mixed-media.docx
cargo xtask run joysheet --headless -- eval "SUM(1,2,3)" --json
cargo xtask run joydeck --headless -- inspect tests/fixtures/joydeck/r1-preview.pptx
```

Word supports inspection, rendering, conversion, mail merge, recovery, and MCP; Sheet exposes workbook inspection, formula evaluation, conversion, scripts, and MCP; Deck exposes inspection, rendering, editing scripts, migration, export, JSON control, and MCP.

Automation access is explicit and product-specific. Word's headless MCP requires authorized read/write roots; its desktop bridge uses an authenticated loopback connection. Deck's headless control/MCP requires an authorized root, and attaching control to a desktop view is opt-in. Query advertised capabilities and command schemas before editing: Deck's retained `joydeck-control-1` envelope uses command schema **2**, with only an advertised subset of legacy string commands. Protocol details: [Word handoff](docs/joyoffice/JOYWORD-R8-HANDOFF.md), [Sheet control](docs/joysheet/CONTROL.md), [Deck control](docs/joydeck/CONTROL.md).

## Checks and development workflow

```sh
python3 scripts/check_docs.py
python3 scripts/check_gates.py

cargo xtask check joyword
cargo xtask check joysheet
cargo xtask check joydeck
```

`check` runs product-scoped formatting, strict Clippy, and tests for the product packages and shared libraries they use. Deck additionally checks imported sources, dependencies, and refactor documentation. `cargo xtask test <product>` runs the selected tests; `cargo xtask check <product> --headless` checks the non-GUI scope. Fetch the locked dependency graph before checks that inspect it offline.

For shared-library, lockfile, resolver, or build/feature changes, run all affected products and the combined feature regression:

```sh
cargo clippy --workspace --locked --all-targets --features joydeck/validation,joysheet/automation -- -D warnings
cargo test --workspace --locked --features joydeck/validation,joysheet/automation
```

Native acceptance hooks for Deck and Sheet use the explicit `--validation` option; ordinary builds do not enable those hooks. Follow [AGENTS.md](AGENTS.md) for branch conventions, module boundaries, exact dependency pins, `unsafe_code = forbid`, and validation requirements. Develop all three products here; archived standalone directories and old milestone reports serve as historical references.

## Packaging and releases

```sh
cargo xtask package joyword --release
cargo xtask package joysheet --release
cargo xtask package joydeck --release
```

Native development packages are written under `dist/joyword/`, `dist/joysheet/`, and `dist/joydeck/`. Packaging runs on the host platform and includes product-specific binaries, resources, build/source fingerprints, and dependency/license metadata. Cross-target build/check supports `--target`; packaging is currently native-only, and headless-only packages are not implemented.

Each product owns its app/CLI version and release tags: `joyword-v*`, `joysheet-v*`, and `joydeck-v*`. Historical `v*` tags route to Word only. Products released in the same batch normally use the same version number while retaining separate metadata and tags. The [release workflow](.github/workflows/release.yml) routes by product and validates versions.

Development packages and successful CI checks do not establish formal distribution readiness. Exact-version license-text gaps, signing/notarization, installers, file associations, upgrade/rollback, and device validation remain tracked per product; strict packaging gates can refuse output while requirements are unmet.

## Platform evidence and next work

| Platform | Recorded scope and remaining work |
| --- | --- |
| Windows | Development-package and native editing checks exist for all three products. Full system IME, screen-reader, high-DPI/multi-monitor, printer/device, and installer acceptance remain scenario-specific. |
| macOS ARM64 | Release apps have been built and relocated, with file-open and editing/save/reopen evidence. Deck has additional system input and text/table checks. Signing/notarization and the remaining native matrix are open. |
| Linux and other architectures | Headless and selected desktop results are recorded separately; Sheet includes UOS 25 X11 basic GUI coverage. Current Deck Wayland/X11 desktop qualification and the complete cross-platform matrix remain open. CI/QEMU results are distinct from hardware acceptance. |

Representative records: [macOS three-product validation](docs/joyoffice/validation/20261104-macos/README.md), [native dialogs and Word/WPS round trips](docs/joyoffice/validation/20261105-macos-desktop/README.md), [recent editing/export checks](docs/joyoffice/validation/20261106-p2/README.md), and [remaining work and Linux CLI rollback](docs/joyoffice/validation/20261106-remaining-work/README.md). UOS/Kylin and other domestic-platform hardware follow-up is recorded when available and does not block ongoing development; untested hardware stays explicitly unverified.

Current priorities are Word legacy-data/format compatibility and native distribution, Deck daily-editing/UI parity and complex-content/automation compatibility, and Sheet broader interoperability, native interaction, and performance qualification. Historical Word M0–M9 and Deck M0–M14 results describe their earlier implementations and do not automatically qualify the current ones.

## Documentation

| Area | Starting points |
| --- | --- |
| Repository | [Development rules](AGENTS.md) · [Architecture](docs/joyoffice/ARCHITECTURE.md) · [Monorepo history](docs/joyoffice/MONOREPO-RESULT.md) |
| JoyWord | [R8 handoff](docs/joyoffice/JOYWORD-R8-HANDOFF.md) · [Current status](docs/joyword/r8/STATUS.md) · [Validation](docs/joyword/r8/VALIDATION.md) |
| JoySheet | [Developer guide](docs/joysheet/README.md) · [Status](docs/joysheet/STATUS.md) · [Feature matrix](docs/joysheet/MIGRATION.md) · [Integration results](docs/joyoffice/JOYSHEET-RESULT.md) |
| JoyDeck | [R9 handoff](docs/joyoffice/JOYDECK-R9-HANDOFF.md) · [Developer guide](docs/joydeck/DEVELOPMENT.md) · [Tasks](docs/joydeck/TASKS.md) · [UI parity](docs/joydeck/UI-PARITY.md) · [Command inventory](docs/joydeck/COMMANDS.csv) |

## License and attribution

The root project is licensed under [MIT](LICENSE). Imported code and assets retain their original licenses and notices: see [WordCraft source records](licenses/wordcraft/SOURCE.json), [DeckCraft attribution](licenses/deckcraft/ATTRIBUTION-upstream.md), [GridCraft/Sheet attribution](docs/joysheet/ATTRIBUTION.md), and [GPUI notices](licenses/gpui/NOTICE.md). The root license does not replace these component-specific terms. System fonts are not committed or redistributed as project assets.

---

<a id="chinese"></a>

# JoyOffice（简体中文）

[English](#english) · [简体中文](#chinese)

JoyOffice 是使用 **Rust 和 GPUI Kit** 构建的桌面 Office 套件，在同一仓库维护文档、电子表格和演示文稿三款独立版本的应用：**JoyWord**、**JoySheet** 和 **JoyDeck**。每款产品都提供原生桌面应用和无界面 CLI，共用 UI、文件容器、构建工具与工程检查等基础设施。

本文概况截至 **2026-11-08**。三款产品均有可运行的开发构建，功能兼容、平台验收和正式发行准备仍在推进；实际验证范围以链接中的状态和证据记录为准。

## 产品与当前能力

| 产品 | 应用／CLI 版本 | 主要文件 | 当前开发基线 |
| --- | --- | --- | --- |
| **JoyWord** | **0.1.1** | DOCX | 已整合到 JoyOffice 的 R8 实现；[交接与兼容表](docs/joyoffice/JOYWORD-R8-HANDOFF.md) · [状态](docs/joyword/r8/STATUS.md) |
| **JoySheet** | **0.1.0** | XLSX | 已整合的 M8-preview6 功能基线及后续修复；[开发指南](docs/joysheet/README.md) · [状态](docs/joysheet/STATUS.md) |
| **JoyDeck** | **0.8.0** | PPTX、`.joydeck` v2 | DeckCraft/GPUI 重构实现的 R9 整合；[交接](docs/joyoffice/JOYDECK-R9-HANDOFF.md) · [状态](docs/joydeck/STATUS.md) |

以上是应用 manifest 中的版本号，不表示所有里程碑或发布门槛均已完成。

### JoyWord — 文档

- 分页编辑，支持字符／段落格式、样式、表格、图片、分节、分栏、页眉页脚和页码字段。
- 查找替换、大纲导航、批注、修订、比较和邮件合并。
- DOCX 打开保存、PNG／PDF 输出、显式字体的 PDF/A-2b 导出、EPUB 导出，以及带损失报告的格式交换。各格式有明确的支持子集。
- 桌面与 CLI 共用编辑会话，提供恢复草稿、旧日志恢复，以及通过本地认证控制桥和 MCP 操作活动文档的部分能力。
- 插入和布局功能区参照本机 Word for Mac 的分组，接通真实命令菜单、段落数值控件，并明确禁用尚未提供的功能。实现与验证范围见[插入记录](docs/joyword/r8/INSERT-UI.md)和[布局记录](docs/joyword/r8/LAYOUT-UI.md)。
- 近期加强导入主题与字体／颜色绑定、未知 XML 保留、恢复与偏好安全，以及复杂对象兼容性回归覆盖。

可选 `agent` 构建特性提供本地选区提案、预览和可撤销的接受操作。Provider、凭据及更广的桌面集成仍开放。已验证流程与剩余条件见 [R8 验证](docs/joyword/r8/VALIDATION.md)。

### JoySheet — 电子表格

- 虚拟化网格，支持冻结窗格、隐藏行列、缩放、范围选择、单元格编辑和与公式栏共享的草稿。
- 公式计算、函数补全、引用高亮、数字格式、单元格样式、排序、筛选、表格、条件格式和数据验证。
- 图表、图片、形状、迷你图、透视表流程、分析／求解表单、批注、保护与名称管理。
- XLSX 工作流、其他电子表格格式读取及文本／数据交换、PDF 导出／预览、工作簿多窗口视图、恢复、脚本、CLI 和 MCP。
- 近期改进边缘拖选与公式引用、外部 HTML 剪贴板数值处理，以及编辑／分析／审阅回归。

不同工作流的格式和 UI 覆盖程度有所区别。[迁移矩阵](docs/joysheet/MIGRATION.md)分别记录已实现功能、自动检查和仍待完成的原生验收。

### JoyDeck — 演示文稿

- 包含 Ribbon、缩略图、画布选择与变换、备注、幻灯片浏览和缩放的编辑工作区，多个视图可共享同一可编辑文稿。
- 文本框、字符／段落格式、旋转／竖排文字、Unicode／输入法、表格、图片、超链接和幻灯片编号／日期字段。
- PPTX 与原生 v2 格式保存重开、撤销重做、恢复、基础比较／审阅，以及旧 `.joydeck` v1 文件的显式迁移。
- 放映和演示者窗口；PDF、图片、GIF、大纲等限定范围的导出；已验证 Windows／macOS 路径上的静态 MP4 输出；CLI、JSON control 和 MCP。
- 近期加强剪贴板隐藏内容处理、粘贴媒体内嵌、页脚元数据与保守页脚命令，以及导出／打印任务处理。

完整母版／主题编辑、复杂内容和媒体、全部命令／UI 矩阵及旧自动化等价仍在推进。详见[任务清单](docs/joydeck/TASKS.md)和[界面功能对照](docs/joydeck/UI-PARITY.md)。

## 架构与仓库布局

套件共用一个 Cargo workspace、锁文件和工具链，各产品独立维护文档模型与编辑引擎，应用之间不互相依赖。GPUI 限于桌面边界，CLI 和核心测试可以在不依赖 GUI 的情况下运行。

```text
apps/
  joyword/           joyword-cli/
  joysheet/          joysheet-cli/
  joydeck/           joydeck-cli/
crates/
  joyword-*          文档模型、编辑、布局、格式、渲染、外壳与 UI
  joysheet-*         工作簿模型、公式、计算、格式、图表与 UI
  joydeck-*          幻灯片模型、编辑、文字、动画、格式、渲染与 UI
  joyoffice-*        公共 UI、OPC、CFB、DrawingML 与 OMML 基础设施
tools/xtask/         按产品选择的工程命令
scripts/            验证、兼容性、打包和平台工具
docs/               架构、产品指南、任务与验证证据
assets/             应用资产与样例文档
tests/fixtures/     已登记的格式与回归输入
licenses/           第三方声明、来源记录与许可正文
```

- **Word：** 引擎负责编辑，布局只读文档状态；屏幕和 PDF 消费共用页面场景数据。桌面与 CLI 通过 `joyword-shell` 使用同一 `Session`。
- **Sheet：** 单一引擎 worker 持有会话，UI 通过有界队列和修订检查消费不可变快照。
- **Deck：** `joydeck-model`／`joydeck-engine` 持有可编辑模型与命令；格式／渲染模块与外壳 IO、GPUI 分离。`joydeck-legacy` 是私有只读迁移桥。
- **公共代码：** 公共模块保持产品中立，格式和字体能力继续逐步收敛，并验证实际使用方。

详见[模块边界](docs/joyoffice/ARCHITECTURE.md)。工程约定要求限制不可信输入预算，对不支持内容保留原始数据或明确拒绝，不静默丢失未知 XML。

## 构建和运行

### 环境要求

仓库固定 **Rust 1.98.1**、**edition 2024**、**GPUI Kit 0.8.1** 和 **GPUI Pre 0.3.10**。工程命令需要 **Python 3.11 或以上**，必要时可用 `JOYOFFICE_PYTHON` 指定解释器。依赖使用已提交的 `Cargo.lock`。

| 平台 | 原生开发要求 |
| --- | --- |
| Windows | PowerShell 7、Visual Studio C++ 构建工具及 Windows SDK、CMake。产品包装脚本可协助定位 Visual Studio 自带的 CMake。 |
| macOS | 安装 Xcode，并确保 `xcrun --find metal` 可找到 Metal 编译器。分发构建使用正常 Metal 路径。 |
| Linux | Clang、CMake、pkg-config，字体与 X11／Wayland 开发库，以及 Vulkan、OpenSSL、ALSA 开发包。运行 GUI 需要可用的显示与图形驱动环境。 |

准确的 Linux 包清单随 [Word CI](.github/workflows/ci.yml)、[Sheet CI](.github/workflows/ci-joysheet.yml) 和 [Deck CI](.github/workflows/ci-joydeck.yml) 维护。中文显示需安装适当的 CJK 字体。Deck 的外部 HTML／RTF 剪贴板适配在 X11 使用 `xclip`，在 Wayland 使用 `wl-clipboard` 提供的 `wl-paste`，对应命令须在 `PATH` 中。Sheet 的 macOS／Linux PDF 预览使用 Poppler 的 `pdftoppm`。

### 获取源码并启动产品

```sh
git clone https://github.com/willmove/joyoffice.git
cd joyoffice
cargo fetch --locked

cargo xtask build joyword
cargo xtask build joysheet
cargo xtask build joydeck

cargo xtask run joyword -- assets/samples/mixed-media.docx
cargo xtask run joysheet -- --sample sales
cargo xtask run joydeck -- tests/fixtures/joydeck/r1-preview.pptx
```

按需选择产品即可，无需构建全部三款。产品构建或运行命令可增加 `--release` 使用优化配置。使用 `xtask` 运行入口，以准备 Deck 所需的平台辅助程序。

## CLI 与自动化

`--headless` 在构建／运行时选择产品 CLI，在产品检查中排除桌面包，同时检查所选普通／构建依赖闭包不包含 GPUI。

```sh
cargo xtask build joyword --headless
cargo xtask build joysheet --headless
cargo xtask build joydeck --headless

cargo xtask run joyword --headless -- inspect --session assets/samples/mixed-media.docx
cargo xtask run joysheet --headless -- eval "SUM(1,2,3)" --json
cargo xtask run joydeck --headless -- inspect tests/fixtures/joydeck/r1-preview.pptx
```

Word 提供检查、渲染、转换、邮件合并、恢复和 MCP；Sheet 提供工作簿检查、公式求值、转换、脚本和 MCP；Deck 提供检查、渲染、编辑脚本、迁移、导出、JSON control 和 MCP。

自动化须显式启用，并遵循各产品边界。Word 的无界面 MCP 需要授权读写根目录，桌面桥使用认证后的本地回环连接。Deck 的无界面 control／MCP 需要授权根目录，控制桌面视图也须显式开启。编辑前先查询已声明的能力和命令 schema：Deck 保留的 `joydeck-control-1` envelope 使用命令 schema **2**，旧字符串命令仅支持已声明的子集。协议详见 [Word 交接](docs/joyoffice/JOYWORD-R8-HANDOFF.md)、[Sheet 控制](docs/joysheet/CONTROL.md)和 [Deck 控制](docs/joydeck/CONTROL.md)。

## 检查与开发流程

```sh
python3 scripts/check_docs.py
python3 scripts/check_gates.py

cargo xtask check joyword
cargo xtask check joysheet
cargo xtask check joydeck
```

`check` 对产品所属包和实际使用的公共库运行格式检查、严格 Clippy 和测试。Deck 还检查导入来源、依赖与重构文档。`cargo xtask test <product>` 运行所选测试；`cargo xtask check <product> --headless` 检查无 GUI 范围。离线检查依赖图前，先获取锁定的依赖。

修改公共库、锁文件、resolver 或构建／feature 配置时，运行所有受影响产品及联合 feature 回归：

```sh
cargo clippy --workspace --locked --all-targets --features joydeck/validation,joysheet/automation -- -D warnings
cargo test --workspace --locked --features joydeck/validation,joysheet/automation
```

Deck 和 Sheet 的原生验收入口使用显式 `--validation` 选项，普通构建不启用这些验收入口。分支约定、模块边界、依赖精确固定、`unsafe_code = forbid` 和验证要求见 [AGENTS.md](AGENTS.md)。三款产品都在本仓库开发，已归档的独立目录和旧里程碑报告用于历史查证。

## 打包与发布

```sh
cargo xtask package joyword --release
cargo xtask package joysheet --release
cargo xtask package joydeck --release
```

原生开发包分别输出到 `dist/joyword/`、`dist/joysheet/` 和 `dist/joydeck/`。打包在宿主平台执行，包含所属产品二进制、资源、构建／源码指纹及依赖／许可元数据。跨目标构建／检查可使用 `--target`；当前打包仅支持原生平台，尚未实现纯 headless 包。

每款产品独立维护应用／CLI 版本和发布标签：`joyword-v*`、`joysheet-v*`、`joydeck-v*`。历史 `v*` 标签仅映射 Word。同一批次发布的产品默认使用相同版本号，同时保留独立元数据和标签。[发布工作流](.github/workflows/release.yml)按产品路由并验证版本。

开发包和 CI 通过不代表已具备正式发行资格。精确版本的许可正文缺口、签名／公证、安装器、文件关联、升级回退及设备验收按产品跟踪；要求尚未满足时，严格打包门禁可能拒绝生成产物。

## 平台证据与后续工作

| 平台 | 已记录范围与剩余工作 |
| --- | --- |
| Windows | 三款产品均有开发包与原生编辑检查。完整系统输入法、读屏、高 DPI／多显示器、打印设备及安装验收仍按具体场景推进。 |
| macOS ARM64 | 已构建并移位运行 Release 应用，有文件打开、编辑、保存重开的证据。Deck 另有系统输入和文字／表格检查。签名／公证与其余原生矩阵仍开放。 |
| Linux 与其他架构 | 无界面和部分桌面结果分别记录；Sheet 包含 UOS 25 X11 基础 GUI 覆盖。当前 Deck 的 Wayland／X11 桌面资格化和完整跨平台矩阵仍开放。CI／QEMU 结果与硬件验收分开记录。 |

代表性记录：[macOS 三产品验证](docs/joyoffice/validation/20261104-macos/README.md)、[原生对话框与 Word／WPS 往返](docs/joyoffice/validation/20261105-macos-desktop/README.md)、[近期编辑／导出检查](docs/joyoffice/validation/20261106-p2/README.md)、[剩余工作与 Linux CLI 回退](docs/joyoffice/validation/20261106-remaining-work/README.md)。UOS／麒麟等信创真机在条件具备时补验，不阻塞持续开发；未实测硬件明确保留为未验证。

当前重点是 Word 的旧数据／格式兼容与原生分发，Deck 的日常编辑／UI 对齐及复杂内容／自动化兼容，以及 Sheet 更广的互操作、原生交互和性能验收。旧 Word M0–M9、旧 Deck M0–M14 结果描述替换前实现，不能直接作为当前实现的验收结论。

## 文档导航

| 范围 | 入口 |
| --- | --- |
| 仓库 | [开发约定](AGENTS.md) · [架构](docs/joyoffice/ARCHITECTURE.md) · [Monorepo 历史](docs/joyoffice/MONOREPO-RESULT.md) |
| JoyWord | [R8 交接](docs/joyoffice/JOYWORD-R8-HANDOFF.md) · [当前状态](docs/joyword/r8/STATUS.md) · [验证](docs/joyword/r8/VALIDATION.md) |
| JoySheet | [开发指南](docs/joysheet/README.md) · [状态](docs/joysheet/STATUS.md) · [功能矩阵](docs/joysheet/MIGRATION.md) · [整合结果](docs/joyoffice/JOYSHEET-RESULT.md) |
| JoyDeck | [R9 交接](docs/joyoffice/JOYDECK-R9-HANDOFF.md) · [开发指南](docs/joydeck/DEVELOPMENT.md) · [任务](docs/joydeck/TASKS.md) · [界面对照](docs/joydeck/UI-PARITY.md) · [命令清单](docs/joydeck/COMMANDS.csv) |

## 许可与来源

项目根目录采用 [MIT 许可](LICENSE)。导入代码和资产保留各自原始许可与声明，详见 [WordCraft 来源记录](licenses/wordcraft/SOURCE.json)、[DeckCraft 归属](licenses/deckcraft/ATTRIBUTION-upstream.md)、[GridCraft／Sheet 归属](docs/joysheet/ATTRIBUTION.md)和 [GPUI 声明](licenses/gpui/NOTICE.md)。根许可不替代这些组件自身的条款，系统字体不作为项目资产提交或再分发。
