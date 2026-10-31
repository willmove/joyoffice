# Windows 输入、DPI、辅助功能与打印

交付版本 **0.1.0 / M8-preview3**。本轮 W5–W8 接续 [W1–W4](WINDOWS.md)，保留原有核心行为与命令入口。macOS/Linux 原生运行及签名继续跳过。

| 项目 | 实现与自动验证 | 人工验收 |
| --- | --- | --- |
| W5 输入法 | 单元格/公式栏组合态统一保护；保存/关闭/新建/切换不提前提交；真实输入控件的 UTF-16 中文/emoji、候选锚点、Enter/Tab/F6、指针和一次提交回归；输入几何改变后刷新 IME 坐标 | 实际微软拼音/日文输入、候选窗口、系统焦点切换 |
| W6 DPI | GetDpiForWindow、原生客户区/逻辑视口诊断；细线像素对齐；100/125/150/175/200/300% 逻辑缩放测试，覆盖冻结、125% 表格缩放、命中与候选锚点；恢复真实 OS 比例 | 真正系统缩放、跨显示器移动、负坐标和休眠恢复 |
| W7 辅助功能 | 实际 Windows UIA 经 AccessKit 读取中文值并聚焦合并格；完整行列索引/跨度、只读/隐藏公式描述、可见节点预算和冻结裁剪；修复虚拟节点漏乘 DPI；175% 的 UIA 矩形与实际客户区坐标对照 | Narrator/NVDA 的实际语音、浏览、提示顺序及键盘操作 |
| W8 打印 | 原生“取消”按钮 UIA 调用及重试、关闭预览时保留打印文档；拒绝过期文档/工作表快照；关闭后后台预览不复活；页码范围、纸张可打印区域、有限栅格预算、损坏 PDF 清理；真实 WPF 分页输出 XPS，无队列提交 | Microsoft Print to PDF 的实际输出、实体打印机/驱动、份数与纸张方向 |

分项证据：[W5](evidence/W5.json)、[W6](evidence/W6.json)、[W7](evidence/W7.json)、[原生 UIA](evidence/W7-uia.json)、[W8](evidence/W8.json)。编译、合成事件、真实平台提供程序和打印队列分别记录，不能相互替代。

## 直接试用与自动回归

解压 Windows 便携包，双击 `Open Windows quality.cmd`。打印样例为 A4 横向、多页打印区域，第一张表含中文与公式；系统打印对话框需要 PowerShell 7。无需 Rust 即可手工试用。

```powershell
pwsh -NoProfile -File scripts/dev.ps1 run --locked -p joysheet -- --sample sales
pwsh -NoProfile -File scripts/dev.ps1 build --release --locked -p joysheet -p joysheet-cli --features joysheet/automation
pwsh -NoProfile -File scripts/dev.ps1 xtask ci
pwsh -NoProfile -File scripts/dev.ps1 xtask smoke --stage M8
pwsh -NoProfile -File scripts/desktop-smoke.ps1 -Stage M1 -WindowsQualityTask 1
pwsh -NoProfile -File scripts/desktop-smoke.ps1 -Stage M1 -WindowsQualityTask 2
pwsh -NoProfile -File scripts/desktop-smoke.ps1 -Stage M1 -WindowsQualityTask 3
pwsh -NoProfile -File scripts/desktop-smoke.ps1 -Stage M1 -WindowsQualityTask 4
```

任务号递增包含此前分项。实际 UIA 提供程序读取不等于读屏语音验收；GPUI 逻辑 DPI 覆盖不改变系统显示设置或原生后备缓冲大小，截图保持真实分辨率。这些测试不代替跨显示器 OS DPI 切换。辅助功能不为百万行创建控件，仅导出可见单元格，完整表格行列数仍向 UIA 报告。

原生打印取消测试只针对本次启动的 Joysheet 子进程、其打印脚本和原生 Cancel 按钮，不调用“打印”。测试用 .NET Framework 的 Win32 UIA 客户端代理，PowerShell 主机仍为 7；该测试探针不属于桌面运行依赖。打印辅助进程监听 Joysheet 进程退出，避免应用退出后留下孤立对话框；已提交的系统队列作业由 Windows 管理。

## 人工验收步骤

以下均为**待人工验收**，请逐条记录“通过/失败/未测”。不要仅凭自动回归勾选。建议使用样例副本，先记录 Windows build、应用包版本、显示器分辨率/排列/缩放、输入法与读屏版本、打印机及驱动。

