# Windows 编辑延迟与大表专项

当前范围：Windows x64 Release；其他系统和发行签名继续跳过。CPU 绘制提交不等于 GPU 呈现或物理输入延迟。

| ID | 任务 | 结果 / 证据 |
| --- | --- | --- |
| P1 | 有界分段采样和优化前基线 | 已编译运行；应用/UI 单测通过，生产 GPUI 两轮 10k/100k × 20 基线，每场景 30 次编辑；[原始采样](evidence/P1-performance.json)、[原生检查](evidence/P1-native.json) |
| P2 | 消除滚动条逐帧全表扫描、无变化输入框重置 | 已编译运行，32 项应用/UI 单测及生产 GPUI 回归通过；按 revision 缓存已用范围，覆盖扩展/清空/撤销；[P2 采样](evidence/P2-performance.json)、[原生报告](evidence/P2-native.json) |
| P3 | 引擎完成通知和恢复写入去重 | 已编译运行；35 项应用/UI 单测与两轮原生复测通过，含实际杀进程/重启恢复公式及草稿；[P3 采样](evidence/P3-performance.json)、[原生报告](evidence/P3-native.json) |
| P4 | 全量回归、性能报告和可测试包 | 已完成：1,377 项测试通过、1 忽略；解压 Release 52 项原生串联回归、包内独立基准/恢复开关对照、安装/运行/安全卸载、打印宿主退出通过；M8-preview4 便携/安装/源码包已生成 |

P1 使用相同窗口、5 次预热、60 次滚动、30 次低依赖公式编辑。恢复副本开启。10k/100k 两轮滚动 CPU paint P95 分别 4.47/4.59 ms、8.66/8.94 ms；编辑提交到匹配工作表 CPU paint P95 分别 52.16/53.05 ms、58.32/54.31 ms。每阶段独立统计分位数，不相加推算总耗时。

10 万行滚动条重复计算 P95 约 5.8 ms；引擎完成到 UI 接收 P95 约 28 ms；一次 127.5 MB 恢复副本序列化约 200 ms、写入约 109 ms。原有 GRIDCRAFT_PROFILE 日志表明常见低依赖重算/提交通常为数毫秒，见 [P1-core.log](evidence/P1-core.log)。

`ui.perf {raw:true}` 提供预检、排队、执行、快照、发布、UI 接收、UI 同步、帧等待、几何、布局、绘制及文字/滚动条分段，含 P50/P95/P99/max 和请求/窗口/文档/版本。最多保留 512 条样本，不记录单元格内容。`engine.profile` 保留最近 512 条命令阶段；`ui.perf.recovery` 提供恢复写入阶段。绘制子阶段包含在 paint 中，非独立可累加工作量；cells 包含样式和部分文字测量，text 是正文绘制。10 秒仍未匹配的编辑过期计入 discarded。预热样本从正式测量排除。

运行：`pwsh -NoProfile -File scripts/desktop-smoke.ps1 -BinaryDirectory target/release -Stage M3 -Performance -PerformanceRepeats 2 -PerformanceRecovery on`。输出 artifacts/desktop/performance.json；切换 off 可诊断恢复线程影响。测试经 CLI 注入生产 GPUI 事件，端到端计时从编辑提交处理开始，不包含 CLI 进程启动，不测真实键盘/IME/GPU 呈现。parse/open、recalc 和 UI 阶段分开报告；重算细分可用原有环境变量 GRIDCRAFT_PROFILE=1，避免改写导入核心。

P2 单轮中间结果：10k/100k 滚动 CPU paint P95 3.70/3.78 ms，scrollbar 阶段低于 0.01 ms。编辑总 P95 55.23/59.51 ms，几何重建 P95 0.92/6.15 ms，UI 接收等待 31.26/31.67 ms。范围扫描移到 revision 变化时，未把工作量转移误报为编辑加速。

