# JoyDeck 当前状态

2026-11-03 R9：按用户明确要求，将 0.8.0 DeckCraft/GPUI 重构整合到 JoyOffice，替换 apps/joydeck、apps/joydeck-cli 与相关产品 crate。今后仅在 JoyOffice 内继续开发，原 joydeck-new 目录退出开发。当前整合、回归、备份和剩余任务见 [R9 交接](../joyoffice/JOYDECK-R9-HANDOFF.md) 与 [R9 验证](evidence/R9/README.md)。

[PR #120](https://github.com/willmove/joyoffice/pull/120) 已在 21 项检查通过后按用户授权合入 main（3691d4d）。当前切片分支 codex/joydeck-r9-formatting-compat，字体/字号/颜色直接控件与旧方法恢复见 [R9-UI1 验证](evidence/R9-UI1/README.md)。备份/恢复及旧软件可运行包已验证；最终本机联合测试 3,573 通过、0 失败、1 忽略，Deck check 873 通过。已交付新的 R9 Windows Release 开发包，实际验证仓库外读取、编辑、Undo/Redo、另存/重启重开和放映，R1–R7 CLI 重跑通过。JD-900–904 完成，JD-905/907 已实现待验收；JD-908 的工程替换 PR/CI/合入已完成；JD-906 旧功能完整无回退继续开放，JD-907 完整功能依赖不提前关闭。UI 与 DeckCraft/PowerPoint 完整一致目标继续保留，下一步优先该产品差距。


## R8 替换前历史切片

2026-11-03：JoyDeck 0.8.0，R8 Windows 编辑器界面与稳定性切片。主参照为实际安装的 DeckCraft，PowerPoint 辅助核对中文交互。真实产品截图发现旧 UI 长期暴露工程路径/诊断入口，本轮重做日常工作区，新增原生文件流程、分组 Ribbon、基础格式、选择手柄、备注、浏览视图、基础对象与图片剪贴板。

Rust 1.98.1、GPUI Kit 0.8.1/pre 0.3.10、官方 Krilla 0.8.2 与 JoyOffice 对齐。源 JoyOffice/DeckCraft 未改动，旧开发包保留。引擎/CLI 不依赖 GUI；所有文稿修改仍走同一事务、撤销、恢复和保存路径。

当次证据见 [R8 验证](evidence/R8/README.md)，参照基线、已实现和完整一致前的差距见 [界面与功能对照](UI-PARITY.md)。R4–R8 整阶段、完整命令矩阵和 macOS/Linux/ARM/信创真机资格未完成。已有 R6/R7 的完整旧媒体/审阅迁移、全部动画编辑、多屏/设备、完整 OOXML/PDF/打印保真等缺项继续保留，不因外观重做而抹去。

旧 JoyDeck M0–M14 状态保留在 [替换前状态](STATUS-pre-r9.md)，不能作为当前实现的通过证据。
