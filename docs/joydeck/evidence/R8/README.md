# R8 Windows 编辑器与界面对照验证

2026-11-03。本轮以实际安装的 DeckCraft 0.14.1 为主参照，PowerPoint 为中文操作和公共 PPTX 读取参照。已交付 0.8.0 Windows 编辑器切片；**完整功能/像素一致和 R8 跨平台资格仍未完成**。原因、对照范围和待补功能见 [UI 对照](../../UI-PARITY.md)。

## 最终开发包

[Windows x86_64 release ZIP](../../../../dist/joydeck/joydeck-0.8.0-r8-windows-x86_64-release-97c23f9b0a.zip)，解压后运行 `joydeck.exe`；命令行程序为 `joydeck-cli.exe`。包内有说明、样例、原生输出/媒体辅助程序、来源台账及 SBOM。应用/模块名称继续为 joydeck。

- ZIP SHA-256：`f989fe72ab22618b0769e9ed3c77470ad415b9787a95d6f508dc1bf35f691b60`。
- 源码指纹：`97c23f9b0aa069e59d796969dafeed71c5830576ba27ba888972ed8c65590e9a`。
- GUI exe SHA-256：`898ce04dc6c67fef0354bd45d33a8bc093605ba584c14b1fdabd156519352226`。
- CLI SHA-256：`2a68b10912fa2b78c3db4114e4a21e39c9d85e5b002e4c2675824b0df61f4bf6`。
- 仓库外中文及空格路径解压，ZIP CRC 和 **784 个文件 SHA-256** 校验通过；运行的是解压后的包。455 项 SBOM 依赖，未签名，22 项许可全文缺项沿用已有台账，正式发行前另行补齐。[包记录](package.json)、[运行记录](runtime.json)、[最终包 CLI 冒烟](final-cli-smoke.json)、[构建日志](package-stack-qualified-build.txt)。打包的 363 个源码文件与 Git 暂存 blob 逐一一致，见 [源码比对](git-source-proof.json)。

## 可运行切片

1. 原生分组 Ribbon、灰白/橙色主题、缩略图、上下文格式、原生另存、备注与浏览视图；release 构建并运行。[初次运行](slices/01-ribbon.jpg)。
2. 干净的新文稿、保存后切换、8 个缩放手柄、选择窗格、缩放命中；release 构建并运行。
3. DeckCraft typed media walker、基础对象/图片/表格跨文稿剪贴板、资源 ID 重映射、系统图片粘贴、字体/段落事务与原生选图；release 构建并运行。
4. 实际桌面发现并修复占位符点击、默认焦点、备注/IME 关闭保护、复制表格的单元格焦点隔离。最后修复 debug Ribbon 栈溢出和窄屏滚动，重新构建最终包。

## 桌面验收

截图保留实际二进制指纹和 a11y 状态，见 [截图索引](gui-index.json)。01–11 属于 a88695dfed 候选包；12–20 属于修复表格焦点后的 1db7fba9b9 候选包；21–22 属于最终源码的未优化 debug；23–25 属于本次交付 97c23f9b0a release。PowerPoint 截图 10 是独立应用读取结果。没有把候选包证据改标成最终包。

| 操作 | 实测结果与证据 |
| --- | --- |
| 新建 / 原位输入 / 微软拼音 | 默认空白稿干净；点击标题有光标；真实 ni 候选、Esc 取消、提交“你”、撤销恢复干净状态。[候选](gui/03-ime-candidate.jpg)、[提交](gui/04-ime-committed.jpg)、[撤销](gui/05-ime-undo.jpg) |
| 字体、表格、备注 | 字号 44、颜色 D95A1A；插入 3×3 表格并拖动、编辑“能力/状态”，Tab/Shift+Tab；直接填写备注。[字体](gui/06-font-dialog.jpg)、[表格](gui/08-table-edit.jpg) |
| PPTX 另存、重开与公共读取 | 原生另存到中文路径；独立 python-pptx 读取标题、字号、颜色、表格、备注；微软 PowerPoint 无修复提示打开并显示。[PowerPoint](gui/10-powerpoint-public-read.jpg)、[独立读取](independent-gui-pptx.json) |
| 对象剪贴板隔离 | Ctrl+C/V 新建表格副本；Enter / Tab 只修改副本为“副本/独立”。原对象仍为“能力/状态”，PPTX 中 ID 260 与 275 分离。[独立读取](gui-clipboard-roundtrip.json) |
| 未保存切换 | Ctrl+O 出现保存提示；取消保留编辑；选择保存后完成原生另存，再继续打开指定 PPTX。[提示](gui/14-save-before-open.jpg)、[保存结果](gui-save-before-open-inspect.json) |
| 文件、视图、选择窗格与主题 | 文件页、普通/浏览切换、按对象名选择、格式入口、明暗切换；文稿颜色不随 chrome 改变。[文件](gui/15-file-page.jpg)、[浏览](gui/17-slide-browser.jpg)、[选择](gui/18-selection-pane.jpg)、[深色](gui/19-dark-theme.jpg) |
| 放映 | F5 打开实际放映窗口，标题和表格可见、空占位符隐藏，Esc 返回。[放映](gui/20-slideshow.jpg) |
| 默认栈与窄屏 | release 与未优化 debug PE 均为 1 MiB；debug 800×600 下逐页显示、左右滚动、文件输出换行，并干净退出。[窄屏](gui/21-debug-narrow.jpg)、[文件页](gui/22-debug-file-page.jpg)、[栈修复](stack-resolution.json) |
| 最终包冒烟 | 打开与 DeckCraft 相同的三页 PPTX，画布/缩略图/备注和继承字体正确；Ctrl+D 新建对象，Undo 返回干净状态。[DeckCraft 同稿](reference/05-deckcraft-same-pptx.jpg)、[最终编辑器](gui/25-final-editor.jpg) |