P3 实现：引擎在回复和快照发布后广播容量 1 的可合并通知，每个窗口主动接收；保留 16 ms 定时器处理控制队列/拖选/恢复。慢窗口不阻塞其他窗口，关闭订阅随下一次广播清理。恢复缓存只保存最后成功的 revision、标题、原路径和草稿，不持有 Workbook；未变化且副本存在时跳过，写入失败仍重试，草稿变化即使 revision 不变也重写，禁用/关闭清理沿用原有保护。新增通知顺序/慢订阅/恢复失败重试/草稿变化回归，35 项应用/UI 单测通过。

最终性能脚本另测 10 次 `=SUM(A1:A100000)+n` 范围聚合，并断言真实结果。该组单列，不能与 30 次低依赖编辑混合计算门槛；打开 execute 阶段包含解析和加载后的计算，不能当作纯解析耗时。内部重算/行高阶段保留原有核心日志独立佐证。工作集同时记录低依赖编辑后及聚合后，均是采样时工作集，不是进程峰值。

人工复查：解压测试包，用 Open sales sample.cmd 启动；连续输入中文、数字和公式，Tab/Enter 切换，撤销重做后保存并重开。打开性能脚本产生的 benchmark-100000.csv，拖动滚动条、Ctrl+G 定位 T100000；再定位 A1000000 验证远端稀疏区域。保持未保存状态至少 15 秒，确认键入/滚动无明显周期停顿；用同机相同窗口尺寸分别运行 recovery on/off 留下原始 JSON。物理键盘/IME、多显示器 GPU 呈现仍按 WINDOWS-QUALITY.md 人工步骤验收，不将合成事件的 CPU 分位数认定为这些门槛已通过。

同机两轮对照（Intel Core Ultra 9 285H，Windows 10.0.26300，逻辑窗口 1440×900，缩放 125%，Release；P1/P3 均开启恢复）：

| 指标 | P1 优化前 | P3 优化后 |
| --- | --- | --- |
| 10k×20 滚动 CPU paint P95 | 4.47 / 4.59 ms | 3.70 / 3.65 ms |
| 100k×20 滚动 CPU paint P95 | 8.66 / 8.94 ms | 3.61 / 4.02 ms |
| 10k×20 提交到 CPU paint P95 | 52.16 / 53.05 ms | 26.90 / 26.97 ms |
| 100k×20 提交到 CPU paint P95 | 58.32 / 54.31 ms | 31.77 / 29.20 ms |
| 100k×20 UI 接收等待 P95 | 28.26 / 28.04 ms | 2.73 / 3.32 ms |
| 100k×20 编辑样本最慢值 | 60.21 / 55.36 ms | 32.47 / 31.30 ms |

每个数对应一次独立运行，不把平均分位数当作合并分位数。P3 共 120 次正式低依赖编辑，全部关联正确文档/版本，pending/discarded 均为 0；100k 几何更新仍耗 5.70/5.97 ms P95，帧等待约 14–16 ms P95，记录为后续优化点。100k 打开引擎执行约 365–367 ms，工作集约 501–502 MiB；没有声称核心解析/重算架构发生加速。绘制约 704 个可见单元格，不随全表 200 万个单元格增长。

P3 的未变化恢复副本跳过计数达到 31，仍对变化中的数据周期写入；避免每 5 秒重写相同快照，不代表每次真实变更的 127.5 MB 序列化成本已经消失。诊断开关 on/off 及剩余大范围聚合的独立结果由最终解压包复核记录补充。

测试包自带 `Run performance benchmark.cmd` 和 scripts/desktop-smoke.ps1，无需 Rust 即可在自己的 Windows 机器复测（需要 PowerShell 7）。双击执行一轮；从包目录运行 `pwsh -NoProfile -File scripts/desktop-smoke.ps1 -BinaryDirectory . -Stage M1 -Performance -PerformanceRepeats 2 -PerformanceRecovery off` 可执行两轮关闭恢复的对照。日志、CSV、原始阶段 JSON 和 GPU 截图写入包目录 artifacts/desktop；使用独立临时配置，不覆盖现有文档。测试时窗口需要保持可见，避免同时编译或运行重负载程序。

