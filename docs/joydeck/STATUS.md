# JoyDeck 当前状态

2026-11-05 完成 JD-302。macOS Release 实测旋转与竖排选区、指针命中、方向键、插入点滚动和 PPTX 另存读回。产品检查 911 项通过，1×/2× 坐标回归通过，2× 实屏另行复验。[原位编辑证据](evidence/R3-TRANSFORMED-TEXT/README.md)

2026-11-05 完成 JD-301。macOS Release 实测同名字体热替换、画布与缩略图刷新、原位选区及 TTC 搜索。包内 CLI 报告选中的 face、冲突和实际替代。JoyDeck 909 项、三产品联合 5,802 项通过。[字体环境证据](evidence/R3-FONT-ENVIRONMENT/README.md)

2026-11-05 完成 JD-300。修复 PDF 搜索层的动态字段、链接与组合缩放坐标。macOS 实测混排编辑和保存回读，Poppler 验证 14 个组合与普通文本搜索词的边界一致。906 项产品测试通过。[排版证据](evidence/R3-SHARED-LAYOUT/README.md)

2026-11-05 完成 JD-202，R0–R2 共 28 项全部完成。整合分支的 macOS ARM64 Release 包在 2× HiDPI 虚拟显示器上通过真实窗口手势复验，另在 1× 桌面通过 R1 查看和 R2 保存重开与恢复。JoyDeck 产品检查 904 项、包内 CLI R1／R2 共 48 项通过，329 条静态命令 ID 已复核。[当次验收](evidence/R2-POINTER/README.md#hidpi-2x-复验)

2026-11-05 完成 JD-209。整合保存发布修复后的 macOS ARM64 Release 包，从实际新建、插入、拖动、另存和重开，验证到重开后的撤销、重做和强制终止恢复。桌面与包内 CLI 独立读回一致，原件散列保持，产品检查 904 项通过。[编辑器包验收](evidence/R2-EDITOR-PACKAGE/README.md)

2026-11-05 完成 JD-208。常用撤销、页与对象编辑、层级操作使用同一引擎预检。实测原生菜单与 Ribbon 的禁用状态、无选择 Delete 提示、快捷键和独立 CLI 对照。产品检查 903 项通过。[命令状态证据](evidence/R2-COMMAND-STATE/README.md)

2026-11-05 完成 JD-203。macOS Release 实测新建文稿和页面、插入形状、层级、副本、删除和撤销。JD-202 已修正旋转命中、手柄锚点和窗口级指针捕获，1× 实屏确认 Esc 与失焦取消不写入编辑历史，拖到窗口外释放只提交一次。2× HiDPI 复验见上。JoyDeck 产品检查 902 项通过。[手势与基础编辑证据](evidence/R2-POINTER/README.md)

2026-11-05 完成 JD-201。「视图」新增「新建文稿视图」，同一规范化路径在多个窗口共享编辑、撤销、保存和恢复，各窗口保持独立的页选与缩放。三窗口实测及过期读取拒绝通过，JoyDeck 产品检查 900 项通过。[共享文稿证据](evidence/R2-SHARED-DOCUMENT/README.md)

2026-11-05 完成 JD-107、JD-108、JD-200。CLI、control、MCP 注册表与 329 条上游静态 ID 已对照。移位后的 macOS Release 包通过打开、翻页、缩放、取消、坏文件和 CLI 读回。引擎与文件服务的隔离及 R2 的 20 次 CLI 调用已复验。逐命令功能覆盖继续跟踪 JD-509。[证据](evidence/R1-VIEWER-AUDIT/README.md)

2026-11-05 完成 JD-104 和 JD-106。macOS Release 实测打开中取消、新请求取代慢请求、坏文件后保留未保存文稿及撤销。当前文稿 ID、修订和恢复快照保持一致。包内 CLI 语义、PNG 和命令查询通过，CLI 依赖隔离通过。[R1 复验](evidence/R1-OPEN-REVISION/README.md)

2026-11-05 完成 R0 验收复核。JD-007 的隔离宿主演练与 Windows/macOS/Linux CI 编译门禁已有证据，JD-008 的原 R0 Windows 开发包启动、导航/面板/主题及产物指纹已对照，任务状态更新为完成。[R0 复核](evidence/R0/README.md#任务验收复核)

2026-11-05 完成 JD-206。同进程保存只允许一份冲突结果发布，macOS 实测替换失败、外部改动、取消关闭和另存读回。三产品本机联合回归 5,791 项通过，开发包均已验证；JD-905 的合并 CI 继续跟踪。[保存复验](evidence/R2-SAVE-PUBLICATION/README.md) · [联合回归](evidence/R9-WORKSPACE/README.md)

2026-11-05 R9 文件与旧控制复验。macOS 原生另存扩展名已修复，四种名称在移位 Release 包中成功落盘并独立读回。旧批次按更新后的页序执行，迁移表格字符串 ID 已接通，比较支持对相同稳定页 ID 集合重排后的对象接受。系统文件打开、坏文件恢复和未保存保护已在当次包复验。各父任务按剩余验收条件保留。[验证记录](evidence/R9-FILE-COMPAT/README.md)

2026-11-05 macOS ARM64 原生打开和另存为面板的取消及确认已实测。选择另一份 PPTX 后窗口切换，另存的 PPTX 独立读回三页。扩展名问题首次在本次补验发现。[桌面证据](../joyoffice/validation/20261105-macos-desktop/README.md#原生文件对话框)

2026-11-05 macOS ARM64 静态 MP4 已接通。移位 Release 包的 CLI 通过导出、逐帧读回和拒绝覆盖检查。嵌入媒体探测与播放仍使用 Windows 适配器，JD-705 的其他平台范围继续开放。[本机证据](../joyoffice/validation/20261105-macos-video/README.md)

2026-11-05 开发包许可复核。按包内 SPDX 索引统计，macOS ARM64 的 13 个实际缺口已补齐 8 个依赖版本，严格检查仍报告 5 个。JD-807/808 继续开放。[复核与包指纹](../joyoffice/validation/20261105-macos-video/README.md#开发包许可复核)

2026-11-05 macOS 补验。JD-801 完成。PPTX 系统文件冷启动和运行中打开、重复打开、坏文件拒绝及未保存保护已用 Release 包验证。新增原生文件与编辑菜单，实际点击撤销与重做通过。其余 R9 功能和发行任务见交接。[本机证据](../joyoffice/validation/20261105-macos-open/README.md)

2026-11-04 macOS ARM64。JD-801 转为进行中。普通 Metal Release 包已在仓库外移位启动，桌面 stdio 通过复制幻灯片、撤销／重做、保存和重启重开，独立 PPTX 读回 4 页。开发 `runtime_shaders` 构建单独启动并通过相同检查。包内 CLI 通过 R1–R5 和适用的 R6–R7/MCP 检查。解锁后实屏验证鼠标选择并复制第 2 页、键盘撤销／重做／保存、正常关窗和重启重开。MP4 导出与媒体探测仍依赖 Windows 适配器，系统文件事件未切换文稿，菜单与其他平台交互继续验收。包清单列出的 49 条许可正文缺口留在 JD-807/808，JD-905–907 仍开放。[本机报告](../joyoffice/validation/20261104-macos/README.md)

2026-11-03 R9：按用户明确要求，将 0.8.0 DeckCraft/GPUI 重构整合到 JoyOffice，替换 apps/joydeck、apps/joydeck-cli 与相关产品 crate。今后仅在 JoyOffice 内继续开发，原 joydeck-new 目录退出开发。当前整合、回归、备份和剩余任务见 [R9 交接](../joyoffice/JOYDECK-R9-HANDOFF.md) 与 [R9 验证](evidence/R9/README.md)。

2026-11-03 R9-UI2：PR #121 已合入 `6689c6e`；PR #119 经 20 项 CI 全绿后按用户授权合入 `8002f27`，仅保留 QEMU 子进程 runner 增量。当前分支 `codex/joydeck-r9-ui-compat`，非草稿 [PR #122](https://github.com/willmove/joyoffice/pull/122) 已提交，CI 见 PR 实时状态，已同步该 main。混合格式、段落／表格控件、整体变换／页面组织、最近／模板／多窗口、旧审阅／媒体语义、比较接受和显式桌面控制已新增实现与自动回归；最终 Deck 894、Word 1,318 项通过，联合 3,596 通过／0 失败／1 忽略。Word emoji 最终包 GUI／独立 PDF 回读已通过；Deck 格式／表格／审阅等 GUI 已独立读回；额外发现的模板／UIA 关闭竞态／主题同步已修复；最终包格式保存、审阅／最近重开、跨窗口主题和关闭补验通过，UOS 输入法按用户最新安排留待之后进入该环境验证。完整旧 16 领域、UI 一致与 JD-906 继续开放，详见 [本轮范围与证据](evidence/R9-UI2/README.md)。下段保留 R9-UI1 历史基线。

[PR #120](https://github.com/willmove/joyoffice/pull/120) 已在 21 项检查通过后按用户授权合入 main（3691d4d）。当前切片分支 codex/joydeck-r9-formatting-compat，最新本机 Deck 875 通过、联合 3,575 通过/0 失败/1 忽略；最终解压包的字号快捷键、PPTX 保存/独立读回/重启重开已通过。字体/字号/颜色直接控件与旧方法恢复见 [R9-UI1 验证](evidence/R9-UI1/README.md)。备份/恢复及旧软件可运行包已验证；R9 整合基线本机联合测试 3,573 通过、0 失败、1 忽略，Deck check 873 通过。已交付新的 R9 Windows Release 开发包，实际验证仓库外读取、编辑、Undo/Redo、另存/重启重开和放映，R1–R7 CLI 重跑通过。JD-900–904 完成，JD-905/907 已实现待验收；JD-908 的工程替换 PR/CI/合入已完成；JD-906 旧功能完整无回退继续开放，JD-907 完整功能依赖不提前关闭。UI 与 DeckCraft/PowerPoint 完整一致目标继续保留，下一步优先该产品差距。


## R8 替换前历史切片

2026-11-03：JoyDeck 0.8.0，R8 Windows 编辑器界面与稳定性切片。主参照为实际安装的 DeckCraft，PowerPoint 辅助核对中文交互。真实产品截图发现旧 UI 长期暴露工程路径/诊断入口，本轮重做日常工作区，新增原生文件流程、分组 Ribbon、基础格式、选择手柄、备注、浏览视图、基础对象与图片剪贴板。

Rust 1.98.1、GPUI Kit 0.8.1/pre 0.3.10、官方 Krilla 0.8.2 与 JoyOffice 对齐。源 JoyOffice/DeckCraft 未改动，旧开发包保留。引擎/CLI 不依赖 GUI；所有文稿修改仍走同一事务、撤销、恢复和保存路径。

当次证据见 [R8 验证](evidence/R8/README.md)，参照基线、已实现和完整一致前的差距见 [界面与功能对照](UI-PARITY.md)。R4–R8 整阶段、完整命令矩阵和 macOS/Linux/ARM/信创真机资格未完成。已有 R6/R7 的完整旧媒体/审阅迁移、全部动画编辑、多屏/设备、完整 OOXML/PDF/打印保真等缺项继续保留，不因外观重做而抹去。

旧 JoyDeck M0–M14 状态保留在 [替换前状态](STATUS-pre-r9.md)，不能作为当前实现的通过证据。
