# Joysheet

基于 **Rust + GPUI** 的表格桌面应用。保留 GridCraft 0.12.1 的计算、模型、文件和命令引擎，独立构建，实际依赖图不包含 egui/eframe。

当前版本为 **0.1.0 / M8-preview6**。已可编辑中文与公式、复制粘贴、撤销重做、保存和重新打开 XLSX，并提供数据工作流、绘图打印、透视分析/求解、审阅保护、脚本信任与桌面控制。全量 M0–M8 迁移仍在进行，详见 [任务矩阵](docs/MIGRATION.md) 和 [验证记录](docs/VALIDATION.md)。

`dist/joysheet-0.1.0-M8-preview6-source.zip` 保存本次源码快照，`docs/source-checkpoint.json` 记录文件 SHA256。后续修改源码后，可运行 `pwsh -File scripts/checkpoint.ps1` 更新快照，再运行 `cargo xtask dist` 同步产出源码包。

## 直接试用 Windows 测试包

本轮调整为常见电子表格功能区，并优化大量自定义尺寸、密集公式和动态数组；新旧分段采样与交付边界见 [界面与性能专项](docs/UI-PERFORMANCE.md)。既有输入法、DPI、UIA 和打印边界及人工步骤见 [Windows 质量交付](docs/WINDOWS-QUALITY.md)。双击 `Open Windows quality.cmd` 打开新样例。此前已补齐复杂格式、窗口排列和多对象操作。解压后双击 `Open complex formatting.cmd` 或 `Open Windows boundaries.cmd`；详见 [Windows 交付与试用](docs/WINDOWS.md)。

M7 高级样例：解压后双击 `Open advanced workflow.cmd`。通过“分析与透视”“审阅”“动作与脚本”“桌面控制”使用新功能；脚本重新打开后默认禁用，运行时先审核。详见 [M7 交付记录](docs/M7.md)。

M5 数据样例：解压后双击 `Open data workflow.cmd`，或运行 `joysheet.exe docs/samples/M5-workflow.xlsx`。数据工作流与已知边界见 [M5 交付记录](docs/M5.md)。

M8 新增设置、跨表公式引用、标签拖放、尺寸调整、验证确认和崩溃恢复；Windows 安装器与分发说明见 [M8 交付记录](docs/M8.md)、[打包说明](docs/PACKAGING.md)。

本轮 M4 Alpha 的分项实现、原生回归及限制见 [M4 交付记录](docs/M4.md)。完整新增回归：`cargo xtask smoke --stage M4`；指定程序目录测试：`pwsh -File scripts/desktop-smoke.ps1 -M4Task 4 -BinaryDirectory dist/joysheet-0.1.0-M8-preview6-windows-x86_64`。

`dist/joysheet-0.1.0-M8-preview6-windows-x86_64.zip` 解压后运行 `joysheet.exe`；双击 `Open sales sample.cmd` 可打开销售样例。无需 Rust 或原项目目录；包内附带 VS 的 app-local CRT；系统打印对话框另需 PowerShell 7（不捆绑），Windows PDF 预览使用内置系统渲染器。也可运行同目录的 `joysheet-0.1.0-M8-preview6-windows-x86_64-setup.exe`，安装到当前用户目录。当前包未签名。

```powershell
.\joysheet.exe --sample sales
.\joysheet.exe "C:\Data\report.xlsx"
.\joysheet-cli.exe eval "=SUM(1,2,3)"
```

演示：新建工作簿，在 A1 输入中文，在 B1 输入 `10`，在 C1 输入 `=B1*2`；修改 B1、按 Ctrl+Z/Ctrl+Y，观察 C1；保存为 XLSX 后重开。名称框可定位 `T1000000`。Ctrl+K 打开命令中心，常用格式提供表单，复杂操作仍使用高级参数。

## 开发与构建

固定 Rust **1.98.1**、GPUI Kit **=0.8.1**、GPUI 上游快照 **0.3.10**。Windows 使用 PowerShell 7、Visual Studio C++ 工具、Windows SDK 和 CMake。普通终端建议通过 `scripts/dev.ps1` 初始化 MSVC、FXC 和 VS 自带 CMake。