| 编号 | 操作步骤 | 预期结果与留存证据 |
| --- | --- | --- |
| IME-1 | Win+Space 切到微软拼音；选 D5、F2，输入 `zhongwen`；移动候选选项，按 Enter 选词，再按 Enter | 首次 Enter 只完成候选选择，第二次才提交单元格；候选位于光标下方，内容仅写入一次；录屏含候选窗口 |
| IME-2 | 在公式栏重复拼音输入；长文本移动光标后继续输入；选词前按 Esc，再按 Esc | 公式栏和单元格草稿一致；第一次 Esc 交给输入法，随后可取消编辑，原值不变 |
| IME-3 | 候选窗口显示期间点“保存”、另一单元格、公式栏，尝试 Ctrl+S、Ctrl+W、F6；完成选词后再保存/关闭 | 未完成组合态不落盘、不关闭、不把文本写到新位置；提示先完成输入；完成后保存重开正确 |
| IME-4 | 日文微软 IME 转换平假名/汉字；输入 emoji（Win+.）；Alt+Tab 切出再切回；有条件时换输入法版本重复 | 不出现重复提交、乱码、候选遗留或光标跳动；记录输入法名称与版本及失败序列 |
| DPI-1 | 在系统显示设置依次选 100/125/150/175/200%（可用时 300%）；每档恢复应用并调整窗口；冻结 B2、表格缩放 125%、编辑合并单元格 | 网格、选框、输入框、候选窗口位置一致；无双重缩放、细线明显断裂或无法点击；每档截图 |
| DPI-2 | 两显示器设不同缩放（如 100%/175%），往返拖动窗口；副屏放到主屏左侧形成负坐标；重复编辑、拖选、窗口排列和打印预览 | 不重开文档、不丢草稿；点击命中和候选位置随新屏更新；排列在工作区内；记录屏幕布局与录屏 |
| DPI-3 | 从跨屏窗口最大化/还原，断开再连接副屏或休眠唤醒；将窗口缩小到常用最小尺寸，再恢复 | 焦点、滚动、选区和草稿保留，内容可继续操作，不留下超出屏幕的弹窗 |
| A11Y-1 | 开启 Narrator（Win+Ctrl+Enter）或 NVDA；Tab/F6 到网格，方向键移动；定位有中文、公式、空值及错误的格 | 能知道工作表、地址、显示值和编辑状态；同一焦点不会误报其他文档；记录语音与按键序列 |
| A11Y-2 | 在合并格、冻结/隐藏行列、100 万行定位、只读保护格之间移动；F2 编辑普通格，Esc 退出 | 合并跨度与选区合理，只读状态可辨；不朗读隐藏区域的无关节点；已隐藏的受保护公式不出现在单元格辅助描述 |
| A11Y-3 | 仅用键盘执行 Ctrl+1 格式、数据表单、保存确认、打印预览的 Tab/Shift+Tab、Enter/Space/Esc；检查亮暗主题与可见焦点 | 能到达必要控件，标签和错误可辨；模态不泄漏到背景；关闭后焦点返回原单元格；记录任何不可达控件 |
| PRINT-1 | 打开质量样例→页面与打印→预览；选系统打印，先取消，再重开；再在对话框开着时关闭预览并取消；另测关闭整个 Joysheet | 取消不提交作业、可重试；关闭预览后原对话框仍有文档；关闭整个应用后不遗留孤立打印对话框 |
| PRINT-2 | 选择 Microsoft Print to PDF，核对 A4/横向，分别打印全部页、仅第 2 页至新文件，再打开生成的 PDF | 页数分别匹配预览与所选范围；中文、公式值、图表、比例/边缘不截断；保留输出 PDF 和队列截图 |
| PRINT-3 | 在实体打印机检查可打印边距、纵横向、Letter/A4、份数/自动分页；另测驱动不可用、无打印机/离线时取消并重试 | 内容位于可打印区域内，不产生额外空页；错误可见、预览保留、可换打印机重试；记录驱动、队列作业 ID 与纸面结果 |

