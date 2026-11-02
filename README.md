# JoyDeck 重构规划

将 DeckCraft 的 Rust 演示文稿能力迁移为与 JoyOffice 一致的 GPUI 桌面应用，在当前目录独立推进，最终整合进 JoyOffice 仓库，替换现有 JoyDeck。

应用、二进制、Cargo package 和产品模块均使用 `joydeck` / `joydeck-*`。当前文件夹名称不进入产品命名、文件格式、配置目录或发布包名称。

建议采用 **DeckCraft 的文稿与编辑能力 + JoyOffice 的 GPUI 技术基线与公共 UI + 现有 JoyDeck 的平台和交互实现**。以可运行的纵向切片推进，每阶段交付能够实际启动的桌面开发包。

## 规划入口

| 文件 | 内容 |
| --- | --- |
| [重构方案](docs/joydeck/PLAN.md) | 源码结论、复用边界、目标架构、阶段、验收与整合方式 |
| [任务清单](docs/joydeck/TASKS.md) | 按阶段编号的实施任务、依赖与完成条件 |
| [依赖对齐](docs/joydeck/DEPENDENCIES.md) | 精确版本、feature、字体/PDF/ZIP 冲突、共享模块同步规则 |
| [基线记录](docs/joydeck/BASELINE.md) | 本机源码范围、事实依据、已执行检查与未验证范围 |
| [命令迁移清单](docs/joydeck/COMMANDS.csv) | 从 DeckCraft 源码静态提取的命令，逐条登记 UI 与无界面迁移状态 |
| [机器可读基线](docs/joydeck/evidence/planning-baseline.json) | 来源、文件指纹、Cargo 元数据摘要与锁定依赖 |

## 当前状态

2026-11-02：JoyDeck 0.4.0 / R3 文字与表格基础切片。真实 PPTX 支持画布原位文字、系统 IME 草稿/候选提交、选区粗体/斜体、表格单元格/行列/合并/底色、备注，以及 Unicode/内部富文字/TSV 剪贴板。沿用 R2 的统一命令、undo/redo、保存重开及恢复。详见 [R3 验证记录](docs/joydeck/evidence/R3/README.md) 和 [开发包说明](packaging/joydeck/README-r3.md)。

依赖仍锁定 Rust 1.98.1、GPUI Kit 0.8.1、GPUI pre 0.3.10。复用 JoyOffice 原位输入适配与 UTF-16/组合草稿行为，文字、光标和命中使用 DeckCraft 同一布局。旋转/竖排原位编辑、完整格式 UI、图片/对象剪贴板、多窗口和 GPUI 原生场景加速继续后续切片；当前边界详见验证记录，R0/R1/R2 包保留。

## 构建与体验

```powershell
cargo build --locked -p joydeck -p joydeck-cli
pwsh.exe -File scripts/joydeck/dev.ps1 run
cargo xtask check joydeck
cargo xtask check joydeck --headless
cargo xtask package joydeck --release
```

Windows 缺少 PATH 中的 CMake 时，优先使用开发脚本或 xtask，它们会定位 Visual Studio 自带的 CMake。应用默认打开真实 native 样例，可传入本地文稿路径。应用可用 `--dark` / `--light` 和 `--window-size=980x800` 指定预览窗口。`joydeck-cli diagnostics` 输出当前能力。

```powershell
.\target\debug\joydeck.exe ".\tests\fixtures\joydeck\independent.pptx"
.\target\debug\joydeck-cli.exe inspect ".\tests\fixtures\joydeck\r1-preview.deckcraft"
.\target\debug\joydeck-cli.exe render ".\tests\fixtures\joydeck\independent.pptx" --output ".\rendered" --all --scale 1
```

输出目录必须尚不存在，父目录需已存在；页码从 1 开始。详细限制见 [开发包说明](packaging/joydeck/README-r2.md)。

后续应从 [TASKS](docs/joydeck/TASKS.md) 选择任务，完成后填写新证据。JoyOffice 原有里程碑和上游自评不计入新 JoyDeck 的完成状态。


基础编辑脚本示例：

```powershell
.\target\debug\joydeck-cli.exe edit ".\tests\fixtures\joydeck\independent.pptx" --script ".\tests\fixtures\joydeck\r2-edit.json" --output ".\edited.pptx"
.\target\debug\joydeck-cli.exe inspect ".\edited.pptx"
```

编辑后也可保存为 `.joydeck` v2；旧 `.deckcraft` 仅导入。保存会先读回核对页面、顶层对象、文字和几何，超出受支持写回范围时拒绝发布，原文件保留。撤销历史为本次会话内，重开/恢复后重新建立。恢复快照包含原始文稿，应按原文件的访问范围保管。