```powershell
pwsh -File scripts/check-environment.ps1
pwsh -File scripts/dev.ps1 run --locked -p joysheet -- --sample sales
pwsh -File scripts/dev.ps1 build --release --locked -p joysheet -p joysheet-cli
pwsh -File scripts/dev.ps1 xtask ci
pwsh -File scripts/dev.ps1 xtask smoke --stage M8
pwsh -File scripts/dev.ps1 xtask dist
```

在已经配置好的开发终端或 macOS/Linux 环境中，可直接使用统一入口：

```text
cargo run --locked -p joysheet -- --sample sales
cargo xtask ci
cargo xtask smoke --stage M8
cargo xtask dist
cargo xtask inventory
cargo xtask bench
cargo xtask wasm
```

`ci` 包含格式检查、全工作区严格 Clippy、所有测试及依赖分层检查。`smoke M0..M8` 在 Windows 构建自动化版本并运行实际 GPUI 桌面；`--engine-only` 仅运行核心文件往返检查，适合无图形会话的 CI。M8 增量包含恢复重启、设置、标签拖放和验证确认；完整新增回归入口为 `cargo xtask smoke --stage M8`。合成事件及 GPU 截图不代替系统输入法、读屏、跨平台原生验收。

`dist` 构建带截图能力的 Release 测试包；自动化控制只在显式传入 `--control PORT` 后启用。普通桌面构建默认没有 GPU 截图功能。`--diagnostics` 显示 CPU 绘制和命令计时。

```powershell
pwsh -File scripts/dev.ps1 build --locked -p joysheet -p joysheet-cli --features joysheet/automation
pwsh -File scripts/desktop-smoke.ps1 -Stage M3
pwsh -File scripts/desktop-smoke.ps1 -BinaryDirectory dist/joysheet-0.1.0-M8-preview6-windows-x86_64 -Performance
```

Linux 构建依赖见 `.github/workflows/ci.yml`，运行需要图形会话和合适的 Vulkan 驱动。macOS/Linux 配置尚未取得实际构建和运行证据。WASM 检查仅覆盖可移植核心。

## 项目记录

- [实施状态](docs/STATUS.md)、[逐任务矩阵](docs/MIGRATION.md)、[验证记录](docs/VALIDATION.md)
- [模块与数据流](docs/ARCHITECTURE.md)、[控制接口](docs/CONTROL.md)
- [355 条实际注册命令](docs/commands.json)、[原始来源哈希](docs/upstream-source.json)、[必要核心适配](docs/UPSTREAM-PATCHES.md)
- [来源与许可证](ATTRIBUTION.md)

M5 数据工作流交付与限制见 [docs/M5.md](docs/M5.md)。完整入口：cargo xtask smoke --stage M5。

M6 绘图与打印：双击 Open drawing workflow.cmd 打开样例；使用「插入与绘图」插入对象，点击后拖动，右下角拖动缩放；在对象属性中调整尺寸。通过「页面与打印」设置页面并预览、导出 PDF。完整回归入口：cargo xtask smoke --stage M6。详见 [M6 记录](docs/M6.md)。Linux/macOS 的内置预览需要 Poppler（pdftoppm），打印通过系统 PDF 查看器；三端完整原生验收仍属 M8。

保存确认弹窗的鼠标穿透、焦点和失败重试修复见 [修复记录](docs/FIX-SAVE-DIALOG.md)。






Windows 编辑和大表性能专项、分段采样接口及 on/off 复测命令见 [docs/PERFORMANCE.md](docs/PERFORMANCE.md)。

测试包内双击 **Run performance benchmark.cmd** 可生成 10k/100k 数据并复测滚动、编辑和范围求和（需要 PowerShell 7，无需 Rust）；结果位于包内 artifacts/desktop/performance.json。

M8-preview5 的几何/复杂公式分段、原始数据、跨版兼容检查和剩余限制见 [docs/PERFORMANCE-GEOMETRY-CALC.md](docs/PERFORMANCE-GEOMETRY-CALC.md)。包内基准默认包含条件聚合、乘积、嵌套和依赖扇出。

本轮 M8-preview6 提供常见电子表格功能区及自定义行列尺寸、密集公式、动态数组优化；证据和剩余限制见 [UI-PERFORMANCE.md](docs/UI-PERFORMANCE.md)。包内双击 **Run dense sheet benchmark.cmd** 可独立生成并测试 10k/100k 行的三个压力场景（需 PowerShell 7，无需 Rust），原始样本写入 artifacts/desktop/stress-performance.json。
