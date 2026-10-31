# CLI、MCP 和桌面控制

```text
joysheet --sample sales --control 7979 --diagnostics
joysheet-cli send 7979 ui.inspect
joysheet-cli send 7979 cell.get {"cell":"B5"}
joysheet-cli mcp --connect 7979
```

PowerShell 中 JSON 使用单引号包裹，例如：

```powershell
.\target\debug\joysheet-cli.exe send 7979 engine.execute '{"command":"cell.set","params":{"cell":"B1","input":"=SUM(1,2,3)"}}'
```

服务只绑定 `127.0.0.1`。客户端自动读取 Joysheet 配置目录下对应端口的 token，也接受 `JOYSHEET_CONTROL_TOKEN`。测试脚本通过 `JOYSHEET_CONFIG_DIR` 隔离配置和 token。连接须先发送 `{"hello":"TOKEN"}`，随后逐行发送 `{id,method,params}`；响应保留 id，使用 `{ok,result}` 或 `{ok:false,error}`。

每条请求可携带 `window` 或 `doc`，二者互斥；未知目标返回错误，不能退回活动窗口。`engine.execute` 的目标位于外层 params，其内部 params 保持原命令语义。

| 方法 | 主要参数/行为 |
| --- | --- |
| engine.execute | `{command,params,window?}`；有序执行引擎命令 |
| engine.commands / engine.journal | 注册命令/脱敏日志 |
| engine.profile | 最近 512 条命令 ID、排队/执行/快照/发布阶段；不记录参数或文件路径 |
| engine.tasks / engine.cancel | 近期脚本任务/请求取消；cancel 使用 `{id,window?}`，目标必须拥有该任务 |
| app.open / app.save / app.new | 提交目标窗口草稿后执行；等待自身命令完成，返回 window/doc/path/dirty/revision/sequence |
| ui.awaitIdle | 等待目标窗口提交、引擎快照与绘制同步；可指定 minRevision/minSequence |
| ui.inspect | 窗口、文档、修订、选区、草稿、几何、组合态及能力 |
| ui.key / ui.text | `{key,shift?,ctrl?,cmd?}` / `{text}`；派发实际 GPUI 事件 |
| ui.cell.click / ui.cell.drag | `{cell}` / `{from,to}`；使用生产命中测试，目标必须可见 |
| ui.click / ui.drag / ui.scroll | 窗口逻辑坐标 x/y，拖动 toX/toY，滚动 dx/dy |
| ui.edit.begin / text / commit / cancel | 编辑控制；begin 支持 inline=false 切至公式栏 |
| ui.dialog / set / confirm / cancel | 命令参数表单；set 使用 `{field,value}` |
| ui.import.preview | 使用当前导入表单的文件或多行文本进行独立会话预览；等待 dataResultKind 为 __joysheet.previewImport |
| ui.resize / ui.focus | 逻辑尺寸与窗口聚焦 |
| ui.set | dark/autosave；persist=false 仅改变本次应用状态 |
| ui.screenshot | `{path?,base64?,minRevision?,minSequence?,allowBusy?}`；automation 功能，真实 GPU 读回；返回身份和版本 |
| ui.perf | CPU worksheet paint 与提交到匹配绘制的分段分位数；raw=true 返回最多 512 条帧/编辑样本；reset=true 清空样本 |
| app.quit | 有未保存工作簿/任一窗口草稿时拒绝；force=true 明确丢弃 |

输入方法的响应只表示事件派发/提交排队成功；检查 `ui.inspect.editor`、引擎值和修订来等待完成。截图额外等待当前引擎序列在生产视图绘制完成。性能统计不包含 GPU 呈现、系统合成器或端到端输入延迟。

`ui.ime.mark/unmark` 仅在 automation 构建中用于组合态回归，不能据此认定某操作系统输入法已经验收。

M5 的 `ui.inspect` 另提供 worksheet（表格/条件格式/验证数量和隐藏行采样）、sheetCount、dataResults 和 dataResultKind，用于等待与核对数据工作流结果。导入表单的 text 字段写入真实多行 Textarea。

