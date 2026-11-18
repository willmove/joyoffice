# JoyOffice 重构与自研替换路线图

实施起点为统一版本 0.2.0、宿主提交 `06aff812`。目标是让用户在复杂文档中输入、修改、保存和重开时获得可验证的改善，并让团队能独立维护关键算法。按产品、场景和可退出的旧调用计进度。各阶段沿用同一可编辑模型、桌面／CLI 会话和现行文件格式。平台记录注明构建、设备和实际操作。信创真机复验可后补。

## 当前剩余任务

2026-11-18 核对至 `6a301fd0`。首轮代码和证据已在任务分支，核对时远端 `main` 为 `f04d8aa5`。Word 另存为已在 macOS 和 UOS 验收，Sheet 标量试点已完成第一轮采用决策。阶段 0 已有这三个试点的语料、探针和基线，后续每个替换组件仍需各自的样本与对照。[首轮验证](validation/20261118-refactor/README.md) · [UOS 实机记录](validation/20261118-refactor/UOS.md)

| 优先级与范围 | 当前位置 | 下一项可交付结果 |
| --- | --- | --- |
| P0 · 中文输入 IME-2／3 | macOS 三产品通过。UOS 已定位到 XIM 注册无所有者，最小 GPUI 控件和三产品均收到直通拼音 | 待同一桌面的 XIM 恢复，先用最小控件确认有组合态，再在 Word 正文、Deck 备注、Sheet 单元格分别选词、取消、切焦点、撤销并保存重开，留候选截图与读回结果。其余任务可独立推进 |
| P1 · Sheet 重算推广 | 100k 行桌面引擎 P95 为 71.14 → 64.10 ms，降约 9.9%，目标 15%。100 行纯重算 P95 由 0.041 升至 0.063 ms。UOS 已验证默认路径的正确性 | 保持试点默认关闭。先分测依赖发现、求值准备及写回，选下一处热点，同机交替复跑大小表和代表性业务工作簿。用户可见延迟另测 GPU 实际呈现，现有首帧记录到 CPU 绘制 |
| P1 · Word 分页与 DOCX 读取 | 已有单段几何不变时的就地重排。中段增删行后的跨页重排、OPC 预检后的重复读取仍待优化 | 分别提交千页中段编辑的逐页差分及输入 P95、同一 DOCX 预检到 reader 的解压字节与峰值内存对照。另用固定字体字节与 face 复核 UOS 的 5 项既有排版失败。[现有增量路径](../../crates/joyword-layout/src/incremental.rs) |
| P1 · Deck 渲染与几何 | 场景构建仍对媒体内容做摘要，圆角矩形和箭头仍走现有预设分派 | 先证明只改标题时未变媒体的重新哈希字节为 0，同 ID 换图能立即更新。再按形状族交付屏幕、命中和 PPTX 往返差分 |
| P1 · Sheet 后续计算和模型 | 首轮只覆盖同表数值常量格与字面量的部分四则运算 | 用跨表、范围、动态数组和迭代样本确定下一类适用表达式，逐格检查脏集、错误传播、汇总及撤销写回。每类单独作采用决定 |
| P0／P1 · 三款桌面应用共享 | 已核对三个桌面入口和三个 CLI 的实际依赖，首批重复实现已定位 | Word／Sheet 的保存更改提示框已完成双平台验证。Word／Deck 图元有界读取已合并并按屏幕场景及 PDF 逐像素对照，下一项为异步另存契约。负责人、第二个调用方和回退路径见下文[桌面应用共享复用](#桌面应用共享复用) |
| P2 · 三产品旧组件退役 | 本轮仍使用 WordCraft、DeckCraft、GridCraft 派生实现 | 按能力删除已替代的生产调用，核对 GUI、CLI、导出及发行包闭包，更新来源账本、SBOM 和资产归属，用上一可读版本演练回退 |

## 分批交付

P0 处理用户阻断和数据完整性，P1 替换有测量依据的热路径，P2 扩展覆盖与退出旧实现。以下百分比是试点目标。先在同机、同语料和同一 Release 配置下建立波动范围，再决定是否切换。

| 阶段与负责模块 | 优先级与组件 | 前置依赖 | 交付与验收 | 失败后的回退 |
| --- | --- | --- | --- | --- |
| 0 · 三产品宿主 | P0 来源、测量、文件保护 | Word R8、Deck R9、Sheet M8 的来源及验收清单 | 为每个试点列出旧入口、生产调用方、固定语料、规范依据、差分结果和负责人所属模块。记录原件哈希、旧格式重开、取消与失败重试 | 保留旧实现与已发布包，所有写回对照使用独立临时副本 |
| 1 · 三产品 UI | P0 中文 IME 接入与焦点 | 同机系统拼音、最小 GPUI 输入控件、三个可编辑入口；历史线索来自 UOS X11 | Word 正文、Deck 备注、Sheet 单元格分别完成候选、选词、取消、焦点切换和一次撤销；记录实际候选与持久化文本 | 仅回退有证据的输入适配改动，保留原键位与组合态保护 |
| 1 · Word UI／shell | P0 原生另存事务 | 本机原生对话框、DOCX 语料副本、可观察的保存阶段；已有 DDE 上的 Word／Deck 对照 | 单次请求、取消后可重试，中文空格路径 DOCX 独立读回，源文件哈希不变，失败不关闭未保存会话 | 恢复上一版对话框适配，继续使用原有验证、另存与恢复事务 |
| 1 · Sheet calc／app | P1 GridCraft 密集标量重算试点 | 固定扇出与汇总语料、单写者、原通用求值器 | 逐格值／类型／错误、公式、撤销与 XLSX 重开一致；候选目标为 100k 引擎 P95 降低 15%，普通小表 P95 和峰值 RSS 回退不超过 5%；另测提交到首帧 | 候选按适用域选择；差异、收益不足或预算回退时，生产继续走原路径 |
| 2 · Word layout | P1 WordCraft 分页与失效判定，先做单栏正文增删行 | 阶段 0；事务给出最早改动块；完整布局可对照 | 千页中段增删行；逐页比较断行、页末状态、光标命中及 PDF，记录受影响页数、键入到提交 P95；随后逐项加入表格／分节／脚注 | 无法证明页末状态收敛时走完整分页 |
| 2 · Word DOCX／OPC | P1 包预检到 reader 的重复读取 | 记录解压字节、解析次数、峰值 RSS；对齐两层输入预算 | 在同一文档集证明减少重复解压；关系、未知 XML、只读／拒绝、保存重开结果等价；OPC 只承担包语义 | 还原原预检和专用 reader，保留原包字节 |
| 2 · Deck render | P1 媒体指纹及场景失效 | 富媒体 PPTX、对象缓存统计、CPU 图像对照 | 改标题时未变媒体重新哈希的字节数降为零；同 ID 换图、撤销、母版、字体、DPI 更新正确。分记构建、提交、实际呈现及缓存峰值 | 退回内容摘要重算和现有 CPU 对照路径，清空派生缓存 |
| 2 · Deck geom | P1 DeckCraft 预设几何，圆角矩形和箭头起步 | 固定 OOXML 预设名、调整参数及独立样例 | 每族覆盖默认／极值、命中、文字框、连接点、屏幕与 PPTX 往返；列出已移除的旧分派 | 按形状族恢复旧生成器，保留原预设名和参数；legacy 桥只读 |
| 2 · Sheet calc／model | P1 扩大已获益表达式、依赖或写回子集 | 试点差分通过，计时确认下一热点 | 增加跨表、范围、溢出阻塞／收缩及迭代语料；对照脏集和最终格值，不改变浮点累加顺序 | 按表达式或操作恢复通用路径，缓存可丢弃重建 |
| 3 · 三产品分别退出旧调用 | P2 Word 校对／字体目录及复杂分页、Deck 后续形状族／合成、Sheet 其余公式族及格式适配 | 各切片均有独立规范、兼容语料和第二个可读版本；公共能力有两个实际调用方 | 按能力核对 GUI、CLI、导出和发行包依赖闭包；清除已退出的生产调用，更新来源账本、SBOM、字典／字体等资产归属和回退演练 | 保留可重建的旧版本，独立打开公共格式副本；出现未处理对象时遵循现有只读或拒绝契约 |

同一轮只改变一个可独立回退的行为。文件语义用 [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) 和 [Office 实现说明](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/) 核对；原始来源指纹与本地独立实现证据分别保存。公共依赖、锁文件或构建边界变动时跑三产品回归。GPUI、塑形、压缩和密码学底座按实际限制另行评估。

## 从当前代码切入

- **Word**。用户看到的长文延迟由编辑事务、正文布局与页面绘制共同决定。先在同一次键入分别测这三段，再用当前完整布局作为逐页对照。字体、分节和页字段影响后续页面，增量路径必须能解释从哪一页开始、到哪一页可以停止。文件侧另有直接入口，宿主预检当前构造并丢弃已读 OPC 包，复用预检结果时应保留源字节、预算和诊断。负责模块及来源见[编辑会话与布局](../../crates/joyword-engine/src/session/layout.rs)、[容器预检](../../crates/joyword-shell/src/reader.rs)、[WordCraft 来源](../../licenses/wordcraft/SOURCE.json)。
- **Deck**。当前场景构建会遍历演示文稿媒体并逐个计算 SHA-256，改一个标题也会支付这笔成本。先让导入和替换媒体的路径维护内容指纹，保持场景对母版、字体及分辨率的失效条件。几何生成器独立按形状族交付，编辑、预览与导出共享同一套参数。负责模块及来源见[场景构建](../../crates/joydeck-render/src/scene.rs)、[预设几何](../../crates/joydeck-geom/src/preset.rs)、[DeckCraft 文件映射](../../licenses/deckcraft/file-map.json)。
- **Sheet**。一次改格会经过依赖发现、求值准备、求值、写回和桌面呈现。先在现有调度器中替换有独立数值语义的一类表达式，保留原求值器作差分对照。即使内核达标，也应按用户提交到首帧的测量决定是否推广。负责人、来源和现有本地改动见[计算入口](../../crates/joysheet-calc/src/recalc.rs)、[求值调度](../../crates/joysheet-calc/src/recalc/pass_host.rs)、[GridCraft 来源](../joysheet/upstream-source.json)、[改动账本](../joysheet/UPSTREAM-PATCHES.md)。

切换单个组件时，由上表负责模块提交同一语料的新旧输出、最差差异、端到端耗时与回退操作。打开旧文件必须经过相同的输入预算和只读判定；试点失败时先关闭新路径，用上一个可读版本打开试点保存的副本，独立核验后再清理派生缓存。保留来源清单与仍在使用的资源声明，逐个核对生产依赖闭包后更新退役状态。

## 桌面应用共享复用

### 已在生产入口使用的公共代码

在本机 macOS 默认特性下，逐一核对六个应用的 Cargo 普通依赖和构建依赖，并追到产品调用处。表中使用 `joyoffice-*` 的后缀。三款桌面共用主题、控件表面、图标和窗口品牌设置，三个 CLI 的依赖闭包均无 GPUI，六个入口均无其他产品的私有 crate。[公共 UI](../../crates/joyoffice-ui/src/lib.rs) · [原生窗口图标实测](validation/20261118-desktop-icons/README.md)

| 产品 | 桌面可达的公共库 | CLI 可达的公共库 | 文件格式的实际使用范围 |
| --- | --- | --- | --- |
| Word | `ui`、`theme`、`opc`、`metafile` | `theme`、`opc`、`metafile` | 宿主用 OPC 做 DOCX 预检及保存规划，旧接口也用它检查文件。完整 DOCX 导入与写回仍有专用处理。[预检](../../crates/joyword-shell/src/reader.rs) · [保存规划](../../crates/joyword-shell/src/storage/plan.rs) |
| Deck | `ui`、`theme`、`opc`、`metafile` | `theme`、`opc`、`metafile` | shell 及旧格式只读桥使用公共 ZIP／预算接口。PPTX 与演示文稿模型转换仍由产品维护。[打开文稿](../../crates/joydeck-shell/src/document.rs) · [只读桥](../../crates/joydeck-legacy/src/document.rs) |
| Sheet | `ui`、`theme`、`cfb` | `cfb` | XLSX 嵌入 OLE 对象使用公共 CFB writer。工作簿包、旧 XLS 和加密容器的读取仍有产品实现。[嵌入对象](../../crates/joysheet-xlsx/src/drawing_objects.rs) · [包读取](../../crates/joysheet-xlsx/src/package.rs) · [CFB 读取](../../crates/joysheet-xlsx/src/cfb.rs) |

`joyoffice-drawingml` 与 `joyoffice-omml` 已有实现，当前六个入口的普通／构建依赖均未用到。复核时可替换下列命令的产品名。依赖图确认库可达，实际功能覆盖由上表中的调用处和往返测试确认。

```sh
cargo tree --locked --offline -e normal,build -p joyword
cargo tree --locked --offline -e normal,build -p joyword-cli
```

### 实施队列

| 阶段／优先级／负责方 | 两个首用产品与共享范围 | 前置依赖与验收 | 回退 |
| --- | --- | --- | --- |
| 1 · P0 · 三产品桌面验收 | **保存与输入契约**。从 Word、Deck 开始，再接 Sheet。共用候选、焦点、选词与文件读回断言，以及取消、重试、延迟返回的测试场景。Word／Deck 使用 GPUI 文件选择器，Sheet 由 platform 调用 rfd | 复用首轮原生探针。保存面板打开期间切换文档或关闭窗口，核对原件摘要、草稿归属和写入次数。优先覆盖 Sheet 普通另存，其回调按窗口提交命令，关闭前保存另有文档和版本检查。按三种编辑位置分别验收 IME | 可按产品恢复原探针和选择器适配，保留已有个人配置、文件与保存事务。[Word](../../crates/joyword-ui-gpui/src/save.rs) · [Deck](../../crates/joydeck-ui-gpui/src/chrome_nav.rs) · [Sheet 普通另存](../../crates/joysheet-ui-gpui/src/desktop.rs) · [关闭前保存](../../crates/joysheet-ui-gpui/src/palette.rs) |
| 1 · P1 · `joyoffice-ui` 与 Word／Sheet UI | **保存更改提示框**。已合并两款产品的遮罩、面板和三按钮区。两处现已调用同一呈现组件，由产品传入文案、状态、可访问名称和动作。Deck 现有关闭流程单独验收 | 以现有 450 逻辑像素面板和两产品的关闭用例为基线。两处旧面板移除后，从正常窗口核对 Esc、Tab、取消、放弃、失败重试和保存后的读回。Word 另验替换原件确认，Sheet 另验多窗口目标 | 逐产品切回原面板，保留各自的关闭及保存状态机。[Word／Sheet 现有对照](validation/20261117-joyword-save-dialog/README.md) |
| 2 · P1 · Word／Deck metafile | **WMF／EMF 有界读取**。原两份 `bytes.rs` 的 SHA-256 均为 `ef49693a…ee606123`。现已共用 [joyoffice-metafile](../../crates/joyoffice-metafile/src/lib.rs) 的记录边界和坐标读取。Deck 的裁剪区域和 EMF+ 继续由其解析器处理 | 先固定截断、长度溢出、16／32 位坐标和正常图片语料。双产品逐例对照读取结果、错误、资源预算、屏幕与 PDF 输出。验收时两个产品调用同一实现，两份私有读取逻辑退出 | 按解析器切回原读取实现并重放同一语料。[Word 读取器](../../crates/joyword-metafile/src/bytes.rs) · [Deck 读取器](../../crates/joydeck-metafile/src/bytes.rs) |
| 2 · P1 · `joyoffice-opc` 与产品格式层 | **包预检与受限读取**。先解决 Word 重复解压，再把验证过的读取接口用于 Deck 的包预检。Sheet 的 XLSX 按需读取和未知部件保留单列对照 | 两产品先分别记录每个部件的读取次数、解压总字节和 RSS。迁移后重复工作下降，关系目标、预算超限、损坏文件判定相符，未知部件原始字节和旧版重开通过 | 单产品恢复其原预检／reader，保留源包及产品格式适配。[OPC](../../crates/joyoffice-opc/src/lib.rs) · [Sheet 未知部件](../../crates/joysheet-xlsx/src/passthrough.rs) |
| 2 · P1 · `joyoffice-ui` 与各产品 UI | **功能区分组和启动按钮**。Word／Sheet 先共用标题、分组和格式面板启动按钮的呈现。Deck 按紧凑布局接入适用部分，命令目录、选区与执行回调由各产品持有 | 以现有三个桌面的功能区折叠用例为基线。覆盖中英文、宽窗、窄窗和键盘导航，比较可访问名称、启用态、折叠行为与实际命令，保留产品快捷键 | 按分组恢复旧组件，保留控件 ID。[Word 分组](../../crates/joyword-ui-gpui/src/ribbon.rs) · [Sheet 分组](../../crates/joysheet-ui-gpui/src/ribbon_widgets.rs) · [Deck 分组](../../crates/joydeck-ui-gpui/src/chrome.rs) |
| 2 · P1 · Word／Deck fonts | **字体发现与 face 标识**。先比对两端的系统字体枚举、实际字节和集合 face，再提取相同的目录或标识逻辑。正文断行、幻灯片文本适配及各产品回退顺序留在产品层 | 使用同名字体不同字节、同一 TTC 不同 face 和字体文件变化样本。固定字体的行末、命中和 PDF 金样一致，记录冷扫描耗时和缓存失效。UOS 的 5 项排版差异保留作对照 | 各产品可恢复原目录或身份实现，重建派生缓存。[Word 字体](../../crates/joyword-fonts/src/lib.rs) · [Deck 字体](../../crates/joydeck-fonts/src/lib.rs) |
| 3 · P2 · 公共格式与产品导入层 | **CFB、DrawingML、OMML 按子集接入**。CFB 先比较 Word 旧 DOC 与 Sheet XLS 的流读取。绘图与公式 XML 分别列出两个有对应内容的实际入口，再选择一种对象接入 | 先补两产品各自的真实样本、预算、兼容读回和拒绝用例。只有双方生产入口均接入且原始部件保留、损坏输入判定与旧版互读相符，才计为共享 | 按对象或容器切回产品 reader，保留未解释的数据。[Word 旧 DOC](../../crates/joyword-doc/src/container.rs) · [DrawingML](../../crates/joyoffice-drawingml/src/lib.rs) · [OMML](../../crates/joyoffice-omml/src/lib.rs) |

**建议开工顺序。** Word／Sheet 保存更改提示框已接入。Word／Deck 的 WMF／EMF 读取也已接入并通过同机前后两套二进制的像素对照。UOS 输入验收在 XIM 可用后独立复测。这样第一批就覆盖三个桌面应用，每个新组件都有两个能从产品入口验收的调用方。

**进度口径。** 共享复用记录调用产品、删除的重复路径和回退结果。自研替换另外记录独立规范来源、差分证据和已退出的上游调用。合并现有上游代码只增加共享覆盖，原始来源与许可继续登记。WMF／EMF 的独立实现可以分别从微软的公开格式规范取证。[MS-WMF](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/) · [MS-EMF](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emf/)

每次改动共享库，运行消费它的产品检查。公共 UI 的三个入口是 `cargo xtask check joyword`、`cargo xtask check joydeck`、`cargo xtask check joysheet`。公共依赖或构建边界变化时补跑三 CLI 的 headless 闭包和三产品联合构建。格式与几何代码用固定语料对照 CPU、内存及往返结果，保留已有产品独立构建、发包和回退的能力。[分层门禁](../../scripts/check_gates.py) · [产品工程入口](../../tools/xtask/product.py)

## 首轮任务与顺序

首轮先在本机 macOS 验收，然后在 UOS 25 X11／DDE 复测。IME 使用各桌面的系统拼音与同机 GPUI 对照；SAVE 使用各平台的原生面板。

| ID／负责模块 | 依赖 | 可执行步骤与完成证据 |
| --- | --- | --- |
| IME-1 · 桌面平台 | 阶段 0 | 记录活动输入源及实际焦点，用最小 GPUI 控件与三个产品复测。macOS 观测 marked text 和原生候选面板；X11 记录 locale、XMODIFIERS、XIM_SERVERS 及对应 selection 的实际所有者。 |
| IME-2 · 故障所在层 | IME-1 定位 | 从预编辑到提交跟踪事件；在有证据的最窄层修复。覆盖候选定位、Escape、失焦、一次提交／撤销，普通快捷键仍生效。 |
| IME-3 · 三产品 UI | IME-2 | 同一桌面三产品分别选词并重开文件；保存截图、组合态和模型读回。平台不可用时保留探针与待验状态。 |
| SAVE-1 · Word UI | 阶段 0 | 对照 Deck 的建议文件名、起始目录、对话框结果、文档身份和落盘回调；为超时划分“等待选择／收到路径／开始写入／完成或出错”。 |
| SAVE-2 · Word UI／shell | SAVE-1 | 修正可证明的参数或生命周期故障；验证取消／重试、对话框重复打开、过时结果、保存失败保留草稿，中文路径独立解包读回。 |
| SAVE-3 · 桌面验收 | SAVE-2 | macOS 与 UOS 实测原生另存到含中文和空格的路径、取消再试和失败重试；原件摘要不变，独立读取目标并在新桌面进程重开。DDE 使用 F12 和 Deck 同机对照。 |
| CALC-1 · Sheet calc | 阶段 0 | 同机 Release，构造 10k／100k 直接依赖及 SUM；预热后至少 20 次编辑，记录原始分段、求值数、结果、P50／P95。历史 Windows 12 次样本仅作参考。 |
| CALC-2 · Sheet calc | CALC-1 | 按复测热点实施候选；数值标量可先限定直接引用和四则运算。记录适用域、错误／空值／跨表和复杂公式回退；对照原求值结果。 |
| CALC-3 · Sheet app／发布 | CALC-2 | 同机交替运行基线与候选；补小表、撤销、重开、峰值 RSS 和桌面首帧。指标足以采用时分批切换；不足时记录结果并保留基线。 |

历史证据与执行入口见 [UOS 原生桌面](validation/20261117-uos-desktop/README.md)、[Word 千页基准](../joyword/r8/VALIDATION.md)、[Deck 对照与开放项](../joydeck/TASKS.md)、[Sheet 性能对照](JOYSHEET-RESULT.md)、[三产品边界](ARCHITECTURE.md)。本轮执行结果在本页末尾追加，区分实现、自动验证、原生实测及待验。

## 首轮执行记录（2026-11-18）

| 任务 | 本轮结果 | 下一项或切换决定 |
| --- | --- | --- |
| IME-1 | 本机通过。GPUI 单输入框在系统拼音下出现 marked text，选词、取消、一次撤销均通过。 | 保留探针作为框架对照，报告记录实际输入源。 |
| IME-2／3 | 本机通过。Word 正文、Deck 备注、Sheet 单元格都实拍到插入处的候选面板。选中“中文”、取消第二段拼音、用快捷键撤销和重做后，三个产品分别在新进程重开并读回。Sheet 默认列宽下的预编辑裁切已修复。 | 保留 Sheet 组合输入期间临时扩展编辑框的改动，回归包含相同单元格上的截图和整行拼音识别。 |
| SAVE-1／2 | 本机通过。重复命令只发起一次选择；回调按文档身份处理，取消释放状态。阶段日志与回归覆盖选择、校验、写入和读回。 | 保留 Word 另存事务修复。 |
| SAVE-3 | 本机通过。原生面板取消、拒绝错误扩展名后草稿仍在，重试中文空格路径成功。再次普通保存到副本，新进程读回两次修改；原件摘要一致。 | 随回归保留本机复跑入口。 |
| CALC-1／2 | macOS M4 Release 已建立基线，开发构建可用 `joysheet-calc/scalar-pilot` 比较同表数值常量格与字面量的四则运算。两条路径共用原有依赖图、批量写回和历史。 | 迭代／本轮已有溢出、其他表达式和数据类型继续交原求值器。 |
| CALC-3 | 100k 每种配置 40 个计时样本，P95 纯重算 41.62 → 34.32 ms，桌面引擎 71.14 → 64.10 ms，提交到首次匹配 CPU 绘制 82.17 → 78.69 ms。桌面峰值 RSS +0.60%；撤销、重做、XLSX 双重读回及 7 份仓库样本往返对照通过。 | 桌面引擎降幅约 9.9%，低于 15% 提案目标，小表纯重算也有回退。保留显式试点，普通发布构建继续使用通用路径。 |

样本、硬件、构建指纹、复跑方式与未测项见[首轮验证](validation/20261118-refactor/README.md)。

### UOS 25 X11／DDE 补验

| 任务 | UOS 结果 |
| --- | --- |
| IME-1 | 定位到 Fcitx 的 XIM 注册失去所有者。根窗口仍列出 `@server=fcitx`，但 XGetSelectionOwner 返回 0，XOpenIM 返回空。GTK 经 Fcitx D-Bus 切到搜狗中文模式后能输入“中文”。 |
| IME-2／3 | 最小 GPUI 输入控件和三产品仍无组合态。Word、Deck、Sheet 各自复现拼音直通。UOS 系统拼音验收开放。 |
| SAVE-1／2／3 | Word 在 DDE 面板取消后保留草稿，拒绝 `.txt`，然后将同一编辑另存为“重构 副本.docx”。独立 XML 与新进程读回通过，原文件摘要一致。Deck 的原生另存与三页 PPTX 读回也通过。 |
| CALC-3 | 默认构建 100k 行桌面重算经过 20 次编辑，每次求值 100,001 格，撤销、重做、独立 XLSX 读回及重开通过。 |

见[UOS 实机记录](validation/20261118-refactor/UOS.md)。


## 共享改造实施记录（2026-11-18）

**保存更改提示框已交付。** JoyWord 和 JoySheet 从实际关闭／新建流程调用同一面板、提示文字布局和动作区。产品仍控制文档身份、错误重试和关闭时机。macOS 与 UOS 的 Word 通过系统键盘验证了取消、确认替换及新进程读回。两端的 Sheet 均实际点击取消、保存及放弃按钮，模拟临时写入路径被占用后保留草稿，重试只关闭目标工作簿。两端三产品均运行产品回归，UOS Word 仍复现原有的 5 项排版失败。[运行记录与复跑命令](validation/20261118-desktop-sharing/README.md)

**WMF／EMF 读取已交付。** 两个图元解析器共用同一个有界字节读取器，原始来源和许可已登记。macOS 和 UOS 各运行四个文档的页面场景及 PDF 导出。每台机器的八份 RGB 输出与其改动前基线逐字节相同，图内均检出测试三角形。两产品各自的记录预算、绘图及导出仍从原入口验收。[回归与像素摘要](validation/20261118-desktop-sharing/README.md#wmfemf-有界读取)
