# 实际验证记录
M8-preview3 Windows 质量增量：完整 CI **1,372 通过、0 失败、1 忽略**；最终解压 Release **52 项**串联回归，以及实际 Windows UIA、原生打印取消/重试、宿主退出联动、安装运行/卸载通过。证据见 [Quality-release](evidence/Quality-release.json)，13 项具体人工步骤、DPI 模拟范围及性能波动见 [WINDOWS-QUALITY.md](WINDOWS-QUALITY.md)。未宣称系统 IME、真实跨屏 DPI、读屏语音、实体打印和 GPU/端到端性能门槛已验收。下文保留历史版本结果。

Windows W1–W4 更新（2026-10-29）：完整 `cargo xtask ci` 通过，**1,369 项通过、0 失败、1 项原有计时测试忽略**，见 [Windows-ci](evidence/Windows-ci.json)。分项构建/生产 GPUI 回归见 [WINDOWS.md](WINDOWS.md)，覆盖复杂格式、窗口排列、对象批处理、公式/焦点与冻结控件边界。WASM 核心检查通过，保留上游 MAX_READ_BYTES 条件编译警告。最终解压 Release 47 项串联回归及隔离安装运行/卸载通过，见 [Windows-release](evidence/Windows-release.json) 和 [Windows 交付](WINDOWS.md)。后文 M8-preview 及更早数据保留作历史基线。
M8 更新（2026-10-29）：完整 `cargo xtask ci` 通过，**1,360 项通过、1 项上游计时测试忽略**；普通桌面检查、WASM 核心、M8 引擎往返和稀疏几何基准通过。M8.1–M8.3 已分别编译、运行生产 GPUI 回归并保存证据。Windows 安装器隔离安装/启动/安全卸载通过，解压 Release 通过 39 项 M3–M8 串联回归，见 [M8](M8.md)。这不等同于 M8 总验收完成。

M8 Release 大表性能报告：[M8-performance](evidence/M8-performance.json)。10k/100k × 20 列的 CPU paint P95 为 4.05/7.72 ms，提交到 CPU 重绘 P95 为 41.08/45.60 ms，每帧约 704 个单元格。GPU 呈现帧耗时、物理输入延迟、真实 IME/读屏/DPI/打印队列及 macOS/Linux 尚未验证。M8 Windows 包已附带 app-local CRT，覆盖下文旧 M3 包未捆绑 CRT 的历史限制。

M7 更新：全工作区格式、严格 Clippy、依赖分层检查通过；**1,357 项测试通过、1 项忽略**。M7.1–M7.4 分项构建/原生回归均通过，覆盖真实透视字段拖放、分析求解、审阅保护、信任与取消和指定窗口 MCP 保存/截图；证据、完整包回归及限制见 [M7.md](M7.md)。后文历史阶段结果保留作为基线。

M7-alpha 最终 ZIP 解压程序通过 **34 项** M3–M7 与保存弹窗串联回归，见 [M7-release.json](evidence/M7-release.json)。普通桌面检查和 WASM 核心检查通过（WASM 保留上游常量未使用警告），原生系统打印取消/队列提交仍不计为已验证。

M5 更新：1,351 项测试通过、1 项原有计时测试跳过；分项桌面回归及完整 M4/M5 Release 包回归通过。实现、证据及边界见 [M5.md](M5.md)。

M4 更新：分项构建与原生回归、完整 CI 通过，当前交付 M4-alpha；详见 [M4.md](M4.md) 和 [M4-report.json](evidence/M4-report.json)。以下 M3 数据保留作为上一阶段基线。

日期：2026-10-28。交付：0.1.0 / M3-preview。**以下结果不代表 M0–M8 全部验收完成。**

## 环境与构建

- Windows x64，系统 build 26300，Intel Core Ultra 9 285H；本机具备 Intel Arc 140T 与 RTX 5060 Laptop GPU，未单独记录 GPUI 选用的适配器。
- Rust/Cargo 1.98.1 MSVC，PowerShell 7.6.5，Visual Studio 2026 / MSVC 14.51.36231，Windows SDK 10.0.26100.0。
- GPUI Kit =0.8.1、GPUI 0.3.10、已提交的依赖锁文件；依赖缓存完备后使用 locked/offline 构建验证。
- Debug 与 Release 桌面/CLI 均已构建启动。普通无 automation 功能的桌面也进行独立编译检查。
- Release 测试包解压至 `artifacts/package-acceptance/`，从包目录启动，完成原生回归；运行时不读取原 GridCraft 目录。
- PE 依赖检查发现 `VCRUNTIME140.dll`，测试机已安装，包内未附带；独立机器需要 Microsoft Visual C++ x64 运行库。安装器及运行库引导仍属 M8。
- macOS/Linux CI 已配置，尚未取得两端构建/运行结果；本机没有已安装的 WSL Linux 发行版。