可选诊断：用 `joysheet.exe --sample sales --control 7979 --diagnostics` 启动，再运行 `joysheet-cli.exe send 7979 ui.inspect`。`display` 是 GPUI 实际比例/视口，`nativeWindows[].native.dpi` 和 `client` 是 Win32 值，`inputGeometry` 为 UTF-16 光标/候选锚点。`ui.accessibility` 返回生产可见单元格语义及当前平台是否启用。不要把 `ui.dpi` 的 automation 测试覆盖当作修改系统 DPI。

每条失败请保留包版本、步骤、期望/实际、截图或录屏、发生时间；正常试用无需开发工具。更新人工结果时保留“未测”，不把无法取得的设备证据标为通过。

PDF 字体覆盖沿用核心现有行为；彩色 emoji 与特殊字形需单独检查，不能由 UTF-16 编辑通过推断打印字形通过。


## 最终自动验收结果

完整 CI **1,372 项通过、0 失败、1 项上游计时测试忽略**，见 [Quality-ci](evidence/Quality-ci.json)。最终解压 Release **52 项** M3–M8/W1–W8 串联回归通过，见 [Quality-release](evidence/Quality-release.json)。其中包含原有保存弹窗、重算/文件/恢复/脚本/MCP，以及本轮两个输入框的 UTF-16 组合更新、DPI 布局、实际 UIA/模态后台保护、页码/边距/XPS、原生取消及重试。

另通过实际 `app.quit` 验证宿主和打印对话框辅助进程正常退出，见 [W8-owner](evidence/W8-owner.json)。用户级安装器的隔离安装、已安装程序实际启动、卸载并保留额外文件通过，见 [安装器](evidence/Quality-installer.json)、[安装后程序](evidence/Quality-installed-runtime.json)。TEST 模式不操作开始菜单/注册表，因此这两项不列为实际通过。普通桌面构建、WASM 核心检查通过；源包显式选择自身 Cargo.toml 后，锁定/离线 metadata 识别 18 个成员，见 [Quality-source](evidence/Quality-source.json)。

分发便携 ZIP、用户级安装器和源码 ZIP，保持二进制与最终回归相同 SHA256，附 app-local CRT、508 项依赖许可/来源文本清单。macOS/Linux 和签名继续跳过，BUILD.json 的 milestoneComplete 为 false。上述 13 项人工步骤未代填“通过”。

截图：[单元格组合态](screenshots/W5-ime-True.png)、[公式栏组合态](screenshots/W5-ime-False.png)、[真实 175% 屏幕上的布局](screenshots/W6-native-scale.png)、[辅助功能场景](screenshots/W7-accessibility.png)、[打印预览](screenshots/W8-print-preview.png)。组合态截图来自 GPUI 合成输入，不包含系统输入法候选窗口的验收证据。

## 性能与范围

额外 IME 坐标刷新仅发生在组合态的视口/DPI 几何变化时，普通光标更新由 GPUI 输入组件处理。以下为最终 Release、默认 1440×900 逻辑窗口，各滚动 60 次、编辑低依赖公式 30 次，来自 [Quality-performance](evidence/Quality-performance.json)。

| 数据规模 | CPU 绘制 P95 | 提交至 CPU 绘制 P95 | 可见单元格 |
| --- | ---: | ---: | ---: |
| 10,000 × 20 | 4.84 ms | 50.08 ms | 704 |
| 100,000 × 20 | 8.20 ms | 54.15 ms | 704 |

此前同机两次样本出现明显延迟波动：10k 为 159.65/50.43 ms，100k 为 49.85/214.08 ms，保留 [首次样本](evidence/Quality-performance-before-initial.json)、[补测](evidence/Quality-performance-before-repeat.json)。不据这些波动数据宣布端到端响应门槛达成；CPU 指标不含 GPU 呈现、系统合成器与真实设备输入。后续性能定位仍开放，未声称比旧 UI 提升某个比例。

系统打印把完整 PDF 页（含原页边距）等比放入驱动的可打印区域，使用最大 1200×2000 的栅格页；纸面细节、纸张匹配及设备实际输出按 PRINT-2/3 验收。PDF 导出沿用原核心输出。辅助节点数量检查对应常用测试视口；极端窗口尺寸/缩放的成本仍随可见区域变化。

M8-preview4 补充：Windows 编辑波动和大表 CPU 分段基线、优化前后两轮对照、范围聚合和包内复测入口见 [PERFORMANCE.md](PERFORMANCE.md)。本记录中的 M8-preview3 数值属于历史运行；真实 IME/DPI/读屏/GPU 与实际打印队列的人工门槛保持有效。