P1/P3 同机对照和最终解压包采样均开启 GRIDCRAFT_PROFILE 核心细分日志，日志开销包含在引擎执行阶段。常规启动和包内双击基准默认不开启该日志；新脚本会在 machine.coreTracing 标明开关，比较时应保持设置相同。此前其他运行中的 159/214 ms 尖峰未在本轮基线复现，本轮数据不能证明所有系统调度/驱动或复杂工作簿的波动均已消除。

最终解压 Release 全链路报告：[Performance-release.json](evidence/Performance-release.json)，52 项串联检查通过；专项原始数据：[Performance-release-samples.json](evidence/Performance-release-samples.json)，[核心日志](evidence/Performance-release-core.log)。本次 10k/100k 的低依赖编辑 P95 分别为 24.41/35.91 ms，最慢 26.52/37.94 ms；滚动 CPU paint P95 3.76/3.67 ms。10 次范围聚合 CPU 响应 P95 26.36/56.05 ms，其中引擎执行 P95 11.14/33.24 ms。范围聚合样本较少，P95 对应本次最大值，不能据此承诺其他复杂公式时延。

完整 CI 记录：[P4-ci.json](evidence/P4-ci.json)，包括新增打包入口后的第二轮格式、严格 Clippy、1,377 项通过、1 项忽略和依赖分层。普通桌面及 WASM 核心构建检查通过；导入核心与 M8-preview3 源码 ZIP 的文件哈希比对一致。未把 Windows 的运行结果外推为 macOS/Linux 或 GPU/物理输入门槛已通过。

同一个包内 M1 入口、相同窗口和数据、相同核心细分日志设置的恢复开关单轮复查：

| 恢复 | 10k 低依赖编辑 P95 / 最慢 | 100k 低依赖编辑 P95 / 最慢 | 100k 范围聚合 P95 |
| --- | --- | --- | --- |
| 开启 | 27.16 / 29.71 ms | 31.55 / 34.27 ms | 57.81 ms |
| 关闭 | 27.60 / 29.02 ms | 34.79 / 35.47 ms | 60.36 ms |

[开启原始采样](evidence/Performance-recovery-on.json)、[关闭原始采样](evidence/Performance-recovery-off.json)、[包内独立运行报告](evidence/Performance-standalone.json)、[默认开启运行报告](evidence/Performance-standalone-on.json)。两种开关均未复现大幅尖峰；单轮差异不构成恢复开关的性能因果证明。常用低依赖 CPU 响应在这些场景低于 50 ms，范围聚合保留引擎开销，GPU 呈现/物理输入仍不在该结论范围。

安装器隔离安装、已安装程序实际运行和卸载保留用户文件：[Performance-installer.json](evidence/Performance-installer.json)、[已安装原生报告](evidence/Performance-installed-native.json)。真实打印窗口随应用退出而结束：[Performance-print-owner.json](evidence/Performance-print-owner.json)，没有提交打印队列。用户级安装器和便携包均使用同一已测二进制，SHA256 为 `df8ce6fbff0763ed7326c46c0a68641e33780b939a29fdbbca2e1619c1e4811c`。

交付：dist/joysheet-0.1.0-M8-preview4-windows-x86_64.zip、同名 -setup.exe、dist/joysheet-0.1.0-M8-preview4-source.zip。源码版本以 docs/source-checkpoint.json 的 SHA256 清单为准。macOS/Linux 运行、发行签名及 WINDOWS-QUALITY.md 的人工门槛继续保留，不宣称整个 M8 迁移完成。

2026-10-30 M8-preview5：几何块缓存和复杂重算的新增分段、同版本基线及复测见 [PERFORMANCE-GEOMETRY-CALC.md](PERFORMANCE-GEOMETRY-CALC.md)。本页 M8-preview4 的数字保留为历史数据；新专项关闭额外控制台日志，不能跨设置直接归因比较。
