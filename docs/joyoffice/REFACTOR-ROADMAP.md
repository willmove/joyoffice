# JoyOffice 重构与自研替换路线图

实施起点为统一版本 0.2.0、宿主提交 `06aff812`。目标是让用户在复杂文档中输入、修改、保存和重开时获得可验证的改善，并让团队能独立维护关键算法。按产品、场景和可退出的旧调用计进度。各阶段沿用同一可编辑模型、桌面／CLI 会话和现行文件格式。平台记录注明构建、设备和实际操作。信创真机复验可后补。

## 当前剩余任务

核对基线为 `f04d8aa5`。保存提示框和图元读取已分别在 `b8fd5500`、`7ed56875` 提交。中文输入、Word 另存和 Sheet 重算三个首轮试点均已完成双平台验收。下表把后续仍有生产调用的范围列在相应产品下。[首轮证据](validation/20261118-refactor/README.md) · [本轮双平台验收](validation/20261118-desktop-sharing/README.md)

| 优先级与负责模块 | 已经替换且经过验证的范围 | 下一项交付及回退 |
| --- | --- | --- |
| P0 · 三产品 UI 和保存事务 | macOS 系统拼音、UOS 25 DDE 独立 Fcitx5 拼音和用户默认搜狗均完成三产品选词、取消、撤销、保存和重开。Word 原生另存取消可重试，Sheet 绑定工作簿版本，Deck 在第二个窗口选择期间能拒绝第一个窗口编辑前的旧结果 | 本轮 P0 全部通过。输入回归分别保留客户端与服务端预编辑。保存回退按单产品恢复选择器，保留文档版本和单写者校验。[输入](validation/20261118-desktop-sharing/README.md#uos-中文输入) · [原生另存](validation/20261118-desktop-sharing/save-target.json) |
| P1 · Word 排版、OPC | DOCX 重复解压由 120,016 字节降为 0。千页中段改字每次更新三页。自然分页的 800 段正文、96 页文档里的嵌入表格已和完整分页逐页、逐命中对照。五类复杂 DOCX 另经两代保存和旧版读回 | 含连续分节、等宽双栏和脚注的长文档，现可按章节重新计算栏平衡、脚注留白及浮动／嵌套表格，并核对后一节的真实首段。独立边界和页数一致时复用两侧页面，跨出边界时自动完整分页。[千页耗时](validation/20261118-desktop-sharing/chapter-reflow.json) · [复杂 DOCX](validation/20261118-desktop-sharing/complex-docx.json) |
| P1 · Deck 渲染、几何 | 富媒体首轮哈希 519,969 字节，随后 25 次移动对象合计 0 字节。已测同 ID 换图、撤销及真实桌面重绘。圆角矩形、四个单向和两个双向箭头共 28 个实例通过预设参数、两代读回及逐像素对照 | 透视层 CPU 三角形改为按扫描行求覆盖区。49 种转场、185 种效果／选项的 925 帧在两端与旧版逐像素相同。九种矩形预设也已全部接入新的参数化角点生成器。45 个带文字实例在两端新旧版本输出相同，几何手柄另验拖动至极值。其余几何按族回退和复验。[场景与显示帧](validation/20261118-desktop-sharing/deck-media.json) · [几何](validation/20261118-desktop-sharing/deck-presets.json) |
| P1 · Sheet 计算 | 至少 512 个脏公式时，合格的同表／跨表数值引用加字面量默认批量计算。100k 行引擎 P95 在 macOS 为 71.23 → 50.63 ms，在 UOS 为 205.17 → 155.91 ms。两端小表和 RSS 均在 5% 预算内。另验混合范围、公式引用、溢出阻塞／收缩和迭代 | 清空远端阻塞格或原溢出区域收缩时，现仅搜索所在工作表内可能再次溢出的锚点。循环引用分组和迭代轮次调度已替换。动态表达式继续由通用求值器解释。已用同一发布二进制实际演练 `JOYSHEET_SCALAR_RECALC=0`、撤销、保存和重开。下一次扩大条件沿用逐格检查及同源计时。[引擎](validation/20261118-desktop-sharing/same-source-recalculation.json) · [实际显示](validation/20261118-desktop-sharing/desktop-presentation.json) · [回退](validation/20261118-desktop-sharing/scalar-rollback.json) |
| P1 · 桌面共享 | Word／Sheet 共用保存提示；Word／Deck 共用 WMF／EMF 有界读取、OPC 已验成员和系统字体遍历；三应用共用功能区分组与有界 ZIP 成员解压，Sheet 保留按需加载。两平台完成 20 组原生可访问树验收，其中 Word／Sheet 的 16 组实际打开并关闭了字体面板 | 功能区命令由产品负责，公共呈现可按产品回退。Deck 现有中文界面和 UOS 最小窗宽分别按真实环境记录。[功能区](validation/20261118-desktop-sharing/ribbon.json) · [字体](validation/20261118-desktop-sharing/font-catalog.json) |
| P2 · Word 旧调用退役 | `craft-font-catalog` 已退出桌面和 CLI 发行依赖。宿主字体清单检测变化，已有语言包目录继续可读。自动与手动断词对所有语言应用自研的段落限制；十个语言标签的 390 组对照覆盖大小写、组合符与复合词。多语拼写现直接流式遍历原文词区间。Liang 断词按前两个字母筛选可能的模式长度，已在五种随附语言数据上逐位置对照。DOC 与 XLS 共用 CFB 扇区，Word 与 Sheet 共用 OMML 上下标 | 拼写词形、部分归档解包及复杂分页仍有旧实现。原生窗口和 CLI 已从既有语言包目录读回断词数据；75 个正式语言包的 450 个输入已核对错误位置和建议顺序；下载校验、词典发布、卸载及进程中断恢复已替换。75 个包已由新旧版交替安装并逐文件核验。下一项为词形、候选生成与包清单／解包。[安装事务](validation/20261118-desktop-sharing/pack-publication.json)[语言包入口](../../crates/joyword-engine/src/packs/store.rs) · [发行依赖](validation/20261118-desktop-sharing/distribution.json) |
| P2 · Deck、Sheet 旧调用退役 | Deck 已移除双向箭头和矩形族的旧分派；普通和投影纹理均有自研的覆盖合成路径。Deck／Sheet 共用 DrawingML RGB。Sheet 十个汇总／逻辑函数改为保序归约；中位数、众数族、大小顺序统计与频数直方图也已替换。新旧版均读回七份工作簿并核对 35 个独立公式期望值。六种查找函数已共用按原数组行／列读取的路径，八个 XLSX 查找公式另经四种读写组合验收 | 其余形状族、特效层生成、其余公式族及 XLS／XLSX 格式语义逐项迁移。每次核对旧版文件读回和来源／许可。Deck 只读迁移桥继续承担旧文件入口。[公式与工作簿](validation/20261118-desktop-sharing/version-rollback.json) · [来源账本](../joysheet/UPSTREAM-PATCHES.md) |


### 尚未退役的生产代码

| 编号与优先级 | 下一项可验收的交付 | 前置条件与验收指标 | 回退 |
| --- | --- | --- | --- |
| W-LAYOUT · P2 | P1 章节重排已覆盖脚注增高、连续分节栏平衡、带浮动定位的嵌套表格和真实窗口编辑。P2 继续替换分节页数改变、页眉页脚／字段、浮动绘图锚点和跨节续注时的传播 | 复用已通过的自然页、表格、双栏脚注和千页桌面基线。对每次新增、删除和撤销比较完整布局的页、行、脚注编号、命中与 PDF；只有下游检查点一致才复用 | 每次编辑都可走已有完整分页，保留原件 |
| W-PROOF · P2 | 建议的带权编辑距离和最终排序已替换。正式索引及词典下载、安装、卸载和中断恢复也已接入 JoyOffice 事务。继续迁移 Hunspell 词形判定、候选生成、各语言特例及索引语义／归档解析 | 沿用旧版语言包目录、已登记的授权数据及用户同意记录。75 个发行包的建议顺序已跨版本验证；扩大到每种语言的派生词形与断词位置。安装测试覆盖实际发行包、摘要错误、取消和旧目录读回 | 按语言保留旧词典后端。恢复原安装器使用同一目录、许可与索引 |
| D-GEOM · P2 | 继续交付流程图、复合箭头、标注等形状族，以及转场层生成和 GPU 呈现 | 画廊登记 183 种预设。已覆盖九种矩形、六种箭头及六种画廊线条／连接线。另五种只从文件导入的连接线按标准补齐拐点；其他预设仍须逐族证明。按默认、非对称和边界参数比文字框、连接点、命中、光栅和两代 PPTX，GPU 效果与 CPU 对照 | 按族恢复生成器或 CPU 渲染。历史文件继续走现有只读迁移桥 |
| S-CALC · P2 | 其余公式族、数组扩展和动态引用求值，及 XLS／XLSX 的产品格式映射 | 两端 CLI 登记 514 个公开函数名、14 种分类。已替换的汇总、统计、查找、循环分组和迭代调度有独立期望；其余按函数族补边界值、错误类型、浮点顺序以及旧版保存读回。文件适配另比格式、未知对象和拒绝结果 | 按函数或对象切回旧路径。密集标量批处理可单独在同一程序中关闭 |

以上登记数包含别名和共用实现，不用来计算自研比例。P0 的三个桌面输入和另存阻断均已关闭。[字体与语言包来源](../../licenses/craft-libs/SOURCE.json) · [形状来源](../../licenses/deckcraft/file-map.json) · [表格迁移账本](../joysheet/UPSTREAM-PATCHES.md)

**最近一次各产品回归。** 采用同一任务分支上各产品最后一次影响其代码的提交。macOS 的 Word、Deck、Sheet 分别通过 3,679、998、1,471 项，UOS 25 真机分别通过 3,683、1,003、1,471 项。三个产品的失败数均为零。首轮 P0、两项指定共享改造及路线图阶段 2 所列 P1 场景已验收。上表四项仍包含运行中的旧代码。[Word 安装与恢复](validation/20261118-desktop-sharing/pack-publication.json) · [Word 校对](validation/20261118-desktop-sharing/spelling-ranking.json) · [Word 排版](validation/20261118-desktop-sharing/section-reflow.json) · [Deck](validation/20261118-desktop-sharing/deck-connectors.json) · [Sheet](validation/20261118-desktop-sharing/sheet-lookups.json)

## 分批交付

P0 处理用户阻断和数据完整性，P1 替换有测量依据的热路径，P2 扩展覆盖与退出旧实现。以下百分比是试点目标。先在同机、同语料和同一 Release 配置下建立波动范围，再决定是否切换。

| 阶段与负责模块 | 优先级与组件 | 前置依赖 | 交付与验收 | 失败后的回退 |
| --- | --- | --- | --- | --- |
| 0 · 三产品宿主 | P0 来源、测量、文件保护 | Word R8、Deck R9、Sheet M8 的来源及验收清单 | 为每个试点列出旧入口、生产调用方、固定语料、规范依据、差分结果和负责人所属模块。记录原件哈希、旧格式重开、取消与失败重试 | 保留旧实现与已发布包，所有写回对照使用独立临时副本 |
| 1 · 三产品 UI | P0 中文 IME 接入与焦点 | 同机系统拼音、最小 GPUI 输入控件、三个可编辑入口；历史线索来自 UOS X11 | Word 正文、Deck 备注、Sheet 单元格分别完成候选、选词、取消、焦点切换和一次撤销；记录实际候选与持久化文本 | 仅回退有证据的输入适配改动，保留原键位与组合态保护 |
| 1 · Word UI／shell | P0 原生另存事务 | 本机原生对话框、DOCX 语料副本、可观察的保存阶段；已有 DDE 上的 Word／Deck 对照 | 单次请求、取消后可重试，中文空格路径 DOCX 独立读回，源文件哈希不变，失败不关闭未保存会话 | 恢复上一版对话框适配，继续使用原有验证、另存与恢复事务 |
| 1 · Sheet calc／app | P1 GridCraft 密集标量重算试点 | 固定扇出与汇总语料、单写者、原通用求值器 | 逐格值／类型／错误、公式、撤销与 XLSX 重开一致；候选目标为 100k 引擎 P95 降低 15%，普通小表 P95 和峰值 RSS 回退不超过 5%；另测提交到首帧 | 候选按适用域选择；差异、收益不足或预算回退时，生产继续走原路径 |
| 2 · Word layout | P1 WordCraft 分页与失效判定，先做单栏正文增删行 | 阶段 0；事务给出最早改动块；完整布局可对照 | 千页中段增删行；逐页比较断行、页末状态、光标命中及 PDF，记录受影响页数、键入到提交 P95；再逐项加入表格、分节和脚注 | 无法证明页末状态收敛时走完整分页 |
| 2 · Word DOCX／OPC | P1 包预检到 reader 的重复读取 | 记录解压字节、解析次数、峰值 RSS；对齐两层输入预算 | 在同一文档集证明减少重复解压；关系、未知 XML、只读／拒绝、保存重开结果等价；OPC 只承担包语义 | 还原原预检和专用 reader，保留原包字节 |
| 2 · Deck render | P1 媒体指纹及场景失效 | 富媒体 PPTX、对象缓存统计、CPU 图像对照 | 改标题时未变媒体重新哈希的字节数降为零；同 ID 换图、撤销、母版、字体、DPI 更新正确。分记构建、提交、实际呈现及缓存峰值 | 退回内容摘要重算和现有 CPU 对照路径，清空派生缓存 |
| 2 · Deck geom | P1 DeckCraft 预设几何，圆角矩形和箭头起步 | 固定 OOXML 预设名、调整参数及独立样例 | 每族覆盖默认／极值、命中、文字框、连接点、屏幕与 PPTX 往返；列出已移除的旧分派 | 按形状族恢复旧生成器，保留原预设名和参数；legacy 桥只读 |
| 2 · Sheet calc／model | P1 扩大已获益表达式、依赖或写回子集 | 试点差分通过，计时确认下一热点 | 增加跨表、范围、溢出阻塞／收缩及迭代语料；对照脏集和最终格值，不改变浮点累加顺序 | 按表达式或操作恢复通用路径，缓存可丢弃重建 |
| 3 · 三产品分别退出旧调用 | P2 Word 校对／字体目录及复杂分页、Deck 后续形状族／合成、Sheet 其余公式族及格式适配 | 各切片均有独立规范、兼容语料和第二个可读版本；公共能力有两个实际调用方 | 按能力核对 GUI、CLI、导出和发行包依赖闭包；清除已退出的生产调用，更新来源账本、SBOM、字典／字体等资产归属和回退演练 | 保留可重建的旧版本，独立打开公共格式副本；出现未处理对象时遵循现有只读或拒绝契约 |

同一轮只改变一个可独立回退的行为。文件语义用 [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) 和 [Office 实现说明](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/) 核对；原始来源指纹与本地独立实现证据分别保存。公共依赖、锁文件或构建边界变动时跑三产品回归。GPUI、塑形、压缩和密码学底座按实际限制另行评估。

## 从当前代码切入

- **Word**。用户看到的长文延迟由编辑事务、正文布局与页面绘制共同决定。先在同一次键入分别测这三段，再用当前完整布局作为逐页对照。字体、分节和页字段影响后续页面，增量路径必须能解释从哪一页开始、到哪一页可以停止。文件侧另有直接入口，基线宿主预检构造并丢弃已读 OPC 包，复用预检结果时应保留源字节、预算和诊断。负责模块及来源见[编辑会话与布局](../../crates/joyword-engine/src/session/layout.rs)、[容器预检](../../crates/joyword-shell/src/reader.rs)、[WordCraft 来源](../../licenses/wordcraft/SOURCE.json)。
- **Deck**。基线场景构建遍历演示文稿媒体并逐个计算 SHA-256，改一个标题也会支付这笔成本。先让导入和替换媒体的路径维护内容指纹，保持场景对母版、字体及分辨率的失效条件。几何生成器独立按形状族交付，编辑、预览与导出共享同一套参数。负责模块及来源见[场景构建](../../crates/joydeck-render/src/scene.rs)、[预设几何](../../crates/joydeck-geom/src/preset.rs)、[DeckCraft 文件映射](../../licenses/deckcraft/file-map.json)。
- **Sheet**。一次改格会经过依赖发现、求值准备、求值、写回和桌面呈现。先在现有调度器中替换有独立数值语义的一类表达式，保留原求值器作差分对照。即使内核达标，也应按用户提交到首帧的测量决定是否推广。负责人、来源和现有本地改动见[计算入口](../../crates/joysheet-calc/src/recalc.rs)、[求值调度](../../crates/joysheet-calc/src/recalc/pass_host.rs)、[GridCraft 来源](../joysheet/upstream-source.json)、[改动账本](../joysheet/UPSTREAM-PATCHES.md)。

切换单个组件时，由上表负责模块提交同一语料的新旧输出、最差差异、端到端耗时与回退操作。打开旧文件必须经过相同的输入预算和只读判定；试点失败时先关闭新路径，用上一个可读版本打开试点保存的副本，独立核验后再清理派生缓存。保留来源清单与仍在使用的资源声明，逐个核对生产依赖闭包后更新退役状态。

## 桌面应用共享复用

### 已在生产入口使用的公共代码

在 macOS 和 UOS 默认发行特性下，逐一核对六个应用的 Cargo 普通依赖和构建依赖，并追到产品调用处。表中使用 `joyoffice-*` 的后缀。三款桌面共用主题、控件表面、图标和窗口品牌设置，三个 CLI 的依赖闭包均无 GPUI，六个入口均无其他产品的私有 crate。[公共 UI](../../crates/joyoffice-ui/src/lib.rs) · [原生窗口图标实测](validation/20261118-desktop-icons/README.md)

| 产品 | 桌面可达的公共库 | CLI 可达的公共库 | 文件格式的实际使用范围 |
| --- | --- | --- | --- |
| Word | `ui`、`theme`、`opc`、`metafile`、`fonts`、`cfb`、`omml` | `theme`、`opc`、`metafile`、`fonts`、`cfb`、`omml` | 宿主用 OPC 做 DOCX 预检及保存规划，旧接口也用它检查文件。完整 DOCX 导入与写回仍有专用处理。[预检](../../crates/joyword-shell/src/reader.rs) · [保存规划](../../crates/joyword-shell/src/storage/plan.rs) |
| Deck | `ui`、`theme`、`opc`、`metafile`、`fonts`、`drawingml` | `theme`、`opc`、`metafile`、`fonts`、`drawingml` | shell 及旧格式只读桥使用公共 ZIP／预算接口。PPTX 与演示文稿模型转换仍由产品维护。[打开文稿](../../crates/joydeck-shell/src/document.rs) · [只读桥](../../crates/joydeck-legacy/src/document.rs) |
| Sheet | `ui`、`theme`、`cfb`、`drawingml`、`omml`、`opc` | `cfb`、`drawingml`、`omml`、`opc` | XLSX 按需读取用公共 OPC 有界解压，仍由 Sheet 控制部件名、未知内容及剩余总预算。OLE 使用公共 CFB writer，XLS 和加密格式与 Word 共用扇区和目录名读取。[嵌入对象](../../crates/joysheet-xlsx/src/drawing_objects.rs) · [包读取](../../crates/joysheet-xlsx/src/package.rs) · [CFB 读取](../../crates/joysheet-xlsx/src/cfb.rs) |

Deck／Sheet 的标准 RGB 进入 `joyoffice-drawingml`，Word／Sheet 的公式上下标进入 `joyoffice-omml`。复核时可替换下列命令的产品名。依赖图确认库可达，实际功能覆盖由上表中的调用处和往返测试确认。

```sh
cargo tree --locked --offline -e normal,build -p joyword
cargo tree --locked --offline -e normal,build -p joyword-cli
```

### 实施队列

| 阶段／优先级／负责方 | 两个首用产品与共享范围 | 前置依赖与验收 | 回退 |
| --- | --- | --- | --- |
| 1 · P0 · 三产品桌面验收 | **保存与输入契约**。从 Word、Deck 开始，再接 Sheet。共用候选、焦点、选词与文件读回断言，以及取消、重试、延迟返回的测试场景。Word／Deck 使用 GPUI 文件选择器，Sheet 由 platform 调用 rfd | 复用首轮原生探针。保存面板打开期间切换文档或关闭窗口，核对原件摘要、草稿归属和写入次数。Sheet 的普通另存及关闭前保存均已由单写者检查窗口、文档和修订号。Deck 已覆盖重复发起及过时回调。两平台原生面板均已复跑，Deck 已实测选择期间新增一页，过时路径未产生文件，重试后的四页均经关系解析和新进程读回。按三种编辑位置分别验收 IME | 可按产品恢复原探针和选择器适配，保留已有个人配置、文件与保存事务。[Word](../../crates/joyword-ui-gpui/src/save.rs) · [Deck](../../crates/joydeck-ui-gpui/src/chrome_nav.rs) · [Sheet 普通另存](../../crates/joysheet-ui-gpui/src/desktop.rs) · [关闭前保存](../../crates/joysheet-ui-gpui/src/palette.rs) |
| 1 · P1 · `joyoffice-ui` 与 Word／Sheet UI | **保存更改提示框**。已合并两款产品的遮罩、面板和三按钮区。两处现已调用同一呈现组件，由产品传入文案、状态、可访问名称和动作。Deck 现有关闭流程单独验收 | 以现有 450 逻辑像素面板和两产品的关闭用例为基线。两处旧面板移除后，从正常窗口核对 Esc、Tab、取消、放弃、失败重试和保存后的读回。Word 另验替换原件确认，Sheet 另验多窗口目标 | 逐产品切回原面板，保留各自的关闭及保存状态机。[Word／Sheet 现有对照](validation/20261117-joyword-save-dialog/README.md) |
| 2 · P1 · Word／Deck metafile | **WMF／EMF 有界读取**。原两份 `bytes.rs` 的 SHA-256 均为 `ef49693a…ee606123`。现已共用 [joyoffice-metafile](../../crates/joyoffice-metafile/src/lib.rs) 的记录边界和坐标读取。Deck 的裁剪区域和 EMF+ 继续由其解析器处理 | 先固定截断、长度溢出、16／32 位坐标和正常图片语料。双产品逐例对照读取结果、错误、资源预算、屏幕与 PDF 输出。验收时两个产品调用同一实现，两份私有读取逻辑退出 | 按解析器切回原读取实现并重放同一语料。[Word 读取器](../../crates/joyword-metafile/src/bytes.rs) · [Deck 读取器](../../crates/joydeck-metafile/src/bytes.rs) |
| 2 · P1 · `joyoffice-opc` 与产品格式层 | **包预检与受限读取**。先解决 Word 重复解压，再把验证过的读取接口用于 Deck 的包预检。Sheet 已从按需读取的正常入口接入同一有界解压，并保留部件名与未知内容 | 两产品先分别记录每个部件的读取次数、解压总字节和 RSS。迁移后重复工作下降，关系目标、预算超限、损坏文件判定相符，未知部件原始字节和旧版重开通过 | 单产品恢复其原预检／reader，保留源包及产品格式适配。[OPC](../../crates/joyoffice-opc/src/lib.rs) · [Sheet 未知部件](../../crates/joysheet-xlsx/src/passthrough.rs) |
| 2 · P1 · `joyoffice-ui` 与各产品 UI | **功能区分组和启动按钮**。Word／Sheet 先共用标题、分组和格式面板启动按钮的呈现。Deck 按紧凑布局接入适用部分，命令目录、选区与执行回调由各产品持有 | 以现有三个桌面的功能区折叠用例为基线。覆盖中英文、宽窗、窄窗和键盘导航，比较可访问名称、启用态、折叠行为与实际命令，保留产品快捷键 | 按分组恢复旧组件，保留控件 ID。[Word 分组](../../crates/joyword-ui-gpui/src/ribbon.rs) · [Sheet 分组](../../crates/joysheet-ui-gpui/src/ribbon_widgets.rs) · [Deck 分组](../../crates/joydeck-ui-gpui/src/chrome.rs) |
| 2 · P1 · Word／Deck fonts | **字体发现与 face 标识**。先比对两端的系统字体枚举、实际字节和集合 face，再提取相同的目录或标识逻辑。正文断行、幻灯片文本适配及各产品回退顺序留在产品层 | 使用同名字体不同字节、同一 TTC 不同 face 和字体文件变化样本。固定字体的行末、命中和 PDF 金样一致，记录冷扫描耗时和缓存失效。UOS 初次出现的 5 项排版差异已在固定字体字节与续页坐标后全部复验通过 | 各产品可恢复原目录或身份实现，重建派生缓存。[Word 字体](../../crates/joyword-fonts/src/lib.rs) · [Deck 字体](../../crates/joydeck-fonts/src/lib.rs) |
| 3 · P2 · 公共格式与产品导入层 | **CFB、DrawingML、OMML 按子集接入**。CFB 先比较 Word 旧 DOC 与 Sheet XLS 的流读取。绘图与公式 XML 分别列出两个有对应内容的实际入口，再选择一种对象接入 | 先补两产品各自的真实样本、预算、兼容读回和拒绝用例。只有双方生产入口均接入且原始部件保留、损坏输入判定与旧版互读相符，才计为共享 | 按对象或容器切回产品 reader，保留未解释的数据。[Word 旧 DOC](../../crates/joyword-doc/src/container.rs) · [DrawingML](../../crates/joyoffice-drawingml/src/lib.rs) · [OMML](../../crates/joyoffice-omml/src/lib.rs) |

**已执行顺序。** Word／Sheet 保存更改提示框已接入。Word／Deck 的 WMF／EMF 读取也已接入并通过同机前后两套二进制的像素对照。UOS 的三产品输入已在同一 DDE 桌面的独立 XIM 服务复测。这样第一批就覆盖三个桌面应用，每个新组件都有两个能从产品入口验收的调用方。

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

这是修复前的 UOS 对照。当前三产品的默认搜狗与独立拼音复验均已通过。[修复前 UOS 实机记录](validation/20261118-refactor/UOS.md) · [修复后复验](validation/20261118-desktop-sharing/README.md#uos-中文输入)


## 共享改造实施记录（2026-11-18）

**保存更改提示框已交付。** JoyWord 和 JoySheet 从实际关闭／新建流程调用同一面板、提示文字布局和动作区。产品仍控制文档身份、错误重试和关闭时机。macOS 与 UOS 的 Word 通过系统键盘验证了取消、确认替换及新进程读回。两端的 Sheet 均实际点击取消、保存及放弃按钮，模拟临时写入路径被占用后保留草稿，重试只关闭目标工作簿。两端三产品均运行产品回归，UOS Word 仍复现原有的 5 项排版失败。[运行记录与复跑命令](validation/20261118-desktop-sharing/README.md)

**WMF／EMF 读取已交付。** 两个图元解析器共用同一个有界字节读取器，原始来源和许可已登记。macOS 和 UOS 各运行四个文档的页面场景及 PDF 导出。每台机器的八份 RGB 输出与其改动前基线逐字节相同，图内均检出测试三角形。两产品各自的记录预算、绘图及导出仍从原入口验收。[回归与像素摘要](validation/20261118-desktop-sharing/README.md#wmfemf-有界读取)

**Sheet／Deck 异步另存。** 两款产品已在 macOS 和 UOS 的原生面板验证取消及重试。Sheet 在选择期间发生编辑会拒绝迟到的路径并保留草稿，后续另存的工作簿能在新进程读回。Deck 的另存副本也经独立解包和新进程读回。[机器结果](validation/20261118-desktop-sharing/save-target.json)


**UOS 中文输入。** Linux 客户端在收到 XIM 创建输入上下文的应答后才更新光标、处理焦点和复位请求。修复前同一独立输入法下 Word 和 Deck 可以选词，Sheet 收到字面拼音；修复后三产品均通过。经授权重启用户输入法，默认搜狗的 XIM 注册恢复，三产品在该服务下也均通过。[候选与重开证据](validation/20261118-desktop-sharing/README.md#uos-中文输入)

**P1 共享与计算首批回归。** Word／Deck 的 OPC 读取和字体目录、三产品功能区、Deck 媒体缓存，以及 Sheet 默认重算路径已通过 macOS 与 UOS 的三产品工程检查。Word 原有五项排版失败在固定同一字体字节并修正续页测试坐标后全部通过。Sheet 另运行启用试点的精度和性能对照。[产品回归](validation/20261118-desktop-sharing/suite.json) · [业务原表与派生副本](validation/20261118-desktop-sharing/business-workbooks.json)


**第一批 Word 分页与 Deck 预设。** Word 在单栏纯文本、显式分页且入口状态可重现的章节中重排，窗口最多容纳 128 个块和 16 页。退出条件包括章节页数改变、引用外部状态或旧片段与原页不一致。Release 的千页样本中，完整分页与编辑加局部重排的 P95 在 macOS 为 2.51 → 0.89 ms，在 UOS 为 3.37 → 1.39 ms。逐页差分和命中比较覆盖全部 1,000 页，屏幕光栅和确定性 PDF 覆盖修改区域及两侧邻页。Deck 按 ECMA-376 Part 1 的调整参数及连接点重写圆角矩形和四个方向的直箭头。两平台新旧程序都能打开重新生成的 PPTX，20 个形状的预设、调整参数和同机像素均一致。[千页记录](validation/20261118-desktop-sharing/chapter-reflow.json) · [形状记录](validation/20261118-desktop-sharing/deck-presets.json)

**公共格式首批对象。** Word DOC 的 FAT 预检和 MiniFAT 兼容处理、Sheet 的 XLS 目录及 FAT 流共用有界扇区切片。完整元数据扇区和未补齐的末数据扇区分别验收。Deck／Sheet 的标准六位 RGB 共用解析与写出；产品继续解释自己的透明度和非标准输入。Word 和 Sheet 共用 OMML 上下标元素与子项顺序，Word 仍写自己的控制属性并优先保存已有原始公式 XML。各子集可独立恢复原调用。两端的发布闭包和文本许可由严格 SBOM 检查。[格式模块](../../crates/joyoffice-omml/src/script.rs)

**Sheet 默认采用范围。** 跨表或同表的有限数值常量与字面量四则运算，达到 512 个脏公式时启用。两平台均完成 AB／BA 顺序、每配置 40 次编辑。100k 行从提交请求到依赖格显示的新帧，macOS P95 为 144.51 → 116.06 ms，UOS 为 203.97 → 138.72 ms。100 行分别为 81.16 → 81.21 ms、47.59 → 42.51 ms。无公式的两份业务工作簿由原件往返和带 916／16 个已知公式的派生副本验收。遇到差异可用 `JOYSHEET_SCALAR_RECALC=0` 启动原程序。[引擎及内存](validation/20261118-desktop-sharing/same-source-recalculation.json) · [实际显示帧](validation/20261118-desktop-sharing/desktop-presentation.json)

**阶段 2 产品回归。** 两端六个桌面／CLI 入口均构建成功，三个产品各自的工程检查退出码为 0。macOS 的 Word、Deck、Sheet 分别通过 3,668、992、1,451 项，UOS 分别通过 3,672、997、1,451 项。分别列出的 UI 专项和产品检查有重叠。两端六份产品发布闭包均找到依赖许可全文。[检查计数](validation/20261118-desktop-sharing/phase2-regression.json) · [发行闭包](validation/20261118-desktop-sharing/distribution.json)


**本批退役与回退。** Word 字体目录的两条生产依赖边已清除。原有语言包目录、同意记录和缓存索引继续由原格式读写。Word 的自然页、表格单元格和命中索引可复用；表头连续行、分节页码和脚注编号按各自上下文计算。Deck 的两个双向箭头已拆除旧构造函数，透明转场另与参考光栅逐通道比较。Sheet 对数字、布尔值和错误依次归约，保留原有强制转换及浮点次序。两台机器分别演练上一可读版本打开三种产品生成的文件。[Word 语料](validation/20261118-desktop-sharing/complex-docx.json) · [Deck 语料](validation/20261118-desktop-sharing/deck-presets.json) · [Sheet 语料](validation/20261118-desktop-sharing/version-rollback.json)


**本批回归。** 在本机 macOS 上，Word／Deck／Sheet 分别通过 3,672／995／1,452 项；UOS 实机分别通过 3,676／1,000／1,452 项，失败数均为零。独立运行的三个字体模块在两端分别通过 159／161 项；语言包存储库的两种特性配置在每端各通过 32 项。产品总数含部分公共用例。[测试结果及日志摘要](validation/20261118-desktop-sharing/retirement-regression.json)


**后续切片的验收边界。** Word 已将多语词边界和文档断词限制接入编辑路径，词典、拼写建议与语言包校验仍有上游代码。Deck 的 CPU 透视光栅已替换，形状库中的其他预设及动画层生成仍有 DeckCraft 实现。Sheet 的 XLS 加密识别和三产品 ZIP 读取已进一步合并，当时的单元格模型、迭代求解和其余函数族仍含 GridCraft 实现。后续已替换循环分组与迭代调度，表达式求值与数组扩展仍有旧调用。六个发行入口的 Cargo 闭包能证明整库退出，内嵌到第一方 crate 的源码按调用与原始指纹逐项登记。[转场逐帧](validation/20261118-desktop-sharing/transition-raster.json) · [来源及实测](validation/20261118-desktop-sharing/README.md#后续替换的验证范围)


**系统键盘与本批验收。** 1,000 页文档第 500 页的系统键入至首次完整文档读回 P95 为 macOS 51.41 ms、UOS 41.77 ms。窗口编辑命令插入 240 个字符与撤销的应答 P95 分别为 36.11 ms、28.68 ms，显示合成时间另行计量。两端三产品工程检查全部通过，Word／Deck／Sheet 在本机为 3,675／996／1,468 项，在 UOS 为 3,679／1,001／1,468 项。合并 ZIP 读取后重新检查了五类 DOCX、七份仓库 XLSX、两份授权工作簿及其派生副本和两产品图元渲染。[千页桌面逐次数据](validation/20261118-desktop-sharing/word-desktop-edit.json) · [双平台回归](validation/20261118-desktop-sharing/extended-regression.json)

**语言包、矩形和循环公式。** Word 桌面与 CLI 已从同一用户存储加载词典。两端实测原生窗口重开、拒绝未授权下载、拒绝摘要不符的安装、安装后重新分页及移除恢复。75 种语言的正式索引和其中的荷兰语发行包均通过固定摘要校验。UOS 直连 GitHub 超时，经本机 HTTPS 代理完成下载。Deck 的九种矩形共 45 个实例在两代读写与两平台渲染中均无像素差异。Sheet 的循环检测与独立可达性判定对照了全部 65,536 种四顶点有向图，并验证了五万节点链。四组真实 XLSX 迭代设置均通过新旧读写器的四种组合。[语言包](validation/20261118-desktop-sharing/language-packs.json) · [矩形](validation/20261118-desktop-sharing/deck-rectangles.json) · [工作簿](validation/20261118-desktop-sharing/version-rollback.json)

本批 macOS 的 Word、Deck、Sheet 分别通过 3,676、997、1,470 项工程回归，UOS 25 分别通过 3,680、1,002、1,470 项。六个桌面与 CLI 入口均构建成功，两端校对库各另通过 475 项。其余生产路径、验收与回退列在页首。[分项与日志摘要](validation/20261118-desktop-sharing/cycle-regression.json)

**复杂分页后的页面刷新。** 全量分页时，栅格线程可比较两次显示的纸色、字体版本、页面尺寸和绘制指令。未变文本页保留先前图像，编辑页重绘。连续多次编辑时，仅相邻两个文档版本可直接继承缓存，避免在别页编辑后旧图重新出现。两端六页双栏带脚注的原生窗口均观测到编辑和撤销时第 1 页重绘、第 2 页复用。另逐页对照增加脚注高度后的完整渲染。跨页改变布局仍由完整分页计算。[原生刷新与产品检查](validation/20261118-desktop-sharing/page-repaint.json)

**Deck 线条与连接线。** 六种画廊线条和五种文件连接线共 55 个实例，使用真实的 PPTX 连接对象。新旧版均能读回形状和调整值，最新版对照公开规范中的自定义几何渲染，两个平台均为 0 个差异像素。画廊组保留旧版外观。五种导入形状修正旧版将不同拐点简化为同一路径的问题，旧版打开时仍显示原先的简化外观。[逐族对照与回退](validation/20261118-desktop-sharing/deck-connectors.json)

**Sheet 查找。** MATCH、XMATCH、LOOKUP、XLOOKUP、HLOOKUP 和 VLOOKUP 直接搜索工作簿中的向量及矩阵的首行或首列。精确匹配与前向／反向的最邻近匹配共用一次遍历。原生窗口在 4,096 行、每键重复两次的工作簿上编辑、撤销、重做并保存，重开后七个依赖格的公式和值均正确。XLSX 的八个查找公式另经两版写入和读回。最终 Release 各 100 次打开并连续编辑的 P95 为 macOS 14.13 → 13.88 ms、UOS 27.95 → 26.59 ms；验收记录同时保留最初 20 次中 UOS 的回退。其他数组扩展与格式语义按页首 S-CALC 继续验收。[桌面与计时](validation/20261118-desktop-sharing/sheet-lookups.json) · [两代互读](validation/20261118-desktop-sharing/version-rollback.json)


**Word 分节重排。** 在前后独立起页的章节组中，编辑正文、脚注或嵌套单元格会重新计算整组的行、栏和脚注位置。两端 70 节、69 页的原生窗口在第 35 页编辑、撤销、重做均只更新第 35 页。保存副本经独立 XML 解析及新进程重开，三条编号脚注、连续分节和带浮动定位的嵌套表格均保留。超长续注、跨节页数变化或下一节入口无法核对时自动完整分页。[普通窗口、Release 计时与产品回归](validation/20261118-desktop-sharing/section-reflow.json)