## 检查结果

| 检查 | 结果 |
| --- | --- |
| cargo xtask ci | 通过；格式、workspace/all-targets 严格 Clippy、全工作区测试、分层检查 |
| 单元、集成和文档测试合计 | **1,347 passed，0 failed，1 ignored** |
| 原有 ignored 测试 | `recalc_with_range_formulas_scales_linearly`：原项目默认跳过的计时敏感测试 |
| 核心公式/依赖/重算/文件/图表/PDF/引擎/MCP/CLI | 原有回归保留并运行；本轮无未解决基线失败 |
| 应用层 | 命令顺序、快照不可变、目标隔离、陈旧修订、认证、超时丢弃、保存失败、文件往返、稀疏几何等通过 |
| cargo xtask smoke --stage M3 | 通过；同时运行引擎往返与实际生产 GPUI 视图场景 |
| Windows 解压后的 Release 包 | 原生 M3 测试及真实大表滚动通过 |
| 核心 WASM | engine/chart/pdf 的 wasm32-unknown-unknown 检查通过；原有 MAX_READ_BYTES 条件编译未使用警告保留 |
| layers | workspace 图无 egui/eframe；GridCraft 核心无 GPUI、平台服务或 Joysheet 上层依赖 |
| 来源差异 | 导入清单仅 io.rs 的保存错误清理和 io/tests.rs 的 Unix gate 发生变化；其余导入核心文件哈希一致 |

`artifacts/ci.log`、`smoke-M3.log`、`wasm-check.log`、`default-build.log` 保存本地完整输出。可随源码分发的精简证据位于 [evidence](evidence/)。

## 原生桌面场景

`scripts/desktop-smoke.ps1` 运行实际 GPUI 桌面，使用生产命中测试、真实输入处理器和窗口事件；截图来自 GPU 读回。

- 销售样例、真实单元格、格式、柱状图和饼图显示。
- 中文文本输入；B1 编辑、C1 公式重算；键盘撤销/重做。
- 合成 marked composition 下 Enter 不提交；结束组合后提交。**未验证系统候选框、真实拼音输入或系统 IME 事件序列。**
- 公式栏/单元格共享草稿、Esc 取消；函数 Tab 补全、范围拖选构造公式。
- 系统剪贴板 Ctrl+C/Ctrl+V 保留相对引用；改变粘贴后的引用源后结果更新。
- TSV 多单元格粘贴、字体参数表单、冻结首行与 125% 缩放。
- 100 万行稀疏视口定位、点击、返回原点、方向键及 Shift 扩展选区。
- XLSX 保存、关闭、重开；2 秒防抖自动保存；取消关闭后数据保留。
- 原生多窗口、同簿独立视图、后台草稿保留、显式目标窗口提交。
- stdio MCP initialize、实时单元格查询及原生截图。

Release 详细结果见 [report.json](evidence/report.json)，实际界面见 [销售样例](screenshots/sales.png)、[编辑往返](screenshots/roundtrip.png)、[参数表单](screenshots/format-dialog.png)。

## 性能记录与限制

以下是**解压后的 Release 包**，默认配置的 1440×900 逻辑窗口，实际 GPU 适配器未记录。数据为确定性数字 CSV，每表 20 列，循环发送 60 次滚动事件；这些是本机诊断样本，不是完整帧率认证。

| 数据规模 | 打开引擎命令总时长 | 打开到视图 | 可见单元格数 | CPU paint P95 | 进程工作集 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 10,000 × 20 | 36.64 ms | 747.00 ms | 704 | 3.31 ms | 约 430 MiB |
| 100,000 × 20 | 380.14 ms | 772.12 ms | 704 | 2.48 ms | 约 617 MiB |