原 MCP 的工具协议和核心操作沿用 `gridcraft-mcp`。M7 修复 app/ui 工具及 screenshot 的目标转发。`save_workbook` 经桌面保存入口提交指定窗口草稿。底层关闭/切表等命令保留原引擎策略，自动化应先提交或取消对应窗口草稿；带 `force` 的关闭会丢弃未提交内容。

M6 检查字段：drawingObjects、objectBounds、selectedObject、decodedImages、printPreview（PDF/PNG 临时路径）和 printStatus。ui.print.dialog 打开当前预览的系统打印对话框；ui.print.close 关闭预览。临时路径在预览关闭后清理，需在关闭前复制证据。ui.command file.print/file.printPreview 进入桌面预览/打印；engine.execute 仍保持原有引擎 PDF 语义。

## M7 完成等待和大请求

`engine.execute file.save/file.saveAs`（另存为须有效 path）、`file.open/file.new` 通过传输层进入对应桌面生命周期入口，等待各自命令完成。新文档保存须指定 path，远程打开须指定 path/base64，不会挂起等待文件选择器。活动确认弹窗或正在提交的草稿返回明确错误；提交进行中可等待 ui.awaitIdle 后重试。后台窗口保存使用其自身草稿，不切换到其他文档执行。返回成功表示该命令已完成；截图和后续界面检查仍可等待返回版本。

`engine.execute` 的其他命令是低层接口，通常不会提交 UI 草稿；`ui.command`/`ui.menu.invoke` 经桌面路由提交草稿后排队执行，初始响应只表示接收。后续使用 ui.awaitIdle，并检查实际数据。`ui.awaitIdle` 不主动提交草稿，存在未提交草稿时会等待至超时。

版本参数须为非负整数。ui.awaitIdle/截图等待 7 秒，UI 传输预算 8 秒，生命周期预算 60 秒。已开始的引擎命令可能在请求超时后继续；超时响应不代表回滚。截图仅在版本已绘制后生成；等待超时不会生成图片。默认等待 UI 命令结束，`allowBusy=true` 可观察正在运行的脚本进度，图中工作簿仍是最后发布的快照。截图可能激活指定窗口。既有请求/图像预算与认证保持不变。

Windows 大 JSON 参数使用 UTF-8 文件以避免命令行长度限制：

```powershell
.\joysheet-cli.exe send 7979 engine.execute '@params.json'
```

文件最多 8 MiB，传输仍受完整请求行预算约束。桌面“动作与脚本”任务列表和 `engine.tasks` 提供已完成步骤，取消通过线程间原子标志生效，绕过繁忙引擎队列；单条命令不能中断。

新增 ui.inspect 字段：dataRaw、advancedBounds、tasks、review（批注、名称、保护、信任和录制状态），以及 capabilities 中的版本等待/生命周期/取消能力。advancedBounds 仅用于生产控件原生指针回归，不构成稳定布局 API。

## 旧桌面协议对照

| 能力 | GPUI 处理 | 验证或边界 |
| --- | --- | --- |
| 注册引擎命令、CLI/MCP 核心工具 | 保留 ID、参数、撤销语义 | 核心测试；目录占位命令不计入实现 |
| 身份、认证、预算、错误 | 回环 token，window/doc 互斥，拒绝未知身份 | 认证/超时单测及原生定向回归 |
| 键盘、文本、鼠标、拖放、焦点、尺寸 | 实际 GPUI 事件/窗口 | 原生生产视图测试；合成 IME 不等于系统 IME 验收 |
| screenshot | GPU 读回、身份/版本等待、MCP 目标传递 | 多窗口、超时不落盘；automation 构建 |
| app.open/save/new 与 MCP 保存 | 目标草稿提交及自身完成响应 | 指定后台窗口草稿保存/隔离验证 |
| 关闭/切表/窗口/退出 | 桌面人机操作统一处理草稿；低层引擎调用保留 force/document 语义 | 不声称所有低层命令具有人机提示策略 |
| ui.ribbon | Home/Insert/Data/Review/View/Automate/Page Layout/Formulas 映射到对应功能面板 | 旧 ribbon 几何和控件树不保留 |
| ui.menu.invoke/command/ui.command | 桌面命令路由 | stats 等原生回归；长任务用 awaitIdle/任务列表观察 |
| ui.message.dismiss/ui.set | dismiss、dark、autosave、formulaBar、autocomplete、recovery、language | ribbonCollapsed/animations 开启仍明确拒绝，不声称旧控件状态完整复刻 |
| 参数、信任、保存、导入和打印请求 | GPUI 表单/自定义确认/预览与平台适配 | options 已打开 GPUI 设置页；复杂字段保留高级参数 |
| 设置页、旧菜单控件树与更新 | GPUI 设置页和功能面板；保留协议错误 | 不复刻旧几何；未提供发布远端/feed，自动更新仍不声称完成 |