GUI 生成的可重开文件保留在 [artifacts](artifacts/ui-authored.pptx)，另有剪贴板隔离 PPTX 和保存后继续的原生文稿；指纹见 [文稿清单](gui-artifacts.json)。原始输入未覆盖。保存提示的“不保存”分支未通过 GUI 点击测试；失败剪切等原子性由引擎测试验证。

## 自动验证

最终源码完整重跑 **870 项 Rust 测试，0 失败、0 忽略，44 组**；5 项 Python 门禁、fmt、严格全工作区全目标 Clippy、依赖/CLI 隔离、来源与文档检查通过。[Rust 日志](tests-workspace-final.txt)、[Clippy](clippy-workspace-final.txt)、[汇总](automated-summary.json)。

新增 7 项行为回归覆盖形状预设失败原子性、粘贴媒体冲突/ID/撤销、坏剪贴板、占位符外观实体化、新建状态/恢复/重开、公共 PPTX/v2 往返、图片预算/损坏输入；另保留媒体引用遍历器的 3 项上游测试。

CLI 回归使用早期 R8 候选 CLI，已核对与最终包 **二进制 SHA-256 完全相同**。R1 28 项、R2 20 项、R3 20 项、R4/R5 23 条调用、R6/R7 32 条 CLI 调用和 38 条 MCP 响应通过；包含强退恢复、编辑/交换、渲染/缓存、图表/图片/母版、迁移/输出/控制。R1/R2 校验器的旧阶段字符串断言改为能力集与 GUI 隔离校验，既有行为断言保留。[R1](cli-r1-regression.json)、[R2](cli-r2-regression.json)、[R3](cli-r3-regression.json)、[R4/R5](cli-r4-r5-regression.json)、[R6/R7](cli-r6-r7-regression.json)。

## 来源、依赖与平台边界

JoyOffice 固定提交 a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d，源工作目录保持干净。Rust 1.98.1、Kit 0.8.1 / pre 0.3.10、官方 Krilla 0.8.2 不变；没有新增 registry 依赖或引入 egui。268 条来源记录；复用旧 JoyDeck 的原生保存和按标签控制栈帧模式。GPUI 21 个包溯源：16 个一致、5 个已审查偏差、0 失败。未复制 PowerPoint 私有图标或系统字体。

本机 Windows 11 x86_64、Intel Arc 140T / NVIDIA RTX 5060 Laptop 等环境见 [environment.json](environment.json)。仅 Windows x86_64 MSVC 有本机运行证据；macOS/Linux/ARM/信创、真实多屏、全高 DPI/设备切换、读屏朗读、长期性能、正式签名/安装关联/升级尚未实测。三平台 CI 定义不计作通过证据。

329 条命令清单中，41 条登记了本轮已接入的基础 UI 范围及未覆盖项；无界面逐命令接口资格没有因通用事务测试而标记通过。完整标签/图库/329 命令、复杂文字、组与 Office 对象互换、图表新建、SmartArt、完整母版编辑、批注/比较、动画编辑和输出保真仍按 [差距表](../../UI-PARITY.md) 与 [任务清单](../../TASKS.md) 继续验收，不把本轮外观重做当成完全一致。