原始数据：[performance.json](evidence/performance.json)。工作集为当时整个进程的采样，包含其他仍打开的测试工作簿，不能当作单表净内存。打开到视图包含 CLI 进程和窗口创建等待。引擎时长包括解析、重算和快照，尚未拆分。

另外，100 万行稀疏几何测试遍历 1,004 个视口，最多 1,100 个可见单元格，布局 P95 约 0.0015 ms；该项只测布局，不包含文字或绘制，见 [geometry-benchmark.json](evidence/geometry-benchmark.json)。原生场景另验证了 `T1000000` 的实际视口和点击。

**GPU 呈现 P95 ≤16.7 ms、端到端编辑响应 P95 ≤50 ms 尚未测量并认证。** CPU paint 只包含 worksheet 绘制命令提交，不包含 prepaint 几何重建、GPU/合成器或显示延迟。未测得与旧 egui UI 的同机性能差异，不能据这些数据宣称加速比例。

## 修复与待验收

恢复原项目 `profile.dev.opt-level=1` 后，调试版递归公式的栈预算回归通过。新增保存失败测试发现并修复了独占创建失败时误删已有 `.sctmp` 文件的问题。原生测试进一步覆盖了 Windows socket 非阻塞继承、输入法预编辑无 Change 通知、合成 Enter 的模拟 IME 处理、输入控件抢占快捷键、范围插入替换及截图帧同步。

仍需三端原生 IME、DPI、系统菜单/文件选择、外部剪贴板、辅助功能、复杂合并和冻结、格式完整度、大文件与全部旧 UI 行为回归。高级功能专用界面、对象交互、打印和安装器等详见 [逐任务矩阵](MIGRATION.md)。

## M6-alpha 补充

全工作区严格 Clippy、测试及依赖分层通过：1,353 项通过，1 项忽略；未引入 egui/eframe。M6.1–M6.4 分项构建/原生 GPUI 回归和实际 PDF 视觉检查通过。打印后端分页及原生“打印”窗口打开已验证；取消和实际队列提交未验证。新增证据、运行方式及完整边界见 [M6.md](M6.md)。
最终解压 Release 包报告：[M6-release.json](evidence/M6-release.json)，桌面系统打印入口报告：[M6-desktop-print.json](evidence/M6-desktop-print.json)。多页预览与输入焦点隔离已纳入生产界面回归。

M6-alpha-hotfix1：最终解压 Release 的 30 项原生场景及保存确认按钮回归通过，CI 1353 项通过、1 项忽略。见 [修复记录](FIX-SAVE-DIALOG.md)。







M8-preview4 Windows 性能专项：P1–P4 分项编译/运行与原始样本见 [PERFORMANCE.md](PERFORMANCE.md)；完整 CI 1,377 通过、1 忽略，解压 Release 52 项串联原生回归，包内独立基准及恢复开关对照、原生打印宿主退出、安装后运行/安全卸载通过。计时为 CPU 提交/绘制口径，未覆盖 GPU 呈现或真实 IME/物理输入；其他系统和签名继续跳过。

M8-preview5：几何与复杂重算 Q1–Q4 本轮完成；CI 1,387 通过、1 忽略，普通桌面/WASM、前版 CLI 差异、最终解压包 52 项原生回归、包内独立脚本、打印宿主退出和安装/安全卸载通过；见 [专项记录](PERFORMANCE-GEOMETRY-CALC.md)。阶段为 CPU 采样，其他系统/签名和物理输入/GPU 等人工门槛继续保留。

M8-preview6（2026-10-30）：功能区与密集大表 R1–R4 证据见 [UI-PERFORMANCE.md](UI-PERFORMANCE.md)。最终 CI 1,390 通过/1 忽略，最终解压 Release 49 项串联、六组包内压力（每组 12 次，结果与求值数断言）、实际 Windows 标签/排序按钮、跨版本完整 JSON 与 XLSX、安装后运行/保留用户文件卸载、原生打印宿主退出通过。100k 主表使用无并发构建的串行旧版复核；首次基线、10k 引擎 P95 波动和首次 F6/阶段白名单失败日志均保留。普通桌面与 WASM 核心编译通过；CPU 首帧与 GPU 读回不代表 GPU 呈现或物理输入。W7/W8 原专项报告保留历史口径，其他系统/签名及既有人工门槛不改为通过。