M7 本机常用场景通过，仍须 M8 三端原生、辅助功能及更多复杂工作簿差异验收。

M8 增加 `ui.openFiles {paths:[...]} `（本地路径或 file URL，先提交草稿）、`ui.recovery.list`、`ui.recovery.restore {id,draft?:0}`。恢复提交后用 ui.inspect 验证目标新窗口及 editor 草稿，再提交/保存。ui.ribbon 另有 Settings/Recovery 面板。ui.inspect 增加 settings、panel、sheetNames、visibleRows/Columns（诊断用途）。ui.perf 返回 editSamples/editP95Ms，为 GPUI 提交到引擎完成与工作表 CPU paint 的耗时，排除 GPU 呈现；空样本返回 null。

## Windows W1–W4 补充

`ui.ribbon {tab:"Objects"}` 打开多对象面板。`ui.inspect` 增加 selectedObjects、formulaReferences/formulaHint、focus（grid/formula/name/sheets）、outlineText（字体就绪及缓存块数）、nativeWindows（实际 Win32 矩形/工作区）及 arrangement（最近排列结果）。这些几何/绘制字段供回归诊断，不构成稳定布局协议。keyTrace 只记录最近 16 个非文本导航键，不记录用户打印字符。

`ui.command {command:"view.arrangeAll",params:{arrange:"tiled|horizontal|vertical|cascade",activeWorkbook:false}}` 经桌面执行原命令后应用原生窗口排列，保留所有视图草稿；初始响应只表示接收，可轮询 arrangement/实际窗口矩形。底层 `engine.execute view.arrangeAll` 仍只返回原引擎计划，不自行操作 OS 窗口。

应用私有 `__joysheet.objectBatch` 仅供 UI 聚合原对象命令：校验文档/工作表/修订、限制命令白名单与 256 项操作、使用 Session.grouped 一次撤销及失败回滚。不把该内部适配接口当作新增稳定引擎命令；程序化调用优先使用原命令 ID。


W5–W8：ui.inspect 的 inputGeometry 为当前真实输入组件的 UTF-16 选区/组合态及候选锚点，display/nativeWindows.dpi/client 区分逻辑/原生坐标。ui.accessibility 返回生产网格的可见语义及平台启用状态，树仅在辅助技术实际连接后存在。automation 的 ui.dpi {scale:1.25} 只覆盖 GPUI 测试布局，{reset:true} 从 OS 恢复，不修改系统显示设置；GPU 后备缓冲仍为实际分辨率。

组合态期间 app.save/open/new 返回明确拒绝，不等待超时；UI 的保存/关闭/导航也保持草稿。ui.ime.mark/unmark 自动选择当前单元格或公式栏输入处理器，仍不能据此宣布系统 IME 通过。printWork 报告 exporting/rendering/dialogOpen/current；过期工作表/修订不允许打印，ui.print.close 使尚未完成的预览失效，已打开的原生对话框保留其 PDF 所有权直到结果返回。

M8-preview5 的 ui.perf.stages 增加 extentMs/dimensionsMs，以及 recalcMs、dependencyMs、evaluationMs、writebackMs、rangeReadMs、usedRangeMs、rowFitMs 的分位数；raw 编辑同时包含 formulasEvaluated、rangesRead 和 sumCacheHits。engine.profile 的 timing.calculation 提供命令级对应字段。嵌套阶段不能相加，rangesRead 统计调用次数（含溢出回退的读尝试），SUM 复用只在同一个不可变求值 pass 内有效。`scripts/desktop-smoke.ps1 -Performance -FormulaCases` 会单列 SUMIFS/SUMPRODUCT/嵌套/扇出，不与普通编辑混合计算 P95。
