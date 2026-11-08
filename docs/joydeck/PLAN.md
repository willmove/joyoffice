# JoyDeck 重构方案

2026-11-03 R9 更新：本计划的独立目录实现已整合到 JoyOffice，原 joydeck-new 退出开发。当前执行依据为 [R9 交接](../joyoffice/JOYDECK-R9-HANDOFF.md)、[状态](STATUS.md) 和 [任务](TASKS.md)；下文保留原设计与验收要求，不应重新从旧目录导入。

日期：2026-11-02。规划基线为本机 DeckCraft 源码快照及 JoyOffice `a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d`。来源和检查范围见 [BASELINE](BASELINE.md)。

JoyDeck 采用 Rust + GPUI Kit，复用 DeckCraft 的演示文稿能力及 JoyOffice 的公共实现，最终在同一个 Cargo workspace 中与 JoyWord、JoySheet 共存。实施采用增量迁移，每个阶段保持桌面应用可构建、可启动、可体验，并保留前一阶段的开发包。

## 核心结论

DeckCraft 已经是 edition 2024 的 Rust workspace。本次工作的重点是 **迁移界面、调整工程边界、接入 GPUI 绘制、保持数据与功能兼容**。文稿模型、编辑事务、PPTX、几何、文字、动画和无界面能力可以作为主要迁移来源。

推荐以 DeckCraft 的 `Presentation` 模型和命令体系为起点，补入现有 JoyDeck 的文件保护、原始包保留、会话与平台契约。JoyOffice 现有 JoyDeck 的 `Deck/Object` 与该模型不同，不能让两个可编辑模型长期并行。旧模型只用于受限的旧文件读取和一次性转换。

GPUI 外壳替换与画布提速分开验收。DeckCraft 的幻灯片目前通过 `vello_cpu` 光栅化，再作为纹理展示，其桌面入口已使用 eframe/wgpu。只换成 GPUI 图像控件，仍保留了主要 CPU 工作。先沿用 CPU 渲染建立正确性与可用性，再逐类接入 GPUI 路径、图像、字形和增量缓存；性能收益由相同场景的实际测量确认。

用户指定的 GPUI 路线和 DeckCraft 重构来源，是本次规划的边界。后续导入时同步更新原有“仅 egui”和 JoyDeck“仅预设几何例外”等旧说明，准确登记新导入代码，不再把这些导入部分描述为独立净室实现。

## 当前项目基础

| 项目 | 已确认的基础 | 对重构的影响 |
| --- | --- | --- |
| DeckCraft | 本机 Cargo 元数据有 28 个成员：24 个库、3 个应用和 xtask | 复用核心层，重新组织桌面入口与工程工具 |
| DeckCraft UI | `crates/ui-egui` 有 172 个 Rust 文件，45,553 行；包含测试、注释与空行 | 需要按功能切片迁移，不能视为一次依赖替换 |
| DeckCraft 引擎 | 静态提取 329 个不同命令 ID；`Session::execute`、pointer、key 构成主要入口 | 保持命令语义，使用同一引擎服务 UI、CLI 和自动化 |
| DeckCraft 平台边界 | engine 实际依赖渲染、格式与导出，并包含文件 IO、恢复等服务 | 先保持行为，再逐步拆到 shell/platform，避免复制后违反 JoyOffice 分层 |
| JoyOffice | 当前主 workspace 有 50 个成员，统一工具链、锁文件与产品工程工具 | 当前独立开发也使用未来 Monorepo 的目录与依赖规则 |
| 现有 JoyDeck | 已有 GPUI 工作区、Ribbon、属性面板、文字输入、Scene 缓存、放映、存储、CLI 和脚本 | 优先提取可复用实现，按新模型适配 |
| 公共 UI | `joyoffice-ui` 已有 Deck 品牌、明暗色板、尺寸、控件、卡片、字段与图标封装 | R0 即复用，保持三产品视觉一致 |

行数用于识别迁移规模，不代表功能完成度。329 条命令是源码宏的静态提取结果，实际注册、可执行性和 GUI 覆盖在实施时验证。

## 复用与重写范围

| 来源 | 具体路径或能力 | 处理方式 |
| --- | --- | --- |
| JoyOffice 公共 UI | `crates/joyoffice-ui/src/lib.rs` | 保留公共实现，直接使用 `Brand::Deck`、Palette、metrics 和控件封装 |
| 现有 JoyDeck 工作区 | `apps/joydeck/src/{header,ribbon,ribbon_menus,workspace_layout,workspace_ui,inspector}.rs` | 复用布局和交互组织；命令与数据绑定改为新快照/适配器 |
| 现有 JoyDeck 场景画布 | `apps/joydeck/src/scene_canvas.rs` | 复用路径/图像缓存、淘汰、纹理释放与统计方法，重新接入新场景；不声称可原样编译 |
| 现有 JoyDeck 输入 | `canvas_text.rs`、`table_ui.rs`、输入与焦点路由 | 复用 `EntityInputHandler` 接入经验；字符范围、排版与撤销绑定到 DeckCraft 文字模型 |
| 现有 JoyDeck 平台服务 | `joydeck-platform`、shell 的 storage/preferences/workspace/control | 按文件审阅后复用原子保存、指纹、恢复、配置、会话及协议契约 |
| 现有 JoyDeck 放映 | `slideshow.rs`、show/playback 模块 | 复用窗口与平台行为，通过新的时间轴和放映状态快照适配 |
| JoyOffice 格式库 | `joyoffice-opc`，以及 DrawingML 的适用子集 | OPC 优先接入包预算与结构检查；DrawingML 比较真实 API/语义后逐项接入 |
| 现有工程工具 | `tools/xtask`、`scripts/joydeck`、产品 CI、provenance 检查 | 复用命令形状和产品选择，修正路径与已变化的行为断言 |
| 现有测试与资料 | fixtures、Windows 栈/a11y 回归、保存故障、IME、原生窗口脚本 | 保留输入与可观察断言；针对新模型重建绑定，重新执行 |
| DeckCraft 核心 | model、geom、color、text、fonts、anim、PPTX、format、render、engine、mcp | 按依赖顺序迁移、重命名并保留来源；逐步符合目标分层 |
| DeckCraft 媒体 | media、容器和编解码器、固定 craft-media 来源 | 隔离可选能力和运行时，避免解码阻塞 UI；后期专门验收 |
| DeckCraft egui 层 | ui-egui、eframe/winit、egui_kittest、craft-ui/带 egui 的 craft-update | 将需求、行为和测试断言迁移到 GPUI；不进入新运行时闭包 |
| DeckCraft 品牌和素材 | 商标、应用图标、字体、样例、模板 | 不批量搬运品牌资源；只引入有明确许可、确有用途的材料并保留归属 |

现有 JoyDeck 已引入 DeckCraft 的预设几何。新模型迁移时核对来源与版本，合并成一套几何实现，防止重复携带两个 vendor 目录及不同调整参数映射。

公共库可直接复用，并不代表产品相关模块可以直接拼接。例如现有 Ribbon 使用旧 `Command` 和 `Deck`，需要先通过统一的快照、命令状态和选择模型解除绑定。

## 技术基线

必须跟随 JoyOffice 的已核实基线，详细冲突与处理见 [DEPENDENCIES](DEPENDENCIES.md)：

- 工具链 `1.98.1`，edition `2024`，resolver `3`；workspace 的 `rust-version` 当前为 `1.98`。
- `gpui-kit = "=0.8.1"`，锁定 GPUI pre `0.3.10`；使用 `gpui_kit` 重导出的 API。
- `gpui-base`、`gpui-component`、`gpui-kit-assets` 在现有锁文件中均为 `0.8.1`。通过 Kit 使用，避免独立升级。
- 沿用现有默认 component/assets；不自行启用 `gpui-fast` 或切到 Zed Git 分支。
- 提交锁文件，构建使用 `--locked`，业务代码 `unsafe_code = "forbid"`，错误路径可恢复。
- Windows 脚本统一使用 `pwsh.exe`，使用 MSVC 工具链；无界面核心和 CLI 不依赖 GPUI。

[GPUI Kit 官方安装说明](https://gpui-kit.com/docs/installation/)确认 Kit 提供 GPUI、组件、基础能力和资源的统一入口。项目版本真值以 JoyOffice 的 manifest、锁文件及来源校验为准。

## 目标工程结构

```text
apps/
  joydeck/                     # 二进制入口、启动参数、平台初始化
  joydeck-cli/                 # inspect / render / convert / commands / mcp
crates/
  joydeck-model/               # 唯一可编辑文稿模型、ID、语义和不变量
  joydeck-engine/              # 内存编辑、选择、命令、历史、指针/键盘归约
  joydeck-shell/               # 会话、工作区、后台任务、保存/恢复、命令编排
  joydeck-formats/             # native / PPTX / legacy 适配与兼容报告
  joydeck-scene/               # 只读布局、样式解析、后端中立的场景
  joydeck-export/              # PDF、图像、讲义、打印内容等输出
  joydeck-platform/            # 文件系统、字体发现、剪贴板、打印、进程/设备
  joydeck-ui-gpui/             # GPUI 视图、输入适配、画布消费者、窗口状态
  joydeck-geom/                # 从上游保留的纯几何模块
  joydeck-color/               # 颜色与变换
  joydeck-fonts/               # 字体字节/face、整形、度量；发现由 platform 提供
  joydeck-text/                # 文字布局、caret/cluster 与命中
  joydeck-anim/                # 时间轴、转场与放映状态
  joydeck-render/              # CPU 参考及 fallback 消费者
  joydeck-media/               # 媒体模型与解码编排，按 feature 隔离
  joydeck-mcp/                 # 调用 shell 的无界面协议适配
  joyoffice-ui/               # 对齐快照，整合时回接宿主公共库
  joyoffice-opc/              # 对齐快照，整合时回接宿主公共库
tools/xtask/
scripts/joydeck/
tests/fixtures/joydeck/
docs/joydeck/
licenses/joydeck/
```

这是职责布局。大量 PPTX 或媒体代码可在确有独立边界时保留为 `joydeck-pptx`、`joydeck-*` 编解码库，避免为压缩 crate 数量而进行无价值合并；新增边必须登记到工程分层规则。

依赖沿以下方向组织，IO 和 GUI 不进入纯模型/编辑层：

```text
joydeck UI ──→ shell ──→ engine ──→ model
                ├────→ formats ──→ model / joyoffice-opc
                ├────→ scene ────→ model / text / fonts / geom / anim
                ├────→ export ───→ scene / formats
                └────→ platform
CLI / MCP ─────→ shell
GPUI / CPU / PDF 消费同一份场景与文字布局
```

图中省略独立的公共库与数据依赖。engine 使用纯文字/几何能力时允许相应下行依赖；不能反向依赖 shell、格式 IO、导出服务或 GPUI。公共 `joyoffice-*` 库不反向依赖产品。JoyWord/JoySheet 应用不能依赖 JoyDeck 应用，反向亦然。

### 会话与后台工作

沿用现有 JoyDeck 会话的语义契约，逐步将 DeckCraft Session 的内存状态与文件/导出服务分离。通过 shell 的单一编辑入口提交命令、安装版本化只读快照。长操作放到受限 worker；简单选择和预览避免反复复制整份文稿。

每个后台任务携带文档 ID、revision、字体环境版本和取消 token。切换、关闭或修改文稿后，陈旧解析/预览/导出结果不得覆盖新状态。保存的是明确 revision；保存期间继续编辑的内容仍保持 dirty。关闭与失败不能使未保存内容被静默丢弃。

命令 ID 尽量保持兼容。UI 的 enabled、菜单、快捷键、参数校验和撤销均来自同一命令入口；对话框只负责收集参数。旧 JoyDeck 控制协议通过适配与版本声明保留，不能只保留 DeckCraft 命令而丢失已有自动化行为。

### 文字与绘制

文档文字使用统一的字体字节 hash、face index、variation 和 cluster 数据。GPUI 控件字体可以沿用 Kit 的 UI 字体，文稿字体则必须对应文稿布局实际选择的字节与 face。不能用“同名字体”证明屏幕与 PDF 一致。

第一版使用 CPU 渲染真实内容。随后由 `joydeck-scene` 输出路径、图像、裁剪、变换、文字位置与原文映射，GPUI 与 CPU/PDF 使用同一布局。GPUI 字形接口的字体身份映射先验证；不足时采用已定位字形遮罩或轮廓显示，PDF 仍保留 Unicode 与可检索文字。

渐变、透明组、滤镜、复杂裁剪和 3D 等 GPUI 原语未覆盖的对象使用可定位的局部 CPU fallback。统计实际覆盖、光栅次数和上传字节；不将每帧整页 CPU 光栅包装为 GPU 优化。

CPU 画布回退仍需要 GPUI 窗口能够在目标系统上创建。Linux 无可用图形后端的情况需要单独诊断，不能据此宣称已具备无 GPU 桌面支持。

## 文件与旧数据策略

默认扩展名为 `.joydeck`。新模型使用新的主版本和能力声明，建议从 `schemaMajor=2` 开始；确定 MIME、minReader 与未知字段策略后再实现 writer。禁止把 DeckCraft JSON 直接改后缀当成现有 JoyDeck 文件。

| 输入 | 读取与保存策略 |
| --- | --- |
| `.deckcraft` / 上游历史格式 | 作为导入格式识别，保留原件，另存新 `.joydeck` |
| 现有 `.joydeck` schema 1.x | 由 legacy DTO 读取并迁移到唯一新模型；保留 source、资源、备注、主题、图表、动画、审阅与 ID；无法完整迁移时明确只读或拒绝 |
| `.pptx` | 按 OPC/PresentationML 导入；未知 XML、关系与部件保留；源包保留写回和明确有损的新建导出区分开 |
| 未来版本或未知必需能力 | 兼容报告说明原因，禁止编辑覆盖；可安全显示时只读 |

原始包保留不等于可以把任意新编辑写回。每类编辑要声明能否映射到源包；超出范围时拒绝保留写回，或由用户选择另存新文件。外部修改通过文件指纹检测，不静默覆盖。

PPTX 文件输入上限为 300 MiB，ZIP 预检允许 4096 成员、单成员 300 MiB、解压总量 600 MiB；其他公共包的预检仍使用单成员 8 MiB、解压总量 128 MiB，旧 `.joydeck` 迁移和新原生格式继续保持 64 MiB 文件限制。嵌入工作簿单独使用压缩 4 MiB、128 成员、解压总量 8 MiB。所有格式继续检查重复 ZIP 成员、路径、链接、压缩比和 XML 深度；大型媒体另设流式读取和资源预算。

## 分阶段可运行交付

所有阶段复用此前成果；新增能力的同时继续执行基础新建/打开/保存/撤销冒烟。平台编译从 R0 开始持续进行，R8 承担完整平台资格化。

| 阶段 | 主要工作 | 桌面上能看到的进展 | 阶段结束条件 |
| --- | --- | --- | --- |
| R0 同栈外壳 | 来源、依赖实验、workspace、公共 UI、工程命令、原生环境 | JoyDeck 窗口、品牌、Ribbon、缩略图/画布/面板布局、诊断信息 | 新源码构建并启动；固定 GPUI 版本；提供首个开发包 |
| R1 真实查看 | 核心与 CPU 渲染迁移，PPTX/原生导入、CLI、异步读取 | 打开真实文稿、页缩略图、翻页、缩放和兼容报告 | 原创样例内容可核对；取消/坏文件不破坏当前文稿；可打包运行 |
| R2 基础编辑 | 内存引擎与 shell 分离、命令适配、选择/变换、保存/恢复 | 新建页面、插入形状、拖动、撤销重做、保存后重开 | 新 `.joydeck` 格式闭环、dirty/关闭提示/故障恢复正确 |
| R3 文字与表格 | 字体/布局、IME、caret/选择、格式、表格与剪贴板 | 中英文原位输入、段落与字体设置、单元格编辑 | 系统 IME 提交/取消/撤销及文字/表格保存重开可验证 |
| R4 GPUI 场景 | 共享场景、增量更新、路径/图像/字形缓存与 fallback | 相同文稿拖动、缩放和滚动更流畅，有明确诊断数据 | CPU/GPUI 正确性对照；目标机器实际呈现性能与内存证据 |
| R5 内容与设计 | 母版/布局/主题、图片、复杂图形、图表/图示、审阅及完整面板 | 可编辑的复杂文稿、上下文 Ribbon、主题/母版预览 | 功能矩阵对应项与持久化、撤销、CPU/GPUI 对照通过 |
| R6 文件与输出 | 旧 `.joydeck` 转换、PPTX 保留写回、PDF/图像/讲义/打印 | 旧文稿可迁移；导出并独立读回；查看兼容/损失报告 | 未知内容与原件保护、公开文件交换和输出质量通过 |
| R7 放映与自动化 | 放映/演示者窗口、时间轴、媒体、CLI/MCP/control | 全屏播放、演示者视图、动画/媒体和脚本驱动 | 时间/状态一致，取消/退出可恢复；平台支持边界有证据 |
| R8 跨平台交付 | Windows/macOS/Linux、可访问性、可靠性、性能与包装 | 可搬离仓库运行的三平台包、关联打开与诊断 | 已验证平台有实际构建/运行记录；未实测项单列 |
| R9 JoyOffice 替换 | 宿主演练、共享库回接、锁文件整合、旧实现归档与联合回归 | 在 JoyOffice 直接构建新 JoyDeck；Word/Sheet 正常运行 | 单 workspace/锁文件，产品独立构建/打包和回退材料完整 |

### 实施顺序

主路径为 `R0 → R1 → R2 → R3 → R4 → R5 → R6 → R7 → R8 → R9`。R0 即做依赖、字体身份、输入与原生窗口的技术实验，避免在 R4 才发现接口障碍。

R6 的格式保护和 schema 设计从 R1/R2 提前实施；R6 完成完整迁移与交换矩阵。R8 的 CI、编译和冒烟从 R0 持续补齐。R9 的临时宿主编译演练从 R0/R1 开始，最终替换仍在验收后完成。

R0–R2 先交付可打开、可编辑、可保存的版本；R3–R4 形成可评估输入体验与性能的版本；R5–R8 完成较完整产品迁移。上游已有的功能不能因为新 GUI 暂未接入而被记为完成，逐条状态见 [COMMANDS](COMMANDS.csv)。

## 每阶段的构建与验收契约

以下统一工程入口已在 R0 建立。当前支持范围和实际验证见 [STATUS](STATUS.md)，各阶段新增能力仍按本方案验收。

```powershell
cargo build --locked -p joydeck -p joydeck-cli
cargo xtask check joydeck
cargo xtask check joydeck --headless
pwsh.exe -File scripts/joydeck/dev.ps1 run
python scripts/joydeck/package-native.py --profile release
```

核心测试不能只用 `cargo test -p joydeck` 代替。产品检查应覆盖所属库、CLI、GUI 适配器及所使用的公共库。阶段增加 GUI 功能时，还要实际启动当次构建的二进制或开发包，通过真实输入/控制路由执行场景，并查看窗口截图。

每个阶段在 `docs/joydeck/evidence/Rn/` 保存源码指纹、工具链、操作系统/架构/GPU/驱动/字体、构建与测试命令/退出码、包与样例 hash、截图和实际结果。自动测试、桌面验收和正式发布分别登记。没有执行的条件保持未验证，不沿用旧包或旧项目测试结果。

每个阶段交付一个可运行 ZIP/App/目录包，包含二进制、实际必需资源、帮助、许可/SBOM 与校验和；名称以 `joydeck-<版本>-rN-<平台>-<架构>` 为模板，并记录构建 revision。阶段包留存，能够直接运行前一版本回退。正式签名/公证资格另行验收，不妨碍未签名开发预览包的交付。

### 体验和性能验收

固定使用自建、有权使用的文稿，不用随机内容代替回归。建议 R0 固化以下语料；规模参数作为初始测试设计，可在首次实际测量后有记录地调整。

| 场景 | 初始规模 | 观察内容 |
| --- | --- | --- |
| BENCH-EDIT | 30 页，每页约 50 个文字/几何/图片对象 | 选择、拖动、连续缩放、撤销、脏区域更新 |
| BENCH-LARGE | 300 页，每页约 100 个对象，图片字节受预算约束 | 打开、取消、缩略图虚拟化、内存淘汰与再次访问 |
| BENCH-TEXT | CJK、Latin、Arabic、emoji、混排和表格的固定样例 | 字体 face、cluster、换行、caret、IME、PDF 文本 |
| BENCH-SHOW | 60 页、固定转场/动画和授权媒体，1080p/60 Hz | 时间轴、实际呈现、媒体同步、演示者窗口 |

初始性能目标：在登记的 1080p/60 Hz 目标机，热缓存基础编辑场景主线程工作 p95 ≤ 8 ms，端到端可见输入反馈 p95 ≤ 50 ms；空闲无持续全量重绘；连续编辑与开关文稿后缓存回收、内存趋于稳定。这些是待校准目标，不是当前测量结果。

同时报告 p50/p95/p99、冷/热缓存、光栅次数、上传字节、CPU/GPU/RSS 和场景规模。命令耗时、`draw` 耗时、FPS 与屏幕实际反馈分别报告。对比使用相同 release 配置、字体、文稿和显示条件；先对比新程序 CPU 与 GPUI 路径，再与可复现的上游和旧 JoyDeck 基线比较。

## 整合进 JoyOffice

当前目录采用未来目标目录布局，默认独立、自包含地构建。`joyoffice-ui` 和所需公共库使用来自固定 JoyOffice 提交的仓库内快照，并保留每个文件的指纹与本地修改清单；不依赖机器上的绝对路径，也不建立指向另一个工作目录的长期 path 依赖。

开发期间每阶段核对宿主版本漂移：GPUI/工具链变化先重新验证并同步，不做自动升级。公共 UI 的新需求尽量保持产品中立；需要宿主修改时先形成具体补丁和使用方测试，避免维护长期分叉。

最终在 JoyOffice 独立分支/临时 worktree 演练：

1. 记录整合时最新 `main` 的旧 JoyDeck 源码、CLI/协议、文件与开发包回退基线，保留已有未提交工作。
2. 用新实现替换 `apps/joydeck`、`apps/joydeck-cli` 和相关 `joydeck-*`，同步产品文档/脚本/测试；共享库以宿主为准合入必要增量。
3. 使用根 workspace、工具链和根锁文件解决依赖；保留旧文件读取器，移除新目录内独立 workspace/锁文件配置。
4. 更新产品选择、架构/IO/panic、依赖/许可、文档、provenance 门禁，以及实际新增 crate 的 CI 路由。
5. Deck 全量检查，Word/Sheet 使用方回归和 workspace 联合 feature 检查；分别构建、运行和打包三个产品。
6. 对旧 `.joydeck` 语料、现有自动化与已声明支持能力做迁移后回归。保留旧历史文档，明确新的迁移状态和来源。
7. 按仓库规则通过 PR 整合；本地工作区和临时归档不能成为合入后构建依赖。发布标签继续使用 `joydeck-v*`。

替换条件是约定迁移范围内的能力通过验收，现有 JoyDeck 已声明支持的行为无未处理回退，旧数据和用户文件有可靠保护。命令表中未接入项要有明确后续归属；不能把“可编译”当作替换完成。

## 主要风险与处理

| 风险 | 具体处理 |
| --- | --- |
| 依赖版本和来源相同名称却不同 | R0 编译实验核对版本、来源、checksum 和 feature；特别检查字体链、ZIP 和 Krilla fork |
| 模型差异导致旧功能或数据回退 | 单一新模型、legacy 转换器、旧/新程序独立读回和语义比较 |
| engine 原有服务职责违反宿主分层 | 以行为测试保护，将 IO/导出编排移到 shell/platform，不直接把上游依赖图复制为最终图 |
| UI 已迁移但画布仍 CPU 满载 | R1 保持正确性，R4 单独按局部场景和实际呈现数据验收 |
| GPUI 字体身份和输入范围不匹配 | R0 技术实验，R3 统一字节/face/cluster、UTF-16/UTF-8 范围和系统 IME |
| 已知 Windows 栈与 a11y 问题复发 | 保留默认线程栈测试，拆分大型视图函数，输入实体逐控件持有并同步草稿 |
| 图片/旧文件/未知 XML 静默丢失 | 预算、source 保留、能力声明、覆盖拒绝与明确另存路径 |
| 共享库快照产生漂移 | 来源清单与补丁表，每阶段宿主演练，最终回接宿主唯一公共库 |
| 跨平台只编译未运行 | CI 与真机证据分开；Windows/macOS/Linux 逐平台验证；信创未实测项明确登记 |
| 重构范围扩展为一次完整重写 | 分阶段交付，CPU 参考先保留，媒体/长尾依照功能矩阵迁移，不为目录整洁重写成熟算法 |

详细执行项和完成条件见 [TASKS](TASKS.md)。
