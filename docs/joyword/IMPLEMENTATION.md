# JoyWord 当前实施与验证记录

记录日期：2026-10-19（Windows 与统信 UOS 验证审计后更新）。这是源码工作区的实施状态，不能替代 SPEC 的目标或宣称所有路线图已经完成。文档基线为 v0.3，工程版本为 0.0.1。

## 进度速览

- **当前阶段**：M0／M1 技术预览；T000、T002 与 T111 已完成（2026-10-17，见下文），M0 退出条件与 M1 发布门槛均未完成。T200 的样式基础、多级编号、制表位、查找替换、格式刷、样式面板，以及项目符号、级别标签、列表对齐、主题 tint／shade、表格样式粗体底纹边框，和查找全半角折叠、正则查找与正则替换捕获组已落地，编号超限标签（甲乙重复、中文 0 与非负 i32、正数罗马数字超过 3999、字母重复超过 64，均受 256 标量预算）和主题色文字绘制与改写也已落地，任务仍 `in_progress`；T201 的合并与拆分、垂直合并视觉跨行、跨页拆行、表头重复、表级边距、单元格 tcMar、double／dotted／dashed／thick 边框、有主题时的主题色边框，以及百分比宽度和 fixed／autofit 已落地，任务仍 `in_progress`；T202 的页面几何、显式分页符、页眉页脚三种变体、正文与页眉页脚页码域、`pgNumType`、下一页／连续／奇偶分节和等宽分栏已落地，任务仍 `in_progress`；T203 的行内图片、简单超链接、安全 HTML／RTF 粘贴与插入图片按钮已落地，拖放与表格内超链接也已落地（自动测试），页面绝对 wrapNone 浮动图与页眉页脚行内图片已落地（自动测试），页面相对 wrapSquare 第一切片已落地（自动测试），wrapTight／Through／TopAndBottom 未做，任务仍 `in_progress`；T204 的文件菜单导出 PDF 与打印到文件已消费屏幕上的同一 PageScene，系统打印对话框未验证，任务仍 `in_progress`；T205 的功能区、状态栏、字体与段落对话框、标尺拖拽，以及第三切片的下划线／sRGB 绘制和三个视图开关已落地，任务仍 `in_progress`；T207 已把有证据的测量接入排版入口，任务仍 `in_progress`；T209 撤销组继续进行中；T210 的 inspect／text／render／convert 已实现，五样例自动确定性、截断语义和 Windows GUI 窗口／CPU render 对照均完成，任务为 `done`；T211 的应用日志、panic hook 与测试 profile 已接入，麒麟与统信 UOS 的 `kill -9` 崩溃后日志检查已完成，任务为 `done`。完整统计现为 12 done／17 in_progress／37 planned。完整统计、逐项勾选与状态口径见 [TASKS 进度总览](TASKS.md#进度总览)。
- **已落地的主要能力（不代表所在任务已全部完成）**：真实 DOCX 会话、中文文本编辑、基础格式与文本剪贴板、另存／重开、原件保存的元数据观测与确认路径、Windows 原件原子替换与稳定导出重绑定、统一排版场景的 PDF／CPU 光栅及无头 PNG／PDF 输出、可见页缓存、打开／保存并发控制和恢复日志。
- **主要未完成项**：精确 ACL／命名流、断电与非 NTFS／网络盘验收（Windows 原件保存确认路径已通过 GUI）、WPS 参考 PDF、信创真机验证、T200 剩余的按字体名换面、罗马数字 0 与负数、超过 256 个标量的编号标签、bar／middleDot／heavy、编号部件改写和人工验收、T201 剩余的嵌套表、列宽拖拽、首列末列与纵向条纹的排版和合同表格人工验收、T202 剩余的图形界面验收、页眉编辑界面与页面设置写回、Word／WPS 章节页码对照、T203 剩余的 wrapTight／Through／TopAndBottom、衬于文字下方、非页面相对位置和 Word／WPS 粘贴对照（拖放、表格内超链接、wrapNone 与页面相对 wrapSquare 已落地，见该任务完成证据）、T204 剩余的双阅读器人工检查、真实打印、系统打印对话框、结构标签、文档内跳转的外部阅读器复核，以及不透明表格和列书签（正文、页眉、页脚和可定位批注的书签导入、PDF 命名目标与写回已落地；不透明表格和列书签仍不是 PDF 目标，写出时也不会被发明）、T205 剩余的 Word 字数对话框对照、导航窗格、审阅栏、粗线／虚线／波浪线／按字下划线、主题色、隐藏段落标记合并、跨段域，以及只用功能区完成报告排版的人工验收、T207 剩余的字符网格／中西文间距／adjustRightInd／字体替代链及 Word／WPS 版式对照、统信 UOS 上仍不可达的 Fcitx IME 与系统文件对话框，以及其余 M2–M9 的审阅／AI 等路线图工作。
- **2026-10-20 文档核对**：上一条把 T201 嵌套表、首列末列与纵向条纹，以及 T202 的 widowControl、不等宽栏、页眉编辑和页面设置写回，仍列在未完成项里。这些能力后来已有自动测试：一层嵌套表、首列末列与纵向条纹排版、widowControl、不等宽分栏、页眉编辑、页面设置写回和页眉页脚部件写回。T203 的页面相对 wrapSquare 上一条已经记为落地。T200 查找折叠记录里「替换文本始终是字面量」只描述该切片，捕获组切片展开 `$0`、`$1`..`$n` 和 `$$`。仍然未做的是：T201 列宽拖拽、第二层嵌套、从样式部件读入纵向条件、合同表格人工验收；T202 图形界面、栏分隔线、`nextColumn`、章节页码、Word 对照；T203 的 wrapTight／Through／TopAndBottom、衬于文字下方、非页面相对位置和 Word／WPS 粘贴；T204 真实打印与系统打印对话框（UOS 双阅读器有记录，不关闭人工验收）；T300 与 Word／WPS 交换。没有任何 M2 或 M3 任务改为 `done`。其中 T201「从样式部件读入纵向条件」和 T203 的 wrapTight／Through 随后由 [#67](https://github.com/willmove/joyoffice/pull/67) 处理：前者已读入，后者只近似为方形绕排，见下文「2026-10-20 验证波次缺口修复（#67）」。
- **2026-10-21 T203 上下型绕排**：页面相对、位于文字前面的 `wp:wrapTopAndBottom` 已有自动测试。与图片带垂直相交的正文行整行放到带下方，不排到两侧。上一条「未完成项」里的 wrapTopAndBottom 以本节文末记录为准。衬于文字下方、非页面相对位置、真正的轮廓绕排和 Word／WPS／浏览器粘贴仍未做。任务仍 `in_progress`。
- **信创真机验收策略（2026-10-18 用户决策）**：有条件时再补验，不阻塞开发、任务依赖、阶段验收或发布。T005 改为 `in_progress`，已完成的构建与 QEMU 自动验证证据保留，真机仍标“未验证／待补验”。本条取代下文历史记录中因缺信创真机而保持 `blocked` 的安排；其他未完成项的门槛不变。
- **T207（进行中，2026-10-18）**：`joyword-layout::cjk` 的测量函数已接到 `layout_document`／`wrap_mixed`。等线名称表命中时，窄于整形结果的 R7 宽度进入页面场景；纯中文行 `both` 等分余量，段末行不拉伸；单倍行距仅对 R4 已测家族使用 1.3×win 跨度并在未关闭吸附时对齐 312 twips。未验证配对、非等线标点、非纯中文两端对齐、黑体与未具名字体的单倍行距、中西文间距和 `adjustRightInd` 保持原行为或拒绝。`w:docGrid` 的 `lines`／`linesAndChars` 正 `linePitch` 替换 312 twips 默认吸附；`charSpace` 与 `snapToChars` 不改变行高。人工 Word／WPS 版式对照未做，不标完成。细节见文末「T207 排版入口接入」。
- **T300（进行中，2026-10-19）**：文字与移动修订。第一切片已有 `w:ins`／`w:del`／`w:moveFrom`／`w:moveTo`、三种投影和可撤销的接受／拒绝。第二切片补上审阅菜单与功能区命令、偏好作者、markup 页边色条，以及已类型化单元格段落里的文字修订。第三切片把成对 `moveFrom`／`moveTo` 当成一次修订：接受或拒绝任一端会同时处理两端，全部接受／拒绝分别落到最终／原始投影，撤销恢复两端，`trackRevisions` 设置补丁不丢掉移动对。`rPrChange` 与表格结构修订仍整篇或整表只读。页眉修订装饰和 Word／WPS 人工交换未做，任务保持 `in_progress`。
- **T300 页眉页脚装饰（进行中，2026-10-19）**：页眉和页脚故事里的文字与移动修订画在共享 PageScene 的页眉页脚带上（下划线、删除线、作者色页边条）。最终／原始投影仍去掉隐藏文字且不画装饰。页眉里的全部接受或全部拒绝各是一步，一次撤销恢复。带图片的页眉页脚段落不画。Word／WPS 人工交换和 T301 仍未做，任务保持 `in_progress`。上一条里的「页眉修订装饰未做」是第三切片结束时的状态。
- **T400（进行中，2026-10-21）**：独立 crate `joyword-agent` 默认关闭，shell／app 的 `ai` feature 默认不启用。启动不调用 Provider。操作系统策略与系统凭据库仍是接口后的 stub，ChangeProposal 不应用，AIGC 标识未做法律评估。任务不标完成。计数改为 12 done／18 in_progress／36 planned／0 blocked。细节见文末「T400 Provider 与关闭基础」。
- **2026-10-22 六分支并行波次**：T110 的 Windows 卷能力观测（[#81](https://github.com/willmove/joyoffice/pull/81)）、T200 的 `numbering.xml` 写回（[#85](https://github.com/willmove/joyoffice/pull/85)）、T202 的栏分隔线与 `nextColumn`（[#84](https://github.com/willmove/joyoffice/pull/84)）、T301 的段落与运行格式修订（[#87](https://github.com/willmove/joyoffice/pull/87)）、T401 的选区授权与受限工具（[#83](https://github.com/willmove/joyoffice/pull/83)），以及 review 分支的 CLI JSON 键序修复（[#82](https://github.com/willmove/joyoffice/pull/82)），曾在集成分支 `chore/integration-wave-20261022` 组合验证；这六项与 T111 门禁加固 [#86](https://github.com/willmove/joyoffice/pull/86) 现已全部合入 `main` `85a24de`，该主干 CI 九项全绿（2026-10-23 核验，见文末实际合并记录）。T301 与 T401 由 `planned` 改为 `in_progress`，计数改为 12 done／21 in_progress／33 planned／0 blocked，任务总数仍是 66。这一波全部是自动证据：没有任何一项拿到 Word／WPS 往返、图形界面、信创真机或打印的人工验收，因此没有任务改为 `done`。上面「主要未完成项」里 T200 的「编号部件改写」与 T202 的「栏分隔线、`nextColumn`」以本节文末记录为准（编号部件只做追加式写回，章节页码仍未做）。各切片细节见文末同名小节。

下文保留按时间排列的实施与验证记录；早期条目中的“尚未实现”如与后续增量不同，以最新记录和 TASKS 的当前状态为准。

## 已落地的内容

- 八份专题规格、目录入口、本记录；需求与任务数量以 check_docs.py 输出为准（v0.3：84 个需求、66 个任务），每项有输入、输出、依赖、错误路径和验收要求。
- Rust workspace：joyword-foundation 物理单位与屏幕投影；joyword-model 类型化文档模型（T100）；joyword-engine 编辑事务、位置映射与撤销（T101）；joyoffice-opc 格式中立的 OPC 读取层（T103 起步，ADR-011）；joyword-shell 命令注册表、本地化、偏好与多文档会话（T106）；joyword-layout 字体身份、整形与页面场景，joyword-pdf 由同一场景写出 PDF（T003）；joyword-render 由同一场景生成 CPU 页面图像；joyword-corpus 确定性生成原创最小 DOCX 样例（T004）；joyword GPUI 工程窗口（菜单、快捷键与按钮由注册表驱动）。
- 精确 Rust 工具链、GPUI 包版本、Cargo.lock；三端 CI（Actions 按 commit SHA 固定）与 cargo-deny 依赖检查。
- 中文／英文独立资源（支持插值与复数）；静态 A4 页面、语言切换和 50%–150% 缩放，均可从菜单、快捷键和界面按钮触发。
- 原创 UTF-8 种子与 5 个原创最小 DOCX 样例、散列清单，以及需求覆盖／依赖／本地链接／语料（含参考结果状态）／语言资源／Actions 固定检查器；语义比较接口与 PDF 行／页断点比较工具。
- 未签名 macOS 开发包装脚本。没有修改系统工具链或安装 Xcode。

第一轮至第三轮及后续增量已接入 DOCX 声明子集导入、保守导出、绑定打开与另存、Unicode 断行、分页命中、CPU 统一场景、会话编辑、恢复日志、原件保存确认路径及可见页光栅缓存。gpui-pre 0.3.9 迁移与来源校验、macOS 菜单和 IME 候选位置修正也已完成。完整 Word 兼容排版、更多格式导出、修订、批注、富文本剪贴板与 AI 未完成；菜单和 IME 候选位置已做约定场景人工检查，其余 GUI／系统 IME 验收待执行。各轮原始记录保留在下文。

### 第一轮集成切片（2026-10-14）

- W01：新增 joyword-docx，按包关系定位正文；导入普通段落、基础属性、规范样式名、兼容模式和保护设置；原始包/部件与未知片段保留，保护/修订文档只读，含未知内容的节点锁定。未实现表格语义、编辑后写回、ExportPlan 与原子保存。局部锁尚未进入 engine；未来接入前需把任何锁定节点保守映射为整篇只读。
- W03：在 joyword-layout 增量提供单字体、LTR、调用者给定合法断点的行拟合，候选行独立整形、EMU 几何、源 byte/scalar/cluster 映射及预算。未实现断点生成、完整中文禁则、混合样式、字体回退、分页和命中。
- W02：独立 macOS 探针以 GPUI 0.2.2 的 GlyphId 可见性补丁、TTC face 抽取和唯一名称注册验证已有外部字形绘制路径；协调者复验 4 项测试和已有截图，恢复实验自有代码 unsafe_code = forbid。只接收实验结果，Linux GL/CPU、Retina、IME 与生产字体处理仍未验收，未据此裁决正式来源，详见[W02 审查](../coordination/reports/W02-REVIEW.md)。
- 总协调者发现并修复 W01 的两个拒绝边界：MCE 前缀未声明时被静默忽略、文字/CDATA/属性中的非法 XML 控制字符未拒绝；两项回归先失败后通过。未修改公开 API 或依赖。
- 源码校验点 2d4093b885eaf5928929f972ca25c48bd330082e：本机强制真实字体的默认成员测试 135 项通过、0 跳过字体标记；三个相关 crate 的 all-targets clippy、fmt、规格检查及 9 项 Python 工具测试通过。W01/W03 交付提交和详细范围见[集成审查](../coordination/reports/WAVE01-REVIEW.md)。这些结果不能替代新版三端 CI、Office 或 GUI 验收。
- 后续 Windows CI 暴露 W03 的 Segoe UI 连字前提不成立，已以 4,164 bytes 的固定 OFL 测试子集替代系统拉丁字体，保留 fi/AV 断言，本机 135 项重新通过；远程修正后运行待确认，见[CI 记录](../coordination/reports/CI-WINDOWS-FONT.md)。

### joyword-model（T100，已完成：2026-10-17 人工验收执行并记录）

| 部分 | 已实现 | 尚未实现 |
| --- | --- | --- |
| 结构共享 | 持久化 B 树 SeqTree（扇出 16，路径复制、缓存摘要、按维度定位）；Document 各字段共享，快照 O(1) | 文档级块索引（随增量排版 LAYOUT-010 加入；当前按 ID 线性查找） |
| 文本 | 段落文本按 ≤1 KiB UTF-8 块存储；UTF-8／标量／UTF-16 偏移互转并拒绝字符内与代理对内位置；不做 Unicode 规范化 | 输入层已采用 Unicode 17.0 默认扩展字素边界（C07）；布局层仍按实际整形簇限制几何 |
| 结构 | story（正文、页眉页脚、脚注尾注、批注、文本框）；段落、表格（网格、gridBefore／After、横向与纵向合并）、块级 SDT、不透明块；行内对象以 U+FDD0 占原子位置并经类型化片段读取；编号定义（abstractNum、num、lvl、lvlOverride／startOverride）与段落／样式制表位 | 修订对象（T300）、批注线程（T302）；`numStyleLink`／`styleLink` 只保存不跟随 |
| 属性 | 原始单位 Measure（twip、半磅、八分之一磅、EMU、百分之一字符／行、pct、auto）；toggle 区分未设置与显式关闭；四槽字体、hint、主题字体与语言；样式层级按文档默认、`basedOn`（根到叶）再直接格式合并，字符 toggle 显式关闭或翻转，段落开关取最近指定值；主题色 tint／shade 按 §17.3.2.6 线性 sRGB 收成 Rgb | 非自动颜色绘制、按字体名加载面 |
| 样式 | 按规范名称识别内置样式（中文 Word 的 `a`／`1`、WPS 数字 ID）；basedOn 链有环、缺失、深度与类型诊断及有界回退；段落／字符样式解析（`w:link` 只配对应用，不叠第二层属性）；主题 fontScheme／clrScheme 的具体字体名与 sRGB；表格样式的整表与 firstRow／lastRow／band1Horz／band2Horz 粗体、底纹和边框 | 编号样式仍不解析；首列、末列与纵向条纹不参与排版；潜在样式；样式部件本身的导出改写 |
| 标注与资源 | 书签、批注锚点、跨段复杂域（含 separate）、权限、行内 SDT 共用 Position（story、段落、标量偏移、偏向）；失锚状态可见；资源按 SHA-256 与来源区分，不自动去重 | — |
| 校验与序列化 | 重复／未分配 ID、run 覆盖、对象位置、空单元格、网格、纵向合并、分节边界、页眉页脚类型、标注范围等诊断；嵌套 ≤64 层、诊断 ≤256 条；确定性模型转储，可忽略 ID 用于语义比较 | 与 T004 比较器的对接 |

### joyword-engine（T101，已完成：2026-10-17 人工验收执行并记录）

| 部分 | 已实现 |
| --- | --- |
| 事务 | Session 持有工作文档；Transaction 携带基准版本、来源（用户／Agent／系统）、前置条件、命令与事务前选区；在 O(1) 候选副本上执行，任一失败即丢弃，文档、版本与撤销栈不变；空操作返回 NoOp 且不增版本 |
| 命令 | 插入文字（拒绝换行、段落分隔与控制字符）、插入行内对象（图片须有资源）、同容器删除（跨段合并、中间块整体删除）、拆分段落、字符格式、段落格式、应用段落／字符样式（一次可撤销事务）、设置或取消编号、改变编号级别并保留各段 numId、查找替换与格式刷（由既有删除、插入、样式和直接格式命令组成一次可撤销事务）、同容器移动块、移除块（容器至少保留一个块） |
| 权限入口 | 只读策略、Agent 写入开关（默认关闭）、内容锁定 SDT 拒绝编辑、删除锁定 SDT 拒绝、跨 story／单元格／SDT 范围拒绝；前置条件校验目标文字未变 |
| 位置映射 | 插入、段内删除、跨段删除、拆分、移除五种步骤；结果为保持、收缩到删除边界或丢失；组合即顺序拼接；书签、域（含 separate）、权限和批注锚点在事务内映射，批注锚点被整体删除时转为失锚 |
| ChangeSet | 受影响的 story、段落、新增、删除、移动节点与标注，供排版失效使用；提交前检查受影响段落的不变量 |
| 撤销／重做 | 记录前后共享快照，恢复内容、格式、锚点与选区；每次撤销／重做发布新版本，标识分配器只增不减；新事务清空重做栈 |

人工验收（跨段编辑后检查选择、书签、批注锚点和撤销结果）已于 2026-10-17 执行：以文档画布所绑定的同一生产 `Session`／`TextInput` 代码路径复核（非 GPUI 壳层），`bookmarks_and_comments_follow_cross_paragraph_edits_and_undo`、`selection_is_mapped_and_restored_by_undo`、`undo_and_redo_restore_exact_states_with_monotonic_versions` 与输入层选择恢复测试全部通过，详见文末“T000 完成与 T111 豁免清单审查”所在记录批次。

### joyoffice-opc（T103 起步）

| 部分 | 已实现 | 尚未实现 |
| --- | --- | --- |
| ZIP 容器 | 按 PKWARE APPNOTE 自写读取器：EOCD／ZIP64 定位，中央目录越界检查，本地头与中央目录名称一致，条目区（含本地头）不重叠，只接受 stored／deflate，拒绝加密与多卷，以声明大小为解压上限并校验 CRC-32；最小写出器（stored／deflate） | 原子保存与元数据保留（FORMAT-004） |
| 资源预算 | 条目数、单部件与总解压量、压缩比（>1 MiB 时）、XML 深度与事件数、关系数，均在相应工作之前检查；默认值待 QUALITY-006 语料校准 | 图像像素预算、用户确认的受控提升 |
| 部件与关系 | 部件名规则（绝对路径、空段、`.`／`..`、结尾点、非法字符、编码的分隔符）、大小写等价去重、部件不能同时是目录；[Content_Types].xml 的 Default／Override；关系 Id 唯一、相对目标解析且不得越出包根；外部目标分类为网页／本地文件／网络共享（含 UNC 与 file://host）／邮件／其他，只记录不访问 | 标记兼容（MCE）与 DOCX 语义映射 |
| 包级 XML | 拒绝 DOCTYPE（因此不处理 DTD、实体定义与外部实体），只展开预定义实体和字符引用，拒绝 UTF-16 | — |
| 保留 | 每个部件的精确解压字节与内容类型、[Content_Types].xml 原文 | PackageBacking 与编辑后保存 |

尚无 cargo-fuzz 目标；当前以测试内两轮各 3000 次的确定性变异冒烟代替（字节级变异与“先变异 XML 再打包”），它已发现并修复两处越界 panic。夜间 fuzz 作业随 M1 加入。

### DOCX 会话接入（C01，T103/T106 增量）

- joyword-shell::docx::DocxSession 持有 importer 的模型、PackageBacking、diagnostics 和 editability；只读原因或任意 locked 节点存在时，在首笔事务前令 engine 整篇只读。可变 engine 不暴露，适配器没有清除导入限制的入口。
- 不受限文档的内存编辑、撤销／重做由既有 engine 执行；AI 写入保持默认关闭。未提交编辑时可取得精确原包字节；提交后即返回 EditedPackage，全部撤销后也不假设原包代表当前会话。此原包接口不写文件、不更新保存版本；编辑后导出由后续 C02 的独立计划接口负责。
- 五个原创 DOCX fixture 与独立保护设置的 8 项集成测试、本机 143 项完整回归通过，见 [C01 报告](../coordination/reports/C01-DOCX-SESSION.md)。GPUI 打开流程、多文档管理器和保存路径尚未接入；后续 DOCX 打开必须持有适配器，不能将其模型传入无格式限制的通用 open_file。

### 保守 DOCX 导出（C02，T103/T106 增量）

- joyword-docx::plan_export 从 backing 重新导入可信基线，对当前只读快照验证非文本字段、节点／顺序、来源和格式边界。对现有正文段落的文字修改进行 XML 叶子字节补丁；默认字符属性的空段落可输入文字，整段清空后保留段落与属性。
- 段落／run／属性 XML 与其间注释保留，所有其他 OPC 部件、内容类型、关系和资源保持精确解压字节。文字叶子含未知属性、未知命名空间、注释或处理指令时拒绝；结构／格式／对象变化、任何导入限制和数字签名包编辑拒绝。DOTX／DOCM／DOTM 只允许精确原包复制。
- 不可变 ExportPlan 绑定文档版本、目标类型、变更／保留部件和警告；文字导出显式报告 ZIP 容器元数据重建，不能声称 ZIP 逐字节不变。返回前在原预算下重新导入，按合并相邻同格式 run 后的语义校验，不将文本节点分割当语义损失。
- shell::DocxSession::export_plan 使用原导入预算，不写文件、不替换 backing、不标记已保存。全部撤销后通过完整验证可返回原包；旧 unchanged_package_bytes 的版本保守契约不变。
- 17 项新增真实链路测试（含 100 轮确定性 Unicode 编辑）；本机强制字体的全默认 workspace 160 项通过。独立读取、release 与完整验证结果见 [C02 报告](../coordination/reports/C02-DOCX-EXPORT.md)。仍未完成文件系统原子保存、元数据、文件锁、GUI 和固定 Word/WPS 人工验收，T103/T106 继续 in_progress。

### DOCX 新副本落盘（C03，T103/T106 增量）

- shell::storage::save_export_copy 只接收已验证 ExportPlan，在同目录私有临时文件中写入、sync 和逐字节回读，再以不覆盖目标的硬链接发布新路径；既有文件／目录／符号链接一律拒绝，文件系统不支持硬链接则失败。
- 发布前失败不产生目标；发布后清理或目录 sync 失败携带已发布收据，不删除目标。Unix 成功记录文件／目录已接受 sync，Windows 当前只记录文件 sync；不等于真实断电验证。收据绑定实际计划版本，不清除会话 dirty 或修改原包／撤销。
- tempfile 精确使用锁文件已有 3.27.0，锁文件仅增加 shell 的一行依赖边，没有新增外部包或升级。新增 12 项测试，本机全默认成员 172 项与相关 crate release 76 项通过；真实新副本可由 textutil 独立读回，重复目标拒绝。完整证据见 [C03 报告](../coordination/reports/C03-DOCX-COPY.md)。
- 这不是既有文件保存／替换：源文件元数据、链接、ACL／扩展属性／来源标记、Office 锁和外部修改检测尚待实现；原件覆盖不开放，T103/T106 仍 in_progress。

### 自动 Unicode 断点（C04，T105 增量）

- layout_paragraph_auto 用锁文件已有 unicode-linebreak 0.1.5（Unicode 15.0）生成候选，与整段整形簇求交；沿用候选行重整形和剩余文本簇复验，不拆组合／连字／emoji。显式断点 API 不变。
- 自动生成与拟合使用同一份文本／候选／shaping／字节／行数预算，保留全部空白与源映射。SHY 的可见连字符定位尚未实现，自动入口明确拒绝；tab／hard separator／bidi 仍拒绝。Unicode 默认标点规则不是固定 Word 中文兼容规则。
- 8 项新增真实字体测试和全默认成员 180 项通过，相关 release 26 项通过；无外部包增加或升级，详情见 [C04 报告](../coordination/reports/C04-UNICODE-WRAP.md)。完整中文规则、混合格式、回退、分页／命中与 GUI 仍未完成，T105 保持 in_progress。

### 文档排版快照与命中（C05，T105 增量）

- 新增只读 layout_document：真实 DocumentSnapshot → 单节物理页框 → 自动断行 → 多页 PageScene。显式 SingleFontProfile 绑定实际字体字节／face、字号、行高、基线与整形开关，不猜测字体名称或 Word 度量。
- LayoutKey 包含单调版本、相关内容指纹、FontKey 与完整 profile；ensure_current 在结果交接时拒绝旧版本、同版本内容变化、字体或 profile 变化。EMU 场景不受缩放影响，engine 保持唯一编辑入口。
- 记录每行源 byte／scalar、段落与场景 run；命中返回 model::Position，支持 wrap 边界 Before／After、空段落、跨页选区与段落标记。连字等 cluster 内位置明确拒绝。按页框支持绝对页边距／gutter、Start／Center／End、段前后距和 page_break_before；段间距为预览 profile 的累加规则，未声称 Word 的折叠或页顶抑制规则。
- 文本、段落、run、元数据、页、行、字形、整形调用／字节、诊断、选区输出均受硬上限与可降低请求预算约束；取消／失败不返回半份快照。表格／对象、额外 story／section、混合格式、未解释属性、域／标注／修订投影拒绝，不静默扁平化。
- 新增 19 项 layout 真实字体测试（含 64 轮 engine 编辑与重排）、1 项文档分页→PDF 不变性测试；全默认成员 200 项通过，4 项新增核对器逻辑测试通过。两页中文样例 28 行／551 字形，独立 Poppler 页／行／可见边框检查与 pdfplumber 全字符基线检查通过；两页渲染已检查。详情见 [C05 报告](../coordination/reports/C05-LAYOUT-SNAPSHOT.md)。
- 不增加／升级外部包，锁文件仅 4 条内部边。未接 GPUI，未完成四槽字体／回退、Word 兼容规则、显式行内分页符、混合格式、增量重排或人工验收。用户选择后台开发，GUI／IME／Word-WPS 验证暂缓，T105 仍 in_progress，T102 尚待窗口接入。

### CPU 页面与只读画布（C06，T003／T005／T105 增量）

- joyword-render 用精确 Skrifa 0.42.1 读取同一 FontKey 字节／face 的未 hint 轮廓，tiny-skia 0.11.4 光栅为不透明 RGBA 页面。只消费 PageScene 的 glyph id、字号和定位，不重新整形或断行，不使用系统同名解析；PDF 继续输出原场景的可检索文字。
- 字体／字节、页面维度／总像素、run／字形、唯一轮廓／命令、填充像素及缓存轮廓的重复边缘扫描均有硬预算；共享单调取消在预检、字形和轮廓命令间检查，失败不返回半幅图。彩色／SVG 字体和纯 bitmap 字体尚未实现；传统 EBDT 与可缩放轮廓共存时使用声明的轮廓路径（Windows CI 回归已补），彩色／SVG 表存在仍保守拒绝；空白字形合法但缺失轮廓明确失败。
- 正式窗口后台加载实际字体与只读 engine 会话，布局与页面图像从真实快照生成；按实际设备倍率与 zoom 生成像素，GPUI 仅显示图像与 UI 标签。版本／内容／字体／profile／投影及请求序号校验阻止迟到结果。空闲、语言切换及同一失败请求不循环调度；新投影取消旧工作，关闭使用弱 view 句柄。
- 20 项新增内核测试（含真实 CJK collection face）、4 项新增应用适配测试和 4 项独立像素核对器反例通过；全默认成员 220 项、相关 release 70 项通过。2 页 24 行原创中英文 CPU 图像与 Poppler 渲染的同场景 PDF 比较：逐行墨迹边框最大差 0 px，全部深色笔画像素在对方 1 px 内有实际抗锯齿墨迹；四幅图均已查看。详情见 [C06 报告](../coordination/reports/C06-CPU-CANVAS.md)。
- 只重用原锁文件外部包，新增内部 render crate；GPUI 不改来源／代码，W02 实验未改。这是 CPU 页面消费者，不等于完整无 GPU 窗口、国产平台真机、GPUI FontId 原生字形通道或 ADR-010 正式裁决。输入／IME／可访问性／Word profile 及 GUI 实测仍待完成。

### 会话输入与原生平台适配（C07，T102 增量）

- 新增 shell::input，UTF-16 与正文／标量位置严格互转；LF 仅为段间投影。组合态以临时 Session 生成显示快照，原会话、原 DOCX backing 和撤销栈在提交前不变；取消／失焦丢弃预编辑，unmark 或确认仅提交一次实际事务。DOCX 保护／locked 限制不变。
- 支持跨段／多行替换、Unicode 17.0 默认扩展字素移动／删除、正反选区、全选、词／段选择和撤销重做；engine 仅增量提供独立映射目标选择与只读历史预览，现有 apply 行为不变。输入、文档、命令、元数据与修订都有硬上限；失败不发布候选快照。
- 正式窗口实现 GPUI EntityInputHandler，并以同一布局提供命中、候选范围、光标、选择和组合下划线。显示修订／字体／profile／投影和请求票据共同拒绝过期任务；不同预编辑即使 engine 版本相同也不会混用。滚动位置实时参与变换，页面视口裁剪覆盖层；提交相同预编辑须验证完整布局键后才复用图像。
- 注册表增加 19 个编辑命令与 Edit 菜单；原生 IME 组合期间把普通导航／删除／Enter 留给平台。窗口关闭与注册的 Quit 统一走未保存不保存／取消提示，默认取消，过期提示不能丢弃后来修改。只在会话内编辑，无保存按钮；OS 强制终止／会话注销和恢复仍无保证，系统退出路径需人工核对。
- 远程新 RustSec 数据库拒绝原有间接 Rustls 0.23.45 后，已按上游修复仅锁定至 0.23.46，未增加安全公告忽略项，见 C07 报告；旧失败记录保留。
- 全默认成员 245 项、相关 release 155 项、应用无窗口 11 项、Python 工具 13 项通过；fmt／全工作区 clippy／开发应用构建及四个默认成员交叉检查通过。自动结果、来源与界限见 [C07 报告](../coordination/reports/C07-TEXT-INPUT.md)。实际系统拼音／Fcitx、候选框、焦点／DPI、鼠标交互、格式按钮与剪贴板仍未验收／实现；没有把 T102 或 M0／M1 门槛标为完成。全量重排及输入投影尚未达到 QUALITY 的性能证明。


### 第二轮集成（2026-10-15，W05–W09，T103／T104／T105 增量）

- W05（T103）：plan_export 以 engine 身份对齐编辑后的正文。默认格式纯文本正文段落可以拆分、合并、插入和删除：删除的源段落精确移除其元素字节，新段落写在前一个存续源块之后，并沿用作用域内已绑定 WordprocessingML 的前缀。受影响段落有非默认格式、内联对象、保留片段、注释／处理指令、未知属性或节结束时拒绝；移动和冒用来源同样拒绝。写出后另行校验节边界位置与全部保留片段字节，保留句柄按片段内容规范化后比较语义。XML 无法表示的字符在写出前拒绝。接续提交把受影响段落的属性改为明确名称清单，拒绝 run／属性容器上的未知属性与未映射内容。shell 的 docx_export.rs 中“拆段必须拒绝”的旧断言经用户授权删去。macOS textutil 独立读回 3 份导出逐字一致，见 [W05 报告](../coordination/reports/W05.md)。
- W06（T105）：layout_document 接受 keep_next／keep_lines 直接段落属性，按单次放置做有界分页：链首标注组高，过高的链有界退化。公共签名、PageScene、ensure_current 与几何接口不变，新增 12 项测试。未实现 widowControl、混合格式与 Word 规则对照，见 [W06 报告](../coordination/reports/W06.md)。
- W07（T103／T106）：shell::storage 新增 open_source／check_source。前者在 stat→open→fstat→有界读取→复查的窗口内锁定源身份（Unix 为设备、inode、时间、权限与链接数，非 Unix 为长度与修改时间）。另检测 Office `~$` 与 JoyWord 锁文件，状态不明即拒绝。save_original 自行重跑冲突验证，但因 ExportPlan 缺少来源绑定，原件替换一律拒绝，不写任何字节；来源绑定与发布策略的提案见 [W07 报告](../coordination/reports/W07.md)。集成后的 CI 暴露 Linux inode 复用与 Windows 弱源身份两类问题，仅修正测试的 PR #9 已合入 main。
- W08（T104 实验）：experiments/recovery-journal 独立 workspace，提供有界追加日志、经真实 engine 的逻辑编辑重放、两代检查点轮转与 inspect／recover CLI。未接入应用会话；Windows 原生与断电未验证，见 [W08 报告](../coordination/reports/W08.md)。
- W09（T103 健壮性实验）：experiments/docx-robustness 独立 workspace，提供 opc／import／roundtrip 三目标 harness、27 份原创种子、固定种子变异、最小化、有界 campaign 与反馈驱动的 `fuzz` 语料扩展驱动。harness schema-1 JSON 自 2026-10-19 T103 会话起携带 `features` 行为特征列表（净化阶段事实，非代码覆盖），`fuzz` 以特征新颖度扩展语料并在四平台 CI 有界运行；未发现生产缺陷。固定的生产目标随该次集成改为 `39193936ccc532ae546dd163b6c244b28994be07`（统信 UOS 渲染修复；上一锚点 `8b7f9c2b5e5b03c0ee6ac9a56c550a8e715f4d32` 留作历史）。本 PR 的 `crates/` 又含单元格 vAlign、行 trHeight 与表格属性，rebase 到 main `5b07db6f7ec72392bd2b123a8235fa1dc0884f31`（T110 存储）后锚点改为 `6f19c88164eda7d1ba1fb6aceba9d5833e756058`，不放宽 `git diff <SHA> -- crates`。再 rebase 到 main `77f71712820da58ee7e13299ecb01d76d5c2768f`（T202 squash）后锚点改为 `391a64f282cb58e4b6e8f39adec6ace2b5a76762`，不放宽 `git diff <SHA> -- crates`。再 rebase 到 main `b39eb05ab7a63ada1bb4189bc1a07394592b090b`（T200 find fold/regex squash）后锚点改为 `6b5193a5a8f045d1028441a7bcbf43d6d023869f`，不放宽 `git diff <SHA> -- crates`。见 [W09 报告](../coordination/reports/W09.md)。
- 集成：按 协作包→W05→W07→W06→W08→W09 以合并提交合入 main（ee4f57d），并修正 W08／W09 两处依赖 C07 行为的断言。本机强制真实字体的全默认成员 312 项通过，结果与范围见 [第二轮协作包](../coordination/wave02/README.md)。

### 第三轮集成与后续切片（2026-10-16 集成，其后增量见各报告）

- W10（T103）：`ExportPlan` 增加来源字节绑定 `origin_bytes()`，私有字段不可伪造；`save_original` 仍拒绝，见 [W10 报告](../coordination/reports/W10.md)。
- W11（T103）：`save_original` 写入前重跑冲突复查、比对来源（OriginMismatch），并按平台能力矩阵放行；Unix 发布步骤存在但公开函数在所有平台仍返回 ReplacementUnavailable，见 [W11 报告](../coordination/reports/W11.md)。
- W12（T103）：受影响段落的 pPr／rPr 可证明完全映射时，结构导出原样复制源属性元素字节，不能证明即拒绝；见 [W12 报告](../coordination/reports/W12.md)。
- W13：W09 harness 重锚定生产基线并复跑冒烟与变异，无异常，见 [W13 报告](../coordination/reports/W13.md)。
- W14（T104）：恢复日志接入生产的设计文档（快照含原包基线、dirty 对齐已保存版本等），未实现，见 [W14 报告](../coordination/reports/W14.md)。
- C09（T103／T106）：应用外壳接入绑定打开与另存副本；JoyWord 锁文件的创建／释放归会话生命周期。
- 原件替换增量（PR #20）：读回确认、未确认发布保留备份、Unix 所有权与 macOS 创建时间恢复并核对、ReadOnlySource 与 FileTooLarge 写前拒绝；协调审查修复了确认读取的长度预算绕过。公开原件保存仍在所有平台改写前拒绝，见 [W07 替换增量](../coordination/reports/W07-REPLACE-INCREMENT.md)。
- 集成与清理按用户授权快进 main 并删除已合并 worktree；本机默认成员 debug／release 各 364 项、应用 11 项通过，W08／W09 独立 workspace 测试通过，八项 CI 门禁全绿，见 [main 集成记录](../coordination/reports/MAIN-INTEGRATION-20261016.md)。
- 集成之后：gpui-pre 0.3.9 迁移与 CI 来源逐文件校验（见“当前环境”与 ADR-010）；macOS 应用菜单顺序修正、IME 候选窗位置修正（GUI 已实测约定场景）、画布“排版先于光栅”拆分（每次按键的几何空窗从约 30–40 ms 缩到约 2.4 ms，见 [画布拆分报告](../coordination/reports/LAYOUT-BEFORE-RASTER.md)）；GPUI Kit／Fast 评估结论存档；健壮性 harness 绑定到最新增量（1367f54）。

### 对照分析复核（2026-10-17）

对一款开源 Word 复刻工程的只读源码审阅与黑盒实测（授权与边界见 [PROVENANCE](PROVENANCE.md)，报告留存本机不入库）同时复核了本仓库源码，确认：画布仍把全部页面放进共享像素预算（2 倍屏约第 5 页起整体失败）、打开期间输入竞争与保存副本同步执行、原件保存恒定拒绝（实现严于 SPEC）、排版无段落缓存、整形器数据未缓存、行高度量未经固定 Word 实测、撤销无合并、无 no-panic lint 与分层自动检查、无日志与 panic hook。这些项已转化为 SPEC／ARCHITECTURE v0.3 条款与 T108–T111、T208–T211 任务；报告同时列出了不应采纳的做法清单（整包重生成当无损、宽容解析后照常编辑、以命令注册数量计兼容度等），与现有契约一致，无需变更。

### 视口附近页面的后台光栅缓存（2026-10-17，T003／T105 增量）

来源：对照调研 §6.5 J1 与 §9 P0-1。初版基线 main `1367f54`，在独立 worktree 分支 `claude/distracted-ishizaka-1b0abb` 上完成（`feat: cache background rasters for pages near the viewport`），随后 rebase 到 main `4e723ae`（M1 编辑－保存闭环）并整合其画布改动。

- 问题：原 `raster_budgeted` 一次光栅全部页面，并让所有页面共用 16,777,216 px 的预算，超出就整次失败，画布显示“无法显示文档”。投影以 96 DPI 乘设备倍率计算，2 倍屏 100% 下每页 A4 约 357 万像素，到第 5 页就会超限。新增测试 `long_document_on_a_2x_display_paints_the_visible_pages`（1000 段、20 页以上、`Projection::new(192.0, 1.0)`）在旧实现上以 `page raster: LimitExceeded(Pixels)` 失败，改动后通过。这是源码推断加无窗口测试的结论，没有在 GUI 中复现。
- 调度：只请求视口内的页及其前后各 2 页（`PREFETCH_PAGES`）。顺序是先可见页，再交替取下一页、上一页。后台同一时间只有一个作业，每个作业一页。滚动、缩放和视口尺寸变化都只调度缺失的页。在途页仍在需要范围内时继续运行，否则由新请求取消。视口内的页由 `input_ui::visible_pages` 计算，与命中测试共用 `Placement`。每次渲染时调度一次；视口尺寸变化时，prepaint 再补调一次。
- 缓存键：`PageKey { document, layout: LayoutKey, page, projection }`，其中 `LayoutKey` 包含版本、内容指纹、FontKey 和 profile。复用同一排版的修订沿用已有像素。某页新排版或新投影的像素落地后，替换该页的旧像素。
- 常驻上限：`MAX_CACHE_PIXELS` 只约束缓存中常驻的像素，不再限制文档页数。超出时先淘汰其他排版或投影的像素，再淘汰离视口最远的页。需要光栅的页集合按上限截断，避免这些页互相淘汰、反复重绘。被替换或淘汰的图像交给 `cx.drop_image` 释放纹理。
- 几何先于像素（保持不变）：排版一落地就安装，命中测试、IME、光标与覆盖层不等像素。某页的新像素落地前，该页继续显示最近一次的像素（可能来自旧排版或旧缩放）；从未有过像素的页显示空白页作为占位。
- 插入符号覆盖层（rebase 时整合自 M1）：光标所在页的光栅作业携带光标几何，输出另含一份从该页新像素复制的覆盖层位图；页面图像本身不含光标。覆盖层只在其来源页显示，键随该页像素，缓存中的页面被替换或淘汰后即不再显示。M1 原实现中随整篇邻域光栅产生的覆盖层与 `viewport_page` 焦点随之移除，视口焦点由按页调度中的 `visible_pages` 取代。
- 单页提示：
  - 某页在当前投影下超出像素上限，或超出渲染器 8192 px 的维度上限时，不为它发起作业，只在该页的页框内提示“第 N 页在当前缩放下超出单页像素上限，请缩小显示比例后查看”。即使有旧像素，也显示提示而不拉伸旧图。
  - 单页光栅失败只在该页显示“无法绘制第 N 页：原因”，不会重复请求；失败记录按 `PageKey` 保存，最多 64 条。
  - 光栅失败不再触发整张画布错误；排版失败仍显示画布错误。
- 陈旧结果：沿用 `RasterRequestGate`、取消令牌与后台任务，只接受最新票据，且键须等于“已安装排版＋当前投影＋该页”。打开其他文档或视图销毁时取消作业。
- 闸门 `renew`：画布已自行跟踪哪些页在途、已缓存或已失败，因此 `RasterRequestGate` 增量新增 `renew`：即使键相同也重新发票，并作废所有旧票。它修复了一个窄缝：缩放 A→B→A 期间某页任务在途，落地时被判陈旧；回到 B 时，原 `request` 因“与最近请求同键”而去重，该页再也不会被请求。测试 `zooming_back_while_a_page_is_in_flight_still_paints_it_later` 在原 `request` 下失败，改用 `renew` 后通过。
- 渲染接口：joyword-render 增量新增 `raster_size`，它与 `rasterize` 按相同规则拒绝无效页面、超维度和超总像素，但不分配内存；`rasterize` 改为调用它，行为不变。M1 的 `NeighborhoodCache` 邻域光栅保留在 joyword-render 中供整篇场景（如导出对照）使用，画布不再经由它取像素。PDF 与导出仍消费完整、稳定的 `PageScene`，未改动。
- 本地化：joyword-shell 的中英文资源各新增 `canvas.page_too_large`、`canvas.page_error` 两个键。
- 测试：
  - 应用无窗口测试 29 项（原 21 项）。删去整篇预算测试，新增 9 项：2 倍屏长文档、常驻上限、最远页与其他投影优先淘汰、只调度缺失页、单页超限提示、单页失败隔离、缩放往返、重开相同内容的文档、视口页范围。原有的几何先于像素、A→B→A、陈旧光栅、提交复用、切换文档等测试改用按页接口，断言意图不变。rebase 后另有 M1 的邻域预算测试保留、覆盖层测试改写为按页接口（覆盖层只在携带光标的页产生），测试总数以验证记录为准。
  - joyword-render 新增 `raster_size` 与 `renew` 各 1 项测试。
  - 10 种单点变异均至少被 1 项测试捕获：淘汰远近颠倒、先淘汰当前投影、不替换同页旧像素、取消仍需要的在途作业、重试失败页、旧像素压过超限提示、不按上限截断需要集合、不预取前页、视口上沿判断错误、闸门同键去重。
  - `cargo fmt --all -- --check`、带 dev-runtime-shaders 的 workspace clippy（`-D warnings`）、默认成员 `cargo test --locked` 均通过；各项数量见验证记录。
- 未覆盖：
  - GUI 人工验收待与用户约定时段，内容包括 2 倍屏长文档的滚动、缩放、窗口尺寸变化，以及单页提示的实际呈现。
  - 光栅作业串行，未并行处理多页；预取距离与淘汰策略未做性能测量。
  - 占位页为空白，没有进度文字。
  - 视口内的页合计超出常驻上限时（极高的窗口加高倍缩放），超出部分保持占位。
  - 远处页面的旧像素会一直保留到被淘汰或重新光栅；滚动到这些页时，新像素落地前会短暂显示旧内容（与此前“最近一次像素”的语义相同）。
  - 光标移动本身不触发所在页重新光栅；覆盖层随下一次该页光栅更新，与 M1 原实现的更新时机相同。


### joyword-shell（T106，进行中）

| 部分 | 已实现 | 尚未实现 |
| --- | --- | --- |
| 命令注册表 | 稳定命令 ID、本地化标签、平台快捷键、菜单分组；`invoke` 是菜单、快捷键、界面按钮和功能区的唯一执行入口，先查可用状态（如缩放上下限）再执行；校验重复 ID、各平台快捷键冲突与缺失标签；只登记已实现命令。T205 第一切片把开始／插入／布局／视图功能区接到这些命令。第三切片登记三个视图开关，它们不修改文档 | 命令搜索 |
| 本地化 | `key[one]`／`key[other]` 复数形式按 CLDR 基数规则选择（中文仅 other，英文 one／other），`{name}` 插值与 `{{`／`}}` 转义；校验语法、重复、未知或缺失复数类别、未闭合花括号，以及跨语言的键与占位符一致 | 更多语言 |
| 偏好设置 | 作者姓名／缩写（缩写可自动推导）、度量单位、自动保存间隔、界面语言、新文档纸型（按语言给默认值）、最近文件；`schema` 迁移（0→1）、非法值逐项回退、损坏文件回退默认并另存原件、较新版本只读不覆盖、未知键原样写回、临时文件加重命名的原子保存。T300 第二切片用 `author.name` 作为 `Origin::User` 新建修订的显示名 | 设置界面 |
| 多文档 | 未命名编号不复用并本地化；同一路径不重复打开；脏状态绑定已保存版本（保存期间继续编辑仍为未保存）；关闭时有未保存更改需确认，取消保持会话，保存不可用时不提供“保存”；退出前列出未保存文档 | 窗口与对话框界面（依赖 T103 打开／保存） |

### 统一字形链路（T003，进行中）

- joyword-layout：字体身份为字体文件 SHA-256 加 face index；HarfRust 整形时 kern、liga、clig、calt 逐项显式开关（默认全关，等文档属性驱动）；字形位置按累计笔位置从字体单位换算为 EMU，整行不累积舍入；缺字记录簇位置而不替换原文；页面场景以 EMU 记录字形 id、位置与簇映射。
- joyword-pdf：用 krilla 的定位字形接口逐字形写出场景位置，与整形使用同一字体字节；字体以子集嵌入并带 ToUnicode，文字可检索；关闭 krilla 自带整形。验证中发现并修复一处缺陷：krilla 按字号 1 询问字形度量，原实现直接返回磅值，导致 PDF 中每字多出 11 em 的间距；修复后同一行 TJ 微调最大 0.004／1000 em。
- 本机证据（2026-10-14，macOS 系统字体 Songti.ttc face 0，sha256 dd2f0628…5a87777，仅本地使用）：原创合同种子 5 行无缺字；PDF 22.8 KB；macOS PDFKit 读回 5 行文字与场景一致；同一场景两次输出字节相同。
- CI 证据：unified-pdf 作业在 Ubuntu 24.04 安装 fonts-noto-cjk（OFL）与 poppler-utils，强制要求测试字体，运行整形／PDF 测试和示例，并用 `pdftotext -bbox-layout` 比对每行文字、行首（容差 0.01 pt）与行尾（容差 0.05 pt），PDF 作为构件上传。运行 38486616823（提交 95affb0）：fonts-noto-cjk 1:20230817+repack1-3 的 NotoSansCJK-Regular.ttc face 2（sha256 b76b0433…690a）、poppler-utils 24.02.0-1ubuntu9.10；整形／PDF 7 项测试在强制要求字体下通过；5 行 0 缺字，PDF 17 KB；读回 5 行文字一致，行首与行尾最大偏差均为 0.0000 pt。
- C06 已通过 CPU 页面消费者接入正式窗口，离线图像与 PDF 光栅对照通过；实际窗口呈现待 GUI 验收。尚未完成：GPUI 原生 FontId 字形侧适配。未经补丁的 0.2.2 无法在外部构造 GlyphId；W02 的独立 macOS 探针已以最小可见性补丁和改名注册验证路径，但不能替代正式来源的 GL/CPU、字体身份、Retina 与供应链门槛。中文 IME、撤销、多显示器 DPI 与两个独立阅读器人工检查仍未完成。

### 语料与比较工具（T004，进行中）

- 清单：4 个文本种子与 6 个格式用例（DOCX-MIN-001…005 分别覆盖最小正文与默认兼容模式、中文样式 ID、未知内容保留、外部引用、限制编辑与强制修订；DOCX-SAMPLE-001 为应用启动示例，正文逐行取自 cjk-contract.txt），全部 `planned`；格式用例的参考结果状态为 `unverified`，检查器禁止在未建立参考结果时标记通过。
- 可重复生成：`joyword-corpus` 以固定条目顺序与时间戳生成样例，测试逐字节核对仓库文件，并确认每个样例是无诊断的 OPC 包、外部目标只被分类。2026-10-14 用 macOS `textutil` 独立读取 5 个样例，均读出预期正文；尚未用 Word 或 WPS 打开。
- 语义比较：`Document::semantic_difference` 比较不含 ID 的模型转储，返回第一处差异行。
- 几何比较：`scripts/compare_pdf_lines.py` 对参考与候选 PDF 使用同一抽取器，比较页数、每页行数、逐行文字（忽略阅读器推断的空格）与行起点／行底偏差（默认 0.5 pt）；`--dump` 输出行与词框 JSON。CI 端到端：同一 PDF 自比通过，把第三行提前换行的变体被检出。精度是行／词框级，不是逐字形原点。
- 未完成：固定版本 WPS 对照环境（Windows 参考机的 Word 2024 永久版已就位，见文末记录）、排版行为规则目录随实测逐步建立。

### T100 文本存储基准

`cargo run --release -p joyword-model --example text_storage`；Apple M4／24 GiB／macOS 27.0.1，Rust 1.98.1 release（thin LTO、codegen-units=1），单次运行。内存为子进程持有 10 万个 36 字（76 字节）短段落的 RSS 增量，含分配器开销，不含 run／属性等段落结构；编辑为 2000 次随机位置单字插入，每次保留前一版本以模拟后台快照。

| 方案 | 每段内存 | 100 字插入 p50／p95 | 1 万字 | 100 万字 | UTF-16 偏移 p95（100 万字） |
| --- | --- | --- | --- | --- | --- |
| 整段 `Arc<str>` 写时复制 | 114 B | 1.17／4.38 µs | 12.0／27.2 µs | 1.08／2.00 ms | 583 µs |
| **分块 B 树（选定）** | 236 B | 0.33／1.50 µs | 1.17／1.62 µs | 1.42／2.08 µs | 0.46 µs |
| Ropey 1.6.1 | 1038 B | 0.21／0.29 µs | 0.29／0.58 µs | 0.67／1.54 µs | 0.21 µs |

结论（ADR-003）：`Arc<str>` 的插入与 IME 偏移换算随段落长度线性增长，排除；Ropey 常数更快，但短段落内存为选定方案的 4.4 倍（5 万段约 50 MiB 对 12 MiB），排除；选定方案在百万字符段落中插入 p95 约 2 µs，远低于 50 ms 输入预算。数值是单机单次测量，只用于选型，不是 QUALITY §5 的性能成绩。

## 当前环境

| 项目 | 实际情况 |
| --- | --- |
| 本机 | Apple M4，24 GiB，macOS 27.0.1，arm64 |
| Rust／Cargo | 1.98.1／1.98.1；工具链文件精确固定；为交叉检查额外安装 x86_64、aarch64 与 loongarch64 的 linux-gnu 标准库目标 |
| GPUI | crates.io gpui-pre =0.3.9 与 gpui-pre-platform =0.3.9（开 font-kit、wayland、x11），即 Zed 主干 93d745668805029476e6d7fbb19652116fed51de 的快照，共 19 个 gpui-pre* 包，crates.io 所有者为个人（huacnlee）；按 ADR-010 选定，迁移过程见[迁移报告](../coordination/reports/GPUI-PRE-MIGRATION.md)。CI 的 provenance 作业逐文件对照 Zed：16 个包一致，3 个包共 9 处偏差已审查接受（[来源校验报告](../coordination/reports/GPUI-PRE-PROVENANCE.md)）。cargo-deny 的“停止维护”公告由 0.2.2 时的 7 条减为 3 条（paste、rustybuzz、ttf-parser），无漏洞公告；许可白名单去掉了不再使用的 NCSA。此前使用 crates.io 0.2.2（最后官方版本，已停更） |
| 依赖锁 | libc 仍锁定为 0.2.186。当初的原因是 0.2.190 删除了 Linux 上已弃用的 ENOATTR，使 gpui 0.2.2 间接引入的 xattr 0.2.3 无法编译；迁到 gpui-pre 后那条间接依赖已消失。joyword-shell 现在直接精确固定 `xattr` 1.6.1 与 `rustix` 1.1.5（`std`、`fs`），用于原件保存的扩展属性与交换改名。本次未升级其他依赖 |
| Xcode／Metal | 2026-10-17 起本机已安装完整 Xcode 27.1，`xcode-select` 指向 `/Applications/Xcode.app`，`xcrun --find metal`／`metallib` 成功（MetalToolchain-v27.1）；强制重编 `gpui-pre-apple` 后普通构建（不启用 dev-runtime-shaders）在本机生成 `shaders.metallib` 并通过。CI 的 macos-15 镜像（Xcode 16.4）同样具备 metal |
| 开发构建 | dev-runtime-shaders 使用 GPUI 官方运行时着色器选项，本机已编译成功 |
| 对照软件 | Mac 上已安装 WPS、Pages。2026-10-17 曾要求 WPS 打开 `m1_edit_save` 另存的 DOCX，`open` 退出码为 0 且进程在运行，没有读回正文。Mac 上没有 Microsoft Word.app。Windows 参考机装有 Word 2024 永久版（Office 家庭版 2024，build 16.0.20430.20182），可作为固定 Word 参考环境（规格已放宽为 Word 2024 永久版并绑定确切 build，见文末记录） |
| Windows／Linux | GitHub Actions 已实际执行并通过，见下表；尚无真实机器人工运行 |

当前直接依赖：GPUI（gpui-pre 与 gpui-pre-platform，代码中仍以 gpui、gpui_platform 名称引用）、本地 crate、joyword-model 的 sha2（资源散列）、joyoffice-opc 的 quick-xml、flate2（纯 Rust 后端）与 crc32fast、joyword-layout 的 HarfRust 0.8.4 与 read-fonts 0.39.2、joyword-pdf 的 krilla 0.8.2；Ropey 仅为基准 dev 依赖。Fontique、ICU4X 仍是选型，尚未接入。

## 验证记录

| 项目 | 结果与范围 |
| --- | --- |
| 开发构建 | `cargo build -p joyword --features dev-runtime-shaders --locked` 成功；2026-10-17 起本机（Xcode 27.1）`cargo build -p joyword --locked` 预编译 Metal 路径亦成功 |
| 真实窗口 | 2026-10-14 用户在 macOS 上人工确认：窗口可启动，中英文切换、缩放和正常关闭均正常（T002 据此完成）。此前的启动冒烟：CGWindowList 中出现窗口，4 秒内未崩溃；按窗口截图因缺少屏幕录制授权失败。菜单与快捷键的人工确认、IME 输入不在此范围 |
| 文档／语料／资源检查（早期） | `python3 scripts/check_docs.py` 通过：82 个需求、58 个任务，阶段一致，M0–M7 均在 T704 前置闭包内，4 处 Actions 引用均按 SHA 固定。v0.3 之后的计数见下方「文档检查（v0.3）」 |
| 核心测试（早期，引擎测试加入之前） | `cargo test --locked` 通过 93 项：基础库 3 项；模型 27 项单元与 12 项集成测试（随机编辑对照 Vec／String 模型、四个原创种子逐码点保留与 UTF-16 往返、表格／SDT／不透明内容保留、非法树有界诊断、1 万段文档单段编辑只复制不超过 5 个块树节点）；引擎 5 项单元与 11 项集成测试（40 个种子×120 步随机编辑与独立“字符＋标记”参考模型逐步对照文字、位置与收缩状态，失败原子性、过期版本、空操作、只读与 Agent 开关、前置条件、跨段编辑后的书签／域／批注锚点、锁定 SDT、选区映射与撤销恢复、随机序列的撤销／重做逐状态还原与版本单调）；OPC 3 项单元与 9 项集成测试（最小包与原始字节保留、相对与外部目标、部件名攻击、DOCTYPE／实体／深层 XML、64 MiB 零字节压缩炸弹在解压前拒绝、条目数与部件大小预算、声明大小造假、CRC、加密标志、未知压缩方法、重叠条目、截断，以及变异冒烟）；外壳 12 项单元测试（发布资源有效且一致、插值与复数、资源错误诊断、注册表与资源一致、菜单／快捷键／名称执行同一命令与禁用状态、分组与冲突校验、偏好往返／损坏回退与备份／新版本只读／迁移、未命名编号、关闭确认与取消、保存版本绑定）；应用 1 项（每个注册命令都有 GPUI 动作与快捷键绑定）；排版 3 项与 PDF 4 项（累计笔位置换算无漂移、特性显式、屏幕／PDF 单位换算误差 ≤0.01 pt；有 CJK 字体时：每簇有字形且位置单调、宽度等于前进量之和、缺字报告不改原文、PDF 确定性／子集嵌入／ToUnicode、未知字体被拒绝）；模型语义比较 1 项；语料 3 项（逐字节可重复、无诊断 OPC 包、外部目标只分类）。另有 `python3 -m unittest discover -s scripts/tests` 的 4 项比较工具逻辑测试 |
| rustfmt／clippy | `cargo fmt --all -- --check` 通过；含 GPUI 的 `cargo clippy --workspace --all-targets --features dev-runtime-shaders -- -D warnings` 无警告 |
| 依赖检查 | 本机 cargo-deny 0.20.2（安装在临时目录，未加入 PATH）：advisories、bans、licenses、sources 全部 ok |
| 三端 CI（修复前） | 运行 38419060698 与 38419419204（提交 97286e8）：macos-15 全部步骤通过，含不启用 dev-runtime-shaders 的预编译 Metal 构建；windows-2022 因检出时 LF→CRLF 使四个种子散列不符而失败（已加 .gitattributes）；ubuntu-24.04 在 clippy 编译 xattr 时失败（已锁 libc，并以 `cargo check -p xattr@0.2.3 --target x86_64-unknown-linux-gnu` 本机验证） |
| 三端 CI（修复后） | 运行 38481758247（提交 ac40043，2026-10-13）全部通过：macos-15-arm64、windows-2022、ubuntu-24.04 各自完成文档检查、fmt、40 项核心测试、clippy 与不启用开发着色器的应用构建；cargo-deny 作业通过。引擎测试在此之后加入，随下次推送进入 CI |
| 多架构 CI | 运行 38484920106（提交 92cb8b0）：ubuntu-24.04-arm 原生构建通过（文档检查、fmt、核心测试、clippy 与完整 GPUI 应用构建）；x86 runner 上交叉编译 aarch64-unknown-linux-gnu 与 loongarch64-unknown-linux-gnu（Ubuntu 24.04 的 gcc 交叉工具链与 glibc 2.39 交叉库）并在 QEMU 用户态运行无界面测试，两个目标均通过。QEMU 结果只证明代码生成与新世界 LoongArch ABI，不等于国产 CPU 或发行版上的运行 |
| T003／T004 CI | 运行 38486616823（95affb0）全部作业通过，unified-pdf 证据见“统一字形链路”；运行 38487093201（751e764）的 unified-pdf 另行验证比较工具：同一 PDF 自比通过、第三行提前换行的变体被检出，各平台构建作业运行工具逻辑测试 |
| C01 公开仓库 CI | 源码 a67db293d3f397eaf76c7d1c429c9479c18ca0b3 的 [38531619157 第 2 次执行](https://github.com/willmove/joyoffice/actions/runs/38531619157/attempts/2) 8/8 作业通过：macOS／Windows／Linux x86_64 与 ARM64 构建、ARM64／LoongArch64 QEMU 测试、依赖检查、统一 PDF。Linux 与 LoongArch64 日志确认全部 8 项新增 DOCX 会话测试通过；PDF 5 行无缺字，Poppler 行首／行尾最大偏差均为 0.0000 pt。公开仓库前的账户额度失败保留为历史，当前运行条件已恢复，见 [C01 报告](../coordination/reports/C01-DOCX-SESSION.md) |
| 第二轮集成 | 2026-10-15，ee4f57d：本机 fmt、强制宋体的全默认成员 312 项、docx／shell／engine／layout release、全工作区 clippy（dev-runtime-shaders）、开发应用构建、W08 15 项、W09 3＋11 项与 81 次冒烟矩阵通过。main CI 在 macOS、统一 PDF、依赖作业通过；Linux 与 Windows 的 W07 测试因平台假设失败，仅修正测试的 [PR #9](https://github.com/willmove/joyoffice/pull/9) 16/16 通过后合入（`5573997a75d9297b24429f90c727dad05b7f90c3`） |
| GPUI 迁移后 CI | 提交 6ee61e9（分支 claude/gpui-pre-provenance，含 gpui-pre 迁移）的 [38889115731](https://github.com/willmove/joyoffice/actions/runs/38889115731) 全部 9 个作业通过：macos-15（含预编译 Metal 构建）、windows-2022、ubuntu-24.04 与 ubuntu-24.04-arm 构建，ARM64／LoongArch64 QEMU 无界面测试，unified-pdf、dependencies（cargo-deny）与 provenance（gpui-pre 来源逐文件校验） |
| 第三轮集成复验 | 2026-10-16：本机默认成员 debug／release 各 364 项、应用 11 项通过；W08 debug／release 各 15 项，W09 各 3 项与 Python 11 项通过；首次 main 的八项 CI 门禁全绿（运行 38831307604），详见 [main 集成记录](../coordination/reports/MAIN-INTEGRATION-20261016.md) |
| 文档检查（v0.3） | 2026-10-17 `python3 scripts/check_docs.py` 通过：84 个需求、66 个任务，阶段一致，M0–M7 均在 T704 前置闭包内 |
| 视口光栅缓存 | 2026-10-17，分支 `claude/distracted-ishizaka-1b0abb` 提交，已 rebase 到 main `4e723ae` 并整合 M1 的插入符号覆盖层（随所在页光栅生成、只在该页显示）：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings`、`cargo test -p joyword --features dev-runtime-shaders --locked`（32 项）、`cargo test -p joyword-render -p joyword-shell --locked`（182 项）与默认成员 `cargo test --locked`（390 项）通过；新增复现测试先以 `LimitExceeded(Pixels)` 失败后通过。GUI 未验收 |
| Office 往返、性能 | 未执行，不能计为通过 |
| M0/M1 收口批 CI | 2026-10-18，f75d2cd：[39080511079](https://github.com/willmove/joyoffice/actions/runs/39080511079) 全部 9 个作业通过：windows-2022、macos-15、ubuntu-24.04、ubuntu-24.04-arm 构建，aarch64／loongarch64 QEMU 无界面测试（含恢复测试经 QEMU runner 重派生修复），unified-pdf、dependencies（cargo-deny，含 zbus 家族 unmaintained 豁免）、provenance；W09 harness 重锚定至 c7abd1c 且冒烟 81 次通过 |

构建出现上游 block 0.1.6 的 future-incompat 提示（proc-macro-error2 2.0.1 的提示随 gpui 0.2.2 一并移除）；目前不阻断固定工具链的开发构建，但依赖升级前需处理。依赖图含 GPUI 间接带入的 rustybuzz；独立排版的 HarfRust 选型不代表已经消除全部传递依赖。

## 未完成的 M0 门槛与下一步

1. T001：三端 CI 已通过；依据 T003、T005 的实测按 ADR-010 选定 GPUI 来源（须支持 GL 或 CPU 渲染），届时复查 deny.toml 中的停止维护公告；本机完整 Xcode 与 Metal 工具链已于 2026-10-17 装好并验证普通构建（Xcode 27.1，见"当前环境"）。
   ADR-010 线索（2026-10-14 只读核对，未实测）：Zed 主干 18787a9a080c 的 GPUI 已把 `GlyphId(pub u32)` 公开、`Window::paint_glyph` 为公开接口，但 `add_fonts` 仍不返回 FontId，只能按名称 `resolve_font`。候选做法是在注册前把字体 name 表的家族名改写为按散列生成的唯一名称再解析，使 FontId 必然指向同一份字节，从而无需分叉 GPUI；需在 T003 中实测屏幕与 PDF 的字形一致，并评估固定该提交的供应链与 Linux 渲染后端。
2. T002：已完成（人工验收于 2026-10-14）；菜单、快捷键触发缩放与切换语言的路径如需单独确认，可在下次运行时顺带检查。
3. T003：整形—场景—PDF 已贯通并有独立读回验证；剩余屏幕侧同一字形绘制（依赖 ADR-010 的 GPUI 来源或补丁，禁止用系统同名字体替代来宣称同一字形）、中文 IME 组合取消／提交／撤销、多显示器 DPI，以及两个阅读器的人工检查。
4. T004：比较工具与可重复生成的最小样例已有；Windows 参考机的 Word 2024 永久版（家庭版，build 16.0.20430.20182）已可用，WPS 对照仍需固定版本；按实测逐步建立排版行为规则目录。
5. T005：ARM64 已有原生构建，ARM64／LoongArch64 已有 QEMU 下的无界面测试；仍需统信 UOS 与银河麒麟（x86_64、ARM64、LoongArch64）真机或镜像，核对旧世界 LoongArch ABI（Rust 的 loongarch64-unknown-linux-gnu 目标要求新世界 ABI 与 glibc 2.36 以上，旧世界系统无法直接运行），验证 GL／CPU 渲染、Fcitx 中文输入和系统字体。LoongArch 上的完整应用构建需要 loong64 系统库（Ubuntu 无此架构），须在目标系统或其 sysroot 上进行。
6. T100、T101：自动验证与 2026-10-17 人工验收均已完成并记录（不支持节点保留经模型／DOCX 导入／会话三层复核；跨段编辑后的选择、书签、批注锚点与撤销经生产会话与输入组件复核），两项为 `done`。系统拼音提交／取消／一次撤销的人工验收仍未完成，T102 保持 `in_progress`。
7. T103、T110：只读导入、保守导出、新副本落盘和默认格式段落的拆分／合并已经接入。原件保存在未确认时列出不能保留或不能观测的元数据且不改源字节；显式确认且观测成功后，用 rustix 1.1.5 的 `RenameFlags::EXCHANGE`（macOS 上为 `RENAME_SWAP`）替换，并核对权限、所有者、macOS 创建时间与可复制的扩展属性。观测失败拒绝替换。macOS ACL 只观测、不回写。含 ACL 或隔离标记文档的图形确认、代码覆盖引导 fuzz 和 T107 的 Word／WPS 正文核对仍未完成，两项都保持 `in_progress`／`planned`。在 T107 通过前不提供真实用户文档的编辑保存承诺。
8. T105、T108：`layout_document` 对东亚或复杂文字调用 `slot_for`、`wrap_mixed` 和 `adjust_cjk_break`；屏幕入口在只有一份字体时用 `FaceSet::uniform`。窗口按可见页及前后预取页调用 `raster_page`，超出像素预算时丢掉远离视口的页。光标由 `paint_caret_overlay` 画到单独缓冲，不改该页像素。固定 Word 的完整中文规则、widowControl，以及长文档滚动缩放的人工验收未完成，两项保持 `in_progress`。
9. T104、T106、T109：命令注册表是打开、另存、最近文件、切换和关闭的唯一入口，参数齐全则执行，缺参数返回待输入。取消关闭保留会话。打开在版本接受前只读，期间输入保留；过期保存不写文件、不清 dirty。生产恢复日志写入原包基线与已提交文本，加密包不写明文。窗口启动时把目录设为 `JOYWORD_RECOVERY_DIR`，否则用偏好目录下的 `recovery`；已有日志则在示例文档之前恢复进会话。文件对话框、最近文件菜单和强制结束后的图形恢复仍待人工验收，三项保持 `in_progress`。
10. T111 已完成（2026-10-17，见文末审查记录）：`scripts/check_gates.py` 进入 CI，含分层、文件系统白名单、panic 基线和反例；engine 事务有 panic 守卫；豁免清单与门禁配置已逐条人工审查并记录。T207 已开始但只限纯函数，尚未接入排版入口；T210 的 inspect／text 切片已开始；T208、T209、T211 与其余 M2 任务以及 M3–M9 保持 `planned`。

2026-10-17 本环境补记：

- 原件保存：未确认时列出不能保留或不能观测的元数据且不改源文件；显式确认且观测成功才替换，并保留权限、所有者、macOS 创建时间与可复制的扩展属性。默认路径是 `save_default_copy`。观测失败拒绝替换。新增精确依赖 `xattr` 1.6.1。确认替换在 macOS 与 Linux 上用 rustix 1.1.5 的 `RenameFlags::EXCHANGE` 交换临时文件与目标；被换下的 inode 若不是已核对的源文件，则交换回去并拒绝。
- 命令注册表是打开、另存、最近文件、切换文档和关闭的执行入口。字符加粗、段落居中和文本剪贴板走同一入口。`m1_edit_save` 从 `BoundDocx` 打开最小 DOCX，编辑中文、格式和剪贴板，另存后重开，未知部件字节保持。受保护文档拒绝编辑。失败的另存不改源字节。
- 排版增加四槽选择、缺字回退和基本中文标点断行调整。窗口按可见页及前后预取页调用 `raster_page`，100 页在 2 倍缩放下按页出图；光标叠加不改该页像素。同一次排版的页面几何不随缩放改变。屏幕与 PDF 使用同一份字体字节，PDF 内容流中的文本矩阵与场景字形原点不超过 0.01 pt。
- 打开在版本接受前只读，期间输入保留；保存结果按会话版本应用，过期结果不清 dirty。恢复日志写入原包基线；子进程中止后能读回最后一次提交。窗口使用 `JOYWORD_RECOVERY_DIR` 或偏好目录下的 `recovery`，启动时若日志可读则先恢复再加载示例。加密包不写明文副本。
- 另存副本：`BoundDocx::save_copy_job` 取走一个已提交版本，计划、校验和写入可以在另一线程进行，保存期间继续输入的内容不进入副本，也不清未保存状态。窗口在选好路径后后台执行该作业；结果只作用于同一编辑世代，世代已变则丢弃，同世代但版本已变则提示副本不含开始保存后的修改。worktree `claude/nice-swartz-b742a2` 里“打开期间拒绝一切编辑”的方案没有并入：当前 `main` 已在版本门闩期间保留输入。
- `scripts/check_gates.py` 进入 CI，含分层、文件系统白名单、panic 基线和反例。engine 事务有 panic 守卫。
- 启动示例文档改为语料用例 DOCX-SAMPLE-001（`joyword-corpus` 确定性生成，正文逐行取自 cjk-contract.txt），以 `DocxSession` 导入而非裸内存文档：启动即可编辑、写恢复日志并另存。`DocxSession` 新增 `open_default` 与 `save_copy_job`；另存可用性改按能否导出判断，无源文档不再静默禁用。命令被禁用或失败时窗口显示原因（本地化键或错误文本），不再无反应。
- 本机 `xcode-select` 指向 `/Applications/Xcode.app`。不启用 `dev-runtime-shaders` 的 `cargo build -p joyword --locked --offline` 连续两次成功。`m1_edit_save` 连续两次通过；`m1_concurrency_recovery` 通过，含子进程终止后的恢复和加密包拒绝。T005 保持 `blocked`：本环境没有统信 UOS、银河麒麟、旧世界 LoongArch 和 Fcitx。Mac 未安装 Microsoft Word.app；2026-10-18 确认 Windows 参考机装有 Word 2024 永久版（家庭版，build 16.0.20430.20182），规格同步放宽。T107 保持 `planned`：WPS 被要求打开这次另存的 DOCX，进程已在运行，但没有正文读回。排版入口对东亚文字使用 `wrap_mixed`。应用光栅只画视口邻域，光标叠加不改页面像素。恢复日志由 `Office` 写入，终止后恢复进会话。保存过期不清 dirty 且不写文件。T005 保持 `blocked`，T107 保持 `planned`。T102、T104、T105、T106、T108、T109、T110、T111 的人工验收仍未完成，状态保持 `in_progress`。T207 纯函数与 T210 inspect／text 切片除外（见进度速览），其余 M2 及以后保持 `planned`。

M0 的真机项（信创、两个独立 PDF 阅读器、多显示器 DPI）仍未关闭。M1 的自动闭环已有测试，T107 的 Word／WPS 人工门槛未过。M2 的 T207 纯函数与 T210 inspect／text 切片落地，T207 排版入口未接；M3–M9 未实施。

### T000 完成与 T111 豁免清单审查（2026-10-17）

- **T000 标记 done**：`py -3 scripts/check_docs.py` 通过（84 个需求、66 个任务、依赖无环、阶段一致、M0–M7 均在 T704 前置闭包内、12 份 Markdown、10 个登记用例、本地化键一致、9 个 Actions 按 SHA 固定）；人工验收（按 [README](README.md) 复核主工作流）已执行：阅读顺序、解释优先级、状态规则、使用与维护与 v0.3／v0.2 变更记录中的已确认决策（信创进入 1.0、DOC 导入属 1.0、WPS 原生格式导入、M8／M9 以 T704 为前置、PROVENANCE 禁止条款恢复）逐条核对无丢失。
- **T111 人工验收（逐条审查豁免清单与门禁配置）已执行**，结论为通过，审查记录如下：
  - 范围：`scripts/panic_baseline.txt` 全部 513 条与 `scripts/check_gates.py` 的门禁配置（LAYERS 分层表、FS_ALLOW 白名单、PANIC_RE／FS_RE 扫描规则、CI 接入 `.github/workflows/ci.yml:46-49`）。
  - 归类：基线条目绝大多数位于 `#[cfg(test)]` 内联测试模块与 examples（T111 允许测试与示例直接断言）；生产代码条目逐条核对源码后归为四类——(a) 先校验后使用的内部不变量（engine `locate` 之后按索引取段／容器、docx XML 构建栈非空、model 树／文本定位结果、layout preflight 拒绝对象后的段落假设、shell input 的“已验证选区”）；(b) 资源耗尽防护（ID／版本／请求世代的 `checked_add().expect(...)`）；(c) 静态输入或启动装配错误（内置本地化资源解析、`action_for` 注册表查找、HarfRust 静态特性串）；(d) 测试注入钩子（`FAIL_NEXT_APPLY`）。抽查了 engine `session.rs`（事务守卫 `catch_unwind` 恢复文档／策略／撤销栈并返回 `TxError::Interrupted`，测试 `a_panic_inside_apply_discards_the_candidate` 先注入 panic 后断言文档与版本不变）、`docx/xml.rs` 栈不变量、`layout/document.rs` preflight、`apps/joyword` 缩放钳制等与基线对应的全部生产条目。
  - 机制验证：本机 `py -3 scripts/check_gates.py` 通过（105 个 Rust 文件）；`--counterexample` 对 `scripts/tests/fixtures/gate_counterexample.rs` 三类检查（分层违例、白名单外文件系统访问、新 panic 点）全部拒绝；新增 panic 点比较按行号无关的 payload 进行，基线为只增不减的单向闸门。
  - 附带修复：`check_gates.py` 全部文件读取显式 `encoding="utf-8"`——此前在 GBK 默认编码的 Windows 上读取含中文期望消息的基线会 `UnicodeDecodeError`（CI 的 Ubuntu 为 UTF-8 环境，行为不变）；修复后本机三类检查与反例均通过。
- **Windows 参考环境（T001 三端人工核对的 Windows 一腿，本机执行）**：`cargo test --locked` 全默认成员通过、`cargo test -p joyword --features dev-runtime-shaders --locked` 36 项通过、`cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings` 无警告、`cargo fmt --all -- --check` 通过。为支撑本机构建，`joyword-shell` 的 `xattr` 依赖改为 `[target.'cfg(unix)'.dependencies]`，非 Unix 的 `observe` 先 stat 源文件（缺失即观测失败，与 Unix 路径“系统调用失败即拒绝”一致），`m1_save_original` 等集成测试在 Windows 原生通过（Windows 下公开原件保存仍在写入前拒绝，行为不变）。注意本机 `python3` 指向 Windows 商店占位程序（无输出、退出码 49），Python 检查器须用 `py -3` 调用。
- **T104 的 Windows 原生补验**：`m1_concurrency_recovery` 在本机 Windows 原生通过（3 项，含 `killed_child_restores_the_commit_and_encrypted_bytes_are_not_copied`：子进程提交后被终止，父进程恢复最后一次提交与原包基线，加密包不写明文副本）。断电与图形界面强制结束仍未验证，T104 保持 `in_progress`。
- **T100 人工验收已执行**：模型层 `contract_structure_preserves_unsupported_nodes`（表格网格／合并、锁定 SDT、`m:oMathPara` 不透明块、`w:object` 行内对象经结构与快照往返保留，13 项通过）；DOCX 导入层 `joyword-docx/tests/import.rs`（真实包 `w:tbl` 等未知元素导入为带保留句柄的不透明块，20 项通过）；会话层 DOCX-MIN-003（未知内容保留、locked 非空、engine 只读、原包字节不变，8 项通过）。不支持节点“保留而非忽略”经三层复核，T100 标记 `done`。
- **T101 人工验收已执行**：以文档画布绑定的同一生产 `Session`／`TextInput` 路径复核跨段编辑后的选择、书签、批注锚点与撤销（`bookmarks_and_comments_follow_cross_paragraph_edits_and_undo`、`selection_is_mapped_and_restored_by_undo`、`undo_and_redo_restore_exact_states_with_monotonic_versions`、输入层选择恢复；engine 39 项、text_input 8 项本机通过）。T101 标记 `done`。
- **T107 的 Word 正文核对已在 Windows 完成（2026-10-18）**：Word 2024 永久版（家庭版 2024，build 16.0.20430.20182）COM 只读打开 JoyWord 编辑副本并读回正文，与包内逐段文字逐字一致；记录批次见文末“Word 参考环境落地与 T107 正文核对”。WPS 一侧未执行：本 Windows 环境未安装 WPS（无 COM ProgID 与卸载清单条目），WPS 在 Mac 上。T107 保持 `planned`（Mac 工作流与 WPS 人工验收待做）。
- **T004 的编辑行为登记表已落地**（2026-10-17）：`docs/joyword/behavior_registry.json`（schema 1）收录 7 条假设条目，分别绑定 IME 一次提交一次撤销、组合取消、预编辑期间的陈旧指针、跨段编辑锚点跟随、事务 panic 原子性、保留节点锁定与失败另存不改源字节的现有测试；`check_docs.py` 新增校验：唯一 ID、假设不得携带 Word 观测、规则条目必须带完整观测（版本／样例 ID／合法日历日期／64 位截图散列）、引用测试名在 crates／apps 下真实存在，并有 `scripts/tests/test_behavior_registry.py` 的 10 项单元测试（Python 工具测试 52 项全部通过）。规则条目与排版规则目录仍需固定 Word 2024 永久版实测后登记，T004 保持 `in_progress`。
- **T110 的 Windows 元数据观测已落地**（2026-10-17）：非 Unix 观测从“stat＋全部不可观测”的占位升级为 stable std 的真实观测——`Metadata::created()` 观测创建时间（存在且不可恢复→列为 Unpreservable；文件系统不支持→Unobservable）、`path:Zone.Identifier` ADS 经 1 MiB 长度上限读取（缺失无损失、存在列为 Unpreservable(ProvenanceMarks)、读取失败即观测拒绝）、所有权／ACL／链接数（stable std 无 API）／其余 ADS 报告 Unobservable。能力矩阵三分（Unix／Windows／其他）并有 Windows 矩阵测试、观测单元测试与公共路径集成测试（带 ADS 的拒绝清单不改源字节与流；`check_gates.py --write-baseline` 的写文件编码同步修为 UTF-8）。公开原件保存在 Windows 仍在写入前拒绝，Windows 恢复路径未接线（矩阵相应记 ObservableNotPreservable）。
- **T003 的 PDF 双独立阅读器检查已执行**（2026-10-17，Windows 参考环境）：`document_flow` 生成的场景 PDF（2 页 32 行，Noto Serif SC VF，sha256 a4aed998…39280）经两个独立实现核对——xpdf `pdftotext`（逐页文字与行数）与 pdfplumber（逐行文字＋几何，行首偏差 0.000 pt、行底偏差 ≤1.44 pt，属阅读器词盒约定差）——两阅读器及场景三者一致。此前记录的“疑似杀毒软件干扰”编译间歇已定位为 `clippy::needless_return`（见下条）。屏幕字形身份、Retina、IME 仍待 Mac 会话。
- **T107 的首个性能冒烟基线已建立**（2026-10-17，Windows 参考环境）：新增可复现的无头基准 `apps/joyword/examples/perf_smoke.rs`（release 运行，显式传入 CJK 字体；`cargo run -p joyword --example perf_smoke --locked --release -- FONT 0`）。本机测量（Windows 11 x86_64，Intel Raptor Lake 16 逻辑核，Rust 1.98.1 release、thin LTO；字体 Noto Serif SC VF，sha256 a4aed998…39280，face 0）：打开启动样例 DOCX-SAMPLE-001 中位 0.099 ms（n=7，样例为 5 行小文件，该数字只代表启动样例的导入开销）；20 页文档（725 段，由样例行重复构成）首次整篇排版 15.997 ms；输入路径中位：事务提交 0.015 ms＋整篇重排 16.463 ms（n=5，T208 增量排版落地前的 M1 现状，远低于 QUALITY 的 50 ms 输入预算）。基线语义：打开=导入；输入=一次字符插入的提交与全量重排；供 T107 门槛与后续性能回归对照，非 QUALITY §5 的正式成绩。
- **本机发现的间歇性编译失败已定位**：连续多次全量构建中出现的 `joyword-shell` lib“could not compile due to 1 previous error”，根因是本轮为非 Unix `observe` 增加 stat 前置时引入的 `clippy::needless_return`——clippy 在重查该 crate 时确定性地以 `-D warnings` 失败，而 cargo 缓存状态决定了哪一次调用触发重查，表现为“间歇”；已改为尾部表达式修复，压测通过。先前记录为“疑似杀毒软件干扰”的判断不成立，予以更正。

### Word 参考环境落地与 T107 正文核对（2026-10-18，Windows 参考机）

- **规格放宽**：经用户确认，Word 参考环境由“Office LTSC 2024”放宽为“Word 2024 永久版（LTSC 或零售永久版，观测记录绑定确切 build）”，SPEC 决策表、QUALITY §4、RESEARCH、TASKS 同步更新。
- **参考机 Word**：Office 家庭版 2024（Home2024Retail），`WINWORD.EXE` 16.0.20430.20182，Current Channel（永久版只收安全更新），Windows 11 x86_64。COM 自动化（`Word.Application`，不可见、只读、禁链接更新、禁宏）无头可用。
- **T004 固定参考结果**：Word 只读打开全部 6 个格式样例并逐样例导出 PDF（`ExportAsFixedFormat`，位图字体替代关闭），保存文本读回与 pdfplumber 几何抽取（页框、逐行文字与行框、内嵌字体）到 `tests/fixtures/reference/word2024/`；清单 `manifest.json` 的 6 个格式用例参考档案置为 `established`（PDF＋几何 JSON＋文本为证据文件）。实测事实：Word 对未显式指定字体的中文默认用**等线（DengXian）**；DOCX-MIN-002 的显式样式解析为宋体＋Times New Roman（常规／加粗）；DOCX-MIN-005 报 `protection_type=3`（只读限制）且强制修订开启，与语料预期一致。DOCX-MIN-003 未知命名空间元素中的文字不出现在 Word 正文读回中（mc:Ignorable 行为）。
- **T107 的 Word 正文核对**：`docx_text_export` 导出编辑副本（DOCX-SAMPLE-001 首段段首插入“校验”，计划验证通过、ZIP 元数据重建警告按契约声明），Word 打开读回正文（144 字符、1 页、无保护），与包内逐段文字**逐字一致**。导出器中替换文本叶子使用自带前缀（`jwe:t` 自带命名空间声明）是设计选择（替换叶子可能携带制表位同级元素，不依赖旧叶子的前缀上下文），Word 正常读取，JoyWord 自身按命名空间解析不受影响。
- **T103 的 Word 互操作闭环（人工验收 Word 一侧）**：Word 在上述导出副本中追加一段并另存（12.7 KB，Word 自带完整 styles/theme/fontTable/webSettings 部件），JoyWord 重新导入：正文 6 段逐字一致（含 Word 追加段），`unrestricted=true` 且 `locked=0`，Word 新增部件按 NotImported 保留诊断记录、未知属性按 PropertiesPreserved 记录，均不丢弃；`export_plan` 对该 Word 文件成功（0 警告）。产物在 `tests/fixtures/reference/word2024/interop/`（JoyWord 导出件、Word 编辑件、重导入报告）。
- **T103 的 Word 生成文件导入矩阵**：6 个格式样例经 Word 只读打开并重存（Word 原生完整部件，12.3–13.5 KB），JoyWord 逐一导入——正文逐段一致；MIN-001/002/003/SAMPLE-001 为 unrestricted 可编辑，MIN-004 的外部超链接按 locked 保留（1 个 locked 节点），MIN-005 的保护设置保留为整篇只读（read_only=true，符合契约），`export_plan` 对全部 6 个 Word 文件成功；Word 写出的 theme/fontTable/webSettings/customXml 按 NotImported 保留、未知属性按 PropertiesPreserved 记录。产物 `interop/wordresaved/`（6 份 Word 文件＋完整诊断的 `import_matrix.txt`）。
- **T105 的 Word 断行对照（规则目录首批条目）**：同字体（等线 DengXian）、同字号（12 pt）、同固定行距（18 pt）、同页框（148×140 mm、页边距 18 mm）下，Word 新建文档与 JoyWord `layout_document`→PDF 断行逐行一致——合同样例（20 段）同为 2 页 24 行、标点压力样例（675 字连续标点）同为 2 页 27 行，行首／行底偏差均 ≤1 pt（pdfplumber 同一抽取器两侧对比）。产物在 `tests/fixtures/reference/word2024/breaks/`（双方 PDF、Word 实测格式 JSON、结果文本）。已登记为 [规则目录](layout_rules.md) R1／R2；两端对齐余量、标点压缩、孤行控制等尚未实测。
- **规则目录 R3／R4（同日实测）**：Word 两端对齐的纯中文行余量分配到字间并抵满版心、段末行不拉伸（R3）；单倍行距实测行高：宋体 12 pt→15.6 pt（1.3 em）、等线 12 pt→31.2 pt（2.6 em，与“等线行距异常大”的社区现象一致，hhea／typo／win 三套跨度均不能推出，公式待推导）（R4）。产物 `breaks/word_justify.pdf`、`breaks/word_single_*.pdf`。
- **规则目录 R5 与 T004 人工验收（同日执行）**：本机 Word 新建文档默认 `WidowControl=0`（关闭）；强制拆分场景下对受影响段落显式设置 `WidowControl=True`（已核实 `w:widowControl` 写入 XML），固定行距与单倍行距两种设置下 Word 仍按 1+1／1+2 拆分，孤儿与寡行均出现——与一般预期不符，已如实登记并建议 GUI 会话复核后再据此实现 widowControl（产物 `breaks/word_widow*.pdf`）。T004 人工验收已执行：4 个文本种子与 6 个 DOCX 夹具逐份审查为原创虚构内容、无敏感信息，原始种子与正式夹具角色区分明确。
- **规则目录 R6 与 R4 机制破解（同日执行）**：R6——`KeepWithNext=True` 的标题段在强制边界处与正文整体移到下一页（正对照，证明分页引擎执行段落标记，R5 负结果更可信）。R4——单倍行距机制经 snapToGrid 决定性试验破解：自然行高 ≈ 1.3 × win 跨度 × 字号（四个 snap-off 实测点全吻合：等线 12→16.2、等线 10.5→14.2、雅黑 12→20.6、宋体 12→15.6），Word 自动创建行网格（linePitch 312 twips = 15.6 pt），`snapToGrid` 默认开启使行距吸附到网格整数倍；“等线／雅黑 12pt 行距 31.2pt”即自然行高超过 1 格后吸附 2 格；仿宋／楷体复核一致（产物 `breaks/word_nogrid*.pdf`、`breaks/word_keepnext.pdf` 与对应 DOCX）。
- **规则目录 R7／R8（同日执行）**：R7——标点压缩分类规则：全角引号 “ ” 固定 0.36 em（12pt 下 4.31pt advance）、连排标点前一个字距半宽（6.0pt）串尾恢复全宽、孤立标点全宽，JoyWord 对照全部 12.0pt（压缩属 T207 未实现）；R8——2 行段落 `KeepTogether=True`（w:keepLines）在固定行距与单倍行距下 Word 仍 1+1 拆分，与 R5 widowControl 同为负结果，keepNext 为正对照；标记矩阵登记：keepNext 生效、keepLines 与 widowControl 未生效，建议 GUI 复核。Noto Serif SC 可变字体在 Word 中名称未解析（落回 Calibri 替代链），已记录。R9——样例行首无标点悬挂，行起点恒等于左边距，悬挂开关行为待专门场景实测。
- 剩余：WPS 一侧的固定参考环境与读回（WPS 在 Mac）、Mac 上的 M1 工作流人工验收；T107 保持 `planned`。

### T107 端到端报告（2026-10-18）

[e2e_report.md](../../tests/fixtures/reference/word2024/e2e_report.md)：DOCX-SAMPLE-001 → JoyWord 打开 → 中文编辑插入 → 已验证另存（2092 字节、sync）→ 进程退出 → 新进程重开（unrestricted、0 诊断、段落一致、export_plan 再成功）→ Word 2024 读回逐字一致。关联证据：`m1_edit_save`（格式／剪贴板／保护拒绝／失败不改源）、`m1_concurrency_recovery`、`perf_smoke` 性能基线、`interop/` 互操作闭环。

### T207 纯函数切片审核（2026-10-18）

- 保留独立目录 `joyoffice-m2-t207`、分支 `m2/t207-cjk-measures` 原有现场，基线 `b097f55374761acf1a3f701afcf365b3aae2c90b`。未修改 CLI／CI、未接入分页计算入口、未增加依赖。
- R4 的 1.3 系数是规则目录的拟合机制；缺少字体身份时一律标 Inferred。R3 等分与余数分配仅为基于行边界观测的算法，不宣称单个字间距已实测。R7 根据原始逐字 advance 表限定连排配对；函数要求调用方建立等线 face，不能从字号确认字体。句号＋空格与逗号＋乙的原始 advance 分别约 12.03／11.88 pt，前者未支持，后者仅以汉字类别的全宽推导返回 Inferred，均未当作精确全宽实测。
- 来源：仓库 [排版规则目录](layout_rules.md) 与 `tests/fixtures/reference/word2024/breaks/word_punct_table.txt`、`word_punct2_table.txt`；百分之一行单位参考 Microsoft 的 [BeforeLines 文档](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.spacingbetweenlines.beforelines?view=openxml-3.0.1)。单位助手只做算术，不决定 Word 的字符／行单位基准或属性优先级。
- 自动验证：Rust 1.98.1 下 `cargo test -p joyword-layout --locked` 通过 70 项（其中新增 CJK 集成测试 8 项），覆盖规则目录带宽、证据分类、未知配对、非法参数、算术溢出与对齐预算边界；`cargo fmt --all -- --check`、`cargo clippy -p joyword-layout --all-targets --locked -- -D warnings`、`python3 scripts/check_docs.py`、`python3 scripts/check_gates.py` 通过。单测不等于真实文档版式验收。
- 未覆盖：排版入口接入、真实字体选择与替代链、字符网格、完整 OOXML 属性组合、中西文间距和 adjustRightInd 实测。未进行 GUI／Word／WPS 人工验收，T207 保持 `in_progress`。
### T210 inspect／text 切片（2026-10-18）

新增 `apps/joyword-cli`，纳入 workspace 与 default-members，不依赖 GPUI、窗口、字体或 GPU。使用既有 `joyword_docx::import` 与模型只读接口，不修改 DOCX／layout 核心，不触碰输入文件、不获取会话写锁。语义沿用既有基于 ECMA-376 的导入实现；CLI 序列化与原创测试独立编写，无其他编辑器实现来源。

使用 `cargo run -p joyword-cli --locked -- inspect FILE.docx` 输出单行 schema 1 JSON，或 `cargo run -p joyword-cli --locked -- text FILE.docx` 将纯 UTF-8 文字写 stdout，并将同样的投影与完整性元数据 JSON 写 stderr。`--max-text-bytes N` 可位于文件之前或之后，取值 0..1048576，默认 1 MiB，按 UTF-8 字节的完整字符前缀截断；不会跳过超预算字符再追加后续较短字符。正常退出 0，使用／读取／格式／预算错误退出 2（stdout 无成功结果），发生文字截断退出 3（inspect 仍输出可解析 JSON，text 仍输出前缀及 stderr 标志）。写出或 flush 失败返回错误。

投影固定为 `imported-main-body`，**包含隐藏文字**，包含插入修订及 moveTo 的导入文字，不含已删除修订、moveFrom、批注、页眉页脚和其他 story；这不是 Word 的最终审阅视图。未知 XML 和当前导入器尚未表示的表格、图片等内容保留在原包，但不从原始 XML 猜测提取。行内对象不输出 U+FDD0 内部占位符；已导入的超链接文字照常读出；软换行为 LF，分页／分栏为 FF，段落及模型单元格段落按 LF 分隔。`projection` 声明上述边界；`partial` 是保守标志：有保留片段、任何导入诊断或省略对象即为 true（包含仅属性或信息级诊断的情况），并独立报告 `preserved_fragments`、诊断数量和 `omitted_objects`；`truncated` 仅指文字字节预算，绝不以它冒充语义完整性。输出无随机节点 ID、绝对路径、时间戳或原始诊断文字，因此同一输入与预算逐字节稳定；错误消息固定且不回显不可信 XML／路径。

输入仅接受普通文件，压缩输入最多 32 MiB（有界读取再次确认实际大小）；OPC 预算为 1024 条目、每部件 16 MiB、总解压 64 MiB、XML 深度 128／每部件 100 万事件、4096 关系，压缩比继续使用 OPC 默认限制。输出文本最多 1 MiB，JSON 转义另有固定倍数上界；导入与模型读取成本受上述输入预算约束，尚未进行性能测量。`render`／`convert` 和其他未实现子命令明确拒绝。

自动验证：`cargo test -p joyword-cli --locked` 的真实 CLI 进程测试覆盖 DOCX-MIN-001..004 与 DOCX-SAMPLE-001 五份原创包的预期文字、重复 inspect 逐字节相同、text 与 inspect 一致、源字节不变；另覆盖隐藏／插入／删除／移入／移出／未知内容投影、对象占位符、UTF-8 边界与零额度、stderr 元数据、错误包／目录／缺失文件／超大文件／XML 深度与部件解压预算／非法参数／未实现命令。`cargo test -p joyword-docx -p joyoffice-opc -p joyword-model -p joyword-corpus --locked` 也全部通过。跨架构测试子进程会使用 CI 设置的 `CARGO_TARGET_*_RUNNER`，仍需远程 CI 运行证据。`cargo fmt --all -- --check`、`cargo clippy -p joyword-cli --all-targets --locked -- -D warnings`、文档与架构门禁在本机全部通过（5 项 CLI 进程测试、84 个需求／66 个任务、108 个 Rust 文件门禁）。未执行 GUI 或人工投影验收，尚无 PNG／render／convert 验证，整个 T210 保持 `in_progress`。


### T210 CLI 使用与输出边界补齐（2026-10-18）

基于 `c049ce0` 中已集成的 inspect／text 实现，在独立 worktree 的 `codex/t210-cli-followup` 分支补齐参数契约：`inspect --help`／`text --help`（也支持 `-h`）成功输出帮助；预算选项可放在文件前后，重复预算明确拒绝；`--` 后只解析文件名，因此 `text --max-text-bytes 3 -- --help` 可读取当前目录名为 `--help` 的文件。没有 `--` 时，短横线开头的参数按选项处理，未知选项拒绝。text 在输出正文前显式 flush stderr 元数据，写出失败退出 2。

Rust 1.98.1、macOS arm64 自动验证：`cargo test -p joyword-cli --locked` 的 8 项进程测试通过，覆盖原有五份原创样例、子命令帮助、选项位置／重复／缺参数、短横线文件名、空正文（导入器规范化成一个空段落）、空段落分隔符的零额度与精确边界、两种命令的完整元数据一致性及源字节不变。Unix 平台通过断开的本地 socket 分别注入 stdout／stderr 写出失败，断言退出 2，元数据失败时正文为空；该项测试在 Windows 上不编译，Windows 写出失败路径仍需验证。CLI clippy（全部 target，`-D warnings`）、workspace fmt、文档与架构门禁通过。未执行 GUI／人工验收，未运行远程 CI；render／convert／PNG 仍未实现，T210 保持 `in_progress`。

### 本轮 main 集成验证（2026-10-18）

- 开始状态：主目录 `main@b097f55` 有 5 个 CI／鲁棒性未提交文件；`joyoffice-m2-t207` 的 `m2/t207-cjk-measures@b097f55` 有 5 个已跟踪修改和未跟踪 `cjk.rs`。工具未发现可联络的原执行会话，因此保留现场后分别续接审核；CI 补丁先完整备份并复制到 `joyoffice-ci`／`codex/ci-robustness`，T207 沿用原独立目录，新增 T210 使用 `joyoffice-t210`／`codex/t210-inspect-text`。三个执行者仅提交各自任务分支，main 由本集成会话操作。
- 任务提交：CI／鲁棒性 `8a216f034761b618a02f347eb7594d2ba832f50e`、T207 `71723be160324144edb43d91859af36d339643df`、T210 `80754ce746a585b82903004965c16117a9c6b963`。CI 原补丁与主目录逐字比对一致后才从主目录移除并集成其已提交成果；三个任务分支和独立目录均保留。TASKS／IMPLEMENTATION 的交叉修改已合并，M2 为 2 项进行中、10 项未开始，未增加 done 数量。
- CI 将 W09 固定 27 个种子 × 3 个目标的 bounded smoke 纳入四平台作业；Windows 使用显式 `.exe` 路径以便读取二进制哈希。smoke 未完成完整矩阵即失败，不再将总时间预算耗尽视为成功；campaign 的有界停止契约保持原样。新增回归同时覆盖完整、提前停止和 campaign 情况。2026-10-19 T103 会话起 CI 在同一作业于 smoke 后追加有界 `fuzz`（固定种子、200 次迭代、60 秒、语料上限 128，行为特征反馈），两者的 summary 均保持 `coverage_feedback=false`。
- W09 严格绑定已提交的集成生产版本（rebase 前为 `b57544e9e81c1650d218d09d518125878298033e`，随远端 c7abd1c 收口提交 rebase 后为 `a7aaaf19cdb8f78468bd1de1fe1ebf2efe615c1a`），同步更新 build.rs、driver.py、lib.rs 与 README，未放宽 `git diff <SHA> -- crates` 校验。集成时还纠正 CLI 测试和说明中的内部占位符为模型实际的 U+FDD0。
- 自动验证：Rust 1.98.1、macOS arm64 上 `cargo test --workspace --locked` 共 443 项通过（含应用 36 项、T207 新增 8 项、CLI 5 项），零失败／忽略；workspace fmt、全部 target 的 clippy `-D warnings` 和普通应用构建通过。W08 恢复实验 15 项、W09 Rust 3 项及两实验 fmt／clippy 通过；W09 Python 12 项、工具脚本 52 项、check_docs、check_gates 与反例拒绝检查通过。重绑后 W09 smoke 执行 81／81，43 ok、38 合约内 rejected，无 crash／timeout／invariant／protocol 异常。
- 验证边界：本轮只做本机自动验证；远程 CI、Windows／Linux／QEMU 未重跑，未推送远程。W09 本机地址空间限制探测失败，记录 `enforced=false`，没有将其宣称为内存隔离通过。现有传递依赖 `block 0.1.6` 的 future-incompat 提示仍存在，本轮未升级依赖。GUI／IME／Word／WPS／信创真机验收未执行；T207 尚未接入分页计算入口，T210 render／convert／PNG 仍未实现，两任务继续 `in_progress`。本轮无需要用户决定的方向冲突或未解决实现阻塞。

### Mac GUI 验收、剪贴板修复与 WPS 参考环境（2026-10-18，Mac 参考机）

会话条件：用户在场并亲自执行 A 批（IME/字素/选择）；其余批次由会话以屏幕截图判读＋像素测量＋System Events/cliclick 输入合成自驾验证，截图与测量过程留存会话记录。本批文档更新在当日曾两次被并行集成会话的合并覆盖，本段为按现存文件证据重写的版本；WPS 读回结论经两轮独立执行一致，参考 PDF 因本机导出通道故障未建立。当日同仓库另一集成线完成 T207/T210 切片与 W09 终态绑定（上文“本轮 main 集成验证”），与本批互不覆盖。

- **剪贴板缺陷修复（T102，提交 e9d3d1c）**：GUI 的 `EDIT_COPY`/`EDIT_PASTE` 原先只用应用内字符串（`input_ui.rs` 的 `self.clipboard`，从未接系统剪贴板），跨应用粘贴无反应——用户实测发现。修复：`EDIT_COPY` 经 GPUI `write_to_clipboard` 写系统剪贴板；`EDIT_PASTE` 优先 `read_from_clipboard`，内部字符串仅作无系统剪贴板环境的回退。实测：ZWJ 全家福 emoji 粘贴生效（宋体无 emoji 字形按缺字占位渲染、原文码点保留，一次退格整组删除）、`é`（U+00E9）粘贴与字素步进经恢复日志字节核对、JoyWord 复制到 TextEdit 反向可用。
- **T102 人工验收**：系统拼音提交（数字键选词）／Esc 取消（无残留、撤销栈无多余步）／候选框贴近光标／一次撤销=一次提交；中文/emoji/组合字符输入与鼠标键盘选择后编辑＋撤销（用户实测）。
- **T105/T108 GUI**：100%↔150% 页宽 466→699 px（1.4998）、分页与行集合不变；30000 段（4.58 MB XML）大文档打开（日志基线 23514 佐证），累计数百页当量滚动、200%（页宽比例 ≈2.02）、深位置插入、回滚顶部无错乱、无画布错误与单页提示。
- **T003 屏幕字形**：截图判读确认正文为宋体衬线字形（画布 Songti.ttc），IME 候选框位置用户实测；Retina／多显示器未验证（本机仅一台 1080p 1x 显示器）。
- **T106 GUI**：原生打开/另存面板、建议名 copy-big.docx、最近文件切换（基线 23535/23514/1356 三态佐证）、未保存关闭提示默认=取消、放弃才关闭；手写 preferences.conf（author_name=GUI-验收作者）→启动→正常退出→值原样保留。
- **T109 部分**：另存落盘后继续输入，关闭提示正确识别未保存；打开确认后立即键入的内容保留在最终文档；长只读窗口未稳定观察（本机导入约 1 秒）。
- **T110 部分**：quarantine 文档（com.apple.quarantine）打开/编辑/另存副本，原件 sha256 与 xattr 完全未动；`file.save`/`save_original` 无 GUI 入口（发现记录，替换损失清单界面待接）。
- **T104 GUI**：提交 QMARK、K1 两笔后 `kill -9`，重启恢复含两笔且顺序正确（截图＋日志双证）；断电未验证。
- **WPS 固定参考环境（T004/T107 WPS 腿，读回材料提交 27bb78b）**：WPS Office for Mac **12.1.29181**（CFBundleVersion 29181）。6 个格式夹具＋e2e 编辑副本全选复制读回 **7/7 逐段一致**（WPS 复制在中西文边界插半角空格，去空格归一后逐字一致；基线为 document.xml 独立解析；两轮独立复现，现存 `reference/wps/DOCX-MIN-001.txt`、`DOCX-MIN-002.txt` 与 [wps_report.json](../../tests/fixtures/reference/wps/wps_report.json)）。**参考 PDF 不可得**：同日上午系统打印面板“存储为PDF”曾成功产出 7 份（产物后被并行清理丢失），下午起系统“存储为PDF”（默认 Downloads 与 Cmd+Shift+G 指定目录均试）、“打开PDF于预览”、WPS 自绘“输出为PDF”（冷启动+默认目录）全部静默无产物，无排队作业、无 TCC 弹窗、无替代落盘位置，cupsd 在跑但无 sudo 无法深查；PDF 参考与几何抽取待环境修复后补建，如实记录。工具链：poppler 26.10（pdfinfo/pdftotext）、pdfplumber 0.11.10（/tmp venv）。
- **T001 Mac 腿**：Rust/Cargo 1.98.1、Xcode 27.1（MetalToolchain-v27.1）、macOS 27.0.1（26A434）arm64（M4）；`target/debug/joyword` 构建并在本会话多次启动（含 kill -9 重启恢复）。Linux 真机腿未核对。
- **W09 锁核查（同日）**：并行集成后 `experiments/docx-robustness/Cargo.lock` 无未提交变更、`--locked` 构建通过、与生产锁共有外部包零版本漂移、独立锁无生产锁之外的外部版本（README 不变量成立），无需同步提交。
- 未覆盖：Retina/多显示器 DPI、断电恢复、T110 GUI 替换路径、T109 长只读窗口、WPS 参考 PDF（本机通道故障）、信创真机（非阻塞待补验）、代码覆盖引导 fuzz（2026-10-19 已交付行为特征语料扩展 fuzz，见其条目；源码行覆盖仍需覆盖引擎与 C08 审查）。

### T207 排版入口接入（2026-10-18）

`layout_document` 与 `wrap_mixed` 开始调用 `joyword-layout::cjk` 的测量函数。屏幕与 PDF 仍只读同一份 `PageScene`：压缩后的 advance 与两端对齐间隙写进字形的 `x`／`advance`，PDF 内容流沿用相邻字形 `x` 之差。布局仍不改文档。

接入范围按已记录的证据等级收紧：

- **R7**：名称表（name ID 16，否则 ID 1）解析为等线时，仅当 `punctuation_advance` 成功且结果窄于整形 advance 时替换该一字一字形簇。Measured、Scaled、Inferred 都沿用函数自己的标记。全宽结果不改写整形宽度，因此 R2 压力样例里跟汉字的孤立标点保持原断行。未验证配对（含行尾无跟随标点、ASCII 标点）和其他字体保持整形宽度。
- **R3**：`w:jc` = both 且全文为汉字加非 ASCII 禁则／引号时，调用 `justify_intercharacter`。非末行余量等分到字间并抵满版心；段末行间隙为 0。单字行函数返回未验证，不加边缘间隙。拉丁字母、ASCII 标点、不能一字一字形对齐的行，以及 `distribute`，返回不支持，不把左对齐当成两端对齐成功。证据仍是 Inferred。
- **R4**：段落单倍行距（auto 240）且字体名称为宋体、仿宋、楷体、等线或微软雅黑时，行框为 `single_line_height`（1.3×该脸的 win 跨度×字号，Inferred）。`snapToGrid` 不是显式 false 时，有正 `linePitch` 的 `lines`／`linesAndChars` 网格吸附到该 twip 间距，否则吸附到实测 312 twips；显式 false 只用自然行高。`charSpace` 不参与行高。黑体不在拟合集合内。其他字体、其他行距、未设置行距时继续用 `SingleFontProfile.leading`。
- 未接入：中西文自动间距宽度、`adjustRightInd`、字符／行单位的缩进（函数仍只做调用方单位的百分之一换算）、字符网格、CJK 字体替代链。禁则仍是既有字符集。

自动验证（Rust 1.98.1，本环境 Linux x86_64）：`cargo fmt --all -- --check`、`cargo clippy -p joyword-layout --all-targets --locked -- -D warnings`、`cargo test -p joyword-layout --locked`（79 项，含新增 `cjk_layout` 9 项：引号压缩、连排半宽、行尾不压缩、未验证标点保持整形宽度、压力样例在等线名称门开关下断行不变、纯中文两端对齐抵满版心且末行不拉伸、单字行不加边缘间隙、拉丁／distribute 仍拒绝、单倍行距吸附与关闭吸附、未测字体的单倍行距拒绝）。`cargo test -p joyword-pdf -p joyword-render --locked` 通过（PDF 5 项、render 25 项）。名称门测试把拉丁夹具的 Windows 家族名改成 DengXian，只证明调用条件；期望宽度是纯函数的 em 分数和该脸自己的 win 跨度，不是等线字节的新测量，也不是 Word 2024 的重新导出。

未覆盖：真实等线／宋体／雅黑字节上的断行与行高、`tests/fixtures/reference/word2024/breaks/` 的 Word PDF 未在本环境重抽、Word／WPS 人工版式对照、`charSpace` 的字符网格、非 312 twips 行距的 Word 对照、中西文间距、adjustRightInd、字体替代与缺字诊断链。T207 保持 `in_progress`。

`crates/` 相对上一绑定有差异，W09 harness 按原契约重锚到 `f2f0a1ece07805fbe3247f18eead8f63e046a2a8`（只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境重跑固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol。这仍不是覆盖引导 fuzz，也不证明中文排版。合入 main 的 T209 与 Windows 原件保存再次改变 `crates/`，锚点改到合并提交 `a8d2c4e9f7a0f8dfed23e3be3843430bd1606970`。独立锁只补上生产锁已有的 `winsafe` 0.0.19，未升级其他包。重跑 smoke 仍为 81／81，43 ok、38 rejected。恢复日志实验的独立锁同样补上这一版本，否则 `--locked` 无法解析 shell。

### T209 撤销组与输入来源切片（2026-10-18）

- 现场：独立 worktree `joyoffice-t209`、分支 `m2/t209-undo-groups`，基于 `main@cbeea1d`；未增加依赖，公共 API 只做增量扩展（`InputBackend` 新增必需方法 `seal_undo_group`，仓库内三个实现均已补齐）。
- engine（`session.rs`）：`Transaction::undo_group` 默认 `Standalone`，`Typing`／`Deleting` 只在以下条件全部成立时并入栈顶条目：会话的组未封口、同类、双方均为 `Origin::User`、事务起始选区等于栈顶的结束选区、栈顶已合并步数小于 `MAX_UNDO_GROUP_STEPS`（100）。合并后栈顶保留最初的 before 状态与撤销选区，after／版本／重做选区更新，ChangeSet 取并集；版本仍逐次单调递增。撤销、重做、独立提交与 `Session::seal_undo_group` 封口；版本过期、只读、命令失败、不变量失败与空事务不改变组状态；panic 守卫一并恢复组状态。新增 `undo_depth`／`redo_depth` 观测接口。
- shell（`input.rs`）：`InputSource`（Keyboard／Paste／Command）与 `TextInput::replace_from`，`replace` 等价于 Keyboard。只有键盘来源、非组合提交、非空、无段落分隔且作用于当前选区（`range` 为空或等于当前选区）的文本记为 Typing；`delete` 记为 Deleting；回车、粘贴（多段粘贴只按段落分隔插入，不走按键语义）、IME 提交（`unmark` 或组合中 `insertText`）、替换当前选区以外的范围、菜单插入、恢复日志回放和打开期间缓冲输入均为独立步骤。`select`／`select_positions`（以及基于它们的移动、点选、全选）只在选区实际改变时封口。host 的 EDIT_INSERT／恢复／缓冲输入使用 Command，EDIT_PASTE 使用 Paste；应用 `DocEditor` 新增 `replace_from`，EDIT_INSERT／EDIT_PASTE／打开缓冲输入接入对应来源，平台文本输入仍走 Keyboard。
- 行为登记表新增 10 条 `EDIT-UNDO-*`（均为 hypothesis，无 Word 观测），`check_docs.py` 以 `REQUIRED_BEHAVIORS` 校验这些条目存在且引用的测试存在。原有两项随机替换测试（`text_input`、`w05_text_input_export`）假定每次替换各成一步，改为显式 Command 来源以保留原意。
- 自动验证（Rust 1.98.1，Windows x64 本机）：`cargo test -p joyword-engine -p joyword-shell --locked` 全部通过，其中 engine `undo_groups` 10 项（含 48 个种子 × 160 步的随机性质测试：撤销链只经过已提交状态、在每个边界停下、回到初始状态，重做回到末状态，版本严格递增）、shell `undo_groups` 8 项；`cargo test -p joyword --features dev-runtime-shaders --locked` 36 项通过；`cargo test --workspace --locked` 共 439 项通过、零失败（Windows 上不编译 Unix 专属测试，总数不与 macOS 记录直接比较）。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings`、`check_docs.py`（17 条行为登记）、`check_gates.py`（112 个 Rust 文件）与 `scripts/tests` 53 项 Python 测试通过。
- 未覆盖：拖动手势与自动更正尚未实现，届时分别以 Paste／Command 来源接入；审阅模式边界需 T300 在切换时调用 `seal_undo_group`；协作多作者未建模，只区分 User／Agent／System。按 Word 行为的撤销粒度（如按词切分）未观测，登记表条目均为假设。系统 IME、拖动与粘贴混合的 GUI 人工验收未执行，远程 CI 未运行，T209 保持 `in_progress`。

### Windows 原件保存、导出绑定与无头视觉输出（2026-10-18）

- **Windows 安全替换（T103／T110）**：非 Unix 原先恒定 `AtomicReplaceUnverified` 的路径已补齐。`joyword-shell` 仅对 Windows 精确使用 winsafe 0.0.19（kernel、MIT）调用 `ReplaceFileW`，不启用忽略 ACL／合并错误标志。来源身份增加卷序列号、文件 ID、链接数、属性和时间；发布前写同目录临时文件、sync、逐字节回读并保留硬链接原件，替换后再核对操作时刻备份。最后身份检查后的竞争写入会被识别并恢复给竞争写者；发布未确认时保留原件备份。创建时间与 `Zone.Identifier` 已实测保留，完整 ACL／ownership／其他命名流仍列在确认清单中。
- **连续会话（T109／T110）**：`SaveOriginalJob` 把导出计划、重导入验证、来源检查与 IO 放到后台。`ExportBinding` 保留 engine 稳定节点 ID、当前编辑和撤销历史，只更新保存后 surviving paragraph 的 XML 定位；修复了直接更换 backing 后第二次保存误报来源变化的问题。保存期间继续输入仍 dirty，连续保存、撤销后再保存和 DocumentId 迟到路由均有测试。`file.save` 已加入命令注册表、菜单和 Ctrl／Cmd+S；原生提示先列风险，默认另存副本，确认后替换，提示期间编辑会重新预检。
- **无头视觉输出（T210）**：`joyword-cli` 在保留 inspect／text 契约的同时新增 `render` 与 `convert --to pdf`。同一 DOCX 导入和 `layout_document` PageScene 分别进入 CPU PNG 光栅与可检索 PDF writer；字体、DPI、页码、输入、页数和输出大小均有硬预算。输出经临时文件写入、sync、回读和不覆盖发布；其他转换目标明确拒绝。
- **自动验证（Windows x64，Rust 1.98.1）**：`cargo test --workspace --locked` 全部通过、零失败；`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`check_docs.py`、`check_gates.py` 及反例、53 项 Python 测试全部通过。`windows_save_original`、`original_save_job` 覆盖连续保存、元数据、竞争与故障；宋体真实 CLI 进程测试覆盖 PNG／PDF 两次输出逐字节一致和输入不变；`pdftotext -enc UTF-8` 独立读回全部中英文（本机默认输出编码会丢中文，不能省略该选项）。`cargo build -p joyword --locked` 成功后，以 `JOYWORD_FONT=C:/Windows/Fonts/simsun.ttc:0` 做进程级启动冒烟，进程保持运行且 stdout／stderr 无启动诊断。
- **验证边界**：未做 Windows 原生 GUI 确认保存、真实 ACL 文档、断电、非 NTFS／网络盘、5 个样例完整 PNG 矩阵和 render 与窗口的人工一致性验收；T109／T110／T210 均不标 done。cargo-deny 不在本机 PATH，本轮未运行依赖公告检查；winsafe 的精确版本、MIT 许可和公开 Win32 文档已在 PROVENANCE 登记。

### T211 应用日志与崩溃诊断（2026-10-18）

`joyword-shell::diag` 在应用启动时打开日志，并接到现有错误路径。无头 `Office` 与 GPUI 应用共用同一实现。没有新增依赖，也没有第三方日志 crate。

- **日志**：配置目录下 `logs/joyword-{序号}.log`。每次启动用 `File::create_new` 新建一份，不截断已有文件。默认单文件上限 256 KiB（含文件头）；超限的整条记录拒绝，并在还能放下时写一条 `log.capped`。目录内匹配名超过 256 个时打开失败，不随意删除。序号到 999999 后不再回绕。新文件头 `sync_all` 之后，才把保留份数（5）以外的更早文件删掉。崩溃发生在新文件建立之前时，上一份仍在。只 sync 文件，不同步目录项。
- **脱敏**：不可信字符串先截到 8 KiB，再处理调用方登记的精确串（8–256 字节，最多 32 条，不写入文件）、凭据键、URL userinfo、引号内容和连续正文。非 ASCII（含中文正文）整段替换。事件名和种类码必须是短代码；非法代码拒绝且不回写原文。界面仍显示原始错误，只有日志脱敏。
- **panic hook**：`set_hook` 替换默认 hook，不链接原 hook，因此 stderr 不再打印 panic payload，只写固定句 `JoyWord panic; see the application log`。日志记录源文件、行、列，以及按行脱敏的回溯（最多 48 行）。第二次安装返回 `AlreadyInstalled`。
- **测试 profile**：`JOYWORD_PROFILE=test` 使用 `JOYWORD_PROFILE_DIR`，未设置时用临时目录下按 pid 区分的根。该根等于用户配置目录时拒绝。未知 profile 在进入 GPUI 前以退出码 2 结束。用户目录不可用时诊断关闭，进程继续。恢复日志目录改为当前 profile 的配置目录；`JOYWORD_RECOVERY_DIR` 仍优先。
- **接入**：`boot_diagnostics` 在 `gpui` 运行前加载该 profile 的偏好（缺省 `ZhCn`，尊重已存 `ui_locale`），记录 `startup` 和偏好通知的种类名（不记录作者、最近路径或备份路径）。`Office::set_diagnostics` 后，命令失败记命令 id（`file.open`、`edit.insert` 等），不记命令参数。导入诊断只记种类名和计数，不记外部目标、属性值、命名空间或批注／修订载荷。输入错误、画布失败、光栅失败和窗口打开失败走同一日志。恢复只在 `recovery.jwr` 存在且失败时记 `recovery.failed`；成功时记文档 id，不记日志正文。
- **自动验证（Linux x86_64，Rust 1.98.1）**：`cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings`；`cargo test -p joyword --locked --offline`（37 项）；`joyword-shell` 的 `diagnostics` 集成测试 7 项（样例正文、修订、批注、凭据不出现；轮转后上一份字节不变；保留 5 份；封顶；脱敏；测试 profile 与偏好目录隔离；`catch_unwind` 触发的真实 hook 含位置且不含 payload）。同包其余测试除下列四项外通过。`python3 scripts/check_docs.py` 通过（84 个需求、66 个任务）。`python3 scripts/check_gates.py` 通过（120 个 Rust 文件）。反例 `scripts/tests/fixtures/gate_counterexample.rs` 被门禁拒绝。
- **本环境未通过、且不是 T211 改动的测试**：`storage::source::tests::identity_change_between_stat_and_open_is_not_a_baseline`、`w07_save_original_rejection::save_original_reruns_the_full_conflict_verification_itself`、`w07_source_conflicts::delete_and_recreate_with_identical_bytes_is_an_inode_change`、`w07_source_conflicts::hard_link_alias_writes_are_detected_through_the_stamped_path`。这四项都依赖 Unix 上删除重建或同长度重写会改变 inode 或时间；本机文件系统上这些操作之后身份检查返回成功。相关源文件未改。不把它们记成产品缺陷修复，也不放宽断言。
- **未覆盖（当时）**：没有启动 GUI、制造崩溃、再打开日志核对无正文泄漏。后续银河麒麟与统信 UOS 的 `kill -9` 会话已补做人工验收，T211 现为 `done`（见文末两节）。未在 macOS／Windows 上复跑。没有把文件 sync 说成断电后目录项一定可见。W09 harness 按原契约重锚到 `a3cec13b3cea9fb4f6e98f6158cdeae01fbe2688`（本任务改 `crates/` 的提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境重跑固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T208 段落缓存与增量重排（2026-10-18）

屏幕和 PDF 仍读同一页面场景。`layout_document` 仍是从零排版的参照；增量入口 `layout_incremental` 复用同一套段落拟合和分页。应用画布的后台布局（`main.rs` 与输入测试的 layout 阶段）走增量入口，并带上自上次安装布局以来的 engine `ChangeSet`。

- **缓存键**：段落正文指纹、解析后的段落／字符样式链与文档默认、版心宽度、实际整形的 FontKey（混合脚本时带上四槽与回退面）、字号和显式特性。profile 行距只在行框来自 profile 时入键；单倍行距（auto 240）不把行距当键。编号标签（或无法绘制的诊断）和默认制表位间隔进入解析样式哈希，因此前面列表项的计数变化会使后面的项键失效。页码、脚注编号、绕排区只在段落里的字段指令含 PAGE／NUMPAGES 或 FOOTNOTE／NOTEREF 时入键。当前预检仍拒绝批注，成功排版的文档没有这些依赖；键把依赖留在字段被接受之后。节点 id 不入键，命中后改写成当前段落。
- **失效**：直接改动的段落、插入、删除和移动来自 ChangeSet。样式 id 按 basedOn 与 link 闭包扩到使用这些样式的段落，以及在默认样式受影响时未指定样式的段落。`defaults_changed` 使每一段失效。闭包失效不调用 `clear()`。组高不入库，分页前按当前邻段重算。
- **停止与不收敛**：先用缓存拟合每一段，再从文首放置。某一段及其后的键与上一快照相同，且页码、段前纵坐标和行下标都对齐时，拷贝上一快照的页尾。对不齐就继续放置。页码／脚注／绕排反馈最多 8 遍；状态重复或用尽遍数返回 `LayoutError::Unstable`（显示“布局未稳定”），不把该遍页面当作快照，也不更新流量记录。
- **内存**：段落缓存默认 32 MiB。单条大于预算的拟合不入库。超出预算时按最近使用淘汰。整形器数据另有进程内上限 8 个 FontKey。
- **正确性**：`joyword-layout` 的 `incremental` 测试用改名为“等线”的测试字体（轮廓仍是拉丁测试字面，与 T207 相同）比较随机编辑序列的增量结果与 `layout_document`：页、行、诊断和字形数一致。序列包含引号、`both` 两端对齐、单倍行距和 keep-next。另有末段编辑的整形次数远小于全量（24 段时增量只整形被改的段）、首段加长后分页仍与全量一致、遗漏页码依赖会命中另一页的拟合、反馈振荡返回未稳定、超预算淘汰，以及样式闭包。既有 `document`、`cjk_layout`、`w06_pagination`、`line_fonts` 与应用画布测试未改断言并通过。
- **性能（本环境实测，不是 QUALITY 的百页 p95）**：`cargo run -p joyword --example perf_smoke --locked --release -- /usr/share/fonts/truetype/wqy/wqy-microhei.ttc 0`。Linux 6.12.94+ x86_64，4 个 Intel Xeon 逻辑核，Rust 1.98.1 release、thin LTO。字体文泉驿微米黑，sha256 `2420e8078af796b19a3f6ef13de527a1a91c1e7171eea115926c614ced1009b3`，face 0。Droid Sans Fallback 对样例报缺字诊断，未采用其数字。20 页、725 段：打开中位 0.049 ms（n=7，最小 0.040 ms）；首次全量排版 15.214 ms；输入路径中位为提交 0.023 ms 加全量重排 15.230 ms（n=5）。同一进程里缓存预热 5.900 ms 后，在末段连续插入 5 个不同汉字的增量排版中位 6.089 ms，每步 `shape_calls` 为 1；改首段一次 6.484 ms，`shape_calls` 为 1，同文档全量 `shape_calls` 为 725。增量结果与当次全量的页和行一致。这是 20 页冒烟，不是 100 页输入到呈现的 p95，也不能替代 Windows 上 Noto Serif SC 的旧基线。
- **未覆盖**：没有在图形界面打开百页文档分别编辑首段和末段。没有测量 QUALITY 的 100 页 p95 ≤ 50 ms。信创与 macOS／Windows 本轮未复跑。字段投影仍被预检拒绝，页码依赖只在键和反例测试里，不在真实字段排版里。T208 保持 `in_progress`。W09 harness 重锚到 `f1bfc58d2ce8e80693953692dc34a5dd58e2db85`（本任务改 `crates/` 的提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。

### T200 样式基础（2026-10-18）

这一刀只做样式继承、主题字体／颜色、可撤销的样式应用、命令注册表入口、样式指定的保守导出，以及排版和 T208 段落缓存对解析结果的消费。样式面板、多级编号、制表位、查找替换和格式刷留在同一任务的后续切片。T200 保持 `in_progress`。

- **解析**：`StyleSheet::resolve_paragraph`／`resolve_run` 按 ECMA-376 Part 1 §17.7.2 叠加文档默认、`basedOn` 链（根到叶）和直接格式。字符 toggle（粗体、斜体、删除线、隐藏）按 §17.7.3：未设置则继承，显式关闭强制关闭，显式打开翻转已继承的值，因此第一层打开使属性为开。段落开关（keepNext 等）取最近指定值，不做异或。缺省段落样式用于未指定 `w:pStyle` 的段。`w:link`（§17.7.4.6）只在应用时配对段落样式与字符样式，解析时不作为第二层属性。环、过深和类型不符记为 `StyleProblem`。表格样式与编号样式留在样式表中，解析报告 `UnresolvedKind`，不套用其属性。主题来自 DrawingML `a:fontScheme`／`a:clrScheme`（§14.2.7、§20.1.4）：latin 对应 ascii 与 hAnsi，ea 对应 eastAsia，cs 对应 bidi；`w:themeColor` 用 ST_ThemeColor 长名解析为 sRGB。空 typeface、未知槽、以及带 themeTint／themeShade 的颜色不假装已解析。
- **编辑**：`Command::ApplyStyle` 在一次事务里改段落或字符的样式 id。插入点应用段落样式写 `w:pStyle`；对插入点应用字符样式是空操作。整段选区应用带 link 的字符样式时改成配对的段落样式；段落样式只覆盖部分选区时，改用其 link 的字符样式。未知 id、表格／编号样式、断裂链和锁定文档显式拒绝。一次撤销恢复指定前的样式。行为登记 `STYLE-APPLY-UNDO`，来源是假设，没有 Word 观察。
- **命令**：`style.list` 列出 id、类型和规范名，挂在编辑菜单，不改文档。`style.apply` 必填样式 id，经同一注册表提交 `ApplyStyle`。GUI 为两条命令各有动作；`style.list` 的结果写在现有状态说明里。这不是样式面板。
- **导出**：样式指定发生变化时，段落须通过 `style_assignment_ok` 才重写 `w:pStyle` 和 `w:rStyle`。样式 id 须非空、无控制字符，且样式存在、类型匹配、链完整。编号、缩进、间距、分页开关、大纲级别、保留属性、段落标记格式、主题字体、主题色和 M1 直接格式以外的属性拒绝。样式部件字节不改写。新建带样式的段落仍拒绝。
- **排版**：屏幕与 PDF 仍共用页面场景。`style_paint` 把解析后的字号、粗体、斜体、对齐、段前后和分页开关交给既有行拟合。拉丁名取 ascii，否则 hAnsi；东亚名单独记录。整形仍用调用者提供的 face，不按名称换面。未解析的主题字体、非自动颜色、混排字号或粗斜体、编号、缩进和保留的属性子节点拒绝。样式元素上的 `qFormat` 等保留标记不拒绝。光栅对粗体做第二次偏移填充，对斜体做剪切；PDF 的第二次粗体绘制使用空簇范围，使可搜索文本只出现一次。PDF 斜体剪切未做。
- **缓存**：段落缓存键包含解析后的样式链、文档默认、主题和直接字符属性。`adopt` 按当前解析重算段间距和对齐，避免命中后用直接属性覆盖样式间距。样式定义变化经既有 `basedOn`／`link` 闭包失效。`a_style_definition_change_refits_dependents_and_reuses_the_rest` 证明：改一个样式的字号后，使用它的段与全量排版一致，未使用它的段命中缓存。
- **语料**：DOCX-MIN-002 无主题部件。文档默认 ascii／hAnsi／cs 为 Times New Roman，eastAsia 为宋体，字号 21 半磅。标题 1（styleId `1`，basedOn `a`）为粗体 32 半磅。`min_002_recognises_builtin_styles_by_name_and_keeps_mode_15` 断言解析结果；Word 2024 参考里的 SimSun 与 TimesNewRomanPSMT 是版式测量，本切片不把调用者 face 换成这些字体。内存中的等价文档由 `resolved_styles_set_size_weight_and_requested_families` 排成 16 pt 粗体与 10.5 pt 正文，并记下请求的字体名。
- **自动验证（Linux x86_64，Rust 1.98.1）**：`cargo fmt --all`；`cargo clippy --locked -p joyword-model -p joyword-docx -p joyword-engine -p joyword-layout -p joyword-pdf -p joyword-render -p joyword-shell -p joyword --all-targets -- -D warnings`；上述包的 `cargo test --locked`。model、docx、engine、layout、pdf、render 与 `joyword` 库测试通过。`joyword-shell` 的集成测试通过；库测试里 `identity_change_between_stat_and_open_is_not_a_baseline`，以及 `w07_save_original_rejection::save_original_reruns_the_full_conflict_verification_itself`、`w07_source_conflicts` 的同长度重建与硬链接别名两项，在本机文件系统上仍失败。这四项与 T211 记录相同，相关源文件未改。`python3 scripts/check_docs.py` 通过（84 个需求、66 个任务、18 条行为登记）。`python3 scripts/check_gates.py` 通过（124 个 Rust 文件）。
- **未覆盖**：样式面板与预览、多级编号、制表位、边框底纹、查找替换、格式刷与自动更正、主题 tint／shade、按字体名加载面、表格样式解析、非自动颜色绘制、PDF 斜体、样式部件改写，以及人工制作多级标题的验收。没有新增依赖。没有启动图形界面。W09 harness 重锚到 `3787156203787884c81a5f36786d204e5a1785d9`（本任务改 `crates/` 的提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境重跑固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T200 编号与制表位（2026-10-18）

这一刀在样式基础之上做多级编号和制表位。样式基础切片里对编号和缩进的整段拒绝，由本切片的列表路径取代；居中、右对齐和两端对齐与列表、缩进或自定义制表位仍拒绝。T200 保持 `in_progress`。语料和 Word 2024 参考里没有 `abstractNum` 样例，测试用合成包和内存文档。

- **定义**：导入 `numbering.xml` 的 `abstractNum`、`num`、`lvl`、`lvlOverride` 和 `startOverride`。`numStyleLink` 与 `styleLink` 存下来但不跟随，标签记为未解析。`nsid`、`multiLevelType`、`lvlJc`、`rPr`、`legacy`、`isLgl` 以及级别段落属性里除缩进和制表位以外的内容标记为未建模，部件字节保留。重复 id 同样标记未建模。
- **计数**：每个 `numId` 单独计数。见到级别 H 时，只重置那些 `lvlRestart` 指向 H 的更深级别：省略表示上一级被使用时重启，`0` 表示永不重启，`n` 表示级别 `n-1` 被使用时重启。第一次显示值是 `start`（或 `startOverride`），不是加一之后。`%n` 引用尚未使用的更高级别时显示该级的 start，不消耗它。`numId` 0 取消样式上的列表，不产生标签。
- **标签文字**：decimal、lowerLetter、upperLetter、lowerRoman、upperRoman、chineseCounting、ideographTraditional、chineseLegalSimplified 和 `none` 按上面的范围绘制。`%%` 是百分号，`%1` 到 `%9` 替换对应级别。字母超过 26、罗马数字超过 3999、中文计数或简体大写超过 99999、甲乙超过 10，以及 0，记为不能绘制。bullet 和其他未列出的 `ST_NumberFormat` 记为不支持，计数仍前进，不用 `lvlText` 里的静态符号当标签。
- **制表位**：段落、样式和编号级别按文档默认、`basedOn`、级别，然后直接段落属性叠加。`clear` 去掉同一位置的继承制表位。对齐为 left、center、right、decimal；`num` 按 left 放置。前导符 none、dot、hyphen、underscore 用重复字形填充。bar、middleDot、heavy 和相对位置使排版失败。默认间隔是 `w:defaultTabStop`，缺省 720 twip；自定义制表位抑制其右侧以左的默认位，默认位从右侧下一个间隔倍数继续。下一个制表位是严格大于当前笔位置的第一位。
- **排版**：只有非空标签、非 tab 后缀、非零缩进、自定义制表位或正文里的制表符才走列表路径，其余仍用原来的行拟合。标签字形放在左缩进减悬挂处，正文从后缀落到的位置开始；续行用左缩进。每行仍是一个 `GlyphRun`，标签前缀在文本里，正文簇保持段落源偏移。不支持的格式成为 `LineDiagnostic::Numbering`，正文照排，没有猜测标签。级别上的 `rPr` 不另绘。屏幕和 PDF 共用这一页场景。
- **缓存**：标签文本、后缀和默认制表位间隔哈希进 `resolved_style`，没有新增键字段。`list_marks` 在排版前按文档顺序算好。计数变化使后续项的键失效，因此不会把旧标签从缓存抄走。`a_list_counter_change_refits_later_items_and_reuses_an_unrelated_paragraph` 把增量结果与全量比较，并要求无关段落命中缓存。
- **编辑与命令**：`SetNumbering` 把选区碰到的每段写成同一个 `numId` 和级别；`None` 与 `numId` 0 都写成 `numId` 0。`SetNumberingLevel` 只改 `ilvl`，每段保留自己的 `numId`（直接值，否则样式，否则缺省段落样式）；没有列表的段是 `NoNumbering`。未知实例和超过 8 的级别拒绝。一次撤销恢复改前的 `numPr`。行为登记 `LIST-APPLY-UNDO`、`LIST-LEVEL-UNDO`、`LIST-REMOVE-UNDO`，来源是假设，没有 Word 观察。外壳 `list.apply` 当时必填 `num`，级别 0，挂在编辑菜单；第四切片起 `num` 与 `bullet` 都可选，见文末「T200 项目符号、tint 与表格样式」。`list.level` 必填 `level`，不进菜单；`list.remove` 无参数，挂在编辑菜单。无文档、打开中和只读时与 `style.apply` 一样禁用。`list_apply_level_and_remove_go_through_the_command_registry` 经 `Office::invoke` 应用、改级、撤销和取消。
- **导出**：`check_envelope` 比较编号存储。存储变化（包括只改定义、不改段落）拒绝，因此不会悄悄用旧的 `numbering.xml` 冒充新定义，也不会重写该部件。段落 `numPr` 变化时，两侧都要通过样式指定子集和编号实例检查，然后重写 `w:pPr` 里的 `w:ilvl` 与 `w:numId`。带制表位、缩进或其他子集外属性的段落拒绝，避免重写丢掉这些 XML。默认制表位属于设置，改变它会因设置不等而拒绝。拆段仍复制源 `numPr` 字节，`mapped_indent_fonts_language_numbering_and_explicit_off_round_trip` 继续通过。
- **未覆盖**：样式面板、查找替换、格式刷、主题 tint／shade、按字体名加载面、表格样式解析、边框底纹、级别字符格式、字母超过 26、甲乙超过 10、bar／middleDot／heavy、居中／右对齐／两端对齐与列表或制表位同用、编号部件改写、非自动颜色、PDF 斜体，以及人工制作多级列表的验收。没有新增依赖。没有启动图形界面。W09 harness 重锚到 `61a24f1ce9c9311df9e323a106d67310f8ca6ea2`（本任务改 `crates/` 的提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境重跑固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T202 页面与页眉页脚第一切片（2026-10-18）

这一刀做显式分页符、纸张大小／页边距／方向、下一页分节的页框，以及主页眉页脚和其中的 `PAGE`／`NUMPAGES`。屏幕与 PDF 仍共用同一次 `layout_document` 的 PageScene。T202 保持 `in_progress`。

- **页框**：`w:pgSz` 的 `w:w`／`w:h` 是实际尺寸，`w:orient` 只是标记，排版不交换宽高。命令 `page.orientation` 在方向改变时交换宽高再写标记。页边距的左边含装订线。页眉距离从页顶量到页眉顶，页脚距离从页底量到页脚底。页眉页脚画在边距带里，不推动正文；墨水越过正文框时记 `StoryExceedsMargin`。正文边距吃掉页面、非正尺寸和非绝对／负值返回 `InvalidPage`。页眉或页脚距离大于页高不再拒绝导入的短页，只在绘制时记超出。
- **分页符**：`w:br w:type="page"` 映射为 `InlineKind::Break(BreakKind::Page)`。文首或连续分页符各占一页空白页。`w:pageBreakBefore` 不额外制造文首空白页。仅含分页符的段落仍有段落标记。分页符与编号或制表位同段时拒绝。
- **分节**：本切片每个分节边界都是下一页。连续、奇数页、偶数页和分栏未做。节链断裂返回 `InvalidDocument`。带保留属性的节在排版时拒绝。
- **页眉页脚**：只应用 Default。省略的 Default 引用继承前面最近一节的 Default story；显式空 story 是引用，不继承。`titlePg`、`evenAndOddHeaders` 和 First／Even 引用会保存，排版记 `HeaderVariantNotApplied`，每页仍画 Default，不猜测首页内容。
- **页码**：预检仍拒绝正文里的域投影。页眉页脚里的 `fldSimple`，指令为 `PAGE` 或 `NUMPAGES`，其余为空或只有 `\* MERGEFORMAT` 时，导入为 `PageField`，不读缓存结果。其他开关、`fldChar` 和正文页码域保持不支持；含这类对象的页眉段落整段跳过并记 `StoryContentSkipped`，不用缓存数字冒充页码。页码在正文分页之后按 1 起替换。若绘制改变了页数，返回 `LayoutError::Unstable`（“布局未稳定”）。
- **约束**：`keep_next`／`keep_lines` 沿用 T105／W06。高于整页内容区的 keep 链记 `KeepTallerThanPage` 后继续，不循环。`LineTooTall` 仍在。widowControl 不实现：layout_rules R5／R8 记录 Word 2024 在 WidowControl 为真且 keepLines 时仍把段落拆成 1+1 或 1+2。
- **编辑**：`Command::SetPageSetup` 改当前节。页面有变化时把全部正文段落记入变更，使撤销成立并让增量排版不能沿用旧页的纵坐标。页面未变则是空操作。`page.break`、`page.size`、`page.margins`、`page.orientation` 走命令注册表。无文档、打开中和只读时禁用。非法页边距拒绝。页眉部件字节不改写；改变页面设置的导出被信封拒绝。正文扁平投影保留分页符的对象标记，使光标偏移与段落标量对齐；页眉页脚不进入该投影。脚注、尾注、批注和文本框故事仍拒绝打开，避免静默丢掉它们的文字。页眉页脚里无法识别的简单域整段保留、不导入缓存结果。
- **缓存**：分页符和页码域进入段落缓存的内容摘要。拟合命中后仍按当前节覆写页框和分节信息，避免窄的后一节用第一节的宽度。增量路径在组装之后绘制页眉，与全量比较页、行、页眉文本和注释。
- **未覆盖**：人工改变页边距并检查缩放后分页；widowControl；首页不同与奇偶页不同的实际切换；复杂域与正文页码；连续／奇数／偶数分节与分栏；页码重新开始；页眉编辑与页眉部件改写；页面设置写回 DOCX。没有新增依赖。没有启动图形界面。W09 harness 先重锚到 `b2b807ae25d19db5cb5a525d67a7bce457c45d0c`（本任务改 `crates/` 的提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。该树上固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol；地址空间限制探测 `enforced=true`。合并 origin/main 的查找、格式刷与样式面板之后，crates 树不再等于该提交。缓存测试夹具里的 `expect` 被门禁判为新的 panic 点，改为按同一组 A4 twip 直接构造页框（`50368ab28d01be1b4b6c470ca83310acc8c4684f`）。harness 改锚到该提交，随后只改 experiments 与本记录。本环境再跑固定矩阵 smoke：81／81，43 ok、38 rejected，`matrix_complete` 为真，地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T200 查找、格式刷与样式面板（2026-10-18）

这一刀在编号与制表位之上做查找替换、格式刷和样式面板。主题 tint／shade 与编号级别的 `w:rPr` 当时仍不做，第四切片补上。T200 保持 `in_progress`。

- **查找**：`search` 在文档快照上按 Unicode 扩展字素匹配，偏移仍是 Unicode 标量。大小写敏感时比较字素字符串；不敏感时比较各字素的 `char::to_lowercase` 展开，不把一个字素拆开。因此 `ß` 不匹配 `SS`。空查询拒绝，查询超过 1024 个标量、替换超过 4096 个标量也拒绝。整词的边界是：字素的首个标量若为 ASCII 字母数字，或为非 CJK 字母，才算词内；CJK（含假名、谚文、注音和表意文字）本身就是边界，所以「中」能在「中国」里作为整词命中，「cat」不能命中「category」。行内对象（含换行）不可拆，匹配不能穿过它们；同一段里跨 run 的文字可以命中。对象占位符 U+FDD0 不参与搜索。先走正文，再按 `stories` 的顺序走其他 story，包括表格和嵌套 SDT，深度受既有嵌套上限约束。命中上限 256，达到后停止扫描并置截断标志。内容锁定、段落或 run 的保留属性、以及保留行内对象上的命中计入 `skipped`，不进入命中列表。不透明块不搜索。
- **替换**：`replace_commands` 把命中从后往前换成 `DeleteRange` 再 `InsertText`，因此前面的偏移保持有效。这些命令放进一次事务，一次撤销恢复全部替换。替换后的文字不再被搜索，把 `a` 换成 `aa` 仍是 `aa`。空替换只删除。插入沿用插入点所在 run 的格式；跨过不同格式的命中，替换文字取插入点的格式。只读策略下查找仍返回命中，提交替换返回只读错误。外壳 `find.next` 挂在编辑菜单，快捷键 Cmd/Ctrl+F；`find.previous` 不进菜单。二者按插入点选取下一处或上一处并环绕。`find.replace` 与 `find.replace_all` 挂在编辑菜单。单次替换优先改当前选区（当它正好是一处命中），否则下一处，再否则环绕到第一处，并把选区留在替换后的范围。全部替换提交一次事务。打开过程中这些命令禁用；只读时查找仍可用，替换禁用。
- **格式刷**：刷子保存可导出的段落样式、对齐和 `RunChange` 能表达的直接字符格式。复制时拒绝隐藏、语言、保留标记、主题字体、主题色、非半磅字号，以及 ascii 与 hAnsi 具名不一致。`hint` 和具名复杂文种字体不进入刷子，也不因此拒绝。插入点、整段或跨段选区同时复制段落样式和对齐；段内局部选区只复制字符格式。应用到会改变样式 id 的目标时，目标须通过样式指定子集（允许已有编号）。样式 id 不变时，目标必须是空白段落：没有样式、段落属性、段落标记格式、行内对象，且 run 没有样式和直接格式。因此不能把粗体刷到已经带样式、却不改样式 id 的段落上。字符样式先于段落样式应用，以便整段链接规则改写 `pStyle` 之后再写回复制来的段落样式。锁定范围和跨容器范围拒绝。`format.copy` 挂在编辑菜单，可选重复；`format.paint` 与 `format.cancel` 不进菜单。重复与否是外壳状态：不重复时刷一次就放下刷子。每次刷入是自己的一次可撤销事务。
- **样式面板**：`style_rows` 用 `resolve_paragraph`／`resolve_run` 列出粗体、斜体、半磅字号、颜色（自动、RGB 或主题）以及东亚和 ascii 具名字体。表格与编号样式只显示类型，点击仍走 `style.apply`，引擎照旧拒绝。`caret_styles` 报告插入点的段落样式和字符样式。GUI 在视口旁放一列可滚动的样式，点击经注册表调用 `style.apply`，当前样式在列首标出并高亮对应行。查找条用可聚焦的普通区域接收字符、退格、回车（查找下一处）和 Esc。这是最小可用界面，没有启动窗口做人工验收。
- **未覆盖（当时）**：主题 tint／shade、按字体名加载面、表格样式解析、边框底纹、级别 `w:rPr`、字母超过 26、甲乙超过 10、中文计数超过 99999 或为 0、罗马数字超过 3999、bar／middleDot／heavy、居中／右对齐／两端对齐与列表或制表位同用、编号部件改写、非自动颜色绘制、PDF 斜体剪切、正则查找、全角半角折叠、格式刷省略的 hint 与具名复杂文种、整段链接字符样式仍走既有应用规则、已有样式的段落在不改变样式 id 时不能刷直接格式，以及人工验收。其中项目符号、级别标签、字母超过 26、列表对齐、tint／shade 和表格样式的粗体底纹边框已由第四切片补上，正则查找与全角半角折叠已由查找折叠切片补上，其余仍未做。没有新增外部包。没有启动图形界面。W09 harness 重锚到 `8eb55cc450176b288858fa398252e3aac5d546ad`（本任务改 `crates/` 的提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境重跑固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁只把既有的 `unicode-segmentation` 1.13.3 记成引擎的直接依赖，无新包。

### T200 项目符号、tint 与表格样式（2026-10-19）

这一刀补 T200 当时未覆盖的项目符号、级别标签字号与粗体、字母超过 26、列表或制表位上的对齐、主题 tint／shade，以及喂给 T201 排版的表格样式。T200 保持 `in_progress`。没有新增外部包。没有启动图形界面。非自动颜色和下划线仍不绘制。

- **项目符号**：`numFmt` 为 bullet 时，标签是 `lvlText` 把 `%%` 换成 `%` 之后的字面量。`%1` 留在标签里，不当计数。计数仍前进。`lvlPicBulletId` 存下来，标签记 `NumberingProblem::PictureBullet`，不发明字形。级别 `w:rPr` 的半磅字号、粗体和具名 ascii／hAnsi／eastAsia 只作用于标签。字号或粗体与正文不同时，标签单独成一个字形 run，以免 PDF 把同一段文字画两次。字体名是 Symbol 或 Wingdings 时，U+F020–U+F0FF 按公开表映射后再用调用者提供的面整形，不加载另一张面。未映射的字符原样留下。Wingdings 2／3 与 Webdings 不映射。
- **字母与罗马数字**：下标 `(n-1)%26`，重复次数 `(n-1)/26+1`。27 为 aa，54 为 bbb。重复超过 64 记为不能绘制。规范示例没有上限，64 是本切片的界限。罗马数字仍是 1..=3999（`ROMAN_MAX`），超出不发明上横线或 `MMMM`。
- **对齐**：列表几乎都有悬挂缩进，所以「有缩进就拒绝居中」不再挡住列表。居中和右对齐只平移带簇的正文，标签留在悬挂位置，行宽设成帧宽，后面的行放置不再加一次偏移。两端对齐只拉伸纯中文正文，拉丁文仍拒绝「inter-character justification」。制表符或自定义非清除制表位保持绝对停靠。只有缩进、没有列表也没有自定义制表位时仍拒绝，错误串仍是 “justification with indent, numbering or tabs”。段落对话框里 justify 加缩进的拒绝没有改。
- **主题色**：`themeTint`／`themeShade` 是两位十六进制。shade 为 `C * byte / 255`，tint 为 `C * byte / 255 + (255 - byte)`，半入向上取整 `(c * factor + 127) / 255`。条款例子：tint 字节 153 把 80 变成 150；50% shade 的字节 128 或 127 把 150 变成 75。两者都有时先 shade 再 tint。结果写成 `Color::Rgb`。导出仍拒绝主题色，不靠丢掉 tint 来改写 run。排版仍拒绝绘制非自动颜色。
- **表格样式**：`tblStyle` 沿 `basedOn` 从根到叶叠整表格式，再按 band、首行、末行叠条件格式。本切片的属性是 run 粗体、底纹和 `nil`／`single` 边框。粗体是指定值：run 没有直接粗体时强制开或关，不是 toggle 翻转。直接单元格边框、直接底纹和直接 run 粗体优先。横向条纹一行一条。首行外观打开时第 0 行不进条纹；末行外观打开时末行不进条纹并盖过条纹。下标 0 是 band1。`tblLook` 的 `w:val` 按 §17.4.56 的位解释，同名属性盖过位。元素省略时用应用默认：首行、末行和横向条纹开，纵向条纹关，两列关。这是假定，不是 Word 观察。元素存在而属性省略则为假。首列、末列和纵向条纹只存储并在改写时写回，不参与排版。未改的表仍复制原字节。改过且 look 有值的表写出 `1`／`0` 属性；look 为空时不发明 `tblLook`。样式部件不改写。主题色边框和底纹仍不解析。有表时增量排版仍是整篇重排，页与全量一致。
- **命令**：`Command::ApplyBullet` 在同一次事务里 `ensure_bullet` 再写级别 0 的 `numPr`。已有无图片的 0 级 bullet 就复用，否则新建抽象编号和实例。新标签是 U+2022、制表后缀、起始缩进 720 twip、悬挂 360 twip。这些距离是 JoyWord 预设，不是 Word 测量。一次撤销恢复编号库和段落。`list.apply` 的 `num` 与 `bullet` 都可选；有 bullet 时优先，否则仍要 `num`，两者都缺的失败信息是 `num`。功能区 `start.list_bullet` 紧挨在 `start.list_remove` 之前，固定参数 `bullet=1`，图标 `list-bullet`。目录下限从 40 升到 41。新建编号定义之后的导出仍拒绝；套用包里已有的项目符号只改 `numPr`，编号部件字节不变。
- **缓存**：段落缓存键哈希标签字号和粗体。只改级别字号或粗体时，该段未命中缓存，无关段落命中，增量页与全量相同。
- **未覆盖**：按字体名换面、甲乙超过 10、中文计数超过 99999 或为 0、罗马数字超出 1..=3999、字母重复超过 64、bar／middleDot／heavy、编号部件改写、非自动颜色与下划线的字形绘制、PDF 斜体剪切、格式刷省略的 hint 与具名复杂文种、整段链接字符样式仍走既有应用规则、已有样式的段落在不改变样式 id 时不能刷直接格式、Wingdings 2／3 与 Webdings、只有缩进时的居中／右对齐／两端对齐、首列／末列与纵向条纹的排版、主题色底纹，以及人工验收。无 tint／shade 的主题色边框、单元格 `tcMar`、`nil`／`single` 以外的边框和百分比宽度已由 T201 第三切片补上；嵌套表仍未做。合并 origin/main 的页码域、分节与等宽分栏之后，W09 harness 重锚到 `547c37466710a9bbef48c3d9d1832f78c545b0a4`（该提交的 `crates/` 同时含项目符号、tint／shade、表格样式和 T202 页码域；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T200 查找全半角折叠与正则（2026-10-19）

这一刀补 EDIT-006 的两个查找选项：全／半角折叠和正则查找，连同外壳命令参数与引擎测试。T200 保持 `in_progress`。没有启动图形界面。rebase 到 main `ce2ad4f5`（T300 slice 2）之后，`crates/` 相对 `2d6ae46738e1d480ae4bc74087d2cc72a3b7f31c` 有差异。W09 harness 曾锚到 `88ec47a39a6327f4df9f4085c817f64a39c6595f`（当时的 `crates/` 含全角半角折叠与线性时间正则查找，并包含 T300 审阅命令、作者偏好与页边标记）。再 rebase 到 main `5b07db6f7ec72392bd2b123a8235fa1dc0884f31` 之后，锚点改为 `01a6003926fae4e215cfed522d0b34ffeda04bab`（该提交的 `crates/` 含全角半角折叠、线性时间正则查找，以及 T110 的 `security.*` 排除与来源字节复读；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。下文 81／81 记在 `88ec47a` 那棵树上，新锚点尚未重跑固定矩阵。再 rebase 到 main `77f71712820da58ee7e13299ecb01d76d5c2768f`（T202 squash）之后，锚点改为 `79821de14d7e4aca6b9e7d80eaf58158c2735815`（该提交的 `crates/` 含全角半角折叠、线性时间正则查找，以及 T202 的 widowControl、页面设置写回与页眉编辑；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。

- **全半角折叠**：`FindQuery` 新增 `fold_width`。打开后，比较前 U+FF01..=U+FF5E 一律减 0xFEE0 落到 U+0021..=U+007E，U+3000（全角空格）落成 U+0020 普通空格，于是 "ABC" 能命中 "ＡＢＣ"、"123" 能命中 "１２３"、半角空格能命中全角空格；汉字等其他字符不受影响。折叠表是文档化假定，不是 Word 观察；大小写折叠仍按整字素进行，ß 不扩成 SS，与既有语义一致。
- **正则查找**：`FindQuery` 新增 `regex`。模式用锁文件里既有的 `regex` 1.13.1（gpui-pre 快照线已带入，非新外部包）编译，只走线性时间匹配，没有回溯面，不存在 ReDoS；模式超长与空查询沿用既有预算，编译失败返回新的 `FindError::InvalidPattern`。引擎按段落把字素簇（宽度折叠后的）拼成干串、行内对象换成 U+FFFC 占位，模式命中后再映回段落标量偏移：拆字素簇、盖住行内对象或零宽的命中一律跳过，整词沿用既有的字素边界规则；替换文本始终是字面量，`$1` 这类组引用不展开。锁定与保留段落的命中仍只计入 skipped。
- **外壳**：`EditRequest::Find`／`Replace` 增加 `fold_width`／`regex`，四个选项在 `find_move`／`replace_query` 里合成 `FindFlags`。`find.next`、`find.previous`、`find.replace`、`find.replace_all` 的参数表新增可选布尔 `width` 与 `regex`，经注册表执行；无效模式映射为既有 `Rejected`。应用的查找栏暂未加新勾选项，只透传命令参数。`apps/joyword` 的拖放、路径表和项目符号仍保持 main，不并入其他任务的改动。
- **自动验证（Linux x86_64，Rust 1.98.1）**：`cargo fmt --all -- --check`；`cargo clippy --locked -p joyword-engine -p joyword-shell --all-targets -- -D warnings`；`cargo test --locked -p joyword-engine` 全部通过（单元 6、engine 22、find_paint 18、images 3、tables 8、undo_groups 10）；`cargo test --locked -p joyword-shell --test find_paint` 2 项通过。接着跑 shell 集成测试：在 `w07_save_original_rejection::save_original_reruns_the_full_conflict_verification_itself` 失败，失败形态是删除后按相同字节重建时 `conflict()` 为 `None`。这与上文记录的本机文件系统身份检查一致，相关源文件未改。跳过该测试以及 `identity_change_between_stat_and_open_is_not_a_baseline`、`w07_source_conflicts` 的同长度重建与硬链接别名两项后，其余已执行的 shell 测试通过，含 `w10_origin`、`w11_save_original`、`w12_text_input_export`。证据：`fullwidth_forms_fold_to_halfwidth`、`width_folding_combines_with_case_folding`、`regex_find_matches_patterns`、`regex_skips_zero_width_and_cluster_splitting`、`regex_never_crosses_an_inline_object`、`regex_width_folding_applies_to_the_haystack`、`regex_hit_count_is_bounded`、`regex_replace_is_one_undo_and_skips_locked`、`find_width_and_regex_flags_go_through_the_command_registry`。`python3 scripts/check_docs.py` 通过（84 个需求、66 个任务）。应用侧本环境未编译。
- **CI 锁与 W09 重锚（同日，Linux x86_64，Rust 1.98.1）**：recovery-journal 与 docx-robustness 的独立锁补上 engine 直接依赖的既有 `regex` 1.13.1，以及既有传递依赖 aho-corasick 1.1.5、regex-automata 0.4.18、regex-syntax 0.8.11，没有新的外部包版本。gpui-font-probe 不把 engine 编进正常依赖，这次变更不改它的锁。固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --locked -p joyword`（38 项）、`cargo build -p joyword --locked`、layout／pdf／render 测试与统一场景 PDF 读回通过。`cargo deny --locked --all-features check` 通过。`cargo test --locked` 在本机与 loongarch64 QEMU 上有三项既有失败，aarch64 QEMU 上只失败其中的保存原件一项：同长度改写落在同一 ext4 时间戳刻度内，身份比较看不到变化。失败项是 `identity_change_between_stat_and_open_is_not_a_baseline`、`save_original_reruns_the_full_conflict_verification_itself`、`hard_link_alias_writes_are_detected_through_the_stamped_path`。相关源文件未改。信创真机未测。再 rebase 到 main `5b07db6` 之后，这三项由 T110 的内容复读与 `security.*` 排除覆盖；本 rebase 没有重跑它们，也不把那次失败改写成查找切片的新验收。
- **未覆盖**：正则替换的捕获组引用、图形界面查找栏的折叠／正则勾选项、与 Word 查找选项逐项对照的人工验收。应用侧只做参数透传。`regex` 成为 engine 的直接依赖，版本仍是锁文件中的既有钉扎，没有新的外部包版本。

2026-10-20 文档核对：上面「替换文本始终是字面量，`$1` 这类组引用不展开」和本条「未覆盖：正则替换的捕获组引用、图形界面查找栏的折叠／正则勾选项」只描述查找折叠切片结束时的状态。随后的捕获组切片展开 `$0`、`$1`..`$n` 和 `$$`；查找栏的全半角与正则勾选也已接入。不勾选正则时 `$1` 与 `$$` 仍是字面量。与 Word 查找选项的人工对照仍未做。

### T201 基础表格第一切片（2026-10-18）

这一刀做类型化表格的导入导出、可撤销的插入表与增删行列、单元格选择，以及把单元格内容放进屏幕和 PDF 共用的页面场景。合并状态被保存。合并、拆分、跨页行和单元格边距见下一节。T201 保持 `in_progress`。T200 与 T202 仍是 `in_progress`。单元格横坐标用所在节的页内容左缘，内宽用网格列宽减去左右各 108 twip，不改页眉选用和页框计算。

- **模型**：表、行、单元格保存网格、`gridSpan`、`vMerge`、宽度、`nil`／`single` 边框和 `clear` 底纹。`table_structure` 检查空单元格、零跨度、孤立的纵向继续和网格列数。
- **DOCX**：只有整表都落在子集里才映射。未知标记整表保持不透明。当时包含 `tblLook`；第四切片起 `tblLook` 进入模型。修订、嵌套表和缺 `tblGrid` 仍然整表不透明，不分配标识。省略的 `vMerge` `w:val` 按 ECMA-376 §17.4.84 为 `continue`。未改的表复制原字节；改过的类型化表整表重写。文档里已有锁定节点时，编辑导出仍返回 `Restricted`，因此不透明表旁边的段落编辑不会生成一份丢掉该表的包。
- **引擎与外壳**：`InsertTable`、`InsertTableRow`、`DeleteTableRow`、`InsertTableColumn`、`DeleteTableColumn` 各是一次独立撤销。带垂直合并时拒绝增删行；带跨列、垂直合并或网格空隙时拒绝增删列。不能删最后一行或最后一列。`table.insert`（默认 2×2）、`table.row.insert`、`table.row.delete`、`table.column.insert`、`table.column.delete`、`table.select` 在编辑菜单。`table.select` 选中插入点所在单元格的全部段落。跨单元格删除仍是 `CrossContainer`。
- **排版**：单元格内宽是所跨网格列之和减去左右各 108 twip。内容与边框、sRGB 底纹进入同一 `PageScene`。垂直合并不画成跨行；继续格的段落仍在自己的行里。行作为整体换页，不从行中间拆开。文档里有表时增量排版改为整篇重排，结果与全量一致。只有段落的文档仍从左页边起排。
- **未覆盖**：合并与拆分编辑、垂直合并的视觉跨行、跨页拆行、嵌套表、`tblLook`、单元格边距、主题色边框、表格样式、`nil`／`single` 以外的边框，以及制作合同表格的人工验收。没有启动图形界面。没有新增外部包。行为登记 `TABLE-INSERT-UNDO`、`TABLE-ROW-UNDO`、`TABLE-COLUMN-UNDO` 是假设，没有 Word 观察。合并 origin/main 的首页与奇偶页页眉之后，W09 harness 重锚到 `32390c25ee6afb2cf0d22eebd0e7a2f78a99bef1`（去掉新增 panic 点之后仍包含 T202 页眉变体的 `crates/` 提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T201 合并、拆分与跨页行（2026-10-18）

这一刀补上合并与拆分编辑、垂直合并的视觉跨行、可拆行与 `cantSplit`、表头重复和表级单元格边距。T201 保持 `in_progress`。导出仍走既有 ExportPlan：未改的表复制原字节，改过的类型化表整表重写，文档里仍有锁定节点时编辑导出返回 `Restricted`。

- **规范**：`gridSpan` 见 ECMA-376 §17.4.17，`vMerge` 见 §17.4.84（省略 `w:val` 为 continue），`tblHeader` 见 §17.4.49，`cantSplit` 见 §17.4.6，`tblCellMar` 见 §17.4.43。元素存在而某条边省略时，该边为 0。省略整个 `tblCellMar` 时规范把它留给表格样式或应用程序；本切片当时不解析表格样式，沿用第一切片的左右各 108 twip、上下 0。表格样式的粗体、底纹和边框见 T200 第四切片，单元格边距仍用这一假定。这是假定，不是 Word 观察。表头重复同样没有 Word 观察：只重复同一张表里、行首连续且 `header` 为真的前缀；前缀比页正文更高时记一次 `HeaderTallerThanPage` 且不重复。后面的 `tblHeader` 仍保存并导出。
- **引擎**：`MergeTableCells` 与 `SplitTableCell` 各是一次快照撤销。向右合并拒绝没有右邻、任一格已有垂直合并、保留标记、非段落块或内容锁定。成功时跨度相加，两端宽度都是 twip 且和未溢出时写入相加宽度，右框改用右邻的右框，段落接在后面而不拼进同一个 run。向下合并从 `None` 或已有 `Restart` 开始，找到这段合并的末行，再合并下一行同一列起点、同一跨度、尚未合并的格子；内容段落上移，继续格至少留一个空段，锚点的底框改用被合并格的底框。拆分优先解开垂直合并，文字留在原处；没有垂直合并且 `gridSpan` 大于 1 时拆成跨度 1 的格子，新格是空段，第一格保留原文。从继续格拆分、未合并、跨度对不上、下方已属于另一段合并，都拒绝且不改变文档。增删行列仍拒绝已合并的表。
- **排版**：Restart 与随后同一列、同一跨度的 Continue 是一个视觉跨行。继续格的文字不绘制，也没有插入点几何。额外内容高度加在这段合并的最后一行；每一行至少一行高。顶框在第一片，底框在最后一片，左右框每一片都画。底纹仍只画 `Color::Rgb`。默认可按行盒边界把一行拆到下一页。`cantSplit` 为真时，当前页放不下但新的一页放得下，就整行移过去；行比页正文更高时记一次 `RowTallerThanPage`，留在一页上溢出，不再拆开。比页正文更高的单行盒同样记这一条并放置一次。诊断受 `LayoutSnapshot` 的上限约束，每个行至多一条。表头字形和同一批图片在正文之前抄到后续页，矩形在分页时按同一位移重画。前缀加上接下来的一整行（`cantSplit`）或一行盒会超出页正文时，不再重复表头，避免空转翻页。有表的文档增量排版仍整篇重排，页和诊断与全量一致。
- **外壳**：`table.merge.right`、`table.merge.down`、`table.split` 在编辑菜单第 4 组，无文档、打开中和只读时不可用。不新增功能区按钮，功能目录下限不变。
- **未覆盖（当时）**：嵌套表、`tblLook`、单元格 `tcMar`、主题色边框、`nil`／`single` 以外的边框、表格样式、百分比宽度，以及制作合同表格的人工验收。`tblLook` 与表格样式的粗体、底纹和边框已由 T200 第四切片补上；单元格 `tcMar`、主题色边框、其他边框和百分比宽度已由第三切片补上；嵌套表仍未做。没有启动图形界面做人工验收。没有新增外部包。行为登记 `TABLE-MERGE-UNDO`、`TABLE-SPLIT-UNDO` 是假设，没有 Word 观察。合并 origin/main 的行内图片之后，重复表头会把该表头行上已经画出的图片按同一位移抄到后续页；这没有单独的自动测试。再合并字体与段落对话框之后，W09 harness 重锚到 `2e886f16801984c21b60490c770825732a6d6179`（该提交的 `crates/` 同时含合并拆分、跨页行、行内图片和字体／段落对话框；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。

### T201 单元格边距、边框样式与百分比宽度（2026-10-19）

这一刀补上单元格 `tcMar`、`nil`／`single` 以外的五种边框里的 `double`／`dotted`／`dashed`／`thick`、主题色边框，以及 `tblW`／`tcW` 百分比和 `tblLayout` 的 fixed／autofit。T201 保持 `in_progress`。导出仍走既有 ExportPlan：未改的表复制原字节，改过的类型化表整表重写，文档里仍有锁定节点时编辑导出返回 `Restricted`。

- **边距**：`tcMar`（ECMA-376 §17.4.68）整组覆盖 `tblCellMar`。元素存在而某边省略时该边为 0，不按边继承表级边距。`w:start` 记为左、`w:end` 记为右，因为 `bidiVisual` 不在子集里；`left` 与 `start`，或 `right` 与 `end`，同时出现则整表保持不透明。百分比边距仍不透明。
- **边框**：子集是 §17.18.2 的 `nil`、`single`、`double`、`dotted`、`dashed`、`thick`。非 `nil` 的厚度都是 `w:sz` 的八分之一磅，省略 `sz` 仍是 4。`thick` 不另乘宽度。`double` 把总厚度分成线、空、线；不足 3 EMU 时画成一条。`dotted`／`dashed` 沿长边画多段矩形，段数不超过 48。`w:themeColor` 在没有 `themeTint`／`themeShade` 时记成主题槽，并盖过同边的十六进制缓存色；tint、shade、`space`、`shadow`、`frame` 不在允许的属性里，整表保持不透明。排版向文档主题取 sRGB。槽存在就画该色；槽不存在就保留厚度、不画墨。`auto` 仍是黑。屏幕和 PDF 仍共用这些矩形。
- **宽度**：`pct` 是五十分之一百分比。`tblW` 的百分比相对页心宽度（§17.4.63）；跨度 1、非 continue 的 `tcW` 百分比相对表宽（§17.4.71），同列后来的百分比忽略，跨多列的百分比只保存和导出、不改网格。表宽为 auto 或省略且存在百分比列时，表宽取页心宽度。省略的 `tblLayout` 按 §17.4.52 为 autofit：首选宽度宽于页心时缩小到页心，不超过时保持网格。显式 `fixed` 不缩小，也不为了铺满页心而放大。这不是按不可断行内容测量的自动调整。`jc`（§17.4.28）为 left、center、right；`tblInd` 只在左对齐时加到原点，负缩进拒绝导入。居中和右对齐忽略缩进。
- **增量排版**：含表的文档仍调用整篇排版。后缀复制要求正文段落一一对应，表行可以拆页、表头可以重复，做不到与全量页相等，所以没有改这条早退。`incremental_layout_of_a_table_matches_a_full_layout` 和 `incremental_layout_of_a_split_table_matches_a_full_layout` 仍比较页。
- **未覆盖**：嵌套表、`vAlign`、`trHeight`（模型里有字段，导入遇到它们仍整表不透明，排版不用它们）、表格属性对话框、列宽拖拽，以及制作合同表格的人工验收。表格样式见 T200 第四切片，本切片不另做 `tblStyle` 解析。没有启动图形界面。没有新增外部包。没有 Word 观察。再合并 origin/main 的下划线、sRGB 文字和视图选项之后，W09 harness 重锚到 `286a4185cc6c28bcdf4526154b7c546d43b76b4d`（该提交的 `crates/` 同时含单元格边距、边框样式、百分比宽度、T200 表格样式、T204 PDF 和 T205 视图选项；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境重跑固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T201 单元格垂直对齐、行高与表格属性（2026-10-19）

这一刀把 `w:vAlign` 和 `w:trHeight` 放进已经类型化的表，并加一次可撤销的表级属性事务和对话框。T201 保持 `in_progress`。模型字段原本就有，本切片不改模型 crate。导出仍走既有 ExportPlan：未改的表复制原字节，改过的类型化表整表重写；重写路径本来就会写出已保存的行高和垂直对齐。没有这些元素的表仍按原子集成为类型化表。

- **垂直对齐**：`w:vAlign`（ECMA-376 §17.4.83）只存 `top`／`center`／`bottom`。`both` 是垂直两端对齐，不在本子集，整表保持不透明。省略 `w:val` 也拒绝，不把缺省换成 `top`。元素省略时字段为 `None`，排版按顶对齐。
- **行高**：`w:trHeight`（§17.4.80）和 `ST_HeightRule`（§17.18.37）。省略 `w:hRule` 记为 `auto`。`auto` 把 `w:val` 存下来，排版时不用它。省略 `w:val` 记为 0。`atLeast` 拒绝负数。`exact` 缺少或非正的 `w:val` 整表保持不透明，不发明零高度的行。未知 `hRule` 同样不透明。
- **排版**：先按原有三遍算出内容高度（含垂直合并长高和行盒长高），此时对齐偏移仍是 0。然后 `auto` 和省略的行高保持内容高度，`atLeast` 抬到 `max(内容, twip)`，`exact` 改成指定 twip。放不进最终视觉单元格的整行盒不绘制，不把字形切成半行。垂直对齐只平移边框和边距之内的剩余空间：居中取一半，底对齐取全部，顶对齐为 0。内容已经高于单元格时偏移保持 0，不把文字拉到单元格上方；Word 如何裁切溢出没有测量。偏移是绘制时的 `align_shift`，不再据此重排行高，避免垂直合并来回长高。
- **属性事务**：`SetTableProperties` 一次写入表宽、对齐、表级 `tblCellMar` 和每个单元格的直接 `tcBorders`，一次撤销（行为登记 `TABLE-PROPERTIES-UNDO`，来源是假设，没有 Word 观察）。空变更拒绝。保留的表、行或单元格拒绝。表宽 `dxa` 为 1..=22×1440 twip，`pct` 为 1..=50000（1000%），`dxa` 0 拒绝。边距每边 0..=22 英寸。非 `nil` 边框厚度 1..=255，`nil` 允许 0..=255。颜色接受 `auto`、sRGB 和主题槽。表宽不改 `tblGrid`，排版仍按既有 `tblW` 缩放网格。单元格自己的 `tcMar` 不清掉。`tblBorders` 不在类型化子集里，所以边框写到每个单元格。
- **对话框**：`table.properties` 必须带 `commit=apply`。缺 `commit` 返回待输入，打开对话框，不开启事务。取消只关掉对话框。各字段都留空时不产生命令，也不开启事务。宽度参数是 `auto`、`dxa:N` 或 `pct:N`；对齐是 left／center／right。任一边距键出现时，缺的边记为 0。边框四边相同，种类是 `nil`／`single`／`double`／`dotted`／`dashed`／`thick`，厚度默认 4。插入页功能区增加 `insert.table`，图标 `table` 画在 `chrome.rs`。功能目录下限从 41 升到 42，条目任务号仍是 T205。与 T300 审阅命令及 T202 页眉／正文开关一起 rebase 到当前 main 后下限为 53。预填取插入点所在表的表宽、对齐、`tblCellMar`；只有每个单元格每条边都是同一条边框时才预填边框。
- **未覆盖**：嵌套表、列宽拖拽、首列／末列与纵向条纹（`TableCondition` 只存首行、末行和横向条纹，补列带要改 T200 样式解析和模型，超出本切片文件范围），以及制作合同表格的人工验收。没有启动图形界面。没有新增外部包。没有 Word 观察。rebase 到 main `ce2ad4f5`（T300 slice 2）之后，`crates/` 相对 `2d6ae46738e1d480ae4bc74087d2cc72a3b7f31c` 有差异。W09 harness 重锚到 `44e2216d910fcdbd99311c677822d82a4e899be1`（该提交的 `crates/` 同时含单元格 vAlign、行 trHeight、表格属性对话框，以及 T300 审阅命令、作者偏好、页边标记和单元格文字修订；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。先前锚点 `c9ed56b81467fe61981b938f29be3108bbb331fc` 上的固定矩阵为 81／81。这仍不是覆盖引导 fuzz。独立锁无新包。再 rebase 到 main `5b07db6f7ec72392bd2b123a8235fa1dc0884f31` 之后，W09 harness 改锚到 `6f19c88164eda7d1ba1fb6aceba9d5833e756058`（该提交的 `crates/` 含单元格 vAlign、行 trHeight、表格属性，以及 T110 的 `security.*` 排除与来源字节复读；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `77f71712820da58ee7e13299ecb01d76d5c2768f`（T202 squash）之后，W09 harness 改锚到 `391a64f282cb58e4b6e8f39adec6ace2b5a76762`（该提交的 `crates/` 含单元格 vAlign、行 trHeight、表格属性，以及 T202 的 widowControl、页面设置写回与页眉编辑；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `b39eb05ab7a63ada1bb4189bc1a07394592b090b`（T200 find fold/regex squash）之后，W09 harness 改锚到 `6b5193a5a8f045d1028441a7bcbf43d6d023869f`（该提交的 `crates/` 含单元格 vAlign、行 trHeight、表格属性，以及 T200 的全角半角折叠与正则查找；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。

### T201 嵌套表第一自动切片（2026-10-19）

这一刀把一层嵌套的 `w:tbl` 放进已经类型化的表。T201 保持 `in_progress`。列宽拖拽和合同表格的人工验收不做。没有启动图形界面。

- **导入**：单元格内容可以是段落，或恰好再一层符合既有子集的 `w:tbl`（ECMA-376 §17.4.64）。深度从正文表算起为 0，一层嵌套为 1。第二层，或内表解析失败（`tblOverlap`、缺网格、`vAlign` `both`、非正的 exact 行高、主题 tint／shade，以及原先整表不透明的其余标记），整张外表保持不透明。类型化单元格里不留一块不透明块。
- **模型**：`Cell.blocks` 本来就能放表。`table_metrics` 不新增接口。嵌套排版把父单元格内宽当作内容宽度：省略的 `tblLayout` 按 §17.4.52 缩到这个框，显式 `fixed` 不缩。
- **排版**：内表的行流相对父格内容原点，放置时加上父格内容横坐标。默认左右边距仍是省略 `tblCellMar`／`tcMar` 时的 108 twip，所以左对齐且贴着内容原点的内表字形比外表字形靠右 108 twip。内表在父行里是一块不可拆的高度，不自己跨页，也不单独重复表头。两层以上返回 `nested table deeper than one level`。有表时增量排版与全量 `pages()` 一致。
- **编辑**：外格或内格的 `InsertText` 各是一次可撤销事务。网格不变量仍拒绝孤立垂直合并。含非段落（含嵌套表）的单元格合并或拆分返回 `UnsupportedCell`。插入点在内表时，结构命令仍要求顶层单元格位点，返回 `NotATable`。
- **导出**：未改的类型化嵌套表复制原包字节。改过的类型化路径重写外表元素，既有 `table_markup` 递归写出内表。不透明嵌套旁边的编辑仍是 `ExportError::Restricted`，不静默丢掉表。不改页眉页脚部件写回。
- **未覆盖**：第二层及更深嵌套；内表已在子集外的标记；内表自己的跨页拆行和独立表头重复；内表插入点上的行列增删与合并；含嵌套表单元格的合并与拆分；列宽拖拽；首列／末列与纵向条纹；制作合同表格的人工验收。没有启动图形界面。没有新增外部包。没有 Word 观察。rebase 到 main `30a76d8583e2ca130b3a0f8ed2b9a6b3f51aab88`（T202 squash）之后，W09 harness 改锚到 `bf91e67d1393064475711478973cd64cd65de662`（该提交的 `crates/` 含一层嵌套表的导入、排版和单元格文字编辑，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。先前锚点 `83c2751cdc9a5c241ebd289a589b4517e009479e` 上的固定矩阵为 81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。固定矩阵尚未在新锚点上重跑。这仍不是覆盖引导 fuzz。独立锁无新包。再 rebase 到 main `26fd7ce2a87440395dddf94cb19c2569c6186669`（T200 find-bar squash，未改 `crates/`）之后，W09 harness 改锚到 `0386738678d03341e03d4cb710d466472e1e2c96`（该提交的 `crates/` 含一层嵌套表的导入、排版和单元格文字编辑，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `02a49a41e800004949857b1065cb33661b6eee3d`（T300 squash）之后，W09 harness 改锚到 `4a54bc81ae1b8f70f213c9425bbab16a3f8f1b4d`（该提交的 `crates/` 含一层嵌套表的导入、排版和单元格文字编辑，以及 T300 的成对移动与 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。

### T202 首页不同与奇偶页页眉页脚（2026-10-18）

这一刀把已保存的 First／Even 页眉页脚画到对应页上，并把省略引用的链接到前一节扩到这三种类型。屏幕与 PDF 仍共用同一次 `layout_document` 的 PageScene。T202 保持 `in_progress`。导出仍走既有 ExportPlan，不改写页眉部件，也不因页面设置变化放行写回。

- **选用**：`titlePg` 为真时，该节第一页的页眉和页脚用 First。否则，`evenAndOddHeaders` 为真时，偶数页码用 Even，奇数页码用 Default。两个开关省略或显式关闭时都不启用。节的首页仍计入奇偶，但首页不同优先。页码与页眉里的 `PAGE` 相同，是从 1 起的文档页序号。不读 `pgNumType/@w:start`，因此不会把重启后的页码当成奇偶依据。
- **链接**：某种类型没有引用时，沿用前面最近一节里该类型的显式 story。页眉与页脚、First／Even／Default 各自独立。显式 story 即使为空也是引用，后面的节继承这份空内容，不退回更早的 story，也不改画另一种。第一节没有任何该类型引用时，这一页的该位置留空。
- **绘制**：仍在正文分页之后画进边距带，不增加页，不推动正文。墨水越过正文框仍记 `StoryExceedsMargin`。无法绘制的页眉段落仍整段跳过并记 `StoryContentSkipped`。若绘制改变页数，仍返回“布局未稳定”。不再为已绘制的首页或奇偶页发出 `HeaderVariantNotApplied`。
- **未覆盖**：图形界面验收；正文里的 `PAGE`／`NUMPAGES`；复杂 fldChar 与其他域开关；连续、奇数页、偶数页分节与分栏；`pgNumType` 重启；widowControl（layout_rules R5／R8 仍为负结果，不实现）；页眉编辑界面与页眉部件改写；页面设置的 DOCX 写回。没有新增依赖。没有启动图形界面。W09 harness 重锚到 `11fa33c2135f2d7d79a78a480c250c5363fafd1f`（本任务改 `crates/` 的提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T202 页码域、分节与等宽分栏（2026-10-18）

这一刀把 `PAGE`／`NUMPAGES`／`SECTIONPAGES` 放进正文和页眉页脚，按 `w:pgNumType` 计算，并让下一页、连续、奇数页和偶数页分节以及等宽栏进入同一 `PageScene`。屏幕与 PDF 仍共用这次排版。T202 保持 `in_progress`。导出仍走既有 ExportPlan：不改写 `sectPr` 和页眉部件；未改的段落复制原 XML；插入域之后的段落因内联对象变化而拒绝，不把对象标记写成正文。

- **导入**：`fldSimple` 与复杂域 `begin`／`instrText`／可选 `separate` 与结果／`end` 在指令是这三个名字，或只再带 `\* MERGEFORMAT` 时，成为一个 `PageField`。缓存最长 32 个标量，不含对象标记、C0 控制符或制表符；空缓存不保存。结果不进入段落正文，段落不因此锁定。其他 `fldChar` 序列仍整段保留并锁定。`w:type` 省略为 `nextPage`；`nextColumn` 或带额外属性的 `type` 使该节不完整并保留。`pgNumType` 读 `fmt` 与 `start`；`chapStyle`／`chapSep` 使该节不完整。`w:cols` 只接受等宽：`num` 1 视为单栏，2..=45 且间距非负（省略 720 twip）、`equalWidth` 不为假、`sep` 不为真、没有 `w:col` 子元素时记为 `EqualColumns`。
- **页码**：正文域参与最多 8 遍的分页反馈。全量排版对含域的段从显示页码 1、总页数 1、节页数 1 起算；增量路径使用同一组种子。收敛后 `PAGE` 用该节格式，另外两个域用十进制。格式失败，或状态重复／遍数用尽且每个正文域都有缓存时，再排一版只画缓存，并记 `PageFieldCached`。没有缓存则返回“布局未稳定”。页眉页脚在分页之后绘制，不改变页数。
- **分节**：节自己的 `w:type` 描述它如何相对前一节开始。文档第一块忽略第 0 节的类型。偶数或奇数节若下一页序号奇偶不对，先为前一节补一页空白（用前一节的页框和页眉），新节内容页再用 `start`，没有 `start` 则用空白之后的下一个序号。`start` 本身奇偶不对时仍显示它，并记 `PageNumberParity`。连续节在页框和栏规格都相同且当前页已有内容时共用该页，页仍属于打开它的节，栏号不变；纸张、页边距或栏不同则记 `ContinuousPageSplit` 并换页，不再补奇偶空白。
- **分栏**：栏宽是（版心减去栏间距）除以栏数的整数商，各行都按这个宽度拟合，余数留在末栏右侧。竖向放不下时先换栏再换页。`w:br w:type="column"` 推进一栏；只有一栏时换页。多栏节里的表直接拒绝。
- **命令**：编辑菜单 `page.number` 与分页符同组，不进功能区，因此本命令不提高功能目录地板。地板现为 40，来自 T203 的插入图片按钮。它插入一个无缓存的 `PAGE`，一次撤销。行为登记 `PAGE-NUMBER-INSERT-UNDO` 是假设，没有 Word 观察。
- **未覆盖**：widowControl 仍不实现。页眉编辑界面、页眉部件和页面设置都不写回 DOCX。不等宽栏、栏分隔线、`nextColumn`、章节页码未做。页码域不能和编号、制表位、图片或分页符放在同一段。连续节若没有打开自己的页，`SECTIONPAGES` 不计它借住的那一页。状态栏页码仍是物理页序，不是 `pgNumType` 之后的显示页码。没有启动图形界面。没有新增依赖。W09 harness 重锚到 `aa2f97faacd447c43aa4987a095ed94e1c494e83`（合入 PR #36 之后的 crates 树；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T202 widowControl、页面设置写回与页眉编辑（2026-10-19）

第四切片补齐 T202 三项实现缺口，任务仍 `in_progress`。`w:widowControl` 为真时排版在段内分页前模拟断点，避免单独一行留在页底或页顶（依据 ECMA-376 §17.3.1.26；与 layout_rules R5 的 Word 负结果分别登记）。`plan_export` 在节信封不变且仅 `PageSetup` 变化时 patch 各节 `w:sectPr` 的 `w:pgSz`／`w:pgMar`（twip 子集）；`sectPr` 保留片段或其它节属性变化仍拒绝。壳层 `page.edit.header`／`page.edit.footer`／`page.edit.body` 与布局功能区「页眉」「正文」按钮切换 `TextInput` 的 `edit_story`，经引擎路径插入文字并可撤销；`layout.edit_header` 与 `layout.edit_body` 在 T300 地板 50 之上把功能目录地板升到 52，图标 `header`／`body` 画在 `chrome.rs`。证据：`widow_control_keeps_two_lines_together_at_a_page_break`、`page_setup_margin_change_round_trips_in_export`、`page_setup_change_exports_through_the_docx_plan`、`header_story_edits_and_undo_through_the_engine_path`。未覆盖：页眉／页脚部件 XML 写回、图形界面验收、Word 对照。rebase 到 main `ce2ad4f5`（T300 slice 2）之后，`crates/` 相对 `2d6ae46738e1d480ae4bc74087d2cc72a3b7f31c` 有差异。W09 harness 重锚到 `ca8529e1629eac5cc40ec420c38c25d9911867d4`（该提交的 `crates/` 含 widowControl、页面设置写回与页眉编辑，并包含 T300 审阅命令、作者偏好与页边标记；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。先前锚点 `db3cf2f02890cd21a9421f3f9df6cf8dcad2544b` 上的固定矩阵为 81／81。这仍不是覆盖引导 fuzz。独立锁无新包。再 rebase 到 main `5b07db6f7ec72392bd2b123a8235fa1dc0884f31` 之后，W09 harness 改锚到 `86c8c04c3d8d0460a7d0e32ac7a733b7d15f72ea`（该提交的 `crates/` 含 widowControl、页面设置写回、页眉编辑，以及 T110 的 `security.*` 排除与来源字节复读；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。

### T202 页眉页脚部件写回（2026-10-19）

第五切片补上页眉／页脚部件 XML 写回，任务仍 `in_progress`。页眉／页脚 story 的文字编辑已经能经引擎 story 焦点完成；导出此前要求辅助 story 签名完全一致，文字一变就报非文本内容变化，部件按原字节复制。现在 `header_footer::stories_differ` 只比较 Header／Footer。签名未变、没有修订、没有页面设置变化、story 也没变时仍返回原始包。story 有变化时 `header_footer::plan` 只改写变化的部件。

- **写回**：每个被改的 story 必须落在同一个已导入部件。根元素必须是与 story 种类匹配的 `w:hdr` 或 `w:ftr`（ECMA-376 Part 1 §17.11.2／§17.11.3）。段落文字、样式和超链接沿用正文 `patch_paragraph`，只替换该 `w:p` 的字节范围；旁边未建模的兄弟节点（例如页眉里的 `w:tbl`）留在原处。空部件（根元素没有元素子节点，合成段落的 locator 覆盖整根）只接受一段纯文本，并在同一根里写出一个 `w:p`。新的外部超链接 id 只追加到该部件已有的关系部件；没有关系部件则拒绝，不新建部件，`[Content_Types].xml` 保持原样。`document.xml` 的关系在纯文字编辑时不改。
- **不变与拒绝**：未改的首页、偶数页或默认部件不进入替换列表，ZIP 写出原字节和原压缩方法。显式空的 `<w:p/>` 仍是空段，再导入不从默认页眉继承。增删或重排页眉段落、页码域段落、缺部件、跨部件、共享部件、根元素种类不对，都返回 `ExportError::Unsupported`，不发明标记。锁定文档在写回之前仍返回 `Restricted`。第二次保存经 `ExportBinding` 按 story 顺序重定位各 story 顶层段落的 `SourceRef`。
- **自动验证**：`edited_header_and_footer_text_round_trips_and_keeps_relationships`、`a_body_edit_copies_an_unchanged_header_part`、`first_and_even_writeback_keeps_the_other_part_and_an_empty_story`、`an_empty_header_part_gains_plain_text_without_a_new_part`、`a_page_field_header_is_refused_instead_of_dropping_the_field`、`a_header_table_is_kept_when_the_paragraph_text_changes`、`header_link_text_keeps_the_relationship_and_a_new_target_is_appended`、`adding_a_header_paragraph_is_refused`。既有 `header_story_edits_and_undo_through_the_engine_path` 与 `page_setup_change_exports_through_the_docx_plan` 仍通过。`cargo fmt -p joyword-docx`、`cargo clippy -p joyword-docx --all-targets --locked -- -D warnings`、`cargo test -p joyword-docx --locked` 通过。
- **未覆盖**：图形界面验收、不等宽栏、`nextColumn`、章节页码、Word 对照。没有启动图形界面。没有新增依赖。W09 harness 重锚到 `cbfa1d62a10f74cd0a9a73529ab0aed65ef2d24b`（该提交改 `crates/`，含页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。

### T203 行内图片与剪贴板第一切片（2026-10-18）

这一刀导入并插入行内静态图片，把它们放进屏幕和 PDF 共用的页面场景，并规定纯文本与图片字节的粘贴。超链接、HTML／RTF、拖放和浮动图留到后续切片。T203 保持 `in_progress`。

- **接纳**：PNG、JPEG、GIF、BMP、TIFF 在 32 MiB、16384 边长和 2500 万像素以内才成为资源。先看文件头尺寸，再解码。SVG、HTML、EMF、WMF、OLE 和 URL 字节拒绝，不执行。新插入的显示尺寸按 96 dpi，长边最多 6 英寸，字节不变。
- **DOCX**：`w:drawing`／`wp:inline` 或 `w:pict` 里的单个内部图片关系成为行内图片。导入按部件保留身份，不去重。`r:link` 和外部目标不取回。诊断里的关系标识去掉含 `://` 的字符串。浮动锚点、OLE 和多 blip 保持原样并锁定。未改动的图片段落导出时复制原 XML。资源发生变化，或改写含图片的段落，显式拒绝，不改写媒体部件。
- **编辑**：`InsertImage` 先按内容哈希、媒体类型和字节查找已有资源。删除选区再插入图片是同一次事务；拒绝的图片丢掉整个候选，包括其中的删除。`edit.insert_image` 读一个普通文件，有字节上限，不打开 URL 或 UNC。Windows 盘符路径（`C:\` 或 `C:/`）是本地文件。
- **粘贴**：`choose_paste` 优先第一张已接纳的图片。存在图片条目但全部被拒时，不回退成文字。只有文件路径时拒绝，不打开路径。纯文本复制去掉 U+FDD0。GPUI 剪贴板按条目读取，不用会把路径拼进文字的 `text()`。
- **排版与输出**：图片不整形。前面的文字能放下时与图片同一行，后面的文字另起一行。超出内容框的显示尺寸按比例缩小。图片底边落在基线上。PDF 用 krilla 的 raster-images 画出，CPU 光栅绘制同一矩形。空字形行不要求字体。
- **未覆盖**：超链接、HTML／RTF、拖放、浮动 `wp:anchor`、把选中图片复制回系统剪贴板、页眉页脚图片、图片后面的同行文字、表格内图片的产品验收，以及 Word／WPS／浏览器双向粘贴。没有启动图形界面。行为登记 `IMAGE-INSERT-UNDO`、`IMAGE-PASTE-REFUSE`、`IMAGE-DEDUP` 是假设，没有 Word 观察。krilla 仍是 0.8.2，只在 joyword-pdf 打开 raster-images；image 仍是 0.25.10，模型打开 png、jpeg、gif、bmp、tiff。Windows 盘符修正之后，W09 harness 重锚到 `d0f9a5ca51ca6f7610394c372ad5e8be580a58dd`（该提交的 `crates/` 含行内图片、功能区、无 expect 的图片样例，以及把 `C:\` 视为本地路径；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。W09 与恢复实验的独立锁记入 image 0.25.10 及其既有传递依赖，没有升级版本。

### T203 超链接、安全粘贴与图片同行文字（2026-10-18）

这一刀把简单超链接、HTML／RTF 的安全粘贴、图片后的同行文字、插入图片按钮和选中图片的复制接到已有编辑路径。T203 保持 `in_progress`。拖放、浮动图和人工对照未做。

- **导入**：`w:hyperlink` 只有 `r:id` 和／或 `w:anchor`，子节点只是带 `rPr`／`t`／`tab` 的 `w:r` 时，链接记在这些 run 上。关系目标是外部字符串，只存储，不请求。`javascript:`、`data:`、`file:` 可以留下，但打开允许列表拒绝它们。允许打开的只有 `http`、`https` 和 `mailto`：`http`／`https` 要有 `://` 和主机，主机不能带用户信息或前导点；`mailto` 要有 `@`，不能有 `://`。只有锚点不是 URL。工具提示、绘图、未解析的 `r:id` 或其他子节点保持原样并锁定段落，不丢未知 XML。
- **编辑与打开**：`SetHyperlink` 设置、修改或清除选区上的链接，空选区不变，每次一次撤销。编辑写入的外部目标必须已经通过打开允许列表。Ctrl+点击（macOS 上还有 Command+点击）命中可打开的链接时，用 `xdg-open`、`open` 或 `explorer` 把 URL 当作单个参数交给系统，不经 shell；启动失败不报成功。本环境没有启动图形界面，测试不拉起浏览器。
- **导出**：未改动的包仍返回原始字节。链接签名不变的文字补丁保留 `w:hyperlink`。链接改变时改写段落；新的外部 URL 只在这次改写写出新 id 时追加到 `word/_rels/document.xml.rels` 的唯一结束标记之前，不重写整个关系部件。写不出的链接返回“hyperlink structure is not rewritten”，不把链接丢掉。表格段落若带链接，表格改写拒绝该段，不在本切片实现表格超链接导出。
- **粘贴**：手写 HTML 与 RTF 子集，不新增 crate。HTML 保留 `b`／`strong`、`i`／`em`、`u`、段落、换行和 `h1`–`h3`；`script`、`style`、`iframe`、`object`、`embed`、`svg`、`img` 的内容丢掉，不取远程资源，属性忽略。实体编码的标签保持为文字，不再解析。RTF 保留 `\b`、`\i`、`\ul`、`\par`、`\line`、`\tab`、`\u` 和 `\'`；跳过 `\object`、`\pict`、`\stylesheet`、`\fonttbl`、`\info`、`\field` 指令，留下 `\fldrslt` 的可见文字，不把域变成超链接。输入超过 1 MiB 拒绝。深度超过 32、字符超过 10 万或段落超过 256 时退回纯文本。整段粘贴是同一种列表，且文档里已有对应编号实例（有序列表为 decimal，无序列表为 `numFmt` bullet）时才套用该实例；否则只分成段落，不发明 `numbering.xml`。命令数超过输入预算时退回纯文本。
- **图片**：图片后面的文字在剩余宽度放得下第一行时留在同一行，其余另起一行。插入图片按钮不在静态功能区里携带路径；窗口弹出文件框后调用已有 `insert_image_file`。选区恰好是一张内嵌图片时，复制用 `ClipboardItem::new_image` 写入系统剪贴板。GPUI 不提供 HTML／RTF 剪贴板类型，文本里看起来像 HTML 或 RTF 的内容按富文本解析。
- **外观**：超链接下划线是页面场景里的矩形（颜色 0x0563C1，约 0.5 磅，紧贴基线之下），屏幕和 PDF 共用，不写成 `rPr` 的下划线。
- **样例**：DOCX-MIN-004 的简单外部超链接现在可编辑，拆开后两段仍指向同一外部目标；`file://` 附加模板仍只记录、不访问。更早的 T103 记录里“超链接段落锁定”只描述当时的导入，不描述本切片。
- **未覆盖**：拖放；浮动 `wp:anchor`；页眉页脚图片；表格内超链接的改写；平台专有的 HTML／RTF 剪贴板格式；Word／WPS／浏览器双向粘贴对照。没有新增依赖。没有启动图形界面。再并入语料 CLI 渲染之后，W09 harness 重锚到 `0be3483ac634d787e13e41a81e83b52f2ab22ad5`（该提交的 `crates/` 含超链接、安全粘贴、行内图片、表格合并拆分、字体／段落对话框，以及五个原创样例的 CLI 渲染；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。

### T203 表格内超链接与本地图片拖放（2026-10-19）

这一刀把简单超链接接进表格单元格（导入、导出、编辑、命中打开、撤销），并新增本地图片文件的拖放插入。浮动 `wp:anchor` 仍走保留路径，本切片不新建浮动排版。T203 保持 `in_progress`。

- **表格内超链接导入**：单元格段落接受与正文同一子集的 `w:hyperlink`（属性只有 `r:id` 和／或 `w:anchor`，子节点只是带 `rPr`／`t`／`tab` 的 `w:r`），链接记在这些 run 上；带工具提示等复杂包装的链接使整个表保持保留并只读，不丢未知 XML。模型、引擎、排版命中与 Ctrl+点击打开原本就按容器无关方式工作，本切片未改它们。
- **表格内超链接导出**：改写过的表整表重写时，单元格段落按 run 写出——一个模型 run 一个 `w:r`，与正文改写不同，相邻 run 不合并（往返校验把 run 边界视作语义）；带同一链接的连续 run 包在一个 `w:hyperlink` 里。`r:id` 复用包里已有的关系，新目标只在写出新 id 时追加到 `word/_rels/document.xml.rels`。表格段落的可写子集检查不再因 run 带链接而拒绝；写不出的链接仍显式拒绝，不静默丢掉。未改的表仍复制原字节。
- **编辑与打开**：`SetHyperlink` 在单元格选区上设置、修改、清除链接，空选区不变，每一步一次撤销；打开允许列表不变（`http`、`https`、`mailto`）。排版在页面场景画 0x0563C1 下划线，屏幕与 PDF 共用，表格单元格与正文走同一绘制路径。
- **本地图片拖放**：窗口画布挂 GPUI 的 `on_drop(ExternalPaths)`；落点取 `Window::mouse_position()`（文件拖入事件持续更新它），先把插入点移到落点，再按路径顺序插入。壳层 `TextInput::insert_image_paths` 复用 `insert_image_file` 的全部检查（普通文件、32 MiB 字节上限、字节嗅探拒绝 URL／UNC 形态与活动内容），被拒路径跳过、已接纳的每张图片各自一次撤销；全部被拒时返回第一个拒绝原因，界面像其他输入错误一样显示。固定的 GPUI 0.3.9 只在 Windows 与 Wayland 携带拖放路径，X11 后端不发路径，因此 X11 上该处理器不触发（如实记录，不以编译成功冒充平台支持）。
- **证据**：`a_table_cell_hyperlink_round_trips_after_a_cell_edit`、`a_new_hyperlink_target_in_a_cell_appends_a_relationship`、`an_anchor_only_cell_hyperlink_round_trips`、`a_complex_hyperlink_keeps_the_table_opaque`（docx）；`a_hyperlink_inside_a_cell_is_set_edited_and_cleared_with_undo`（engine）；`a_hyperlink_inside_a_table_cell_is_set_edited_removed_and_openable`（shell）；`a_hyperlink_in_a_table_cell_is_an_underline_in_the_page_scene`（layout）；`dropped_image_paths_insert_in_order_and_skip_refused_files`（shell）。行为登记新增 `LINK-CELL-EDIT-UNDO`、`IMAGE-DROP-LOCAL`（均为假设，没有 Word 观察）。
- **未覆盖**：浮动 `wp:anchor` 排版（layout 没有锚点／绕排概念，本切片不新建该子系统；锚点仍保留原字节并锁定段落）；页眉页脚图片；拖放的 GUI 人工验收（本环境无图形会话，且 X11 无路径载荷）；Word／WPS／浏览器双向粘贴对照；平台专有 HTML／RTF 剪贴板格式。没有新增依赖。没有启动图形界面。本切片改了 `crates/`。rebase 到 main `ce2ad4f5`（T300 slice 2）之后，`crates/` 相对 `2d6ae46738e1d480ae4bc74087d2cc72a3b7f31c` 有差异。W09 harness 重锚到 `ee03c2d6cab4d256578d44ef85bf8ae7130c9de2`（该提交的 `crates/` 含表格单元格超链接与本地图片拖放，并包含 T300 审阅命令、作者偏好、页边标记和单元格文字修订；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。单元格段落若同时带文字修订和超链接，导出拒绝，不静默丢掉其中一边。先前锚点 `ebb3609697f26bb64e5df4492824e746cd507a93` 上的固定矩阵为 81／81。这仍不是覆盖引导 fuzz。独立锁无新包。再 rebase 到 main `5b07db6f7ec72392bd2b123a8235fa1dc0884f31` 之后，W09 harness 改锚到 `72840ee7371e3ae595e84090794b0aaf1338cae6`（该提交的 `crates/` 含表格单元格超链接、本地图片拖放，以及 T110 的 `security.*` 排除与来源字节复读；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `77f71712820da58ee7e13299ecb01d76d5c2768f`（T202 squash）之后，W09 harness 改锚到 `a49461023436c59640a9591bc752f1d374bf548c`（该提交的 `crates/` 含表格单元格超链接、本地图片拖放，以及 T202 的 widowControl、页面设置写回与页眉编辑；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `b39eb05ab7a63ada1bb4189bc1a07394592b090b`（T200 find fold/regex squash）之后，W09 harness 改锚到 `700025e7559833934f58eaf85f0546fd54f65485`（该提交的 `crates/` 含表格单元格超链接、本地图片拖放，以及 T200 的全角半角折叠与正则查找；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `1eec33e28e30dd426d055a501a10d487c704d367`（T201 cell vAlign、行 trHeight 与表格属性 squash）之后，W09 harness 改锚到 `818785ff21d38fa31e4bd609d59c7d996ee69eed`（该提交的 `crates/` 含表格单元格超链接、本地图片拖放，以及 T201 的单元格 vAlign、行 trHeight 与表格属性；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `fe50e7479e43ce24bd7f1415223fb5fe9617343b`（Linux GUI entry squash）之后，W09 harness 改锚到 `2195889ab5884b0693a6c0e305a168c47c6061a9`（该提交的 `crates/` 含表格单元格超链接、本地图片拖放，以及 Linux 文件条与 `ctrl-e`；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。

### T203 页面绝对浮动图与页眉页脚图片（2026-10-19）

这一刀只放置最简单的浮动图：`wp:anchor` 在文字前面、`wp:wrapNone`、位置相对页面原点。绕排和衬于文字下方仍保留原字节。页眉页脚里已经导入的行内图片画到屏幕和 PDF 共用的 PageScene 上。T203 保持 `in_progress`。

- **导入**：单个内部光栅 blip，且 `w:drawing` 只有 `wp:anchor`、没有 `wp:inline` 时，才尝试浮动位置。`behindDoc` 缺省或为假，直接子元素是 `wp:wrapNone`，并且没有 `wp:wrapSquare`、`wp:wrapTight`、`wp:wrapThrough`、`wp:wrapTopAndBottom`。位置有两种：`simplePos` 为真时用 `wp:simplePos` 的 x／y；否则 `positionH` 与 `positionV` 的 `relativeFrom` 都必须是 `page`，并且只有非负的 `wp:posOffset`。偏移和 `wp:extent` 仍受既有图片预算约束，边长不超过 100 英寸。`r:link`、外部目标、OLE、SVG、EMF、WMF 在判断位置之前就拒绝，不取回、不执行。图片诊断只保留关系标识，不写入 URL。
- **不放置的锚点**：负偏移、超过 100 英寸的位置、`behindDoc`、非 `page` 的 `relativeFrom`、`wp:align`，以及上面四种绕排，诊断为 `Unsupported`。原 XML 留在包里，段落锁定，模型里不建资源。超大 blip 仍是 `Oversize` 并锁定。同时有 `wp:inline` 时仍走行内路径。
- **排版**：页面锚点的 x／y 是页面坐标，不占行宽，也不按版心缩小。同一段后面的文字仍按行内宽度排列。页眉和页脚 story 里的行内图片复用正文的图片拟合，画在该条带的基线之上；页眉从 `header` 起，页脚在底边距之上。页面锚点即使出现在 story 里也仍用页面坐标。CPU 光栅和 PDF 都读同一张 PageScene；相同 PNG 字节在 PDF 里合成一个图像 XObject。
- **证据**：`a_page_anchor_imports_and_wrap_or_remote_targets_stay_preserved`、`an_anchor_refuses_svg_emf_wmf_and_ole_without_fetching`、`a_header_and_footer_inline_image_imports_with_the_part`、`a_page_anchor_is_a_positioned_rectangle_shared_with_the_cpu`、`a_header_and_footer_image_paint_on_their_bands`、`a_floating_anchor_and_a_header_image_share_pdf_and_cpu_geometry`。行为登记新增 `IMAGE-ANCHOR-PAGE`、`IMAGE-STORY-PAINT`，来源是假设，没有 Word 观察。
- **未覆盖**：`wrapSquare`、`wrapTight`、`wrapThrough`、`wrapTopAndBottom`、`behindDoc`、非 `page` 的 `relativeFrom`（column、margin、character、paragraph、line）、`wp:align`、负偏移、表格内图片、拖放与页眉页脚图片的 GUI 人工验收、Word／WPS／浏览器双向粘贴对照、平台专有剪贴板格式。没有新增依赖。没有启动图形界面。本切片改了 `crates/`。rebase 到 main `30a76d8583e2ca130b3a0f8ed2b9a6b3f51aab88`（T202 squash）之后，W09 harness 改锚到 `6ecf8139ec7db904f2c38621da01dd1faa18b6be`（该提交的 `crates/` 含页面绝对 wrapNone 浮动图与页眉页脚图片绘制，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。先前锚点 `12884fa55b989852160ec31a598cba64f726c108` 上的固定矩阵为 81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。固定矩阵尚未在新锚点上重跑。这仍不是覆盖引导 fuzz。独立锁无新包。再 rebase 到 main `26fd7ce2a87440395dddf94cb19c2569c6186669`（T200 find-bar squash，未改 `crates/`）之后，W09 harness 改锚到 `e6a146b43d244a80be84d83412d7af18315ffacb`（该提交的 `crates/` 含页面绝对 wrapNone 浮动图与页眉页脚图片绘制，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `02a49a41e800004949857b1065cb33661b6eee3d`（T300 squash）之后，W09 harness 改锚到 `ceda23592802ca91f76170be0680a0ce62468dc2`（该提交的 `crates/` 含页面绝对 wrapNone 浮动图与页眉页脚图片绘制，以及 T300 的成对移动与 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `e51cdb86de5ef6cea54d2010b20b7dadfe97a3b9`（T201 nested-tables squash）之后，W09 harness 改锚到 `a78b79af422933491b4df10f845845633aba108b`（该提交的 `crates/` 含页面绝对 wrapNone 浮动图与页眉页脚图片绘制，以及 T201 的一层嵌套表、T300 的成对移动与 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。

### T203 页面相对 wrapSquare 第一切片（2026-10-19）

这一刀在已经放置的页面绝对 `wp:wrapNone` 上，增加页面相对、非负偏移、位于文字前面的 `wp:wrapSquare`。正文行避开方形排除区。`wrapTight`、`wrapThrough`、`wrapTopAndBottom` 仍保留原字节。T203 保持 `in_progress`。来源是 JoyWord 假设，不是 Word 观察。

- **导入**：位置规则与 wrapNone 相同。`simplePos` 为真时用 `wp:simplePos` 的 x／y；否则 `positionH` 与 `positionV` 的 `relativeFrom` 都必须是 `page`，并且只有非负 `wp:posOffset`。绕排必须恰好是一个没有子元素的 `wp:wrapSquare`，不能再有 `wp:wrapNone`。距离先读 wrapSquare 的 distT／distB／distL／distR，缺省再读锚点上的同名属性，再缺省为 0。这不是 Word 省略距离时的 0.125 英寸。负距离诊断为 `Unsupported`。`wrapText` 省略、`bothSides` 或 `largest` 记为较宽一侧；`left`／`right` 只用那一侧；其他取值拒绝。非零 `wp:effectExtent`（l／t／r／b 去掉空白后不是 `0`）拒绝。图片字节、像素、边长和 100 英寸上限不改。`r:link`、OLE、SVG、EMF、WMF 仍不取回；图片诊断不写入 URL。
- **排版**：排除区是页面坐标上的 extent 加上四边距离。放置每一行时从本页已提交的方形里挖空；本行自己的方形在提交前也算进去，所以锚点所在行的文字避开该矩形。较宽一侧是版心里剩下的更宽间隙，宽度相同用更靠左的间隙。文字不拆到方形两侧。间隙比整行窄时按已经整形的簇边界切开；第一个簇比间隙还宽时仍放下。带列表标签的行不重切。间隙为空则把行顶移到方形底边再试，最多 4 次，仍放不下就按原行宽放下。`wrapNone` 没有方形，行保持原位。锚点段之前、同一页上已经放下的行不回头重排。表格单元格走自己的放置路径，不挖这些方形。页眉页脚 story 不经过这段放置，其中的方形不写入排除区，因此不改变正文行。只改图片、不改正文时，既有文档指纹仍可能复用上一张快照；本切片不把图片写入指纹。屏幕、PDF 与 CPU 读同一张 PageScene 矩形。
- **证据**：`a_wrap_square_page_anchor_imports_and_other_wraps_stay_preserved`、`a_wrap_square_keeps_text_outside_the_rectangle_and_wrap_none_does_not`、`a_wrap_square_rectangle_is_shared_by_pdf_and_the_cpu`。行为登记新增 `IMAGE-WRAP-SQUARE`、`IMAGE-WRAP-SQUARE-PLACE`，来源是假设，没有 Word 观察。`IMAGE-ANCHOR-PAGE` 的期望改为只拒绝 wrapTight／Through／TopAndBottom，wrapSquare 另记。
- **未覆盖**：两侧同时排字、锚点段之前已经放下的行、表格单元格文字、页眉页脚方形参与正文绕排、非零 `wp:effectExtent` 的放置、负偏移、非 `page` 的 `relativeFrom`、`wp:align`、`behindDoc`、GUI 人工验收、Word／WPS 观察。没有新增依赖。没有启动图形界面。本切片改了 `crates/`。rebase 到 main `5704bf6226cde228d37e21034c9a8d84b51eb5f7`（T200 捕获组 squash）之后，W09 harness 改锚到 `edd6480d4805af517c056d21ad14a66a327ec807`（该提交的 `crates/` 含页面相对 wrapSquare，以及 T200 的正则替换捕获组；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。该树上固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `cd087577a67cf5efb9164de15cde166ba7da028e`（T201 首列、末列与纵向条纹 squash）之后，harness 改锚到 `bb6454922d904ac5bfa026477d5c0ac66bbc4b8e`（该提交的 `crates/` 含页面相对 wrapSquare，以及 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `a6fe0cf6cacd360f1e000aa26ee705a0804e5eb8`（T300 页眉页脚修订装饰 squash）之后，harness 改锚到 `8309c9e22fdb57fe869fefe82b61eb7fb4f21165`（该提交的 `crates/` 含页面相对 wrapSquare，以及 T300 的页眉页脚修订装饰和 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `5310fabb612bb3a9495fc5b123b77a8f3346266d`（T202 不等宽分栏 squash）之后，harness 改锚到 `9026d230e7e4d34349c3c4f5567f231b9f4d4c52`（该提交的 `crates/` 含页面相对 wrapSquare，以及 T202 的不等宽分栏、T300 的页眉页脚修订装饰和 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T205 功能区、标尺与状态栏第一切片（2026-10-18）

这一刀做开始、插入、布局、视图功能区，当前段的水平标尺，以及状态栏上的页码和字数。T205 保持 `in_progress`。字体与段落对话框、制表位拖拽、编辑标记／隐藏文字／域代码切换，以及 Word 字数对话框对照都未做。

- **功能区**：控件表在 `joyword-shell::chrome`。每个按钮绑定一个已注册命令；注册表里没有的命令被滤掉，不会画出来。点击走 `Registry::invoke_with`。查找在查询为空时返回待输入，窗口把焦点放到查找框。列表级别 1／2 传入 `ilvl` 0／1。A4 与普通页边距使用 `PageSetup::a4_zh_cn` 的 twip；Letter 是 8.5×11 英寸；窄页边距是四边 720 twip 的 JoyWord 预设，不声称等于 Word 的 Narrow 模板。视图页的缩放和语言切换 `modifies_document` 为假。样式应用仍由已有样式面板按样式 id 调用 `style.apply`；`list.apply` 需要编号实例，功能区不编造 numId。
- **状态栏**：页码来自当前布局的插入点（1 起），没有插入点几何时只显示总页数，没有布局时显示页码暂不可用。字数来自会话正文投影，不来自可能过期的页面像素。汉字、假名、谚文按字；西文连续字母按词，字母间的 `'`／`’` 留在词内；连续十进制数字（含全角）按一个词，数字间的 `.`／`,` 留在数字内；标点和空格单独计数；分页符对象标记不计入。字素用既有 `unicode-segmentation`。这是单元测试证据，不是 Word 对话框对照。
- **标尺**：用当前段解析后的缩进和制表位。悬挂缩进移动正文标记，首行缩进移动首行标记；两者同时存在时按存储值显示，不互相抵消。右缩进从版心宽度减去。自定义制表位用绝对 EMU；默认位从 `w:defaultTabStop` 或 720 twip 延续到版心右缘。字符单位标为不能换算，不猜测位置。版心未知时用六英寸作显示比例，不补默认制表位。标记画在标尺上，不能拖拽。
- **图标与目录**：28 个图标是代码里的 16×16 格子，署名在 [feature_catalog.json](feature_catalog.json)。目录把 29 个命令控件和 5 个状态／标尺表面绑到 `PLATFORM-003` 或 `EDIT-009`，证据等级均为 `unit`，地板 34。`check_docs.py` 要求地板不降，并拒绝应用与 shell 目录里的未署名图片。
- **未覆盖**：字体与段落对话框及中文字号；制表位与缩进的拖拽编辑；编辑标记、隐藏文字、域代码显示切换；导航窗格与审阅栏；视图选项进入布局快照键；与固定 Word 字数对话框的数字对照；只用功能区完成报告排版的人工验收。没有新增依赖。没有启动图形界面。合并 `origin/main` 的表格切片之后，W09 harness 重锚到 `16780940111387c848aa1553fd309a9d3c72f0d7`（该提交的 `crates/` 同时含功能区与表格；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。本段是第一切片当时的记录；字体对话框、段落对话框和标尺拖拽见下一节。

### T205 字体与段落对话框第二切片（2026-10-18）

这一刀做字体对话框、段落对话框，以及标尺上的缩进和制表位拖拽。三个视图开关登记在视图功能区，因为排版没有对应选项而保持禁用。T205 保持 `in_progress`。

- **字体**：`format.font` 的参数必须带 `commit=apply`。功能区按钮不带参数，返回待输入，窗口再打开对话框；取消不调用命令。确认后的变更列表作为一次事务。空选区返回空列表，不开启事务。家族非空时把同一具名字体写入 ascii 与 eastAsia；空家族保持原值。名称最长 64 个字符，拒绝 `<`、`>`、`&` 和控制字符。字号接受中文号或半点小数（1 pt 到 1638 pt，即半点 2 到 3276）。中文号用 Office 公开表：初号 42、小初 36、一号 26、小一 24、二号 22、小二 18、三号 16、小三 15、四号 14、小四 12、五号 10.5、小五 9、六号 7.5、小六 6.5、七号 5.5、八号 5。二号不是 21，三号不是 15.75，四号不是 13.75，六号不是 7.875，七号不是 5.25。粗体、斜体可显式开或关。下划线只接受 single 或 none。颜色只接受 auto 或 `RRGGBB`。主题色拒绝。下划线和非自动颜色会写入模型并导出，页面场景仍拒绝绘制这些段，应用成功后界面显示 `dialog.font.paint`。这不是已经画出下划线或彩色字。
- **段落**：`format.paragraph` 同样要求 `commit=apply`，一次事务覆盖选区所在的各段。对齐为 start、center、end 或 both。缩进用 twip，左右为 ±22 英寸，首行与悬挂为 0 到 22 英寸，不能同时写入。段前段后为 0 到 22 英寸的 twip，不写自动段距。行距为省略、Auto(1 到 31680)、Exact 的正 twip，或 AtLeast 的非负 twip。居中、右对齐或两端对齐若与会移动文字的缩进同时出现，拒绝 `dialog.paragraph.justify_indent`，不产生撤销步。`distribute` 拒绝。`snapToGrid` 已为真且行距不是省略或 Auto(240) 时拒绝。字符单位和行单位仍标为不能换算。
- **行距排版**：Auto(240) 的既有中文路径不改。其他正的 Auto 倍数把 profile 行距乘以 n/240 后做整数截断，不是 Word 的行网格测量。Exact 与 AtLeast 用绝对长度；AtLeast 取 profile 行距与该长度的较大值。行框放不下基线时仍是 `LineTooTall`。对话框拒绝 Exact 0。这组规则有单元测试（18 pt 行距、Auto(360) 得到 27 pt），没有 Word 对照。
- **导出**：可写的直接缩进、间距和制表位随段落重写进入 `w:ind`、`w:spacing` 和 `w:tabs`。空的直接制表位列表写出空的 `w:tabs`（明确不要停止位）；省略则不写该元素（继续继承）。Bar、Clear、MiddleDot 和 Heavy 拒绝，不静默丢掉。段落仍含未知标记或不能表示的直接格式时不重合成，避免丢掉未知 XML。字体改动落在已有可表示缩进的段上时，缩进随导出保留。给已有可写制表位的段落指定 `numPr` 会同时写出编号和制表位，`numbering.xml` 仍不改写。
- **标尺拖拽**：拖首行时保持正文左缘，拖正文左缘时保持首行位置，拖右缘时用版心宽度反算右缩进。拖动生成的默认制表位会在该段的直接列表里新增一个 Left、无前导符的停止位。解析结果里的自定义位若没有全部出现在直接列表中，拒绝 `ruler.drag.inherited`，不改写样式里的表。指针移动不足 3 像素不提交。制表位在标尺下方足够远处松开则删除该位。
- **视图开关**：`view.formatting_marks`、`view.hidden_text`、`view.field_codes` 的 `modifies_document` 为假，可用状态在无文档时也是禁用，原因分别是排版不绘制格式标记、拒绝隐藏文字、不投影域代码。它们不进入布局快照键。点击只显示这条原因。
- **目录**：T205 第二切片把地板从 34 升到 39。新增的字体、段落和三个视图控件证据等级是 `unit`。五个新图标仍是 `chrome.rs` 里的 16×16 格子。T203 的插入图片按钮再把目录地板升到 40，图标 `picture` 同样画在 `chrome.rs`。
- **未覆盖**：Word 字数对话框对照；导航窗格与审阅栏；视图选项进入布局快照键（排版没有这些选项，不能把禁用开关说成已经切换显示）；下划线与非自动颜色的页面绘制；Exact 小于基线时画布仍会报错，对话框不把 Exact 悄悄抬到基线；GUI 人工验收未做。没有新增依赖。合并 `origin/main` 的行内图片之后，W09 harness 曾锚到 `347d44aebbf3d7f34e58ad88828f3ca7c031d0a7`（该提交的 `crates/` 同时含字体／段落对话框与行内图片）。随后与表格合并拆分再次合入，当时锚点见上一节 T201 记录，不放宽 `git diff <SHA> -- crates`。本段是第二切片当时的记录；下划线绘制和视图选项见下一节。

### T205 下划线、颜色与视图选项第三切片（2026-10-19）

这一刀让页面场景绘制单线、双线和点线下划线以及 sRGB 文字颜色，并把格式标记、隐藏文字和域代码放进布局快照键。三个功能区开关可以切换，且不修改文档。T205 保持 `in_progress`。

- **绘制**：下划线矩形和文字颜色写在 `PageScene` 上。CPU 光栅按 `GlyphRun.color` 填充字形，PDF 在画字形前设置同一 sRGB 填充。下划线位置和粗细优先读 OpenType `post.underlinePosition` 与 `underlineThickness`（y 向上，负值在基线之下；厚度应与 `OS/2.yStrikeoutSize` 一致）。`post` 厚度为正时保留其位置，包括位置为 0。厚度缺失或非正时，若 `yStrikeoutSize` 为正，则用它作厚度并把顶边放在基线下一倍粗细。两者都没有时，厚度为 `unitsPerEm/20`（至少 1），顶边同样在基线下一倍粗细。换算后的厚度至少 1 EMU。双线在第一笔下方再隔一倍粗细画第二笔。点线是边长等于粗细的方块，间隔也是这一粗细。这两种间隔是 JoyWord 的重复，不是 Word 测量。斜体不剪切这些矩形。粗线、虚线、波浪线和按字下划线，以及主题色，仍以「character formatting」或「character colour」拒绝。自动色是黑色。`dialog.font.paint` 不再记录，字体对话框也不再显示这条拒绝。
- **视图键**：`LayoutKey.view` 含格式标记、显示隐藏文字和域代码。默认是标记关闭、隐藏文字省略、显示域结果。内容指纹仍只含文档。隐藏文字和域代码改变整形字符串，因此进入段落缓存键；格式标记不进入，同一拟合可以共用。增量流在视图不同时不复用上一张页面。格式标记是灰色 `(128,128,128)` 矩形：空格是字号十分之一的方块，制表位是横条加方头，段落末是竖茎加方块。它们不改变前进量或断行。`w:vanish` 为真的显示或省略会改变排版。隐藏的段落标记仍拒绝，因为那会合并段落。单段域在结果模式下省略分隔符之前的指令，在代码模式下省略分隔符之后的结果；没有分隔符时结果投影省略整段域，代码投影保留指令。跨段域、脱离锚点的域和其他批注仍拒绝。正文和页眉页脚的 `PAGE`／`NUMPAGES`／`SECTIONPAGES` 在域代码打开时显示这三个名字，不打印花括号或开关；关闭时仍显示计算后的页码。超链接的蓝色下划线仍是单独的场景矩形。
- **字数**：状态栏规则不变。新增测试用手算样例「你好，Word 2024 的字数：don't count 12.5。」对照 `census`：汉字 5、西文词 3、数字 2、词数 10、标点 3、空格 4。这是 EDIT-009 的单元规则，不是 Word 字数对话框对照。
- **目录**：地板是 41，含 T203 的插入图片按钮和 T200 的项目符号按钮。三个视图控件证据等级仍是 `unit`。没有把本切片说成参考对照。
- **未覆盖**：与固定 Word 字数对话框的数字对照；导航窗格与审阅栏；粗线、虚线、波浪线和按字下划线；主题色；隐藏段落标记导致的段落合并；跨段域；斜体剪切下划线；只用功能区和对话框完成报告排版的人工验收。没有新增依赖。没有启动图形界面。W09 harness 曾锚到 `f7ddec86e36f55489a3a68a99fddb1b51a822b82`（下划线与视图选项合入页码域之前的 `crates/`）。再并入 T200 的项目符号、主题 tint／shade 和表格样式之后，锚点曾改到合并提交 `6bf8318298d533d52483630bd643fb1b2d570549`（该提交的 `crates/` 同时含下划线、sRGB、视图选项、编号标签和表格样式）。当时固定矩阵 smoke：81／81，43 ok、38 rejected。再并入 T204 的 PDF 导出与打印到文件之后，当前锚点见下一节。

### T210 五个原创样例的 CLI 渲染与转换（2026-10-18）

这一刀让 DOCX-MIN-001..005 都能经 `joyword-cli` 的 `render` 与 `convert --to pdf` 输出。T210 保持 `in_progress`。没有启动图形界面，也不把 render 与窗口显示说成已经一致。

- **设置导入**：`w:settings` 的子元素已有模型字段时不再整段 `preserved`。已识别的子元素是 `compat`（仅当每个子元素都是 `w:compatSetting`，`w:name` 为 `compatibilityMode`，`w:uri` 为 Word 兼容 URI，且 `w:val` 能解析为 `u32`；最后一个有效值写入 `compatibility_mode`）、`trackRevisions`、`evenAndOddHeaders`、`defaultTabStop`、`documentProtection`、`writeProtection` 和 `attachedTemplate`。`attachedTemplate` 只把 `r:id` 记入 `DocumentSettings::attached_template`（元素在而没有 id 时为 `Some("")`），不打开文件、UNC 或 URL（QUALITY-005）。未知 `w:` 子元素、外部命名空间子元素、非 compatibilityMode 的 `compatSetting`、错误 URI，以及无法解析的 compatibilityMode，把整个 settings 根保留为 `Properties` 片段。部件字节仍留在 OPC 包里。未编辑的 `plan_export` 在进入 `Restricted` 判断之前返回原包字节，因此只读文档的无编辑保存仍是逐字节复制。
- **排版**：`settings.preserved` 仍拒绝布局。`track_revisions == Some(true)` 不再单独拒绝布局，这样 MIN-005（正文是普通段落，设置子元素都已建模）可以分页；编辑权仍由既有只读原因保持只读。正文 `Block::Opaque` 在预检、全量段落路径、混合路径和增量路径里跳过：不占几何，计入 `LayoutSnapshot::omitted_blocks`，指纹纳入不透明块的标识、元素限定名和保留句柄。内容控件仍返回 `Unsupported`。含表的文档增量排版仍整篇重排。
- **增量下标**：流向量（段落标识、缓存键、页起点）仍按段落序号。未改段落的查找用这个序号，不再用正文块下标。节几何和 `section_break_before` 继续用正文块下标。不透明块向依赖向量推入默认标志，使 `produced_contexts` 与正文块对齐。`walk_back_keep_next` 的 `keepNext` 回走按段落序号写 `same`，分节判断用该段的正文块下标。缓存与全量相等的测试覆盖：不透明块计数与指纹、不透明块两侧段落的二次增量（稳定边界为 0）、编辑后增量等于全量、不透明块之后的分节、以及表加不透明块仍等于全量。
- **CLI**：`render` 与 `convert` 的 schema 1 JSON 增加 `omitted_blocks`。帮助文本说明不透明块被跳过、计数、且不占页面几何。非空布局诊断或 `omitted_diagnostics != 0` 仍以“字体无法完整显示文档”拒绝；`omitted_blocks` 本身不使渲染失败。未支持的转换目标、覆盖已有输出、预算和先写后校验的契约不变。输入文件不修改。
- **自动验证**：`cargo test --locked -p joyword-docx -p joyword-layout -p joyword-model -p joyword-cli --offline` 通过。`JOYWORD_TEST_FONT` 指向文泉驿微米黑、`JOYWORD_REQUIRE_TEST_FONT=1` 时，`joyword-cli` 的 visual 测试 4 项通过：五个样例各 `render` 与 `convert --to pdf` 两次，PNG、PDF 与报告 JSON 逐字节相同，MIN-003 的 `omitted_blocks` 为 1，覆盖已有输出与 `--to html` 退出码为 2，输入字节不变。上述四个包的 `cargo clippy --all-targets -- -D warnings` 与 `cargo fmt --all -- --check` 通过。没有新增依赖。
- **未覆盖**：render 与窗口显示的人工一致性；信创真机；没有安装宋体、Noto CJK 或 `JOYWORD_TEST_FONT` 时 visual 测试按既有契约跳过，不把跳过说成已在该环境画出五个样例。Droid Sans Fallback 缺少样例里的全角标点，会因缺字诊断拒绝渲染，这是字体覆盖而不是导入或分页失败。W09 harness 曾锚到 `655cbccdd1737827da29ea182c45ac6923e74828`（本任务改 `crates/` 的提交）。当时固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。再并入超链接与安全粘贴之后，当前锚点见上文 T203 超链接记录，不放宽 `git diff <SHA> -- crates`。

### T204 统一 PDF 与打印到文件第一切片（2026-10-19）

这一刀让文件菜单的导出 PDF 和打印到文件使用屏幕已经安装的同一份 PageScene，不再另做一次分页。CLI `convert` 在它自己已经排好的同一场景上调用同一个写出函数。T204 保持 `in_progress`。没有做两个独立阅读器的人工检查，也没有在真实打印机上打印。

- **同一场景**：应用导出读取 `ready_layout` 的 `LayoutSnapshot`，不调用第二次 `layout_document`。布局未稳定或没有编辑器时命令禁用，原因是 `file.export.unstable`。没有页面场景的 Office 宿主把这两条命令始终禁用。页范围在已有场景上切片；书签页号按切片重映射，落在范围外的条目丢掉。CLI 的 `convert` 仍用自己的一次分页，然后 `render_export`。
- **文档信息**：Producer 固定为 `JoyWord`，不含时间或构建戳。标题只在 OPC `/docProps/core.xml` 的 `dc:title` 非空时写入。拒绝 DOCTYPE、超过 64 KiB、非 UTF-8 和嵌套元素，并展开预定义实体。不读时钟，不写 CreationDate 或 ModDate。`SerializeSettings.xmp_metadata` 为 false，避免 XMP 里的实例标识。同一输入两次写出逐字节相同。
- **文字与页面**：字形仍按场景坐标交给 krilla 的定位字形接口，嵌入子集并带 ToUnicode。`pdftotext -enc UTF-8` 在本机有该程序且能读出文字时核对抽出的文字。未设置 `JOYWORD_REQUIRE_TEST_FONT` 时，程序缺失、写入中断或退出码非零都跳过，不把跳过说成已抽出；统一 PDF 作业设置了该变量，因此那里失败仍是失败。页面宽高来自各页场景。后一节可以有自己的宽高；`orient` 为横向时不交换已存储的宽高，MediaBox 跟该页场景。场景图片成为图像 XObject。
- **书签**：只在段落样式或其 `basedOn` 链（最多 64 层）解析为内置 Heading 1 到 9 时生成。级别用已解析的 `outlineLvl`（0 到 8），否则用标题号减一。该段在快照里没有放置行、标题为空或页超出范围时不写。最多 512 条，标题截到 200 个字符。普通段落即使带 `outlineLvl` 也不成书签。
- **链接注释**：已放置行上的超链接，外部目标通过既有 `open_target`（`http`、`https`、`mailto`）时，按该行插入点的横向范围和行框写成 URI 动作。矩形来自已有分页，不再排一次。页范围切片会丢掉范围外的注释并重映射页号。`javascript` 等名单外方案，以及只有书签锚点的链接，不写动作；屏幕上的下划线仍由排版绘制。最多 512 个矩形。这不是带标签的结构，也没有把锚点解析成文档内跳转。
- **发布**：`publish_pdf` 与另存一样先写临时文件、sync、回读，再硬链接到新路径，不覆盖已有文件。文件必须以 `%PDF-` 开头并含 `%%EOF`，扩展名为 pdf，最大 256 MiB。打印到文件的份数（1 到 99）只记在收据上，文件里仍是一份 PDF。范围接受省略、`all`、`N` 或 `N-M`（从 1 起、含端点）；0、倒置和超出拒绝。审阅标记只接受省略或 `laid-out`；`markup` 与 `original` 拒绝，因为修订打印没有实现。
- **系统对话框**：`macos_print_dialog`、`windows_print_dialog` 与 `linux_print_dialog` 按 cfg 编译，返回未验证，不调用系统 API。菜单上的打印走无头打印到文件，不打开这些对话框。三端原生打印对话框都未验证。
- **结构检查**：`joyword-pdf` 自带与 krilla 不共享对象的读取器。它按 ISO 32000-1 读经典交叉引用表或交叉引用流、对象流，以及单层 FlateDecode（复用已钉扎的 flate2 1.1.10 rust_backend，没有升级）。偏移用整数保存。测试里手写的经典 PDF 与写出的文件都经该读取器核对页框、Producer、标题、书签、ToUnicode 和图像 XObject，并确认没有 CreationDate 与 ModDate。没有把 qpdf 放进产品依赖。
- **命令**：`file.export_pdf`（Cmd/Ctrl+Shift+E）与 `file.print`（Cmd/Ctrl+P）在文件菜单第 1 组，`modifies_document` 为假。缺目标路径返回待输入，窗口再询问。写出在后台进行。文档版本变了只提示较早版本已写出，不把文档标成已保存。复制世代变化时丢掉迟到结果。
- **自动验证**：`cargo test --offline --locked -p joyword-pdf -p joyword-shell` 通过，含 `export_pdf` 9 项与 `print_to_file` 5 项；`pdftotext -enc UTF-8` 在本环境抽出字母 A。应用注册表测试确认两条命令都有动作和快捷键。上述包与 `joyword-cli`、`joyword` 的 `cargo clippy --all-targets -- -D warnings` 通过。`scripts/check_docs.py` 与 `scripts/check_gates.py` 通过。没有启动图形界面。
- **未覆盖**：两个独立阅读器的人工检查；真实打印机；系统打印对话框；不可打印区域作为用户可见选择；结构标签；仅书签锚点的文档内跳转；审阅标记视图打印；份数在文件里重复页面。CPU 光栅与打印预览是既有路径，本切片不把它们说成新完成。没有新增外部包版本。Cargo.lock 只为 joyword-pdf 记下已有的 flate2 1.1.10 直接边。合并 origin/main 的页码域、分节、等宽分栏、超链接与安全粘贴，并让 PDF 页框跟随各节已存储的宽高之后，W09 harness 曾锚到 `4c54d2829263983a43f74afee54ee8bacd700f93`。再合并 origin/main 的项目符号、主题 tint／shade 与表格样式之后，锚点曾改到 `eae9e9b3dc5303ff5d2eceb5d759fbdb7da26fd2`。再并入下划线、sRGB 与视图选项之后，锚点改到合并提交 `b81407697fbcd96472352ea78458961f2847ddb5`（该提交的 `crates/` 同时含这些改动与 PDF 导出；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。



### Windows 待验审计与渲染缺陷复验（2026-10-19）

本轮从 `26f46c3f2582de88cb09012487def0744788f9c7` 检查全部 66 项任务，完整待验清单、命令、环境、自动与人工边界见 [验证审计](VALIDATION-20261019.md)，原始日志与机器可读结果见其中证据链接。全量基线 652 项无头测试、38 项应用适配器、53 项 Python 工具测试、15 项恢复实验、固定健壮性矩阵 81/81 通过；fmt、clippy 和工程门禁通过。固定矩阵先于本轮生产修改执行，不冒充覆盖引导 fuzz 或本轮修复后 harness 结果。

独立 PDF／光栅验证发现并修复两个问题：合成粗体的第二次文本绘制共用 ToUnicode，导致复制与提取重复标题，改为轮廓副本；CPU 一般路径光栅器会丢失亚像素细下划线，改为带覆盖率抗锯齿的专用矩形光栅器。新增单次可检索文字与五个亚像素位置回归，PDF／render 相关测试和最终产物复验见审计。没有改变分页／EMU 几何、升级依赖或引入字体资产。

20 页／725 段 release 无头性能冒烟：首次排版 14.271 ms，输入提交中位 0.011 ms，全文重排 13.414 ms，增量输入重排 8.492 ms（每次整形 1 段）。未采集真实呈现、电源与温控条件，不能作为 1.0 性能验收。

用户确认 GUI 时段并授权鼠标键盘后，Windows GUI 以隔离 test profile 完成 16 个步骤并保存 16 张截图：打开 DOCX-MIN-001；`Ctrl+S` 原生风险提示、默认另存副本和显式替换原件；创建时间与 `Zone.Identifier` 保留；真实系统中文 IME 候选、取消、提交和一次撤销；三次物理按键合并撤销；系统剪贴板独立撤销；第二次保存、保存后撤销和撤销后再保存。GUI 风险提示如实列出 ownership、ACL、扩展属性／其他命名流不可观测；ACL 的 owner 与 ACE 内容相同，但 raw SDDL 的 DACL 从 `D:` 变为 `D:AI`，不宣称精确标志保留。

T210 还把 80% GUI 页面按页宽归一后与 DOCX-MIN-001 的 CPU PNG 对照：同一单行正文，水平墨迹边界差不超过 2.23 px、垂直边界差不超过 6.32 px；这是截图边界下的视觉一致性检查，不是像素同一性。结合五样例逐字节确定性与 inspect 截断证据，T210 标记 `done`；当时任务计数更新为 11 done／18 in_progress／37 planned，随后 T211 完成，当前为 12 done／17 in_progress／37 planned。

未覆盖仍包括慢打开只读、保存中关闭 dirty 提示、真实崩溃、GUI PDF 阅读器和系统打印、精确 ACL 与命名流、非 NTFS／网络盘及设备平台。完成上述 GUI 证据后 Computer Use 原生管道中断，重置 node session 并重试一次仍返回 `native pipe is unavailable`。cargo-deny 未安装，本轮未验依赖公告；信创真机仍按用户决策非阻塞待补验。

### 银河麒麟 V10 SP1 真机会话（2026-10-19，x86_64）

在银河麒麟桌面操作系统 V10 SP1（x86_64，Intel Xeon Platinum 虚拟机、Cirrus GD 5446 虚拟 VGA、无 Vulkan 硬件、UKUI/X11 2560×1352）完成一轮真机验证，机器可读记录与截图见 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json) 与 `tests/fixtures/reference/kylin/`。工具链经 rsproxy 镜像安装 rustup 1.98.1（minimal + clippy + rustfmt，与 `rust-toolchain.toml` 一致），本机需补装 `g++` 与 `libxkbcommon-x11` 后 `cargo build -p joyword --locked`（不启用 dev-runtime-shaders）成功；窗口经机内 SwiftShader（`VK_ICD_FILENAMES=/usr/share/trae-cn-enterprise/vk_swiftshader_icd.json`）加软件 EGL 启动并完整渲染，mwv207 国产 GPU 桩库打开设备失败后正常回退。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings` 与各 crate 测试通过；`joyword-shell` 47 项中 2 项失败，均为本机环境特性并已定位（见下）。

- **T001／T005**：Linux 真机参考环境腿核对完成（工具链与可启动产物）；本机非国产 CPU+GPU，信创真机专项仍为非阻塞待补验。Fcitx 4 在运行，但 GPUI X11 窗口不接入 IME——拼音直通文档、无候选框；中文输入经系统剪贴板粘贴（xclip 所有者）可用并计入状态栏字数。系统字体清单含 Noto Sans/Serif CJK 与思源系。
- **发现一（KYSEC 扩展属性，高）**：KYSEC 为每个文件挂 `security.ksip`／`security.boxadm`，其中 `security.ksip` 用户态不可写回（EPERM，python 复现）。T110 的确认替换按契约拒绝：`ReplacementUnavailable[UnpreservableMetadata(ExtendedAttributes)]`，默认另存副本不丢数据，但「确认替换原件」在麒麟与同类 SELinux 系统上对普通文件必然不可用；宜把内核管理的 `security.*` 命名空间排除出可保留集合。对应测试 `confirmed_replacement_copies_a_user_extended_attribute` 在本机失败。
- **发现二（mtime 粒度，中）**：内核 5.4 的 mtime 按 jiffy 粒度落盘（实测相距 1 ms 的两次写 `st_mtime_ns` 相同），stat→open 身份守卫对同长度快速改写存在约一个 jiffy 的盲区，`identity_change_between_stat_and_open_is_not_a_baseline` 在本机失败；测试宜注入不同长度改写或跨 jiffy 等待，产品可补内容散列比对。
- **发现三（混合格式整画布拒绝，高，已修复）**：早期构建（`26f46c3`）上对段内单字加粗使整画布以 `Unsupported { feature: "mixed character formatting" }` 拒绝渲染（撤销可恢复，截图 `kylin-gui-mixed-format-canvas-error.png`）；同日 `da5f192` 改为按 run 跨度绘制后，在本机新构建复验通过（段内「合作**作**议」部分粗体正常渲染，`mixed_formatting` 7 项回归通过）。
- **发现四（项目符号按钮缺参，中，未修）**：功能区「项目符号」按钮在 GUI 上必报「输入未完成：num」——应用侧 `execute_input` 的 `LIST_APPLY` 分支只读 `num`、忽略 `bullet` 参数（shell 层 `host.rs` 已有 bullet 分支但应用未走），在 `da5f192` 之后仍复现，待修。
- **发现五（Linux GUI 命令可达性，高）**：X11 窗口不渲染菜单栏；Ctrl+Shift+E（导出 PDF）被 UKUI 全局快捷键分配给文本编辑器（XTEST 注入实测弹出编辑器窗口，按键到不了应用），Ctrl+Shift+S（另存副本）亦无反应；Ctrl+O 的未保存提示正常但「不保存」后文件打开对话框不出现（无新窗口、无诊断）；未绑定文档上 Ctrl+S 静默无反应。文件菜单命令与打开对话框在默认麒麟桌面不可用，`table.*` 命令亦无 GUI 入口（T201 人工验收受阻）。
- **WPS 交换（T103/T004）**：WPS Office 12.1.2.1129.AK.preload.sw（麒麟预装版）打开 JoyWord 导出的编辑副本正常，Ctrl+A 复制读回与包内正文去空白归一后逐字一致（`wps_readback.txt`）；其「输出为PDF」菜单渲染为黑块、Ctrl+P 无对话框——WPS PDF 通道在本机与 Mac 一样不可用，WPS 参考 PDF 仍缺。
- **T204 双阅读器**：`joyword-cli convert`（Noto Serif CJK face 2）对编辑副本出 1 页 PDF，poppler `pdftotext -enc UTF-8` 与 pdfminer.six 两个独立读取器逐行一致，结构核对内容流／ToUnicode／Producer=JoyWord（`joyword_e2e_cli.pdf`）。
- **T210 一致性（Linux 旁证）**：CLI `render` 的样例页 PNG 与窗口 100% 截图逐行同断行、同内容。
- **T104／T211**：对含未保存修改的窗口 `kill -9` 后，日志完好且文档正文关键词零命中；同 profile 重启日志记录 `recovery.restored kind=document count=1`，状态栏「会话文档」，正文为最后完整事务状态（`kylin-gui-recovery-restored.png`）。T211 的崩溃人工验收就此完成并标 `done`。
- **GUI 人工验收（部分通过）**：双击按字素选中、整段加粗、查找替换（状态栏「1 处匹配」）、格式刷取源—应用（「已应用格式」）、撤销组（一次 Ctrl+Z 撤整组打字）、布局选项卡横向／纵向与分页符翻页（「第 2 页，共 2 页」）、缩放（Ctrl+=／Ctrl+0）与中英界面切换均正常。T202 部分、T200 部分、T205 部分如上；T208 的百页人工验收被打开对话框缺陷阻塞。

本机未验证：真实打印机与系统打印对话框、断电场景、国产 CPU+GPU 硬件、Retina/多显示器 DPI、Word 参考环境复核。这些仍按各自任务记录为待补验。

### 统信 UOS Desktop 20 Professional 会话（2026-10-19，x86_64）

在统信 UOS Desktop 20 Professional（x86_64，内核 4.19.0-amd64-desktop，glibc 2.28，海光 C86 7390 四 vCPU 的 KVM 虚机，X11 2560×1440，无 Vulkan 硬件）完成另一条信创环境验证。机器可读记录、截图和原始日志见 [uos_report.json](../../tests/fixtures/reference/uos/uos_report.json) 与 `tests/fixtures/reference/uos/`。

- **工具链与构建**：rustup 安装固定 Rust 1.98.1（minimal + clippy + rustfmt），另装 g++ 8.3、cmake 与 libxcb1-dev、libxkbcommon-dev、libxkbcommon-x11-dev。UOS 仓库的 `libfontconfig1-dev` 依赖 freetype 2.9.1.2，与本机已装 2.9.1.4 冲突，直接安装会降级系统库；因此先用 crate 既有的 `RUST_FONTCONFIG_DLOPEN=1` 完成验证，再添加本地 `/usr/local/lib/pkgconfig/fontconfig.pc`（`Libs: -L/usr/local/lib -lfontconfig`）和 `/usr/local/lib/libfontconfig.so -> /usr/lib/x86_64-linux-gnu/libfontconfig.so.1`，使裸 `cargo build -p joyword --locked` 也不需环境变量即可成功。未新增仓库依赖。`cargo fmt --all -- --check`、全工作区 clippy `-D warnings`、`scripts/check_docs.py` 与 `scripts/check_gates.py` 通过。
- **自动测试**：默认成员 `cargo test --locked --no-fail-fast` 为 681 passed／2 failed／0 ignored；应用适配器 `cargo test -p joyword --locked` 为 38 passed。两项失败均与内核 4.19 的 jiffy 时间粒度有关：同长度快速改写（`identity_change_between_stat_and_open_is_not_a_baseline`）和硬链接别名同字节写入（`hard_link_alias_writes_are_detected_through_the_stamped_path`）在同一个时间片内不改变 mtime/ctime。测试失败不改判为通过；与麒麟 KYSEC 不同，`confirmed_replacement_copies_a_user_extended_attribute` 在本机通过。Python 工具测试 40 passed／2 failed／11 errors，失败集中在依赖 git ≥2.28 的 `--initial-branch` 与 partial clone 辅助流程，本机 git 为 2.20.1。
- **窗口与字体**：`JoyWord · Preview` 经机内 SwiftShader（`/opt/namiwork/vk_swiftshader_icd.json`）加软件 EGL 启动，中文界面、A4 页面、标尺和功能区完整渲染。裸 `cargo run -p joyword --locked` 现在也会自动回退，不需要手工导出 `VK_ICD_FILENAMES`。GUI 字体为 Noto Sans CJK Regular face 2（SHA-256 `b76b0433…41690a`），PDF 字体为 Noto Serif CJK Regular face 2（`5d9c31a0…96a7d`），系统字体字节未提交。
- **GUI 路径**：80%→90%→100% 缩放实测页宽 635→713→793 窗口像素，Ctrl+0 复位到 100%；ASCII 键入、系统剪贴板中文粘贴、撤销、状态栏字数与 dirty 状态均更新。格式标记、隐藏文字、域代码三个视图开关可连续切换，格式标记画出空格点和段落标记且不改变断行。`kill -9` 前后的恢复日志、哨兵正文检查和重启恢复通过（`recovery.restored kind=document count=1`）。
- **软件渲染回退**：首次裸启动时 GPUI 只枚举到 llvmpipe GL，`no compatible surface formats`，窗口打开失败；Vulkan loader 默认 ICD 也没有可用物理设备。应用现在在没有 `/dev/dri/renderD*` 且未显式设置 ICD 时，先查找 `JOYWORD_SOFTWARE_VULKAN_ICD`、已知 SwiftShader 路径，以及 `/usr/share/vulkan/icd.d`、`/opt`、`/usr/share` 下的 `*swiftshader*.json`／`*lvp_icd*.json`／`*lavapipe*.json`，然后以 `VK_ICD_FILENAMES`／`VK_DRIVER_FILES` 执行自身一次。也保留窗口打开失败后的同方式重试，以覆盖有 render node 但 Vulkan 不可用的机器。显式 ICD 优先，找不到软件 ICD 时保留原诊断。实测裸启动 2 秒内出窗，日志选中 SwiftShader Vulkan；新增 `a_software_vulkan_icd_is_found_by_name`。
- **IME 与文件对话框边界**：Fcitx 4.2.9.32.49 与搜狗输入法处于活动状态，但 GPUI X11 窗口不接入 IME；键入 `nihao` 直接以 ASCII 进入文档，无候选框。Ctrl+O、Ctrl+S、Ctrl+Shift+S 均未出现系统文件选择/保存对话框（本次会话未运行 xdg-desktop-portal 主服务），因此 GUI 打开、保存、导出与百页 GUI 验收在本机不可达，不能据此宣称这些路径通过。
- **PDF 双阅读器**：`joyword-cli convert` 用 Noto Serif CJK face 2 对 DOCX-SAMPLE-001 写出 1 页 PDF，Poppler `pdftotext -raw -enc UTF-8` 五段与 inspect 投影逐字一致；qpdfview 0.4.17 显示该页并在搜索框高亮「远山」；WPS PDF 12.1.2.1128 显示同一页并报告「精确查找 (1)」。系统打印对话框与真实打印机仍未验证。
- **发现与修复（栅格断言）**：切换隐藏文字时 debug 构建在 tiny-skia 0.11.4 `scan/hairline_aa.rs:124` 触发 `assertion failed: false`。页面场景中存在 1–2 px 宽、左边缘为分数像素的矩形；该库的覆盖率矩形光栅器在内部整像素宽度归零时断言。`joyword-render::rasterize` 现在把亚 2 px 宽的矩形吸附到整像素边界（至少保留 1 px），不改变 EMU 场景坐标，也保留亚像素下划线的可见性。新增 `narrow_fractional_rects_keep_their_pixels_without_panicking` 覆盖分数相位的竖向与横向矩形；修复后三个视图开关连续切换不再崩溃。
- **未验证**：国产 CPU+GPU 物理硬件、ARM64／LoongArch64 UOS、Fcitx 接入、系统文件对话框、系统打印、断电、Retina／多显示器。T005 仍为 `in_progress`，按用户 2026-10-18 决策不阻塞开发与发布。

### T110 Linux 元数据切片（2026-10-19）

针对银河麒麟 V10 SP1 记录的两项存储失败做了限定修复。T110 仍为 `in_progress`：精确 ACL、其他命名流、非 NTFS／网络盘和信创真机没有在这一刀验收。

- **`security.*` 不再挡住确认替换**。Linux 上 `security.` 前缀（SELinux、KYSEC 的 `security.ksip`／`security.boxadm`、`security.capability` 等）在 `observe` 里记到 `kernel_xattrs`，不读取、不回写、不进入 `UnpreservableMetadata(ExtendedAttributes)`。新 inode 由内核重新打标；用户态写回这些属性经常是 EPERM，把它们放进必须恢复集合会让普通文件无法替换。`user.*` 仍复制并回读核对，`user.security.note` 不算内核命名空间。macOS 没有这个内核命名空间，同名属性仍按用户属性处理。ACL 与来源标记的原规则不变。
- **stat→open 的内容复读**。`open_source` 在打开钩子之前按预算读一次，打开后再读两次并与样本逐字节比较，同时仍比较设备、inode、权限、链接数、mtime、ctime，以及平台能提供的 birth time。内核 5.4 的 jiffy 时钟上，同长度改写可以不改变 mtime／ctime；内容不一致时返回 `UnstableRead`，不发基线。测试用线程局部的粗时间戳把 mtime／ctime／birth 压成同一值，证明拒绝来自内容而不是时间戳碰巧前进。Windows 仍用卷序列号、文件 ID 与 100ns 时间；这条复读在各平台上执行，不改变「元数据未变且字节相同则接受」的结果。
- **相同字节的删除重建**。ext4 会立刻复用 inode 号，且同一 jiffy 内 ctime 与 birth 可以不变。`FS_IOC_GETVERSION` 的 generation 会变，但工作区 `unsafe_code = forbid`，已固定的 rustix 1.1.5 只通过 unsafe `ioctl` 提供该调用，这一刀没有取 generation。`delete_and_recreate_with_identical_bytes_is_an_inode_change` 与 `save_original_reruns_the_full_conflict_verification_itself` 改为等到 inode 或时间戳实际变化再断言冲突。硬链接别名改为写入同长度的不同字节，粗时钟下由内容比较报告 `ContentChanged`。
- **自动验证**：`cargo fmt -p joyword-shell`；`cargo clippy --locked -p joyword-shell --all-targets -- -D warnings`；`cargo test -p joyword-shell --locked`。本环境是 Linux 6.12 上的 ext4，时间戳在约一个 jiffy 内不前进，因此能复现同长度改写的盲区；不是银河麒麟真机。信创真机未复验。rebase 到 main `ce2ad4f5`（含 `39193936` 的亚 2px 取整与其后的 T103／T300）之后，W09 harness 重锚到 `8e3f4e29e90ca003ba09f5d3947007ec4e7d1089`（该提交的 `crates/` 含 Linux `security.*` 排除与来源字节复读；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### Linux GUI 入口修补（2026-10-19，对应麒麟发现四、五）

这一刀只修应用入口，不改编号引擎、PDF 写出或 T110 的元数据身份。T200／T106／T204 均不因此标 done。麒麟真机没有在本环境重开窗口。

- **项目符号（发现四）**：`gpui-pre-linux` 无关。功能区 `start.list_bullet` 经 `RibbonControl::bind` 送出 `list.apply` 且 `bullet=1`，shell `host.rs` 已按 bullet 调用 `apply_bullet`。应用 `execute_input` 的 `LIST_APPLY` 只读 `num`，缺 `num` 就报「输入未完成：num」，`bullet` 被丢掉。现与 shell 对齐：有 `bullet` 就 `apply_bullet`（同时有 `num` 时 bullet 优先），否则仍解析 `num`，两者都缺的失败信息仍是 `num`。`DocEditor` 增加 `apply_bullet`。证据：`ribbon_bullet_reaches_the_document_without_num`（功能区参数打到内存文档，段落得到级别 0 的编号，一次撤销恢复）。
- **打开对话框（发现五的一半）**：GPUI Linux 的 `prompt_for_paths`／`prompt_for_new_path`（`gpui-pre-linux` 0.3.9）把请求交给 XDG Desktop Portal 的 FileChooser，并先等窗口标识；门户缺失、UKUI 后端不画窗或请求不返回时，应用把错误收成「没选路径」，所以不保存之后没有新窗口、也没有诊断。未绑定文档的 Ctrl+S 走同一条另存门户，因此也静默。Linux 上打开、另存副本、导出 PDF 和打印到文件改为先在窗口里要路径（可粘贴），确定后走既有 `open_default`／`SaveCopyJob`／`queue_pdf`。打印路径仍以 `print: true` 排队，不改成普通导出。若 PATH 上有 `zenity`、`qarma` 或 `kdialog`（按此顺序），「浏览」再调它们；用户取消留在路径栏，程序缺失或 stderr 非空（例如无法打开显示）则换下一个，都不可用时仍可手输路径。macOS／Windows 仍走原来的 `prompt_for_paths`／`prompt_for_new_path`。证据：`typed_open_requires_a_file_and_save_requires_a_parent_directory`、`dialog_cancel_stops_and_a_display_error_tries_the_next_program`、`dialog_programs_prefer_zenity_and_build_save_argv`。没有在麒麟上点过浏览按钮。
- **菜单栏与 Ctrl+Shift+E（发现五的另一半）**：`LinuxPlatform::set_menus` 只把菜单存进内存，不绘制，所以 X11 窗口没有菜单栏。窗口在 Linux 增加一行「文件／编辑」：打开、保存原件、另存为新副本、导出 PDF，以及撤销、重做、复制、粘贴，都走 `Registry::invoke`。`file.export_pdf` 的主快捷键仍是 macOS `cmd-shift-e`、Windows 与 Linux `ctrl-shift-e`。Linux 另注册 `ctrl-e`（无 Shift，避开 UKUI 抢走的 Ctrl+Shift+E，也避开 X11 上经常送不到应用的 Ctrl+Shift+字母）。证据：`export_pdf_keeps_desktop_chords_and_adds_a_linux_alternate`、`linux_export_keeps_the_primary_chord_and_adds_ctrl_e`、`linux_bar_covers_open_save_and_export`。Ctrl+Shift+S 仍是另存的主快捷键；另存按钮和路径栏不依赖它。
- **Fcitx（kylin-8，未做）**：GPUI 的 X11 窗口不接入 XIM／text-input，拼音仍直通文档，没有候选框。这是 GPUI 平台缺口，不在本切片。中文输入仍可用系统剪贴板粘贴。T005／T701 的输入法条款未完成。
- **仍未覆盖**：麒麟真机上路径栏与 `ctrl-e` 的点击验收；`table.*` 仍没有单独的窗口入口（功能区插入选项卡已有表格按钮）；门户对话框在 GNOME 上是否仍应作为 Linux 默认，本切片没有改回，因为门户请求可以一直不返回。UOS 会话里的系统文件对话框同样未出现，路径栏尚未在统信窗口上点过。

rebase 到 main `5b07db6f7ec72392bd2b123a8235fa1dc0884f31` 之后，W09 harness 改锚到 `8690efd218d0c8be17c80104a021a93a78fca9dd`（该提交的 `crates/` 含 Linux 文件条与 `ctrl-e`，以及 T110 的 `security.*` 排除与来源字节复读；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `77f71712820da58ee7e13299ecb01d76d5c2768f`（T202 squash）之后，W09 harness 改锚到 `77ea6a90c91e229f979ad87eb0f4480fdb95fed1`（该提交的 `crates/` 含 Linux 文件条与 `ctrl-e`，以及 T202 的 widowControl、页面设置写回与页眉编辑；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `b39eb05ab7a63ada1bb4189bc1a07394592b090b`（T200 find fold/regex squash）之后，W09 harness 改锚到 `ae71fb713d39358b181c9e0d8d33308fdd44c3d3`（该提交的 `crates/` 含 Linux 文件条与 `ctrl-e`，以及 T200 的全角半角折叠与正则查找；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `1eec33e28e30dd426d055a501a10d487c704d367`（T201 cell vAlign、行 trHeight 与表格属性 squash）之后，W09 harness 改锚到 `e4a776a0ac78949d3d291467de886bbe23d8380b`（该提交的 `crates/` 含 Linux 文件条与 `ctrl-e`，以及 T201 的单元格 vAlign、行 trHeight 与表格属性；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。

### 麒麟缺陷修复会话的机器侧证据（2026-10-19，分支 verify/kylin-v10sp1-session-20261019）

同日真机会话的九项发现里，可由代码解决的部分先在分支 `verify/kylin-v10sp1-session-20261019` 上修复并逐项在银河麒麟 V10 SP1 本机复验；该分支的代码后来没有整体合入，等价修复以下述方式落进 main。本节补记机器侧验证证据与两边的映射，机器可读记录（含逐项 status、`fix_session` 与 `launch_requirement`）见 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json)。

- **KYSEC `security.*`（kylin-1）与 mtime 粒度（kylin-2）**：分支版在写回被拒时跳过内核管理的 `security.*`；`restore_xattrs` 跳过后，确认替换在本机走通、GUI 损失清单只剩创建时间（不再列扩展属性）。同族测试改为注入不同长度改写并在重写前跨一个时间戳粒度（50 ms），本机此前从未跑到的 `w07` 集成测试全部转绿。main 经 PR #44（`5b07db6`，T110 Linux 元数据切片）落地等价修复但设计不同：observe 侧把 `security.*` 记入 `kernel_xattrs` 不进恢复集合，产品侧 `open_source` 增加内容复读。分支验证的是分支变体，main 变体未在麒麟真机复验。
- **项目符号按钮（kylin-5）**：应用侧 `LIST_APPLY` 补 `bullet` 优先、回落 `num`，与 shell host 对齐；本机 GUI 实测按钮正确套用 U+2022 圆点列表（`kylin-gui-bullet-applied-after-fix.png`）。main 经 PR #46（`fe50e74`，Linux GUI 入口修补）以相同语义落地。
- **文件命令可达性（kylin-3／6）**：分支实现为五命令「文件」功能区页经门户对话框取路径（`kylin-gui-file-ribbon-tab.png`）；本机补装 `xdg-desktop-portal`＋gtk 后端后链路全通——经对话框打开 DOCX-MIN-002、另存副本落盘、导出 PDF 落盘且 poppler 与 pdfminer.six 双读回一致（`min002_export.pdf`）。提示测试中出现过一次未解释的应用退出（无 panic、无段错误，重启 `recovery.restored`），已如实记录在报告。main 经 PR #46 落地为窗口内「文件／编辑」条＋路径栏（不依赖门户），设计不同且未在麒麟真机点击复验；UKUI 劫持 Ctrl+Shift+E 属桌面设置，两边都以不依赖该快捷键的入口替代。
- **裸跑建窗失败（T002 错误路径）**：分支版把建窗错误原文加固定 Linux 软件渲染提示打到 stderr 并 `exit(1)`；本机复验裸跑打印真实错误后立即退出，带 SwiftShader ICD 启动窗口正常（`kylin-gui-window-with-swiftshader-icd.png`，首帧约 15–25 秒）。main 的 `4bf14f2` 以更强的方案取代：自动查找软件 ICD 并自我重启，无需手工环境变量；该自动路径在 UOS 验证，未在麒麟复测。机器启动要求（SwiftShader ICD 路径、门户依赖）记录在报告 `launch_requirement`。
- **未修项**：GPUI X11 不接 IME（kylin-8，上游缺口）、WPS PDF 通道（kylin-9，WPS 自身），与真机会话记录一致。

### T200 查找栏全半角与正则勾选（2026-10-19）

这一刀把上一查找切片已经接到命令上的 `width`／`regex` 放进应用查找栏。T200 保持 `in_progress`。没有改 `crates/`，因此不重锚 W09，也不重跑固定矩阵。没有启动图形界面。

- **勾选**：查找栏在「区分大小写」「全字匹配」之后增加两项。中文是「全半角」和「正则」，英文是 Match width 和 Regex。外壳语言包还没有 `find.width`／`find.regex`，缺键时用这两句回退；以后目录里有同名键就改用目录。按下时底色与既有开关相同，文字前标 `[x]`／`[ ]`。默认都关。
- **命令**：`find.next`、`find.previous`、`find.replace`、`find.replace_all` 以及查找框里的回车，都带上 `width` 与 `regex`。只有 `Bool(true)` 打开对应选项，所以两者为 `false` 时和以前不传这两个参数一样：不折叠全半角，查询按字面比较。功能区查找按钮仍由壳层只绑定查询、大小写和整词；应用在调用前若参数里没有 `width`／`regex`，就补上当前勾选，已经写明的值不覆盖。
- **自动验证（Linux x86_64，Rust 1.98.1）**：`cargo fmt -p joyword`；`cargo clippy --locked -p joyword --all-targets -- -D warnings`；`cargo test --locked -p joyword`。勾选全半角后半角 `ABC` 命中 `ＡＢＣ`，查找下一个、上一个和单次替换都走这条路径；不勾选时半角查不到全角，全角字面量仍能命中。勾选正则后 `\d+` 单次替换只改第一处数字，全部替换改全部，一次撤销恢复；不勾选时 `\d+` 是字面量，正文不变。模式 `(` 经既有 `FindError::InvalidPattern` 变成 `Rejected("query is not a valid pattern")`，正文不变。证据：`find_bar_checkboxes_set_width_and_regex_on_every_find_command`、`find_bar_width_hits_fullwidth_and_regex_rejects_invalid_patterns`。这两项检查放在 `apps/joyword/src/tests.rs`（`#[cfg(test)]` 路径模块）：no-panic 门禁扫描 `main.rs` 全文，包含测试模块，该文件名则被跳过。
- **未覆盖**：查找栏勾选的图形界面点选验收；与 Word 查找选项逐项对照；正则替换的捕获组引用；信创真机。这些不把 T200 标成 `done`。

### T300 移动配对切片（2026-10-19）

第三切片加强成对 `w:moveFrom`／`w:moveTo`（ECMA-376 §17.13.5.21–§17.13.5.26 的 `w:name`）。T300 仍是 `in_progress`。没有做 T301 的格式或表格结构修订，也没有做页眉／页脚修订装饰和 Word／WPS 人工交换。没有启动图形界面。

- **两端一起处理**。接受或拒绝某一端的修订 id 时，按 `w:name` 找到完整的四枚范围标记，同时处理来源和目标。接受删除来源文字并清掉目标上的移动标记；拒绝清掉来源标记并删除目标文字。单次处理不碰标记范围内的普通文字、插入或删除，所以旁边的插入修订还在。全部接受之后正文等于最终投影（来源隐藏、目标与插入留下、删除去掉），全部拒绝之后正文等于原始投影。撤销恢复两端文字、修订 id 和四枚标记。
- **不猜测切开的 run**。移动 run 只有一部分落在自己的标记里，或一侧里出现另一种移动，返回 `RevisionConflict`，文档不变。缺标记或不交叉检查失败时仍是 `IncompleteMove`／`CrossedMove`，同样不变。
- **导出不丢掉这一对**。只把 `w:trackRevisions` 从开改到关时，计划只替换设置部件，正文 XML 不动，四枚标记和两端种类都还在。在移动段落后面追加文字再导出，重导入仍是同一对 `w:name`。按接受结果删掉来源、清掉目标标记并移除范围标记后，导出的正文只有目标文字，没有半对标记，文档仍可编辑。
- **审阅命令**。`review.accept` 在光标落在来源端时接受整对，撤销后 `review.reject_all` 回到来源、`review.accept_all` 回到目标。三种视图命令不改正文。`SetTrackRevisions` 仍是独立事务，会封口旁边的打字组；封口后一次撤销先恢复跟踪开关，再一次撤销整组打字，移动标记还在，之后全部接受仍等于最终投影。
- **Markup**。移动来源按删除画删除线，移动目标按插入画下划线和作者色，并在共享页面场景的左页边画作者色条。最终投影只留下目标，原始投影只留下来源，这两种视图不画页边条。页眉和页脚故事仍只按投影省略隐藏文字，不画这条页边条。
- **自动验证**：`cargo fmt -p joyword-engine -p joyword-docx -p joyword-layout -p joyword-shell`；`cargo clippy --locked -p joyword-engine -p joyword-docx -p joyword-layout -p joyword-shell --all-targets -- -D warnings`；`cargo test -p joyword-engine --locked`；`cargo test -p joyword-docx --locked --test revisions`；`cargo test -p joyword-layout --locked --test document -- move_markup`；`cargo test -p joyword-shell --locked --test review`。证据：`accept_all_and_reject_all_restore_the_move_projections`、`accepting_either_move_end_resolves_the_pair_and_leaves_other_revisions`、`a_move_run_that_crosses_its_marker_is_refused`、`incomplete_and_crossed_moves_leave_the_document_unchanged`、`switching_tracking_seals_a_typing_group_beside_a_move`、`a_settings_patch_keeps_the_move_pair`、`rewriting_a_move_paragraph_keeps_both_ends`、`an_accepted_move_exports_without_leaving_half_the_pair`、`move_markup_paints_both_ends_and_the_projections_do_not`、`review_commands_accept_a_paired_move_and_a_settings_patch_keeps_it`。既有 `user_tracking_accept_and_reject_undo_and_a_mode_switch_seals_typing`、`review_commands_use_the_registry_and_the_author_preference` 与 `text_revisions_round_trip_and_tracking_is_a_settings_patch` 仍通过。
- **W09**：本切片改了 `crates/`。rebase 到 main `30a76d8583e2ca130b3a0f8ed2b9a6b3f51aab88`（T202 squash）之后，harness 改锚到 `d6588d052ff8fb898127c437f3558842a03d3770`（该提交的 `crates/` 含成对移动的接受／拒绝，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。先前锚点 `9ca3fead40ba70ba9707150e38bf143f89f0db0b` 上的固定矩阵为 81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。固定矩阵尚未在新锚点上重跑。这仍不是覆盖引导 fuzz。独立锁无新包。再 rebase 到 main `26fd7ce2a87440395dddf94cb19c2569c6186669`（T200 find-bar squash，未改 `crates/`）之后，W09 harness 改锚到 `e3dca7a9d7ecf45c5ec9e108a3dc2f153628a31f`（该提交的 `crates/` 含成对移动的接受／拒绝，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。
- **未覆盖**：页眉／页脚故事上的修订页边条（绘制在 `pages.rs`，本切片不改该文件，也不写回页眉页脚部件 XML）；表格单元格里的移动对；格式与表格结构修订；Word／WPS 人工交换；图形界面与信创真机。

### T200 正则替换捕获组（2026-10-19）

这一刀让正则替换展开捕获组引用。T200 保持 `in_progress`。没有启动图形界面。没有新的外部包。`regex` 仍是锁文件里的 1.13.1。

- **展开**。只有正则命中（`FindHit::captures` 为 `Some`）才展开。`$$` 变成一个 `$`。`$` 后面连续的 ASCII 数字是一个组号：`$0` 是整段命中，`$1` 到 `$n` 是对应组。`$10` 是第 10 组，不是第 1 组再加一个 `0`。组号超出模式拥有的组（含第 0 组），或数字放不进 `usize`，则 `$` 和这些数字原样留下，所以 `(a+)(b+)` 的 `$9` 仍是 `$9`，整次替换不因此拒绝。没参加匹配的可选组换成空串。`$` 后面既不是 `$` 也不是数字时，这个 `$` 原样留下。组里的文字直接插入，不再扫描。这是文档化假定，不是 Word 观察，也不是 Word 通配符里的 `\1`。
- **原文**。匹配用的干串仍做宽度折叠，插入的是段落里的原标量。大小写不敏感时 `$1` 保留原文大小写；打开全半角折叠时 `$1` 仍是全角，不是折叠后的半角。
- **事务**。展开后的文字仍不得超过 `MAX_REPLACEMENT_SCALARS`，并且必须是普通文字；超限或非法字符拒绝整次计划，不部分写入。命中仍从后往前组成 `DeleteRange` 与 `InsertText`，放进一次事务。锁定、保留和重叠的命中跳过。一次撤销恢复全部替换。非正则替换不读捕获，`$1` 与 `$$` 都是字面量。
- **外壳**。`find.replace` 与 `find.replace_all` 仍走既有参数。单次替换之后的选区按实际插入的标量长度，不再按模板长度，因此 `$$` 选中那一个 `$`。命令表没有新参数。
- **自动验证（Linux x86_64，Rust 1.98.1）**：`cargo fmt -p joyword-engine -p joyword-shell`；`cargo clippy --locked -p joyword-engine -p joyword-shell --all-targets -- -D warnings`；`cargo test --locked -p joyword-engine`；`cargo test --locked -p joyword-shell --test find_paint`。证据：`regex_replace_expands_groups_and_leaves_unknown_dollars`、`regex_replace_keeps_original_case_and_width`、`regex_replace_skips_locked_and_preserved_and_undoes_once`、`regex_capture_replacement_goes_through_the_command_registry`。行为登记 `FIND-REPLACE-CAPTURE`，来源是假设，没有 Word 观察。
- **未覆盖**：查找栏上点选正则后的人工验收；与 Word 通配符 `\1` 的对照；命名捕获和 `${1}` 花括号写法；信创真机。这些不把 T200 标成 `done`。
- **W09**：本切片改了 `crates/`。harness 改锚到 `59364d1355c4a41fdb8622c96efde1855ffecf20`（该提交的 `crates/` 含正则替换捕获组展开；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T201 首列、末列与纵向条纹（2026-10-19）

这一刀让表格样式的首列、末列和纵向条纹参与排版。T201 保持 `in_progress`。列宽拖拽和合同表格的人工验收不做。没有启动图形界面。没有新增外部包。没有改 OOXML 导入。

- **层叠**：依据 [ECMA-376 第五版 Part 1](https://ecma-international.org/publications-and-standards/standards/ecma-376/) §17.7.6。整表格式仍从 `basedOn` 根走到叶。条件格式接着按这个顺序叠，后一层盖过同一属性：纵向条纹、横向条纹、首行、末行、首列、末列。四角单元格（`nwCell`／`neCell`／`swCell`／`seCell`）不在本切片。粗体仍是指定值，不是 §17.7.3 的 toggle。直接单元格边框、直接底纹和直接 run 粗体由排版在样式之后应用，优先。MS-OE376 记载 Word 把横向条纹放在纵向条纹之前，并把首末行放在首末列之前。那是实现偏差，本切片不采用。
- **条纹**：纵向一条一列，区域内下标 0 是 band1。`column` 是单元格的网格起点，已经加上 `gridBefore`，不按 `gridSpan` 再拆。首列外观打开时第 0 列不进纵向条纹；末列外观打开时最后一列不进纵向条纹。横向条纹仍按首行、末行排除。`w:noVBand` 关闭纵向条纹。省略的 `w:tblLook` 继续用既有应用默认：纵向条纹关，两列关。这仍是假定，不是 Word 观察。
- **绘制**：粗体、底纹和 `nil`／`single` 边框走原来的单元格路径。首列、末列和两条纵向条纹都能画出这三项。有表时增量排版仍是整篇重排，页与全量一致。
- **导入**：`w:tblStylePr` 的 `firstCol`、`lastCol`、`band1Vert`、`band2Vert` 这一刀不扩解析。样式部件里出现这些类型时，样式仍按原来记为不完整并保留原字节，类型化的 `table_conditionals` 里没有这四条，排版也就画不到。模型里已经放进 `TableCondition` 的才会画。`tblLook` 的首列、末列和纵向条纹位本来就会存，改写时写回 `1`／`0`。
- **未覆盖**：从样式部件读入这四条条件、四角单元格、主题色底纹、列宽拖拽、合同表格人工验收。没有启动图形界面。没有 Word 观察。
- **自动验证**：`cargo fmt -p joyword-model -p joyword-layout`；`cargo clippy --locked -p joyword-model -p joyword-layout --all-targets -- -D warnings`；`cargo test --locked -p joyword-model -p joyword-layout`。证据：`table_styles_layer_vertical_bands_under_columns_and_rows`、`table_styles_paint_first_and_last_columns_and_vertical_bands`。既有 `table_styles_paint_bands_and_direct_formatting_wins` 与 `table_styles_layer_bands_under_first_and_last_rows` 仍通过。
- **W09**：本切片改了 `crates/`。rebase 到 main `5704bf6226cde228d37e21034c9a8d84b51eb5f7`（T200 捕获组 squash）之后，harness 重锚到 `ce1ef0c3f79be63992f974d8aedd8c5161e9bcfb`（该提交的 `crates/` 含首列、末列与纵向条纹的排版，以及 T200 的正则替换捕获组；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

### T300 页眉页脚修订装饰（2026-10-19）

这一刀给页眉和页脚故事补上与正文相同的修订装饰。T300 仍是 `in_progress`。没有做 T301 的格式或表格结构修订，也没有做 Word／WPS 人工交换。没有启动图形界面。没有改 `pages.rs`（不等宽分栏仍由并行任务占用）。

- **绘制**。`pages` 先把页眉页脚拟合进共享 PageScene。最终带标记时，本切片再按同一可见字符串整形，找到已经放下的字形 run，换成作者色分段，并补上下划线、删除线和左页边作者色条。插入与移动目标走下划线，删除与移动来源走删除线。色板仍是按作者名散列的 8 色，不是 Word 的修订色。最终／原始投影继续由页眉页脚拟合在整形前去掉隐藏文字，本切片不为这两种视图加装饰。
- **接受／拒绝**。页眉故事里的文字修订本来就走全部故事的同一事务。全部接受后页眉等于最终文字，全部拒绝后等于原始文字；`undo_depth` 为 1，一次撤销恢复文字和修订 id。没有改引擎。
- **不画的段落**。页眉或页脚段落里如果还有图片，仍由图片拟合摆放行框，好让后面的文字行对得上；这类段落自己的修订不另画。图片拟合本来也不做修订投影。
- **自动验证（Linux x86_64，Rust 1.98.1）**：`cargo fmt -p joyword-layout`；`cargo clippy --locked -p joyword-layout --all-targets -- -D warnings`；`cargo test -p joyword-layout --locked`。证据：`header_and_footer_markup_paints_and_projections_omit_it`、`accepting_or_rejecting_a_header_revision_is_one_undo`。既有 `markup_paints_a_margin_bar_beside_a_changed_line`、`move_markup_paints_both_ends_and_the_projections_do_not`、`revision_views_change_pagination_text_and_match_the_incremental_cache` 仍通过。
- **W09**：本切片改了 `crates/`。rebase 到 main `5704bf6226cde228d37e21034c9a8d84b51eb5f7`（T200 捕获组 squash）之后，harness 改锚到 `23fb2faf872b5bd5a3b103e2b33867a9b033068f`（该提交的 `crates/` 含页眉页脚修订装饰，以及 T200 的正则替换捕获组；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。该树上固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `cd087577a67cf5efb9164de15cde166ba7da028e`（T201 首列、末列与纵向条纹 squash）之后，harness 改锚到 `b59c8d4c8f429ff2fa6e5e26290d368e107f76e9`（该提交的 `crates/` 含页眉页脚修订装饰，以及 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。
- **未覆盖**：带图片的页眉／页脚段落上的修订装饰；表格单元格里的移动对；格式与表格结构修订；Word／WPS 人工交换；图形界面与信创真机。这些不把 T300 标成 `done`。

### T202 不等宽分栏（2026-10-19）

这一刀把 `equalWidth` 为假、每个 `w:col` 都有正的 dxa 宽度、且没有分隔线的不等宽栏放进模型和排版。T202 保持 `in_progress`。栏分隔线、`nextColumn`、章节页码和图形界面验收仍未做。

- **导入**：依据 ECMA-376 §17.6.4 `w:cols` 与 §17.6.3 `w:col`。`equalWidth` 为 `0` 或 `false`，`sep` 不为真，子元素全是 `w:col`，个数 2..=45，每个 `w:w` 为正的 twip。`w:space` 省略记为 0，写出时必须非负。父级 `num` 与父级 `space` 描述等宽栏，合法的 `num` 在不等宽路径上忽略。最后一栏的 `space` 留在模型里，不作为整行栏之后的空隙。`equalWidth` 不为假且没有 `w:col` 时仍是原来的 `EqualColumns`。`sep` 为真、缺 `w:w`、只有一栏、空的 `w:cols`、非 `col` 子元素或未知属性使该节不完整并保留，导出仍含原来的 `w:cols`。
- **排版**：各栏宽度和栏后间距从 twip 换成 EMU。宽度加间距超过版心，或宽度不是正数，返回 `columns do not fit the page`。栏的横坐标从版心左缘累加前面各栏的宽度和间距。竖向放不下时先换栏再换页。`w:br w:type="column"` 推进一栏；最后一栏之后换页，新页从第一栏开始。有分栏符时，符后的文字按那一栏的宽度拟合，窄栏和宽栏的折行因此不同。没有分栏符时，行宽取最窄栏。不等宽的栏规格进入段落缓存键。多栏节里的表仍以 `table in newspaper columns` 拒绝。
- **PDF**：屏幕与 PDF 共用同一次 `layout_document` 的 PageScene。内容流里的文本矩阵与场景字形原点不超过 0.01 pt。测试用已钉扎的 flate2 1.1.10 解压内容流，没有新的外部包版本。本切片不改写 `w:cols`；未改的包仍复制原字节。
- **自动验证（Linux x86_64，Rust 1.98.1）**：`cargo fmt --all -- --check`；`cargo clippy --locked -p joyword-model -p joyword-docx -p joyword-layout -p joyword-pdf --all-targets -- -D warnings`；`cargo test --locked -p joyword-layout --test pages -- unequal_columns`；`cargo test --locked -p joyword-docx --test import -- unequal_columns`；`cargo test --locked -p joyword-pdf --test pages -- unequal_columns`。证据：`unequal_columns_place_glyphs_at_different_x_and_a_break_advances`、`a_table_in_unequal_columns_is_refused`、`unequal_columns_import_and_a_separator_or_missing_width_stays_preserved`、`unequal_columns_share_one_page_scene_with_pdf`。
- **W09**：本切片改了 `crates/`。rebase 到 main `5704bf6226cde228d37e21034c9a8d84b51eb5f7`（T200 捕获组 squash）之后，harness 重锚到 `675c9b82e3ee16dc225166d94953af1f7331906a`（该提交的 `crates/` 含不等宽栏的导入与排版，以及 T200 的正则替换捕获组；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。该树上固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `cd087577a67cf5efb9164de15cde166ba7da028e`（T201 首列、末列与纵向条纹 squash）之后，harness 改锚到 `3ffb58dbf7e8a8987068ff61b425901dd749a42a`（该提交的 `crates/` 含不等宽栏的导入与排版，以及 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `a6fe0cf6cacd360f1e000aa26ee705a0804e5eb8`（T300 页眉页脚修订装饰 squash）之后，harness 改锚到 `89e32568cb93d2c74f03fe72df5b73b89a3e8b79`（该提交的 `crates/` 含不等宽栏的导入与排版，以及 T300 的页眉页脚修订装饰和 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。
- **未覆盖**：栏分隔线 `sep`、`nextColumn` 分节、章节页码、图形界面验收、Word 对照。没有启动图形界面。

### 2026-10-20 文档与验收核对

这一节只核对文档和 `main` 上已经合并的行为，不改任务状态，也不改 `crates/`。没有任何 M2 或 M3 任务同时具备自动验证和人工验收，因此没有一项改为 `done`。进度计数仍是 12 done／17 in_progress／37 planned／0 blocked。

1. **过时的「未做」**。TASKS 进度总览和各任务证据后追加了同日说明：T201 一层嵌套表与首列、末列、纵向条纹排版，T202 的 widowControl、页眉编辑、页面设置写回、页眉页脚部件写回和不等宽分栏，T203 的页面相对 wrapSquare，T300 的页边标记、审阅命令、单元格文字修订、成对移动和页眉页脚标记，都已有自动测试。T200 查找折叠切片里「`$1` 不展开」只描述该切片。历史句子没有改写。上一条「主要未完成项」里仍把嵌套表、条纹、widowControl、不等宽栏、页眉编辑和页面设置写回列成未完成，以本段和进度速览下的同日核对为准。
2. **验收清单**。[acceptance](acceptance/README.md) 把 T103、T200、T201、T202、T203、T204、T205、T300 的自动验证、人工验收、产物和错误路径拆成原子准则。状态只用清单图例。证据里的测试名都在 `crates/` 或 `apps/`。覆盖引导 fuzz 写成 pending, separate PR，没有链接。人工验收、Word／WPS 对照、真实打印和信创真机都不在本环境关闭。
3. **检查器**。`scripts/check_acceptance.py` 只使用标准库，并复用 `scripts/check_docs.py` 的测试名收集。它要求图例状态、证据列里的测试函数真实存在、本地链接指向已有文件，以及每份清单引用该任务的自动验证和人工验收原文。单元测试在 `scripts/tests/test_check_acceptance.py`。没有改 `scripts/check_docs.py`，也没有改 CI 工作流。
4. **功能指南**。[guide](guide/README.md) 说明查找替换、表格、页面与页眉页脚、图片绕排、修订，以及 `joyword-cli` 的 inspect、text、render、convert。每页分开写使用者和开发者，并标出限制和「JoyWord 假定，没有 Word 观察」。
5. **入口**。[docs/joyword/README.md](README.md) 的阅读顺序加上验收清单和功能指南。仓库根 README 的文档列表只加了一行链接。

并行切片不在本分支：S1 黄金场景、S2 T103 覆盖引导 fuzz、S3 功能夹具、S5 场景演练。它们在各自的 PR 里；收口时再链接，本段不指向尚未合并的文件。

### 2026-10-20 验证波次收口

五个验证切片已全部 squash 合入 main，顺序与提交如下。上一段「并行切片不在本分支」写于 S4 合并之前，以本节为准。

| 切片 | PR | main 提交 | 证据 |
| --- | --- | --- | --- |
| S4 验收清单与功能指南 | [#61](https://github.com/willmove/joyoffice/pull/61) | `1bae1231` | [验收清单](acceptance/README.md)、[功能指南](guide/README.md) |
| S2 T103 覆盖引导 fuzz | [#62](https://github.com/willmove/joyoffice/pull/62) | `8a12e6dd` | [T103 覆盖引导 fuzz](verification/t103-coverage-fuzz.md) |
| S5 场景无界面排练 | [#63](https://github.com/willmove/joyoffice/pull/63) | `e615d308` | [场景排练](verification/scenario-rehearsals.md) |
| S1 页面场景金样 | [#64](https://github.com/willmove/joyoffice/pull/64) | `0c80d8bb` | [页面场景金样](verification/golden-scenes.md) |
| S3 特性样例 | [#65](https://github.com/willmove/joyoffice/pull/65) | `f0ccc659` | [特性样例](verification/feature-fixtures.md) |

- **状态**。这些都是自动证据，没有人工验收、Word／WPS 对照或信创真机。没有任务改状态，计数仍是 12 done／17 in_progress／37 planned／0 blocked。TASKS 的进度总览和 T103、T200–T204、T209、T300 各追加了一条收口说明；验收清单补了排练、金样和样例的行，T103 的覆盖引导 fuzz 一行从 not-started 改为 partial。
- **收口时的已知缺口**。下列缺口由验证文档报告，交给分支 `fix/verify-wave-gaps` 处理。该分支已作为 [#67](https://github.com/willmove/joyoffice/pull/67) 合入 main `caf84147`，各条现状写在括号里，详见下一节。
  1. T201：样式部件 `w:tblStylePr` 的 firstCol、lastCol、band1Vert 没有读入模型，页面上没有对应填充。期望记录在被忽略的测试 `vertical_column_bands_are_imported_from_table_style_pr`（[页面场景金样](verification/golden-scenes.md)）。（已修复。）
  2. T200：CPU 光栅要求字形横坐标等于此前前进量之和，编号加制表位的行因此以 `InvalidGeometry` 被拒绝。S1 的光栅金样是 refused，S3 样例 DOCX-FEAT-T200-NUM 的 `render` 也被拒绝（[特性样例](verification/feature-fixtures.md)）。（已修复。）
  3. T300：`text` 投影的元数据声明排除删除和移出，标准输出仍含这些文字；S3 的黄金文件锁定的是当前输出。（已修复。）
  4. T203：图片资源已经导入，四条图片关系仍另报 `not-imported`。（不在 #67 范围，已由 [#69](https://github.com/willmove/joyoffice/pull/69) 修复，见「图片关系 not-imported」一节。同一样例里 `wrapTight` 锁定段落、编辑导出返回 `Restricted` 的问题由 #67 处理，见下一节。）
  5. T202：对已有正文段落执行 `page.number` 后，导出计划以 `paragraph formatting, source or inline objects changed` 拒绝，正文新页码写不回文件（[场景排练](verification/scenario-rehearsals.md)）。（已修复，只限无格式域。）
- **合并组合复跑**。在 main `f0ccc65944e6b961f3dcad935d5d43f5b6484c1c` 上按 `.github/workflows/ci.yml` 逐步本机复跑一次（Ubuntu 24.04 x86_64，Rust 1.98.1，apt 安装 CI 同名系统库、fonts-noto-cjk 与 poppler-utils，cargo-deny 0.20.2）。全部步骤退出码为 0，工作区复跑后仍干净，金样没有被改写。五个合并组合在一起没有引入失败。
  - build 作业：`check_docs.py`、`check_gates.py` 与反例、`scripts/tests` 单测、`cargo fmt --all -- --check`、`cargo test --locked`（781 通过、0 失败、1 忽略，忽略项即上面第 1 条）、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --locked -p joyword`（52 通过）、恢复日志实验、DOCX 健壮性实验（固定矩阵 81／81，43 ok、38 rejected，`matrix_complete` 为真，地址空间限制 `enforced=true`；种子 20261019 的行为特征 fuzz 执行 200、ok 46、rejected 154、异常 0）、`cargo build -p joyword --locked`。本机装了 Noto CJK，所以 `feature_fixture_render_and_convert_follow_the_subset` 的视觉部分实际执行了；CI 的 build 作业没有该字体，这部分会直接返回。
  - unified-pdf 作业：强制真实字体的 layout／pdf／render 测试 226 项通过；统一场景、文档流、CPU 光栅对照、断行对照（改动断行被检出）与 12 个特性场景的 pdftotext 回读都通过。
  - dependencies 与 provenance：`cargo deny --locked --all-features check` 四项 ok；gpui-pre 来源 16 个相同、3 个已审偏差、0 失败。
  - cross-linux：aarch64 与 loongarch64 在 QEMU 用户模式下各 781 通过、0 失败、1 忽略。这只证明代码生成和 ABI，不是信创真机。
  - 夜间工作流 `fuzz-nightly.yml` 也复跑一次：覆盖反馈 `llvm-source-regions`，种子 20261020、60 次变异，ok 10、rejected 50、异常 0，`crates/` 区域 3594／44812，`coverage_merge_matches_scheduler` 为真，与 [T103 覆盖引导 fuzz](verification/t103-coverage-fuzz.md) 的数字一致。
  - 本机没有 macOS、Windows 与原生 ARM64。GitHub Actions 上 main 同一提交的 run 39338072041 九个作业全部 success，包括 macos-15、windows-2022 和 ubuntu-24.04-arm 的 build。
- **未覆盖**：`scripts/check_acceptance.py` 仍不在 CI 里，本次只在本机运行并通过。图形界面、Word／WPS、真实打印和信创真机都没有在本次执行。

### 2026-10-20 验证波次缺口修复（#67）

[#67](https://github.com/willmove/joyoffice/pull/67)（分支 `fix/verify-wave-gaps`）squash 合入 main `caf84147`。根因、修复与回归测试见 [验证波次缺口修复](verification/verify-wave-fixes.md)；W09 harness 在该 PR 里重锚到 `7da6e738e1acb011c90d776007c49ccc43589f1f`。本节只更新文档，不改 `crates/` 与 `apps/`。

| 缺口 | 现状 | 限制 |
| --- | --- | --- |
| T201 表格样式 firstCol、lastCol、band1Vert 导入时丢失 | 已修复：四条列与纵向条纹条件读入模型；样式部件仍原样写回 | 无 |
| T200 编号标签后接制表位或点线引导符时光栅返回 `InvalidGeometry` | 已修复：正文从标签后的制表位开始，光栅允许笔位向前跳过间隙，仍拒绝回退 | 无 |
| T202 已有正文段落 `page.number` 后导出拒绝 | 已修复：写成 `w:fldSimple` | 只限无行内对象段落里的无格式 PAGE／NUMPAGES／SECTIONPAGES；带格式域、超链接里的域、原有行内对象的段落仍拒绝，复杂域仍不支持 |
| T203 `wrapTight` 让排版拒绝、编辑导出返回 `Restricted` | 已修复：页面相对的 `wrapTight`／`wrapThrough` 按锚点范围近似为方形绕排，原绕排 XML 照原样写回 | 近似，不沿 `wp:wrapPolygon`，不是真正的轮廓绕排；wrapTopAndBottom、衬于文字下方、非页面相对位置仍保留并锁定 |
| T300 `text` 投影含删除与移出文字 | 已修复：投影跳过最终视图里不可见的 run | 无 |

- **随后修复**：T203 四条图片关系另报 `not-imported` 已由 [#69](https://github.com/willmove/joyoffice/pull/69) 修复。本段写于 #67 合入时，当时诊断仍在。
- **状态**：只有自动证据。没有任务改状态，计数仍是 12 done／17 in_progress／37 planned／0 blocked。M2／M3 任务仍需图形界面、Word／WPS、真实打印和信创真机的人工验收。
- **同步的文档**：TASKS 的收口总览和 T200–T203、T300 条目；验收清单 README 与 T200–T203、T300；功能指南的图片与表格页、页面页的页码限制；行为登记 `IMAGE-ANCHOR-PAGE` 的期望，并新增 `IMAGE-WRAP-TIGHT-APPROX`；样例清单 DOCX-FEAT-T203-IMAGES 的期望与 [tests/fixtures/README.md](../../tests/fixtures/README.md)；[verification/README.md](verification/README.md) 新建索引。
- **CI**：`scripts/check_acceptance.py` 加入 dependencies 作业，作为第一步运行（标准库、无网络，本机约 0.1 秒）。上一节「仍不在 CI 里」以本节为准。

### 2026-10-20 图片关系 not-imported（#69）

[#69](https://github.com/willmove/joyoffice/pull/69) 修复 T203 样例里四条已导入图片关系仍报 `not-imported` 的误报。任务状态不改，T203 仍是 `in_progress`。没有图形界面操作，没有 Word／WPS 对照。

- **根因**：主文档关系在读正文之前遍历。不是样式、主题、设置、编号、页眉、页脚或 VBA 工程的内部关系，当时立刻记为 `NotImported`。图片关系（ECMA-376 Part 1 §20.4.2.7 的 `a:blip`／`v:imagedata`）落在这一支。随后 `import_picture` 已经把目标字节写入资源库，但没有把该关系记为已消费。
- **修复**：图片关系推迟到正文读完再判断。目标已经写入资源库的关系不再报告。内部图片关系如果没有写入资源（包括被拒绝的图片），以及未引用的自定义部件，仍然报告。缺失部件仍是 `MissingPart`。外部目标仍只走外部目标诊断。
- **回归**：`an_imported_image_relationship_is_consumed_and_unused_parts_stay_reported`、`an_inline_drawing_imports_the_raster_and_round_trips_beside_a_text_edit`、`a_remote_link_is_not_fetched_and_an_oversize_header_is_kept`（超宽内部图片仍报 `not-imported`）、`feature_fixtures_import_round_trip_and_keep_unknown_parts`。`DOCX-FEAT-T203-IMAGES` 的 inspect 黄金文件把 `diagnostics.reported` 从 4 改为 0，LF 行尾；`partial` 仍为真，因为 `omitted_objects` 为 4。
- **W09**：本切片改了 `crates/`。harness 重锚到 `8268f62b7e3606db57baa6987914fc992bb5f1d3`（该提交的 `crates/` 含上述消费记账；随后只改 experiments 与文档，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。
- **未覆盖**：真正的轮廓绕排、wrapTopAndBottom、衬于文字下方、非页面相对位置、Word／WPS／浏览器粘贴。没有把 T203 标成 done。

### 2026-10-21 W09 改为绑定检出的 `crates/` 树

main 的 build 作业自 `c198425` 起在四个平台的「DOCX robustness experiment」一步失败，只改文档的 [#70](https://github.com/willmove/joyoffice/pull/70) 也被同一步阻塞。任务状态不改。

- **根因**：W09 的 build.rs、driver.py 与 lib.rs 把生产目标固定为提交 `8268f62b`，要求 `git diff --quiet <SHA> -- crates` 成立。该提交是 #69 分支上的提交；#69 以 squash 合入为 `02046d9` 后分支被删除，检出里不再有这个对象（`fatal: bad object`，失败在 build.rs 断言）。`02046d9` 之所以通过，是因为当时分支仍在，`fetch-depth: 0` 还能取到它。随后 `c198425` 改了 `crates/` 而没有重锚，即使对象可取也会失败。README 记录过 82 个固定绑定，build.rs 在 main 上被改过 57 次，都依赖同一手工重锚机制。
- **修复**：目标改为当前检出中已提交的 `crates/` 树，用 Git 树对象 id（`git rev-parse HEAD:crates`）标识。build.rs 把它嵌入 harness，`crates/` 有未提交或未跟踪的改动时拒绝构建；driver.py 在同样状态下拒绝运行，harness 报告的 `target_sha` 与当前树不同（过期二进制）时仍判为 `protocol`。树 id 按内容寻址，squash 合并、删分支和浅克隆都不影响，只改文档的提交不改变它（`8268f62b:crates` 与 `02046d9:crates` 同为 `45bff81`）。提交仍由摘要里的 `harness_git_sha` 记录。ci.yml 与 fuzz-nightly.yml 不再需要 `fetch-depth: 0`。
- **验证**（Linux x86_64，Rust 1.98.1，`c198425` 之上）：实验 cargo test 3 项、fmt、clippy `-D warnings`，Python 驱动 26 项（跳过 1 项 Windows 专属）通过；固定矩阵 smoke 81／81，43 ok、38 rejected，`matrix_complete` 为真；CI 同参数 fuzz 200 次，46 ok、154 rejected，无 crash／timeout／invariant／protocol。反例：`crates/` 有未提交改动时 build.rs 拒绝构建，driver 以 64 退出；把期望树改成其他值时同一二进制的结果判为 `protocol`。
- **未覆盖**：Windows 与 macOS 只由本 PR 的 CI 执行。这仍不是覆盖引导 fuzz；覆盖模式的 nightly 未在本机运行。

### 2026-10-21 文档同步（#71 与 #70 已在 main）

上一节写于 [#71](https://github.com/willmove/joyoffice/pull/71) 合入时：当时只改文档的 #70 还被同一 CI 步挡住。随后 [#70](https://github.com/willmove/joyoffice/pull/70)（`f7cbeadd`）已合入 main。任务状态与计数不改（12 done／17 in_progress／37 planned／0 blocked）。没有任何任务因此改为 `done`。

- **#71**（`e8c47764`）：W09 绑定当前检出已提交的 `crates/` 树 id（`git rev-parse HEAD:crates`），不再手工重锚固定提交。`ci.yml` 与 `fuzz-nightly.yml` 去掉 `fetch-depth: 0`。上一节的根因、修复与本机验证仍然有效。更早条目里的「重锚到某提交、不放宽 `git diff <SHA> -- crates`」描述的是改绑之前的契约。
- **#70**（`f7cbeadd`）：银河麒麟 V10 SP1 修复会话的机器侧证据已落在 main，见上文「麒麟缺陷修复会话的机器侧证据」与 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json)。截图、`min002_export.pdf` 和报告里的 `fix_session`／`main_landing` 随该 PR 入库。分支代码没有整体合入。KYSEC／mtime、项目符号和文件入口的等价修复此前已由 #44、#46 进入 main，软件 Vulkan 回退由 `4bf14f2` 进入 main。分支变体的麒麟点击复验不能代替 main 变体的真机复验。T005、T110 与 T200 保持原状态。

### T200 编号超限与主题色文字（2026-10-21）

编号标签和主题色文字继续落地。T200 仍是 `in_progress`，没有改任务状态，计数仍是 12 done／17 in_progress／37 planned／0 blocked。没有启动图形界面，没有 Word／WPS 观察。

- **编号**：ECMA-376 §17.18.59 的字母示例是 27 为 aa、54 为 bbb，没有给出重复上限。本切片把重复次数放到 256 个 Unicode 标量（`LABEL_CHAR_BUDGET`）。这是不可信计数的输入预算，不是规范条文。0 与负数仍不可绘制。`ROMAN_MAX` 仍是 3999，表示古典三次重复的最后一值 `MMMCMXCIX`；更大的正数沿用同一加法表，4000 为 `MMMM`，不用上横线；将超过 256 个标量时停止分配。`ideographTraditional` 超过 10 时按天干重复（11 为甲甲、21 为甲甲甲）。示例只列出十个天干并停止，这个延续是 JoyWord 假定，不退回十进制。中文计数与简体大写覆盖每一个非负 `i32`：0 为 U+96F6「零」，四位一组继续到万和亿；跳组或更高组之后不足 1000 时插入一个零；十作为整个数字的首字时省略前导一。负数不可绘制。MS-OI29500 记载的 Word 零／万码位偏差不采用。
- **主题色文字**：解析后的 `Color::Theme` 在方案命中时收成 Rgb 再绘制，顺序仍是先 shade 再 tint。缺方案项时段落拒绝。改写过的 run 按 §17.3.2.6 写出必需的 `w:val`（计算后的 sRGB）、`w:themeColor`，以及可选的两位大写十六进制 `w:themeTint`／`w:themeShade`。重新导入恢复 `Color::Theme`。主题部件不改写。格式刷仍拒绝主题色。表格底纹的 `color_value` 仍拒绝主题色。带 tint 的边框与下划线主题色不在本切片。
- **证据**：`letters_roman_and_ideographs_follow_the_repeat_and_the_label_budget`、`chinese_counting_and_legal_follow_place_value`、`numbering_edges_place_labels_past_the_old_caps`、`theme_colour_text_paints_the_resolved_srgb`、`theme_colour_text_round_trips_when_the_run_is_rewritten`。本机 `cargo fmt`、`cargo clippy --locked -D warnings` 与 `cargo test --locked` 覆盖 `joyword-model`、`joyword-docx`、`joyword-layout`。
- **W09**：harness 自 `e8c4776` 起绑定检出的 `crates/` 树标识，未提交的 `crates/` 会拒绝构建。本切片不重锚某个提交 SHA。固定矩阵 smoke 没有在本机重跑；CI 的 DOCX robustness 步骤会在该树上执行。
- **未覆盖**：按字体名换面、bar／middleDot／heavy、`numbering.xml` 改写、罗马数字 0 与负数、超过 256 个标量的标签、格式刷里的主题色、主题色底纹、下划线主题色，以及人工验收。没有把 T200 标成 done。

### T203 页面相对 wrapTopAndBottom（2026-10-21）

这一刀在已经放置的页面相对 `wp:wrapSquare` 上，增加页面相对、非负偏移、位于文字前面的 `wp:wrapTopAndBottom`。正文与图片带垂直相交时整行移到带下方，不进入两侧空隙。`wrapTight`／`wrapThrough` 仍只是方形近似，不沿 `wp:wrapPolygon`。衬于文字下方和非页面相对位置仍保留原字节。T203 保持 `in_progress`。来源是 JoyWord 对 ECMA-376 `CT_WrapTopBottom` 的阅读，不是 Word 观察。

- **导入**：位置规则与 wrapSquare 相同。`simplePos` 为真时用 `wp:simplePos` 的 x／y；否则 `positionH` 与 `positionV` 的 `relativeFrom` 都必须是 `page`，并且只有非负 `wp:posOffset`。绕排必须恰好是一个没有子元素、没有 `wrapText` 的 `wp:wrapTopAndBottom`。距离只读 `distT`／`distB`，先取 wrap 元素，缺省再取锚点同名属性，再缺省为 0。这不是 Word 省略距离时的 0.125 英寸。`distL`／`distR` 不读取，不打开侧向空隙。负距离诊断为 `Unsupported`。非零 `wp:effectExtent` 拒绝。图片字节、像素、边长和 100 英寸上限不改。`r:link`、OLE、SVG、EMF、WMF 仍不取回；图片诊断不写入 URL。drawing 不是片段，未改写的段落在导出时保留原绕排 XML。
- **排版**：排除带是图片顶边减去 `distT` 到图片底边加上 `distB`，横向盖住当前行所在的栏框。带内有字形的行整行放到带的下沿，并使用该栏框的左缘和全宽，不拆到图片两侧。没有字形的锚点行留在原处，避免在图片下方再插一行空白。带上方的行不移动。`wrapSquare` 仍把字形留在矩形外侧。锚点段之前已经放下的行不回头重排。页眉页脚故事不参与这套排除带。屏幕、PDF 与 CPU 仍画同一个页面坐标矩形。
- **证据**：`a_wrap_top_and_bottom_page_anchor_imports_and_its_markup_survives_an_edit`、`a_wrap_top_and_bottom_pushes_overlapping_lines_below_and_square_stays_beside`、`a_wrap_top_and_bottom_rectangle_is_shared_by_pdf_and_the_cpu`。行为登记新增 `IMAGE-WRAP-TOP-BOTTOM`、`IMAGE-WRAP-TOP-BOTTOM-PLACE`，来源是假设，没有 Word 观察。`IMAGE-ANCHOR-PAGE` 不再把 wrapTopAndBottom 记为拒绝。
- **未覆盖**：真正的轮廓绕排、`behindDoc`、非 `page` 的 `relativeFrom`、`wp:align`、负偏移、表格单元格、页眉页脚上下型参与正文绕排、锚点段之前已经放下的行、GUI 人工验收、Word／WPS／浏览器粘贴。没有新增依赖。没有启动图形界面。没有把 T203 标成 done。本切片改了 `crates/`。W09 自 2026-10-21 起按检出的 `crates/` 树标识生产代码，不再为这一刀手工重锚历史 SHA。

### T109 打开与保存并发的自动补强（2026-10-21）

慢打开只读窗口和保存进行中的关闭提示以前只靠真实耗时，本机往往在 1 秒内结束，图形会话看不稳。这一刀加上可放行的 `StepGate`，并让窗口与外壳走同一套判断。T109 仍是 `in_progress`：没有做 Windows 或 macOS 的人工窗口验收，也不把这次自动测试当成那个验收。

- **慢打开**。`BoundDocx::open_default_paused` 在读文件之前等待测试放行。等待期间外壳把除 `edit.insert` 以外的编辑和页面命令标成只读；插入写入打开缓冲，不改当前文档。平台键入（`replace_text_in_range`／预编辑）在 `is_opening` 时同样只进缓冲。`accept_bound` 把缓冲放进新文档。过期的 generation 或打开失败时，缓冲写回仍在的文档，不无提示丢掉。状态栏在打开期间使用 `file.open.reading`。
- **保存进行中的关闭与退出**。`SaveCopyJob::run_paused` 与 `SaveOriginalJob::run_paused` 在计划与 IO 之前等待。`close_prompt` 同时给关闭和退出用：当前版本等于已保存版本则直接离开；在途保存的版本就是当前版本时仍提示尚未落盘（`close.unsaved`）；在途版本比当前版本旧时提示保存开始后的修改不在这次保存里（`close.dirty_after_save`）。取消关闭保留会话。另存不改 dirty；原件保存只把已写出的版本标成已保存，随后的键入仍脏，迟到结果仍按 DocumentId 路由。
- **自动验证**：`t109_open_save_gate`（慢打开只读与键入保留、过期／失败打开不丢键入、另存在途时关闭提示、原件在途时关闭提示与跨文档路由）。既有 `m1_concurrency_recovery`、`save_copy_job`、`original_save_job` 不改断言。`close_and_quit_share_the_dirty_after_save_prompt` 与 `only_a_writing_save_of_this_editor_counts_as_in_flight` 覆盖提示文案和“正在写入”的判定。
- **未覆盖**：Windows 与 macOS 上对着真实窗口做慢打开只读、取消打开，以及保存进行中点关闭／退出。没有把 T109 标成 done。

### T400 Provider 与关闭基础（进行中，2026-10-21）

M4 的第一刀只建立关闭基础。编辑、DOCX、排版和 GPUI 窗口默认都不链接 Provider。任务保持 `in_progress`：没有人工验收，没有真实网络传输，也没有把提案应用到文档。

- **隔离**。新 crate `joyword-agent` 没有默认 feature，Cargo.toml 的 `[dependencies]` 为空。它不依赖 model、engine 或 docx。`joyword-shell` 与 `joyword` 增加可选 feature `ai`，默认 feature 列表仍是空。`--features ai` 才编译 `joyword-shell::ai::AiToggle`。功能区不注册 AI 命令，避免在关闭状态下露出一个假入口。`scripts/check_gates.py` 把 `joyword-agent` 登记成不允许依赖其他 JoyWord crate 的层，并拒绝核心 crate 的清单出现 `joyword-agent`，也拒绝 agent 源码点名 socket 客户端。
- **默认关闭**。`AiConfig::default` 的 `ai.enabled` 与 `telemetry.enabled` 都是 false。配置文本拒绝名称里带 secret、api_key、password、token 或 authorization 的键，值不进入未知键列表，错误信息也不回显该值。比本构建新的 schema 直接拒绝，不会被当成开启。
- **关闭状态机**。阶段是 Disabled、EnabledIdle、Running、Stopping。`begin_stop` 发布 Stopping，此后新请求和尚未接纳的提交失败，generation 先不变。`commit_disabled` 是线性化点：同一把锁里把阶段设为 Disabled 并把 generation 加一。`kill_switch` 把这两步放进同一次加锁；已经关闭时不再增加 generation。重新打开不恢复旧 generation 的 ticket。管理员 `force_disabled` 优先于用户开关，Provider 没有打开它的接口。
- **启动零请求**。`startup_sequence` 只记下首次启动、打开文档和空闲三步。关闭和用户已打开两种配置都不调用 Provider。`FakeProvider::bomb` 一旦被调用就计次并返回错误；默认启动用的就是它。
- **假 Provider**。契约测试使用进程内 `FakeProvider` 和 `RecordingTransport`。HTTP 形态只规定 endpoint、取消和脱敏后的 Authorization 记录。没有把任何 HTTP 库放进依赖，CI 不需要云密钥。endpoint 拒绝带用户信息的 URL。缺失凭据、策略白名单不匹配时，传输的 `calls` 保持 0。离线与限流由假 Provider 返回，`allowed_commits` 仍是 0，运行时没有文档句柄。
- **策略与凭据**。`PolicySource` 的 Windows 组策略、macOS 配置描述文件和 Linux 系统配置变体都是 stub：`is_stub` 为真，`load` 返回未管理，不读注册表或系统文件。测试用 `StaticPolicy`。`Secret` 的 Debug 和 Display 固定写成 redacted。`UnavailableSystemStore` 表示系统凭据库尚未接入。
- **明确不做**。`ToolId::dispatch` 对包括 `propose_comment` 在内的全部工具返回不可用。`ChangeProposal::apply` 固定返回 `DeferredUntilT402`。`aigc::write_label` 返回 `AssessmentPending`。GB 45438-2025 与欧盟 AI Act 第 50 条的义务主体评估还没做，这里不写显式或隐式标识，也不声称合规。已有包内标识仍由格式层保留，本 crate 不触碰包。
- **自动验证**（Linux x86_64，Rust 1.98.1）。`cargo fmt --all -- --check` 通过。`cargo clippy -p joyword-agent --all-targets --locked -- -D warnings`、`cargo clippy -p joyword-shell --all-targets --locked -- -D warnings` 与 `cargo clippy -p joyword-shell --all-targets --features ai --locked -- -D warnings` 通过。`cargo test -p joyword-agent --locked` 的契约测试 18 项通过。`cargo test -p joyword-shell --locked --test ai_toggle` 在默认构建确认没有 `CARGO_FEATURE_AI`；`--features ai` 的同名测试确认启动时 Provider 调用数为 0，强制关闭后入口不可用且调用数仍为 0。`cargo tree -p joyword-agent` 只有它自己；`joyword-engine` 与默认 `joyword-shell` 的 normal 依赖树不含 `joyword-agent`，`--features ai` 的 shell 才含有。合入 main `305906e`（#75）之后重跑 `cargo test -p joyword-engine -p joyword-model -p joyword-docx --locked --offline`，263 项通过、0 失败。`python3 scripts/check_gates.py` 与 `python3 scripts/check_docs.py` 通过（84 个需求、66 个任务）。CI 增加了「AI off by default (T400)」一步，覆盖 agent 测试、shell 的 ai feature、核心 crate 的 `--no-default-features` 检查，以及上述 clippy。
- **未覆盖**。首次启动离线编辑、配置服务后开关的人工验收没有做。没有真实操作系统策略后端，没有系统钥匙串，没有 socket 传输。选区工具是 T401，提案应用是 T402。AIGC 法律评估仍是 TODO。没有启动图形界面。因此 T400 保持 `in_progress`。T400 单独计入时进度表为 12 done／18 in_progress／36 planned／0 blocked；与 T302 同为进行中后为 12 done／19 in_progress／35 planned／0 blocked。

### T302 批注线程第一切片（2026-10-21）

T302 从 `planned` 改为 `in_progress`，不标 `done`。这一刀是类型化批注模型和 DOCX 导入导出，没有图形界面，也没有 Word／WPS 人工交换。当时进度计数改为 12 done／18 in_progress／36 planned／0 blocked。T400 随后也改为进行中，当时进度表为 12 done／19 in_progress／35 planned／0 blocked。

- **模型**：`AnnotationKind::Comment(Comment)`。字段覆盖正文故事、`w:id`、作者、日期、缩写、`w14:paraId`、回复的 `w15:paraIdParent`、`w15:done`、`w16cid:durableId`、`w16cex:dateUtc`，以及未映射属性和非 `w:p` 子元素的原文。`w:id` 可以是 0。日期按原文保存，不再解析。段落标识是八位十六进制，比较时不分大小写。文档计数器分配新的批注 id 和段落标识，撤销恢复内容时一起恢复。解决只改 `resolved`，不改锚点，也不是删除（REVIEW-006）。
- **导入**：依据 ECMA-376 Part 1 §17.13.4 `w:comment`、`w:commentRangeStart`、`w:commentRangeEnd`、`w:commentReference`，以及 [MS-DOCX] 的 commentsExtended（`w15`）、commentsIds（`w16cid`）和 commentsExtensible（`w16cex`）。关系类型分别是 `http://schemas.microsoft.com/office/2011/relationships/commentsExtended`、`http://schemas.microsoft.com/office/2016/09/relationships/commentsIds`、`http://schemas.microsoft.com/office/2018/08/relationships/commentsExtensible`。格式正确的范围标记和引用不再锁定段落；缺 `w:id` 的标记仍保留并锁定。回复通常没有范围标记，锚点复制父批注。`w15:done` 缺省表示未解决，不用把省略当成真。people.xml 不导入。
- **错误路径**：缺作者的批注保留，诊断 `CommentAuthorMissing`。根批注没有标记则为 `CommentAnchorMissing` 并 Detached。标记落在不透明表格里则为 `CommentAnchorUnmapped` 并 Detached，表格字节不改。重复、颠倒、跨故事或回复环则为 `CommentAnchorInvalid` 并 Detached。`w15:paraIdParent` 对不上任何批注则为 `OrphanReply`，批注保留且 Detached，模型校验不算错误，文档不因此只读。未匹配的 `commentEx` 为 `CommentExtensionUnmatched` 并保留。重复 `w:id` 为 `CommentIdDuplicate`，先出现的生效。没有 `w:id` 的 `w:comment` 为 `CommentIdMissing`，不另造批注。部件形状不对为 `CommentPartInvalid`。空正文为 `CommentBodyEmpty` 并补一个空段。存在的部件若 XML 解析失败，仍按包拒绝。引擎侧：空作者或过长作者是 `CommentAuthorMissing`；父批注不存在或不是批注是 `OrphanReply`；父批注没有 `paraId` 是 `CommentThreadUnavailable`；未解决却重开是 `CommentNotResolved`；已解决再解决是 `CommentAlreadyResolved`。这些命令失败时文档不变。删除非空范围使锚点 Detached，不把批注标成已解决。
- **导出**：已有 `w:comment` 不重新序列化。解决和重开只在 commentsExtended 里就地改 `w15:done`，comments.xml 字节不变。新的根批注在缺少部件时创建 `/word/comments.xml` 和 `/word/commentsExtended.xml`，并补关系与内容类型；已有部件则只追加。回复不写范围标记。commentsIds 与 commentsExtensible 读入模型后按原字节保留，新回复的 `durable_id` 为空，本切片不改写这两个部件。从 Range 变成 Detached 的正文标记会去掉；本来就 Detached 的表格标记不去掉。纯文字修改仍走叶子补丁，不挪已有标记。仍含批注标记的段落若要整段重写，导出拒绝，原因是 `comment range is kept only for a text change`。跨 `w:t` 的新范围会拆开简单 run；拆不开则返回 `comment range cannot be written`，不静默丢掉标记。因此第一次写出跨 run 范围后，再导入看到的是拆开的 run；对该结果再保存，语义比较保持稳定。
- **自动验证（Linux x86_64，Rust 1.98.1）**：`cargo fmt --all -- --check`；`cargo clippy --locked -p joyword-model -p joyword-engine -p joyword-docx --all-targets -- -D warnings`；`cargo test --locked -p joyword-model -p joyword-engine -p joyword-docx`；`python3 scripts/check_docs.py`；`python3 scripts/check_gates.py`。引擎：`add_reply_resolve_and_reopen_are_undoable`、`deleting_the_range_detaches_and_does_not_resolve`、`resolving_does_not_detach`、`orphan_reply_missing_author_and_bad_reopen_leave_the_document_unchanged`。DOCX：`range_comment_imports_author_date_and_cross_paragraph_ends`、`reply_thread_resolve_and_reopen_round_trip_keeps_unknown_xml`、`deleting_the_anchor_exports_detached_without_resolving`、`text_inserted_inside_a_range_stays_inside_after_export`、`orphan_reply_and_missing_author_stay_visible`、`opaque_table_comment_anchor_is_detached`、`a_new_comment_and_reply_are_written_back`。这一刀之前，批注部件不导入，格式正确的范围标记会锁定段落，批注集合不相等会挡住导出，也没有解决、回复和孤立回复的命令错误。
- **未覆盖**：图形界面与气泡；people.xml 身份；commentsIds 和 commentsExtensible 对新回复的改写；不透明表格内的锚点映射；仍带范围标记的段落整段重写；页眉和页脚批注的导出专项测试（导入遍历会记下页眉段落上的标记，本切片没有单独的页眉夹具）；Word／WPS 人工交换。没有把 T302 标成 `done`。

### 键盘编辑与开始功能区向 Word 对齐（2026-10-21）

按体验差距分析，把主流文字处理软件的键盘编辑习惯和开始功能区常用按钮补齐。任务状态不改，计数沿用 main 当前进度表的 12 done／19 in_progress／35 planned／0 blocked。没有启动图形界面，没有 Word／WPS 观察。用法与限制见 [键盘编辑指南](guide/keyboard.md)。

- **移动与删除**：`TextInput::move_by` 提供按词、按段、到文首文末的移动，扩展选区保持锚点；`delete_word` 删到同一词停靠点，一步撤销。词停靠按 UAX #29，表意字逐字，字素簇不拆。PgUp／PgDn 在窗口里按已排版行累计约一屏，并同步滚动。
- **格式切换**：Ctrl+B／I／U 按样式解析后的效果判断，有一处缺就整体加上，全有就整体取消。只在样式结果与目标不同处写直接格式，其余清除，所以样式本身加粗的文字也能来回切换，不依赖直接切换属性与样式的叠加规则。`shows` 供功能区显示按下状态。
- **命令与快捷键**：新增剪切、下划线、左对齐、右对齐、两端对齐、按词／段／文档移动及其扩展、翻页及其扩展、删除前后一个词，补上 Ctrl+Enter 分页符、Ctrl+H 替换（macOS 为 Control+H）、Ctrl+D 字体。居中的 Ctrl+E 用 `Shortcut::windows_only`，Linux 上 Ctrl+E 仍是导出 PDF。
- **功能区与窗口**：开始功能区加剪切、下划线、左对齐、右对齐、两端对齐，布局功能区加页脚，功能区地板从 53 升到 59。格式、对齐、视图开关与修订视图按钮显示按下状态。Linux 编辑栏加剪切。窗口标题为「文件名 - JoyWord」，未保存时加 `*`，关闭提示写出文档名。格式刷「连续／单次」和查找栏「全半角／正则」改用语言包文字。
- **证据**：[word_navigation.rs](../../crates/joyword-shell/tests/word_navigation.rs) 11 个测试，`cut_toggles_and_alignment_presets_go_through_the_command_registry`、`protected_documents_refuse_edits_and_a_failed_save_keeps_bytes`，应用内 `word_processor_chords_reach_their_commands`、`window_title_names_the_document_and_marks_unsaved_changes`、`page_keys_move_about_one_screen_and_stop_at_the_ends`、`every_word_unit_command_maps_to_one_caret_move`。行为登记新增 `EDIT-WORD-MOVE`、`EDIT-PARAGRAPH-DOCUMENT-MOVE`、`EDIT-WORD-DELETE`、`FORMAT-TOGGLE-MIXED`、`FORMAT-TOGGLE-STYLE`、`CLIP-CUT`，来源都是假设。本机 `cargo fmt`、`cargo clippy --locked -D warnings`、`cargo test --locked`、`scripts/check_gates.py` 与 `scripts/check_docs.py` 通过。
- **未覆盖**：真实窗口里的按键、IME 组合期间的快捷键、PgUp／PgDn 滚动手感和按下状态的视觉效果没有人工验收；麒麟、统信真机未验证。中日韩没有词典分词。对齐快捷键不做「再按一次回到左对齐」。多段混合对齐时按钮只看插入点段落。

### Linux 冒烟测试四处修复（2026-10-21）

Linux X11 冒烟测试（main c0c3ed7f）报了四个问题。任务状态和进度计数不改。

- **命令行打开**：`main` 原先不读命令行参数，窗口一直显示内置样例，所以标题和关闭提示都是「未命名 1」。应用内「打开」本来就绑定文件，标题正确。现在第一个非选项参数（或 `--` 之后的第一个参数）经 `confirm_if_dirty` 走 `begin_open_path`；启动时恢复了日志的文档会先弹未保存提示。
- **关闭提示**：GPUI 在 Linux 的后备提示框宽度固定、文字不换行，第二行被截断。Linux 上改用自绘提示框：440 px 卡片，说明文字换行，Esc 取消，Enter 只触发确定类按钮。
- **两端对齐**：Justify 写出的是 `both`，没有写错；排版原先只接受纯中文行，含 ASCII 的段落按 `inter-character justification` 报错，整页无法显示。`cjk::justify_line`／`justify_run` 改为在中日韩字符两侧和空格处分配余量，拉丁词与数字内部不拉开，行尾空格悬挂，没有可分配间隙的行保持左对齐；纯中文行与 R3 等分结果相同。正文与列表共用同一入口。`distribute` 仍拒绝。
- **Esc 关闭字体面板**：Esc 原先在整个 `JoyWordInput` 上下文绑定 `CancelComposition`，按键绑定先于元素按键处理，面板收不到 Esc。现在只在 `JoyWordInput && composing` 下绑定；根视图的按键处理关闭格式对话框或路径栏，并把焦点还给文档。
- **证据**：layout `justify_line_matches_r3_on_pure_chinese_and_spares_latin_words`、`mixed_justification_fills_lines_at_cjk_gaps_and_spaces_only`、`distribute_stays_unsupported_and_lone_latin_words_keep_their_shaping`；应用内 `justify_on_mixed_script_paragraphs_keeps_the_document_displayable`、`a_document_named_on_the_command_line_names_the_window_and_the_close_prompt`、`escape_is_bound_only_while_composing_so_panels_receive_it_otherwise`、`prompt_keys_answer_cancel_on_escape_and_ok_on_enter_only`。本机 `cargo fmt`、`cargo clippy --workspace --all-targets --locked -D warnings`、`cargo test --locked`、`cargo test --locked -p joyword`、`scripts/check_gates.py`、`scripts/check_docs.py` 通过。在 Xvfb 桌面（llvmpipe）上实际运行，看到标题「sample.docx - JoyWord」与「*sample.docx - JoyWord」、Ctrl+J 后混排段落拉伸且段末行不拉伸、Ctrl+D 后 Esc 关闭字体面板并能继续输入、关闭提示写出 sample.docx 且说明换行、Esc 取消提示、「不保存」退出。
- **另见**：当时 word_edited.docx 被排版预检以 `preserved settings` 拒绝，关闭时选「不保存」后恢复日志仍保留。这两处随后由「图形界面复测三处修复」处理。
- **未覆盖**：真实 fcitx／ibus 组合期间按 Esc；Wayland 下的提示框；不同窗口尺寸下提示框外观；混排两端对齐没有 Word 对照；麒麟、统信真机未验证。

### 图形界面复测三处修复（2026-10-22）

真机窗口复测（main cf8ab435 之后）报了三处。任务状态和进度计数不改。

- **路径栏按键**：另存为、打开、查找和格式对话框里的单行输入都在文档焦点的后代上。文档编辑快捷键绑在祖先上下文 `JoyWordInput`，GPUI 在冒泡阶段让动作默认停止传播，所以 Backspace、Delete、方向键、Home、End、Ctrl+A 等改的是正文。这些绑定改为同时要求 `!JoyWordField`；字段获得焦点时加入该上下文。字段自己维护插入点和选区，处理上述按键以及 Ctrl+C／X／V、按词移动和按词删除。Ctrl+S 等文件命令仍是全局绑定。
- **关闭提示禁则**：GPUI 的换行可以在任意汉字前断开，所以说明文字会把全角「。」单独放在下一行。提示框改为按文档排版的 `break_opportunities`（UAX #14 断点再经 `adjust_cjk_break`）自行折行，行首不能是 `cjk::prohibits_line_start` 里的收尾标点。卡片宽度仍是 440 px，窄窗口时按视口的 90% 收窄。
- **word_edited.docx**：Word 2024 另存的设置含 `w:zoom`、`w:rsids`、`m:mathPr` 等无排版效果的子元素，文档默认 `w:rPr` 含 `w:kern`、`w:szCs` 和 `w14:ligatures`。导入仍整段保留这些 XML，但只在每个未建模子元素都不改变行或页时把 `preserved_layout_neutral` 标为真；排版只拒绝其余保留内容。空的主题 `typeface` 用 `w:themeFontLang` 对应的补充 `a:font`（该文件的 `zh-CN` 取 `Hans` 的「等线」），没有补充字体的空槽记为未命名，解析成无字体而不是未解析主题字体。部件字节仍原样往返。会改变分页的设置（自动断字、页边镜像、非恒等颜色映射、未分类的兼容项等）仍拒绝排版。
- **不保存**：关闭提示选「不保存」时删除 `~/.config/joyword/recovery` 下的 `recovery.jwr`（以及临时文件）。文件不存在不算错误。干净关闭不走这条路径。
- **证据**：`props::invisible_run_children_are_preserved_as_layout_neutral`、`a_word_saved_document_keeps_its_settings_and_is_marked_layout_neutral`、`settings_that_can_move_lines_are_not_layout_neutral`、`an_unnamed_theme_slot_resolves_to_no_font`、`layout_neutral_preserved_settings_and_run_defaults_lay_out`、应用内 `a_word_saved_document_opens_lays_out_and_lays_out_again_after_an_edit`、`prompt_lines_never_start_with_closing_punctuation`、`editing_keys_are_unbound_while_an_in_app_text_field_is_focused`、`editing_and_navigation_keys_edit_the_field`、`clearing_removes_a_committed_journal`。图形界面人工验收留待复测。

### T200 `numbering.xml` 追加式写回（2026-10-22）

编号定义第一次能写回 DOCX，但只在"追加"这一种形状下写回。任务仍 `in_progress`，没有人工验收。

- **入口与不变量**：新模块 `crates/joyword-docx/src/export/numbering.rs`。`plan` 比较原始与当前 `NumberingStore`，相等时返回 `None`——part 字节原样复制、在 preserved 列表里报告，导出预算也不为它增长。`export.rs` 的 OriginalPackage 提前返回因此加上 `numbering_changed` 条件，预算的 `projected` 与 `grown` 两处都加上 `numbering_delta`。
- **追加式差异**：`appended()` 要求旧的 abstractNum 与 num 序列是新序列逐元素的严格前缀，且 `unmodeled` 标志一致；既有定义被修改、被删除或标志变化时返回 `existing numbering definitions are not rewritten`，part 绝不重建。也就是说这一刀把"编号库任何变化都拒绝"收窄成"只有追加可以写，其余照旧拒绝"。
- **既有 part 的拼接**：`patch_part` 解析 `word/numbering.xml`，为新条目生成标记，按 ECMA-376 §17.9.17 的 `abstractNum*` → `num*` → `numIdMacAtCleanup?` 顺序拼在最后一个既有 abstractNum 与最后一个既有 num 之后、其他尾随子元素之前，自关闭根元素也能处理。除这两个插入点外没有任何字节移动；测试把插入的标记删掉后与原 part 逐字节比对。`nsid`、`lvlJc`、`numIdMacAtCleanup` 等未建模子元素与 `numStyleLink`／`styleLink` 全部原样保留。
- **缺 part 时新建**：`create_part` 写出带 XML 声明的 `word/numbering.xml`；`extend_relationships` 只在 `word/_rels/document.xml.rels` 追加一条 numbering 关系，rId 取现有最大数字加一，追加位置在 `</Relationships>` 之前；`extend_content_types` 只在 `[Content_Types].xml` 追加一条 Override。两者都在超链接与批注阶段已经产出的字节上继续追加，关系追加发生在超链接之后、`comment::apply` 之前，顺序不能换。
- **写出的子集**：级别标记按 §17.9.11 的顺序写 `start`、`numFmt`、`lvlRestart`、`suff`（仅在不等于 tab 时写）、`lvlText`、`pPr`（按 §17.17.86 把 tabs 写在 ind 之前）、`rPr`（按 §17.18.74 的 rFonts＜b＜sz）；`w:num` 按 §17.9.14–15／§17.9.19 写 `abstractNumId` 再写 `lvlOverride*`，`startOverride` 为空元素、替换级别为 `lvl`。级别制表位的全部 7 种对齐与 6 种引导符都能写出并往返。
- **显式拒绝**：链接定义、未建模条目或级别、图片项目符号、Theme 字体、超出子集的段落／字符属性、非 twips 的缩进与制表位、非半磅的字号、XML 属性规范化无法往返的控制字符与 U+FFFE／U+FFFF、指向不存在 abstract id 的实例、空格式记号，全部返回可定位的 `ExportError::Unsupported`，不猜测也不假装成功。
- **取代 `check_envelope` 的编号整体拒绝**：`check_envelope` 里 `before.numbering() != after.numbering()` 就整体拒绝的那一行被删除，判定改由本模块的追加式差异承担，所以 `list.apply bullet=1` 新建定义之后的导出不再被一律拒绝。合并时若把这一行加回去，追加编号的导出会重新全红。导出后的重导入校验另外显式比较编号库，因为 `canonical_dump` 不含它。段落 `w:numPr` 的改写规则本切片未动。
- **模型侧最小改动**：只加 `NumberSuffix::token()`（§17.18.88 的 `w:suff` 记号）；`NumberFormat::token` 本来就是 `pub`，未映射记号保留原拼写，所以写回不会发明格式。没有新的外部依赖，`Cargo.lock` 未动。
- **自动验证**：`crates/joyword-docx/tests/numbering_writeback.rs` 六项——`appended_bullet_definitions_are_inserted_without_touching_other_numbering_bytes`、`a_new_numbering_part_gets_a_relationship_and_a_content_type_override`、`an_unchanged_numbering_store_keeps_the_part_verbatim_and_preserved`、`restoring_the_previous_store_keeps_the_package_unchanged`、`modified_or_unrepresentable_numbering_changes_are_refused`、`numbering_level_tabs_write_every_align_and_leader`。端到端要求（`ensure_bullet` 追加定义 → 导出写出 → 重导入复原编号库并画出 `1.`／`•`／`startOverride` 的 `4)`，以及直接恢复上一份模型快照后导出与原包字节相同，不执行 engine 撤销）在模型与 docx 层覆盖；没有新增引擎测试，因为 `Command::ApplyBullet` 就是 `ensure_bullet` 加 numPr 赋值，撤销沿用引擎既有的快照恢复。切片内 `cargo test --locked`（CI 口径）、`cargo test --locked -p joyword`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`check_docs.py`、`check_gates.py`（含反例）、`check_acceptance.py` 通过。
- **未覆盖**：可选的制表位引导符一项只是部分关闭——编号部件写出侧能写出并往返全部 6 种引导符与 7 种对齐，但排版侧 `crates/joyword-layout/src/list_layout.rs` 仍拒绝 `Bar` 对齐与 `MiddleDot`／`Heavy` 引导符，段落制表位的导出拒绝也仍在 `export/structure.rs` 的 `geometry_supported`／`tabs_markup` 里；`heavy` 没有规范字形，发明一个会违反不猜测原则，所以 bar／middleDot／heavy 的端到端支持没有做，不按完成记。Word／WPS 打开含追加编号定义的导出件、图形界面里点项目符号后保存重开、信创真机与打印均未做人工验收。

### T202 栏分隔线与 `nextColumn`（2026-10-22）

`w:cols/@w:sep` 与 `w:type w:val="nextColumn"` 从"整节保留、排版拒绝"变成类型化并参与排版。任务仍 `in_progress`。

- **分隔线画进共用 PageScene**：排版新增 `paint_column_separators`（`crates/joyword-layout/src/pages.rs`），每个栏间空隙画一条竖直 `SceneRect`：1 pt（`Emu::PER_POINT`）黑色细线，横向取前一栏右边缘与后一栏左边缘的中点（等宽与不等宽都成立），纵向从页面正文框的 `geometry.top` 到 `geometry.bottom_edge()`；不足两栏的节什么都不画。屏幕、PDF（`joyword-pdf` 遍历 `scene.rects`）与 CPU 光栅（`joyword-render`）消费同一批矩形，这三个 crate 一行未改。
- **假定，不是观察**：ECMA-376 §17.6.4 只记录 `sep` 打开，既不定线宽也不定精确位置。"1 pt 细线、空隙居中、纵向铺满正文框而不按最高那一栏的文字裁切"是 JoyWord 假定，代码注释与本记录都这样写明，不当成 Word 观察。
- **单一汇合点**：该通道挂在 `finish_stories` 的第一行，那是全量、混合与增量排版的唯一汇合点，所以增量与全量按构造一致，也不存在第二套几何。副作用如实记录：分隔矩形是组装之后追加的，增量的 `suffix_copies` 快路径要校验 `page.rects.len()` 相等，因此带分隔线的文档失去后缀复制复用、退回完整放置（结果正确，略慢）。
- **模型、缓存与往返**：`Section` 新增 `column_separator: bool`（`Document::new` 默认 false）；导入侧 `read_columns` 用既有 `props::on_off` 解析 `sep`，等宽（`w:num`）与不等宽（`w:col` 子元素、`equalWidth="0"`）两种形态都认，非法值（如 `sep="maybe"`）仍走 preserved／opaque 通道。`column_separator` 折进 `LayoutKey.content`（SHA-256）的节指纹，标志变化时快照缓存失效；它有意不进入段落拟合缓存键 `column_mix`，因为分隔线不改变行的拟合。导出往返是免费的：`rewrite_sectpr_page` 除 `pgSz`／`pgMar` 外逐字节拷贝 sectPr 子元素；再导入的相等校验在 `export/compare.rs` 的 `same_section_envelope` 里加一个 `column_separator` 比较项。
- **`SectionBreak::NextColumn`**：`nextColumn` 是 §17.6.22 `ST_SectionMark` 的分节起始值，含义是"本节从当前页的下一栏开始"，不是域也不是 `w:br`。模型新增该变体，导入在 body.rs 的 `w:type` 分支识别，排版新增 `SectionEdge::NextColumn`：纸张框与分栏几何不变且当页已有墨迹时经 `advance_column` 在当前页下一栏续排（`advance_column` 在末栏之后本就换页，单栏流因此自然退化为新起一页），页码连续，该页仍归打开它的那一节；纸张或分栏不同、或当页为空时退化为连续式的页拆分并发出既有的 `LayoutNote::ContinuousPageSplit`。复用既有注记是为了不改 `apps/joyword-cli/tests/golden_scenes.rs` 里的穷尽匹配。
- **页归属限制**：连续节或 `nextColumn` 节与打开该页的节共页时，页眉页脚与分隔线都跟随打开该页的节（与既有 Continuous 行为一致），所以在页中途才打开 `sep` 的节不会在已经打开的那一页上画线；归给前一节的奇偶补白页也带前一节的分隔线。
- **自动验证**：新增 12 个测试，全部在新文件里——`crates/joyword-layout/tests/columns.rs` 的 `equal_column_separator_paints_one_centered_rule`、`unequal_column_separator_paints_a_rule_per_gap`、`separator_off_paints_no_rule`、`incremental_layout_matches_full_with_separators`、`next_column_section_break_advances_one_column`、`next_column_with_different_columns_splits_the_page`；`crates/joyword-docx/tests/column_separator.rs` 的 `equal_column_separator_imports_typed_and_round_trips`、`unequal_column_separator_imports_typed`、`separator_false_or_omitted_is_not_flagged`、`invalid_separator_value_stays_preserved`、`next_column_section_break_imports_typed_and_round_trips`；`crates/joyword-pdf/tests/column_separator.rs` 的 `the_column_separator_rule_is_in_the_pdf_page_scene`（分隔矩形出现在交给 `joyword_pdf::render` 的场景里，渲染字节与关闭分隔线的同一文档不同）。切片内 `cargo test --locked`（CI 口径）、`cargo test --locked -p joyword-layout -p joyword-pdf -p joyword-render`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`check_docs.py`、`check_gates.py`（含反例）、`check_acceptance.py` 通过。没有新依赖，没有金样或夹具变化。
- **一处名称过时的测试**：`crates/joyword-docx/tests/import.rs` 的 `unequal_columns_import_and_a_separator_or_missing_width_stays_preserved` 里两个 `w:sep` 用例已移进 `column_separator.rs` 成为更强的正向断言；该测试现在只覆盖不等宽栏的类型化导入、等宽栏，以及三种仍在子集外的形态（缺宽度、`equalWidth="0"` 而无 `w:col`、只有一个 `w:col`）。名字里的"a separator … stays preserved"已不描述它覆盖的内容。改名要动 `crates/`，不在本次文档范围内，因此只在 [验收清单](acceptance/T202.md) 里更正引用，测试名保留原样。
- **未覆盖**：`chapStyle`／`chapSep` 章节页码本切片明确未做——章节号需要一个与标题编号互锁的章号来源，而编号写回属 T200、当时正在并行分支上进行，现在实现只能发明未经测试的互锁，所以二者仍留在 preserved／opaque 通道。图形界面里看分隔线与 `nextColumn` 续排、Word／WPS 对照（含线宽与位置这条假定的核对）、信创真机与打印均未做人工验收。

### T301 段落与运行格式修订（2026-10-22）

T301 由 `planned` 改为 `in_progress`。这一刀只做正文段落（表格之外）的 `w:pPrChange` 与 `w:rPrChange`，没有人工验收。

- **模型**：新增 `FormatRevision { id, author, date, previous_run, previous_para }`。run 上的 `format_revision` 来自 `w:rPrChange`、存变更**前**的运行属性；段落上的 `props_revision` 来自 `w:pPrChange`、存变更**前**的段落属性。`Paragraph` 增加 `stamp_format_from`、`has_run_format_revision`、`resolve_format_revision`、`resolve_all_format`；run 合并（`join_runs_at`、`push_text_marked`）把 `format_revision` 当作 run 身份的一部分，`split_off` 把段落级变更留给前半段；规范化 dump 打出 `pPrChange#id author=… date=… prev=[…]`／`rPrChange#…` 行，所以投影能看到记录而不是只看到结果。
- **导入子集与退回**：`props::para_with_change`／`props::run_with_change` 捕获单个 `w:pPrChange`／`w:rPrChange` 子元素而不是把整块属性留白，`revision::import_format_change` 才把它类型化：要求合法的 `w:id` 与 `w:author`（日期可选、按原文保存），至多一个正确种类的变更前属性子元素，且变更前属性完全可表示（不含前一样式、段落标记、节属性或嵌套修订）。子集之外一律返回 `None`，调用方退回"整块属性元素按原字节保留＋锁定所在段落＋`RevisionSubset` 诊断"，绝不静默丢弃；空 run 上的类型化变更同样锁定段落，防止记录消失。`scan_in` 传入 `in_table`，`blocks_for_review(local, in_table)` 只对 `w:tbl` 之外的段落类型化格式变更。
- **接受／拒绝语义（不可颠倒）**：按 ECMA-376 §17.13.5.29／.30，**接受**一条格式修订保留当前（变更后）的格式并丢弃记录；**拒绝**恢复记录的变更前格式（没有记录前值时用空默认），随后重新合并相邻 run。引擎的 `AcceptRevision`／`RejectRevision`／`AcceptAllRevisions`／`RejectAllRevisions` 复用既有命令承担这件事：`resolve_one` 在没有文本命中时回落到只带格式修订的 id（不再误报 `UnknownRevision`），同一个 `w:id` 同时承载文本与格式修订时一条命令一并解决，锁定段落以 `CommandError::Locked` 拒绝，`current_revision` 在没有触及文本修订时也能报告插入点处的格式修订（段落级在段内任意位置生效，run 级只在其 run 上生效）。每条命令在一个 `Transaction` 内执行，撤销基于快照，一步撤销完整恢复先前状态。
- **导出的"绝不合成掉未决 `*Change`"策略**：未决格式修订只能以原字节存活。`Group` 与组／元数据比较都纳入修订记录，所以未打戳的重发不可能被判为相同；`Paragraph` 的 `PartialEq` 纳入新字段，丢弃一条记录永远算作一次编辑，不会留下陈旧字节的洞。`patch_paragraph` 新增五条拒绝，全部由 `after_unresolved = after.has_run_format_revision() || after.props_revision.is_some()` 守卫：与文本修订重写组合（`unresolved format revision cannot be combined with a text revision rewrite`）、改样式或改编号（`… cannot be restyled`）、段落重写（`… its paragraph cannot be rewritten`）、run 边界变化（`… its run boundaries cannot change`）、空段新增文本（`… new text cannot be added`）。已解决的修订在 `after` 里已不存在，走正常重写路径输出不含 `*Change` 的干净格式；未改动的段落仍由既有对齐机制逐字节拷贝。
- **表格结构修订明确不实现**：`w:tblPrChange`、`w:trPrChange`、`w:tcPrChange`、`w:cellIns`、`w:cellDel`、`w:cellMerge`（每一个仍只经 `ns::is_revision` 被引用一次），以及 `w:tbl` 子树内的任何 `w:rPrChange`／`w:pPrChange`，都让整篇文档只读（`ReadOnlyReason::Revisions`）、标记逐字节保留、改写式导出以 `ExportError::Restricted` 拒绝；没有任何代码路径接受、改写或丢弃它们。同样仍只读的有 `w:sectPrChange`、`w:numberingChange`、`w:tblPrExChange`、`w:tblGridChange` 与自定义 XML 修订。因此本任务产物里的段落标记修订、编号修订、节属性修订与表格网格结构修订都还没有实现。
- **登记**：`docs/joyword/behavior_registry.json` 追加 12 条（只追加，既有条目未动，检查器报 65 条）。
- **自动验证**：`crates/joyword-docx/tests/format_revisions.rs` 十项与 `crates/joyword-engine/tests/format_revisions.rs` 五项（名单见 [TASKS](TASKS.md) 的 T301 完成证据）。切片内 `cargo fmt --all -- --check` 干净，`cargo clippy --workspace --all-targets --locked -- -D warnings` 零告警（修掉 WIP 留下的 `collapsible_if` 与 `redundant_closure`），CI 范围 `cargo test --locked` 全绿（含 15 项新测试），`cargo test --locked -p joyword`、`check_docs.py`、`check_gates.py`（含反例）、`check_acceptance.py` 通过。
- **未覆盖**：Word／WPS 往返交换、审阅栏与图形界面上逐项接受／拒绝、信创真机、打印，全部没有人工验收。锁定容器（表格单元格／SDT 锁）内格式修订的 `CommandError::Locked` 拒绝已实现但没有直接单测，改由 docx 侧的锁定与 `Restricted` 导出拒绝覆盖；页眉页脚与批注 story 里表格之外的格式修订走正文同一路径（`body::paragraph`），但没有专门的 story 级测试；段落标记 `w:rPr` 上的 `w:rPrChange` 经普通 `props::run` 路径退回"保留＋锁定"，行为由构造保证正确，只被子集外测试间接覆盖。

### T401 选区授权与受限工具（2026-10-22）

T401 由 `planned` 改为 `in_progress`。改动只在 `crates/joyword-agent/**`，AI 仍默认关闭，shell 与 apps 未接线。

- **端口 trait 与结构性隐私**：新增 `src/document.rs`。`DocumentPort` 是只读端口 trait，将来由 shell 实现；本切片只有内存假实现 `MemoryDocument` 供契约测试。不可变投影 `DocumentSnapshot` 只提供最终可见文本与大纲偏移字段，不提供独立的隐藏修订或批注字段。这限制了接口表达范围，但不自动证明宿主没有把隐藏内容混入可见字符串；真实宿主投影仍须独立实现和验证。`MemoryDocument` 内部带 `deleted`／`comments` 隐藏字段，投影时丢弃，`edit_visible` 升版本。不可信输入预算：段落数 ≤4096、单段 ≤64 KiB、全文 ≤4 MiB、隐藏条目 ≤64。
- **`Grant` 的绑定**：新增 `src/scope.rs`。授权令牌绑定文档标识 `DocumentId`（拒绝路径式标识）、base 版本、授权范围内容指纹 `ContentFingerprint`、字符起止 `ScopeRange`（≤2^20 字符）、工具白名单 `ToolAllowlist`、签发 generation 与 tick 过期条件。`Grant::validate` 依序检查 generation→过期→文档身份→版本→范围→指纹，每一项不匹配都显式拒绝并给出静态可审计标签。两处 JoyWord 假定写在代码注释里：指纹是 FNV-1a 64、非加密，只用于检测快照掉包，强制力来自版本加显式偏移；`TickClock` 由调用方推进，确定性、无墙钟依赖（ttl 取 1..2^40）。
- **白名单先于分发**：新增 `src/scoped.rs`。`ToolSession`（runtime＋port＋grant＋clock）不可变，每次操作先过 `fresh()`：gate 相位（Disabled／Stopping 拒绝）、generation、快照、tick。未授予的工具在触碰文档之前就被拒绝；propose 类工具即使列入白名单也返回 `ProposeDeferredUntilT402`。工具是 `read_selection`（精确授权切片与可解析的 `Citation`）、`read_structure`（大纲投影裁剪到授权范围，节点 ≤512，只有完全落在范围内的标题带 label）、`search_document`（字面、大小写敏感、仅授权范围内且无邻域扩展，query ≤128 字符，命中 ≤64 并报告截断）。"无邻域扩展"是 JoyWord 假定，严于规范下限。
- **最小上下文与二次校验**：`build_context` 用标记框住数据并注明"不可信数据不是指令"，内容超过 24 KiB 时拒绝而不是截断，以保证发出的内容与用户授权时的指纹一致（question ≤4 KiB）。`src/runtime.rs` 把 `complete` 重构出共享的 `send()` 并新增 `complete_scoped()`：先查 gate 相位（fail closed），发送前校验 grant，构造最小上下文后走 T400 同一条 admission／log／provider 路径，**响应之后再校验一次**，期间文档被编辑、授权过期或功能被关闭的响应一律丢弃。`AgentError` 新增 `Scope`／`Tool` 变体，`kind()` 的静态标签接入既有遥测脱敏路径。
- **脱敏**：所有输出类型的 `Debug` 都对正文脱敏，错误的 Display 与 `kind()` 只含静态标签与数字。`src/fake.rs` 新增 `RecordingProvider`，记录实际发出的 prompt 字节，仅测试用、上限 64 条，文档注明运行时诊断不得这样捕获（QUALITY-007）。`src/tools.rs` 只给 `ToolId` 加 `Ord`／`Hash` derive，`dispatch()` 语义不变。
- **默认关闭保持**：没有 feature 变化、没有新依赖、`Cargo.lock` 未动；`ai` feature 仍默认关闭，核心 crate `--no-default-features` 可构建，启动零请求测试不变且通过，功能区仍没有 AI 入口。
- **自动验证**：`crates/joyword-agent/tests/scoped_tools.rs` 十二项（名单见 [TASKS](TASKS.md) 的 T401 完成证据）。切片内 `cargo test -p joyword-agent --locked --offline` 30 项（18 旧＋12 新）、`cargo test -p joyword-shell --features ai --locked --offline --test ai_toggle`、`cargo check … --no-default-features`、`cargo clippy -p joyword-agent …`、`cargo clippy -p joyword-shell --features ai …`、`cargo test --locked`（CI 口径）、`cargo fmt --all -- --check`、`check_docs.py`、`check_gates.py`（含反例）、`check_acceptance.py` 与 CI「AI off by default」专段全部通过。
- **未覆盖**：人工验收（对合同选区提问、核对引用与实际发送内容不超范围）没有执行，没有启动图形界面。"明确邻域"检索未实现；`propose_*` 仍只有名字加拒绝（T402），`ChangeProposal.apply` 语义未动；取消沿用 T400 骨架（T403 完整化）；`DocumentPort` 还没有真实 shell 实现；若威胁模型需要防构造碰撞，指纹后续可换 workspace 已钉扎的 sha2（本切片被禁止新增依赖，未引入）；流式响应未涉及；信创真机未验证；AIGC 标识的法律评估仍是 TODO，本切片不写显式或隐式标识，也不声称合规。

### T110 Windows 卷能力分类（2026-10-22）

原件保存的替换契约在 Windows 上多了一道卷能力前置检查。改动仅限 `crates/joyword-shell/src/storage/metadata.rs`，`winsafe` 仍是锁文件里的 0.0.19，没有新依赖，没有 unsafe（仓库 `unsafe_code = forbid`）。

- **观测**：`observe_volume` 经 kernel feature 的 `GetVolumePathNameW`／`GetVolumeInformationW`／`GetDriveTypeW` 观测源卷的文件系统名、驱动类型与能力标志，存进以原生基元表示的 `VolumeFacts`，便于用合成输入覆盖各分支。
- **分类**：`classify_volume(&VolumeFacts)` 是纯函数，先判驱动类型（`DRIVE_REMOTE` 网络共享；`DRIVE_UNKNOWN`／`DRIVE_NO_ROOT_DIR` 不可识别），再判文件系统名（只有 `ntfs`／`refs` 通过，大小写无关），最后逐项核对 `FILE_VOL_PERSISTENT_ACLS`／`FILE_VOL_NAMED_STREAMS`／`FILE_VOL_SUPPORTS_HARD_LINKS`（即 Win32 的 `FILE_PERSISTENT_ACLS`／`FILE_NAMED_STREAMS`／`FILE_SUPPORTS_HARD_LINKS`）。六种 `VolumeRejection`——`Remote`、`UnknownDriveType`、`UnsupportedFileSystem`、`NoPersistentAcls`、`NoNamedStreams`、`NoHardLinks`——各有互不相同、可定位的诊断文本。
- **接入保存路径**：Windows 的 `observe_platform` 先观测源卷再分类，不支持的卷经 `volume_rejected` 变成 `ErrorKind::Unsupported` 的 `io::Error`，使 `save_original` 即便在显式确认下也拒绝替换（走 `ObservationFailed`）。这是硬拒绝，语义上不属于"用户可确认的损失项"，所以不进损失清单；默认另存副本路径不受影响（`save_default_copy` 容忍观测失败仍写副本）。把它接进 `file.save` 的原生提示需要改共享的 UI 与语言包，超出切片边界，未改。
- **进程内不可观测的部分**：查证结论写进模块文档——`winsafe` 0.0.19 没有暴露 `GetFileSecurityW`／`GetSecurityInfo`／`ConvertSecurityDescriptorToStringSecurityDescriptorW`，也没有 `FindFirstStreamW`／`GetFileInformationByHandleEx(FileStreamInfo)`。所以 raw SDDL（含 `D:`／`D:AI`／`D:PAI` 标志）与除 `Zone.Identifier` 外的命名流无法用已审计依赖在进程内观测，二者按 FORMAT-004 契约保持 `UnobservableMetadata`，在替换前列出、需显式确认；`Zone.Identifier` 的保留行为未退化。
- **一次进程外实证**：本机 Windows 参考环境（NTFS）用 PowerShell + `ReplaceFileW` 观测到：继承 `D:AI` 与受保护 `D:PAI` 两种 DACL 下，替换前后 `Get-Acl .Sddl` 完全相同；`::$DATA`、自定义流与 `Zone.Identifier` 全部存活且内容不变。因此先前记录的 `D:`→`D:AI` 差异并非 `ReplaceFileW` 固有。但因为进程内无法观测，仍不宣称精确 ACL 标志保留；这次实证也不是仓库测试，不进 CI。
- **自动验证**：六项 `#[cfg(windows)]` 测试——`classify_volume_accepts_a_local_ntfs_volume_with_every_required_capability`、`classify_volume_refuses_network_and_unrecognized_drive_types`、`classify_volume_refuses_non_ntfs_filesystems`、`classify_volume_refuses_each_missing_capability_flag`、`observe_volume_reports_a_supported_volume_for_the_temp_directory`（对本机 NTFS temp 卷做一次真实观测）、`volume_rejection_messages_are_distinct_and_non_empty`。切片内 `cargo test --locked`（CI 口径）、`cargo test --locked -p joyword-shell`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`check_docs.py`、`check_gates.py`（含反例）、`check_acceptance.py` 通过。对 macOS／Linux（rustix）路径是纯增量、未改语义。
- **未覆盖**：真实 FAT／exFAT／网络共享卷上的端到端替换拒绝没有真机验收（本机无此类卷可挂载，分类分支只以合成 `VolumeFacts` 覆盖）；非 Windows 路径未在本机验证；Windows 原生 GUI 的确认操作、真实带 ACL 文档的损失清单交互、断电场景、信创真机与打印均未做人工验收。

### CLI JSON 键序与 cargo feature 归一化解耦（2026-10-22，review 分支）

`cargo test --workspace` 会纳入 `apps/joyword`，其依赖 gpui-pre 0.3.9 启用 `serde_json/preserve_order`；feature 归一化把 CLI 共享构建的 `serde_json::Value` map 从 `BTreeMap`（键排序）换成 `IndexMap`（插入序），`inspect` 输出的键序随之改变，`fixture_roundtrip` 的金样字节比较失败。默认成员口径的 `cargo test --locked` 一直是绿的，所以 CI 测不到这个脆弱点；失败只在 `--workspace` 口径出现（`feature_fixture_cli_projections_match_goldens`：实际输出以 `"schema_version"` 开头，金样以 `"diagnostics"` 开头）。任务状态与计数不改。

- **根因取证**：`cargo tree -e features -i serde_json` 显示默认成员口径下 `serde_json 1.0.154` 只有 `default`／`std`（`Value` map 是 `BTreeMap`），`--workspace` 口径下 `preserve_order`（及 `raw_value`、`indexmap`）由 `gpui-pre` 与 `gpui-pre-http-client` 0.3.9 启用，而 gpui-pre 只被非 default-member 的 `apps/joyword` 依赖。同一次构建内输出仍是确定的，T210 的"两次运行逐字节相同"在单一构建内成立；但输出字节依赖 feature 归一化，任何人给任何 workspace 成员加一个启用 `serde_json/preserve_order` 的依赖就会让两种口径分叉。
- **修法选择**：两个候选方向里选了"消除 CLI 对 `Value` map 顺序的依赖"，而不是放宽键序比较——后者会削弱 T210 逐字节确定的强度，前者是最小修法且金样文件一个字节都不用动。
- **改动**：`apps/joyword-cli/src/main.rs` 新增 `canonicalize_keys(&mut Value)`，递归把每个 JSON object 的键经 `BTreeMap` 重插为升序（`BTreeMap` 下是 no-op，`IndexMap` 下规范化），`inspect` 的标准输出与 `text` 的标准错误报告在序列化前统一调用；`src/visual.rs` 的 `write_report` 同样调用，`render`／`convert` 报告一并免疫。没有 unwrap／expect／panic，no-panic 门禁通过。
- **把危险配置变成 CI 口径**：`apps/joyword-cli/Cargo.toml` 的 `[dev-dependencies]` 加 `serde_json`（与主依赖同版本、启用 `preserve_order`，indexmap 已在锁文件里，`--locked` 不受影响），使 `cargo test -p joyword-cli` 与 `cargo test --locked` 也始终在插入序 map 下构建被测二进制；新增 `inspect_json_keys_stay_sorted_whatever_the_feature_set` 先断言测试构建确实启用了 preserve_order（防止退化为空洞通过），再对 `inspect` stdout 与 `text` stderr 的原始键序递归断言升序。金样测试从"CI 口径测不到这个脆弱点"变成"CI 口径天然锁住它"。
- **修前／修后**：只加 dev-dep 与新测试、未改 src 时，`cargo test --locked -p joyword-cli --test fixture_roundtrip` 里 `feature_fixture_cli_projections_match_goldens` 与新测试双双失败（新测试打印出插入序与排序序两份键列表），即加锁后连 CI 默认口径都能复现该红；改完 src 后 `cargo test --locked -p joyword-cli` 全绿，`cargo test --workspace --locked` 与 `cargo test --locked` 均 exit 0，`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`check_gates.py`、`check_docs.py`、`check_acceptance.py` 通过。金样文件零改动，没有用 `JOYWORD_BLESS_GOLDEN`。
- **未覆盖**：`cargo deny`、cross-linux（aarch64／loongarch64 + QEMU）与 unified-pdf 三个 CI 作业本机没有工具链，未验证，由 GitHub 覆盖。joyword-cli 的测试构建从此固定带 preserve_order，若将来有测试依赖 `IndexMap` 顺序，会在 CI 默认口径就暴露（这是设计意图）。

### T111 门禁加固（2026-10-22，#86）

加固改动 `scripts/`（含门禁反例夹具）和仓库根目录的 `GATES-REPORT.md`，没有跑 cargo，没有改 `crates/`、`apps/`、`docs/`、`.github/` 或文档格式语料。T111 的 `done` 状态本次不改；本节记录的是加固之前门禁的实际强度，以及加固之后新暴露的东西。

- **层检查此前几乎未读取内部依赖**：`dependencies()` 的 `DEP_RE` 只认 `name = "..."` 内联写法，而本仓库几乎所有内部依赖都写成点号形式 `name.workspace = true`，13 个 manifest 里只解析出 1 个内部依赖。ARCHITECTURE §1 的层表因此在本次之前基本没有强制力，唯一可靠触发的路径是新 crate 未登记。现在 `dependency_tiers()`（`DEP_KEY_RE` 取代 `DEP_RE`）认全三种写法（`name.workspace = true`、`name = { ... }`、`[dependencies.name]`）以及 `[target.'cfg(...)'.dependencies]`／`.dev-dependencies`／`[build-dependencies]`，build 依赖按普通层约束。普通依赖严格照 `LAYERS` 校验、不留基线容忍。实测所有 `[dependencies]` 与 target 专属依赖边本来就合法，加固没有改任何 `Cargo.toml`。
- **`DEV_LAYERS`**：dev 依赖照 `LAYERS ∪ DEV_LAYERS` 校验。`DEV_LAYERS` 是显式登记的许可表，六对：`joyword-cli`→`joyword-corpus`、`joyword-docx`→`joyword-corpus`、`joyword-layout`→`joyword-engine`、`joyword-layout`→`joyword-render`、`joyword-pdf`→`joyword-engine`、`joyword-render`→`joyword-pdf`，每条注释指明需要它的 manifest 行；键与值都必须已在 `LAYERS` 里，写错不能悄悄放宽。原任务描述只列了四对，少列的两对（render→pdf、cli→corpus）是既有的合法边，只登记四对会让门禁在这两处红。
- **no-panic 基线双向漂移**：删掉 12 条指向已不存在代码的条目（陈旧条目本身就是绕过：同文件同文本再出现会直接放行），补录 19 条被旧"集合差"语义藏住的真实位点。19 条全部位于 `#[cfg(test)]` 模块体内，属 T111 归类的第一类测试代码，生产位点一条未增；它们是基线已列过一两次的重复载荷（`let dir = tempfile::tempdir().unwrap();` 之类）。净 −12／+19，`scripts/panic_baseline.txt` 由 554 行变为 561 行。TASKS 里 T111 记的"基线 513 条"是 2026-10-17 的数字，此后在加固之前就已漂到 554。`check_panic()` 改为按 `(文件, 代码文本)` 的 `Counter` 多重集比较，报出两侧计数与当前全部行号，行号漂移仍容忍；死代码 `extra = sorted(set(current) - set(expected))` 删除。`--write-baseline` 与检查现在共用 `gated_panic_sites()`／`panic_scope()`（口径取两个旧判定中更严的那个，所以重新生成既不能放宽也不能收窄门禁），另加 `--update-baseline` 只追加缺额、既有条目字节不变：全量重写与可审阅的增量补录两种模式从此可区分，增量是默认的补录路径。
- **`#[cfg(test)]` 截断**：旧的 `text.split("#[cfg(test)]", 1)[0]` 会丢掉第一个标记之后的全部内容，而本仓库把 `cfg(test)` 用在 `use` 语句、结构体字段和单个 fn 上，不只是测试模块——`apps/joyword/src/document_canvas.rs:6` 是一条 `#[cfg(test)] use`，旧逻辑把该文件 1945 行里的 1939 行整个丢掉，共 44 个文件被截断。现在 `test_module_ranges()` 只认下一非属性行为 `mod NAME {`／`mod NAME;` 的 `#[cfg(test)]`（含 `cfg(all(test, ...))`），并在 `mask_rust()`（注释与字符串／字符／原始字面量挖空、偏移保持）的输出上做花括号配对，字面量里的花括号不能挪动边界；其余行由 `production_lines()` 全扫，所以 `use`／字段／fn 上的 `cfg(test)` 什么都不再隐藏。顺带记录一个真实的偶然：`apps/joyword/src/tests.rs` 的第一处 `#[cfg(test)]` 出现在它自己第 3 行的文档注释里（那句注释正是在描述本门禁），旧逻辑因此在第 3 行截断了全部 1821 行——描述门禁的句子把该文件的门禁关掉了。
- **panic 正则与 `allow` 绕过**：`PANIC_RE` 增加 `todo!`／`unimplemented!`（`assert!`／`assert_eq!`／`debug_assert!` 按指示不加，它们是合法的"先校验后使用"守卫，会把基线冲垮）。含 `allow` 子串就跳过整行的绕过已关闭，改为 `lint_suppressed()`：只有同行或紧邻上一行的 `#[allow(...)]`／`#[expect(...)]`、且点名 panic 族 lint（`unwrap_used`、`expect_used`、`panic`、`unreachable`、`todo`、`unimplemented`、`indexing_slicing`）才抑制，比原任务描述更严——`#[allow(dead_code)]` 不再挡住 `unwrap`。这两项当天在 `crates/`＋`apps/` 里各命中 0 处（树里 36 处 `#[allow]` 全是 `clippy::too_many_arguments`／`dead_code`，没有一处压在 panic 行上或紧邻其上一行），属关闭绕过而不是清点违规。
- **文件系统扫描扩面**：`FS_RE` 补上 `OpenOptions::new`、`File::open`／`create`／`open_options`、`read_dir`、`DirBuilder`、`remove_dir[_all]`、`hard_link`、`symlink`／`symlink_file`／`symlink_dir`／`symlink_metadata`、`canonicalize`、`read_link`、`std::process::Command`／`Command::new`、`env::set_var`／`remove_var`；模式匹配调用与 import 形式，所以文档注释里的散文（例如"dangling symlinks"）不会误触白名单。`"tests/" in shown` 的子串判定换成 `is_test_path()`（`tests` 路径分量，或文件名 `tests.rs`）与 `is_example_or_bin()`（`examples` 分量或 `src/bin/`）；文件系统门禁此前根本没有 `tests.rs` 规则，只有 panic 门禁有。另外 `rust_files()` 改为排序（错误顺序确定），文件系统错误带 `file:line: code`，`panic_sites()` 按 `"\n"` 切分（已核对 `crates/`＋`apps/` 没有 `\r`、`\v`、`\f`、`\x1c`–`\x1e`、`\x85`、`\u2028`、`\u2029`，行号不变）。
- **唯一的新违规者，待批准**：加固后的扫描在 main `dd5d0dd` 上只多出一个真实的文件系统违规者 `apps/joyword/src/linux_files.rs`（第 11 行 `use std::process::Command;`、第 198 行 `Command::new(&argv[0])`）。它是 GUI 应用里的无条件生产代码（`mod linux_files;`）：`run_program()` 为 zenity／qarma／kdialog 里第一个已安装的助手拼 argv，`run_first_dialog()` 对着 `PATH` 解析，然后用 `std::process::Command` 派生并读取子进程的 stdout／stderr。理由是 gpui-pre-linux 0.3.9 的 XDG portal 提示在 UKUI/X11 上可能挂住，所以该模块在窗口内问路径；与已在白名单里的 `apps/joyword/src/input_ui.rs`（派生进程打开 URL）同类，且不能移进 `joyword-shell`，因为该 crate 不得依赖 GPUI。T111 的评审没有看到它，是因为旧 `FS_RE` 不覆盖 `process::Command`，而该文件的 `std::fs::remove_dir_all` 全在 `#[cfg(test)]` 模块里（标记在第 240 行，晚于两处违规行，所以这一发现纯粹来自扩面后的正则，与截断修复无关）。协调者已把它加进 `FS_ALLOW` 并在 `scripts/check_gates.py` 的白名单注释里记录上述理由。**这条白名单新增仍待项目负责人批准。**
- **反例与测试**：`scripts/tests/fixtures/` 新增 4 个反例夹具（层、panic 的 `allow` 绕过、被 `cfg(test)` 隐藏的文件系统访问、`tests` 路径的相似拼写），`scripts/tests/test_check_gates.py` 新增 38 个测试。加固后 `python -m unittest scripts.tests.test_check_gates` 38 项全通过；同一套测试对着加固前的脚本（`git show dd5d0dd:scripts/check_gates.py`）是 failures=14、errors=14，即 38 项里 28 项失败。`.github/workflows/ci.yml` 未改：第 48 行的 unittest 发现自动纳入新测试，第 46 行的旧反例调用不变，旧反例仍触发层、文件系统与 panic 三项。加固分支上 `python scripts/check_gates.py` 输出 `gates ok (216 rust files)`（该分支基于 main `dd5d0dd`，不含本次波次的代码）。
- **有意不改**：`FS_ALLOW` 只加上面一条且待批准；`panic_sites()` 仍扫描 `#[cfg(test)]` 模块体（T111 已把这些位点计入基线，排除会把基线砍半并削弱门禁，属另一次策略决定）；`check_panic()` 仍是单向的（基线可以比树里多，这是基线存在的漂移容忍），只清掉当天已经陈旧的 12 条，并用 `PanicGate.test_repo_baseline_has_no_entry_for_code_that_is_gone` 钉住"不得再有陈旧条目"；`examples/` 与 `src/bin/` 仍在文件系统门禁之外（既有行为，保留：`crates/joyword-corpus/src/bin/generate_corpus.rs` 与 `crates/joyword-model/examples/text_storage.rs` 因此不受白名单约束）。
- **文档跟进**：#86 已作为 `74086bb` 合入 `main`。本次收尾在 [ARCHITECTURE §1](ARCHITECTURE.md#1-模块与依赖方向) 补充了 `DEV_LAYERS` 的独立审阅规则，并明确测试例外不放宽普通依赖或 build 依赖。原六特性集成分支没有包含 #86，历史验证仍区分旧门禁、加固门禁交叉扫描和含 #86 的模拟树。

### 2026-10-22 并行波次集成复验（六分支）

集成分支 `chore/integration-wave-20261022`，基线 `main` = `dd5d0dd`。六个 PR 以 squash
方式按依赖顺序合入，全部零冲突：#85 `c8155ed` → #84 `ddc5985` → #81 `0b7877a` →
#83 `5e9ddd7` → #82 `043c1e1` → 编号+分栏复合样例 `19531ad` → #87 `941e1cd` →
三特性复合样例 `55f2e2e`；随后 cherry-pick 文档证据 `bfe13f1`。代码部分合计 43 文件、
+6947/−89，含文档后为 50 文件、+7270/−111。

**变更文件覆盖**：脚本比对六个分支触及文件的并集与集成分支的变更文件集，并集 42 个文件
全部出现在集成分支的 43 个变更文件中，`comm -23` 为空说明没有遗漏变更文件名，
不证明文件内部的全部修改都被保留；内容语义另由 diff 审查和测试验证。多出的 1 个文件是
集成阶段新增的 `crates/joyword-docx/tests/composite_numbering_columns_revisions.rs`。

**门禁陷阱复核**：`export::check_envelope`（`crates/joyword-docx/src/export.rs:760`）不再
比较 numbering store，改由 `export::numbering` 单独对齐（该处 784–786 行留有说明注释）。
若合并把 `before.numbering() != after.numbering()` 恢复进 `check_envelope`，追加的编号定义
会在信封层被拒绝、永远到不了写回逻辑，T200 会被静默禁用。合并后逐次确认该比较仍不存在；
同时 T301 的 6 处 `after_unresolved` 拒绝路径与其共存，未被覆盖。

**完整 CI 复现**（Windows 11、Rust 1.98.1、独立 `CARGO_TARGET_DIR`、串行、失败继续，
覆盖 `.github/workflows/ci.yml` build 作业在本机可执行的全部命令）：22 步全绿。

| 步骤 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo test --locked` | 884 passed / 0 failed |
| `cargo test -p joyword-agent --locked --offline` | 30 passed |
| `cargo test -p joyword-shell --features ai --test ai_toggle` | 1 passed |
| `cargo check --no-default-features`（model/engine/docx/layout/shell） | exit 0 |
| `cargo clippy` agent / shell+ai / `--workspace --all-targets -D warnings` | 0 / 0 / 0 |
| `cargo test --locked -p joyword` | 64 passed |
| `cargo build -p joyword --locked` | exit 0 |
| recovery-journal test / fmt / clippy | 15 passed / 0 / 0 |
| docx-robustness test / fmt / clippy / python / build | 3 passed / 0 / 0 / 26 tests OK / 0 |
| Rust 合计 | **997 passed / 0 failed** |

`cargo test --workspace --locked`（**非 CI 命令**，Review 3.8 建议纳入）在集成分支上
exit 0、**948 passed / 0 failed**、116 个测试二进制全绿，恰好等于 `cargo test --locked`
的 884 加 `cargo test -p joyword` 的 64，即 `--workspace` 口径覆盖了默认成员外加
`apps/joyword`。#82 新增的 `inspect_json_keys_stay_sorted_whatever_the_feature_set`
在该口径下实际运行并通过，说明六个 PR 全部在场时 CLI 金样仍然与 cargo feature 归一化无关
（gpui-pre 经 `apps/joyword` 启用 `serde_json/preserve_order` 的那条路径已被锁住）。

W09 健壮性实验（`driver.py`，`--seed 20261019 --iterations 200 --seconds 60
--max-corpus 128`）：

- smoke：`requested_runs 81`、`executed 81`、`43 ok / 38 rejected`、`matrix_complete true`。
- fuzz：`executed 200`、`46 ok / 154 rejected`、`features_discovered 42`、
  `novelty_events 18`、`corpus_size 45/128`、`network none`。
- 两者 `harness_git_sha 55f2e2ed`、`harness_worktree_dirty false`。
- 首跑 S12f/S12g 返回 64，原因是 driver 用 `out.mkdir(exist_ok=False)` 拒绝覆盖上一次
  留存的证据目录（`main` 上同一行为）；把五分支的 `ci-smoke`/`ci-fuzz` 改名保留为
  `ci-smoke-five-branches`/`ci-fuzz-five-branches` 后重跑，两步 exit 0。这是本机复跑
  的环境残留，不是代码缺陷，CI 每次全新检出不会遇到。

**Python 门禁**（文档证据并入后在集成分支最终树上复跑）：`check_docs.py` OK
（84 requirements、66 tasks、依赖无环、阶段一致、
38 Markdown、15 registered cases、locale parity、12 workflow actions 按 SHA 固定、
65 behavior registry entries、feature catalog floor 59）；`check_gates.py` OK
（228 rust files）；`check_gates.py --counterexample` 三条全部命中（跨层依赖、
allowlist 外文件系统访问、新 panic 点）；`check_acceptance.py` OK（**9** checklists，
新增 `acceptance/T301.md`；legend 状态合法、**所引用的测试名全部真实存在**）。

`scripts/tests` 单测在集成分支上 69 个中 1 个失败（在含 #86 的模拟树上是 107 个中同 1 个
失败）：`test_committed_bytes_and_manifest_hashes_match`。
本机 Python 3.14.7 链接 zlib-ng 1.3.1，其 level-6 deflate 输出与 CI 的 stock zlib 不同，
导致 5 个 fixture 的“新生成字节”与已提交字节哈希不一致（解压成员与 CRC 相同）。
同一 5 个 fixture（T200-NUM、T201-TABLE、T202-PAGES、T203-IMAGES、T300-REVIEW）在干净
`main` 上以**完全相同的新生成哈希**失败，因此不是本波次引入，属已知本机环境差异。

**用 #86 加固后的门禁交叉扫描集成树**：`cross_check_gates.py gates/scripts .` →
`HARDENED GATES: clean`（228 rust files）。即集成树同时通过旧门禁与真正会执行的加固门禁。
六个分支**各自单独**也用加固门禁扫过（t110 216、t202 219、t200num 218、t401 220、
review 216、t301 218 个 rust 文件），全部 clean，因此这七个 PR（含 #86）以任意顺序并入
`main` 都不会被门禁挡住。

**三特性复合样例**：`crates/joyword-docx/tests/composite_numbering_columns_revisions.rs`，
集成分支上 5 tests passed。追加的 bullet 编号定义、`w:cols/@w:sep` 栏分隔线与未解决的
`w:pPrChange`/`w:rPrChange` 出现在同一个导出计划里：恢复编辑前的模型快照后，栏分隔线与
numbering part 保持原样；带未解决修订的段落再被编号时按 `ExportError::Unsupported` 拒绝；
接受修订后导出干净格式（无 `pPrChange`/`rPrChange`、`w:jc w:val="both"`、
无 `w:jc w:val="center"`、有 `<w:b/>`、无 `<w:i/>`）。这些是模型与格式层组合测试，
不经过 engine 撤销命令；收尾将原来的 undo 命名改为恢复模型快照，避免夸大覆盖范围。

**Review 阶段一风险清单（`REVIEW-REPORT.md` 第四节）逐条跟进**：

- R1（高，export.rs 三分支语义汇聚）：`joyword-docx` 全量 159 个测试通过（含 `import` 36、
  `w05_structure` 18、`tables` 12、`w12_structure` 11、`format_revisions` 10、`hyperlinks` 10、
  `images` 10、`header_footer_writeback` 8、`comments` 7、`numbering_writeback` 6、
  `column_separator` 5、复合样例 5、`w10_origin` 4、`revisions` 3、lib 单测 14）；
  复合测试对导出 XML 逐项断言，提供了可重复的自动证据。清单要求的“复合样本人工抽查”
  仍未执行，不能用这些断言替代 Word／WPS 或 GUI 人工验收。
- R2（高，TASKS.md 汇总行计数）：`check_docs.py` 严格校验每行计数与状态一致，当前 OK；
  文档并入后须再跑一次。
- R3（中，层门禁对 `foo.workspace = true` 失明）：已由 #86 修复，并用加固后的脚本交叉
  扫描集成树，结果 clean。
- R4（中，panic 门禁 `allow` 子串逃逸）：已由 #86 修复；panic 基线双向漂移
  （12 条陈旧 + 19 条被遮蔽的多余项）在 #86 中一并纠正。
- R5（中，docx-robustness 要求 `crates/` 干净）：smoke 与 fuzz 的报告均记
  `harness_worktree_dirty: false`。
- R6（低，fixture 测试对 zlib 实现敏感）：见上文，已用“干净 main 上同 5 个 fixture、
  同新生成哈希”证实为本机环境差异。
- R7（低，本机无 cargo-deny）：本机确实缺失，须由 CI `dependencies` 作业确认。降低风险的
  事实是六个 PR **未改动 `Cargo.lock` 与 `deny.toml`**，唯一的 `Cargo.toml` 变更是
  `apps/joyword-cli` 的 `[dev-dependencies]` 增加 `serde_json = { version = "=1.0.154",
  features = ["preserve_order"] }`（精确固定、非升级；`indexmap 2.14.2` 在 main 的锁文件中
  已存在），依赖图没有新增 crate，许可/公告/banned 检查的对象与 main 相同。
- R8（提示，CLI 测试构建从此固定带 `preserve_order`）：属设计意图，已随 #82 合入。

**Review 阶段一预测的冲突点（`REVIEW-REPORT.md` 3.7）实际结果**：预测为“必然冲突”的
`TASKS.md` 与 `IMPLEMENTATION.md` 一次都没有冲突——派工时就把这两个文件连同
`feature_catalog.json`、`acceptance/*` 划为所有开发 Agent 的禁区，只允许 t301 追加
`behavior_registry.json`，文档证据统一由集成阶段并入，冲突被设计掉了而不是被解决掉。
预测为“高”的 `export.rs` 三分支汇聚（t200num / t202 / t301）实际落在互不重叠的行区间
（t200num 6–561 与 ~781，t301 828–1281），文本零冲突；语义汇聚由上面的复合样例覆盖。
`Cargo.lock` 预测“中”，实际六个 PR 一个字节都没动它。建议的合并顺序是
t110 → t401 → t200num/t202 → t301 最后；实际集成分支按各分支完成先后合入
（#85 → #84 → #81 → #83 → #82 → #87），与建议顺序不同，但两者都把足迹最大的 t301 放在
最后，且七步全零冲突。下文“逐个合并等价性模拟”用的正是建议顺序
（#86 → #81 → #82 → #83 → #84 → #85 → #87），同样零冲突；这为当前变更的合并路线
提供了证据，不代表任意后续变更或任意顺序都没有风险。

**“逐个合并七个 PR”等价性模拟**：在 `main`（`dd5d0dd`）的独立 worktree 上按
#86 → #81 → #82 → #83 → #84 → #85 → #87 顺序逐个 `git merge --squash` 并各自成一次提交，
七步全部零冲突。代码与门禁范围的比较显示两组预期差异：#86 的门禁加固
（`GATES-REPORT.md`、`scripts/check_gates.py`、`scripts/panic_baseline.txt`、4 个 fixture、
`scripts/tests/test_check_gates.py`）只在模拟树上，集成阶段的复合测试
`composite_numbering_columns_revisions.rs` 只在集成分支上；`crates/` 与 `apps/` 除此之外
逐字节相同。两者不是完整仓库树的字节等价证明：集成分支还承载了单独整理的文档证据。
集成测试可作为共同代码的验证依据，但不能替代实际合并后含门禁与收尾测试的最终树 CI。

在模拟树上直接用 #86 加固后的真实脚本复跑：`check_gates.py` → `gates ok (227 rust files)`；
`--counterexample` → 四条发现全部命中（跨层依赖、allowlist 外文件系统访问、新 panic 点，
以及加固后新增的“出现次数与基线不符”）；`check_docs.py` OK；`check_acceptance.py` OK；
`scripts/tests` 107 个测试（#86 新增 38 个门禁测试全绿）中仍只有
`test_committed_bytes_and_manifest_hashes_match` 一项失败，即上文的本机 zlib-ng 差异。

**七个 PR 的 GitHub CI 状态**（2026-10-22 复核）：#81、#82、#83、#84、#85、#86、#87
每个 18/18 检查项 pass（build 矩阵 macos-15 / windows-2022 / ubuntu-24.04 /
ubuntu-24.04-arm、cross-linux aarch64 与 loongarch64 QEMU、unified-pdf、dependencies、
provenance），且均为 `MERGEABLE / CLEAN`。本机缺 cargo-deny，`dependencies` 作业以 CI 结果为准。

**验证边界（明确标注未验证项）**：本机复现只覆盖 `build` 作业的命令行部分，且只在
Windows 11 上跑。`cross-linux`（aarch64 / loongarch64 QEMU 用户态）、`unified-pdf`
（Noto CJK + poppler，仅 Linux）、`dependencies`（`cargo deny`，本机未安装）、`provenance`
（需 github.com 与 static.crates.io）以及 macos-15 / ubuntu-24.04 / ubuntu-24.04-arm 三个
runner，本机都无法复现。

**六特性集成树的 CI**：`.github/workflows/ci.yml` 的 `on: push` 没有分支过滤，
所以推送集成分支可触发完整 CI，而不需要先合并到主干。
run [39793719141](https://github.com/willmove/joyoffice/actions/runs/39793719141)
（`push` 事件，sha `c714832`）结果 **completed / success**，9 个作业全绿：
`build (macos-15)`、`build (windows-2022)`、`build (ubuntu-24.04)`、
`build (ubuntu-24.04-arm)`、`headless tests (aarch64-unknown-linux-gnu, QEMU)`、
`headless tests (loongarch64-unknown-linux-gnu, QEMU)`、`unified-pdf`、`dependencies`、
`provenance`。该树包含六个实现 PR、复合测试与文档，不含 #86 的门禁加固。
R7（本机无 cargo-deny）的集成分支验证缺口由 `dependencies` 成功覆盖；
`unified-pdf` 也覆盖了 T202 新加的 `crates/joyword-pdf/tests/column_separator.rs`
在 Noto CJK 下的场景读回。随后仅更新证据文档的 `673a84f` 对应
run [39798316908](https://github.com/willmove/joyoffice/actions/runs/39798316908) 同样成功。
这些结果不能被记为七个 PR 实际合入 `main` 后的最终树 CI；后者须单独核验。

上述自动证据不覆盖 GPU 窗口、IME、无障碍、真实打印与系统打印对话框、
Word／WPS 往返及信创真机。仅信创真机缺环境按项目约定不阻塞开发、依赖、阶段验收或发布；
其余人工项继续按各任务验收要求挂账，不能一概视为不阻塞，也不能据此标记任务完成。

**合并授权**：用户已明确授权协调者按 #86 → #81 → #82 → #83 → #84 → #85 → #87
顺序合并，要求各分支先包含当时最新 `main` 并通过 CI，随后单独提交复合测试和文档收尾。
这项授权不将人工验收记为完成，也不替代最终合并状态与实际提交的核验。

**文档并入方式**：六个开发 Agent 与 review Agent 一律被禁止改动 `TASKS.md`、
`IMPLEMENTATION.md`、`feature_catalog.json` 与 `acceptance/*`（只有 t301 获准追加
`behavior_registry.json`），证据统一由一次文档切片并入：分支 `docs/wave-evidence-20261022`
的 `a523a28` cherry-pick 为集成分支的 `bfe13f1`，7 文件 +177/−22，只动 `docs/joyword/**`。
T301 与 T401 由 `planned` 改 `in_progress`，进度表改为 12 done／21 in_progress／33 planned／
0 blocked、总数仍 66；**没有任何任务改为 `done`**，因为这一波全部是自动证据。
`acceptance/T301.md` 新建并登记进 `acceptance/README.md`，T200／T202／T300 的清单按新测试
更新，其中 T202 的栏分隔线由 `not-implemented` 改 `partial`。

**待项目负责人裁决的三项**：（1）`apps/joyword/src/linux_files.rs` 进 `FS_ALLOW`
是否追认（理由与先例见上文 T111 小节，另一条路是把该派生进程当设计缺陷重构掉）；
（2）`GATES-REPORT.md` 目前提交在仓库根目录，是留在根、移到 `docs/joyword/verification/`，
还是折进本文档后删除；（3）T111 的 `done` 是否重开——本次加固证明当初人工评审所依据的
三道门禁中，层检查几乎未读取内部依赖、`#[cfg(test)]` 截断在 44 个文件里丢掉了生产代码、
含 `allow` 子串的行整个跳过 panic 检查，评审记录的强度高于门禁当时真正能抓到的东西。
文档切片建议重开或至少补一次针对加固后门禁的复审，本节只记录该意见，未改动 T111 状态。

### T401 合并前授权加固（2026-10-23）

第二波提案契约预审发现并核实了第一波授权缺陷，协调者在 #83 合并前暂停队列，
修复以 `7a453a2` 推送到原 PR。范围为 `joyword-agent` 的四个实现文件与一个集成测试文件；
没有新增依赖、没有修改门禁或默认关闭策略，也没有提前实现 T402 提交接口。

- **公开读取入口**：分发与直接调用共同检查对应工具白名单。未授权读取和检索在文档端口访问之前拒绝，未授权搜索也先于空查询等参数错误拒绝。
- **快照前后状态**：工具、上下文构建以及 Provider 请求前后的快照读取共用内部校验；端口调用期间发生停止、停用或重启时，读取结果或响应被拒绝。测试覆盖这三种状态变化，不把此项描述成真实网络或宿主接线验收。
- **坐标与版本**：段间换行按段索引插入，保留前导、连续和末尾空段及非 BMP 字符的标量偏移。内存假文档追加段落前检查全文预算与版本上限，成功才递增版本，失败保持原状态。
- **身份与期限**：签发授权时先核对端口返回的文档 ID；时钟原子饱和，期限加法溢出拒绝，到期刻本身即过期，防止计数回绕使旧授权复活。
- **回归证据**：开发 Agent 报告新增 17 个回归在修前为 15 失败、2 通过。修后协调者使用独立 target 复跑 `cargo test --locked -p joyword-agent`，实际 47 项通过（7 单测、18 既有契约、22 受限工具）；`cargo fmt --all -- --check`、该 crate 的全目标 clippy（`-D warnings`）、加固门禁（220 个 Rust 文件）和 `git diff --check` 均通过。本机结果不替代该提交的跨平台 CI。

T401 仍为 `in_progress`。真实 shell 投影、GUI、网络与人工选区验收不在此补丁内；
FNV 指纹仍不抗恶意构造碰撞。快照类型没有独立隐藏字段，并不自动保证未来宿主投影不会误混入隐藏字符串。

### 收尾复合回归预验（2026-10-23）

收尾测试文件 [composite_numbering_columns_revisions.rs](../../crates/joyword-docx/tests/composite_numbering_columns_revisions.rs)
由 5 项增为 6 项。新增拒绝格式修订与编号追加、栏分隔线同处一个导出计划的用例；
接受和拒绝两条路径均重导入并核对段落对齐、粗体／斜体、编号归属与模型语义相等。
拒绝路径另核对原编号部件除追加定义外逐字节不变、两栏和分隔线保留；收尾独立审查后，
接受路径也补齐编号部件字节守恒断言，避免未知 XML 变化仅凭模型语义比较漏检。
对含未决修订段落应用编号的拒绝用例同时断言目标节点和原因，避免只命中不相关错误。
原 undo 命名已改为 `restoring_the_model_snapshot_preserves_the_separator_and_numbering_part`，
明确这是模型快照恢复后导出测试，不是 engine 撤销测试。

提前验证将增强后的测试文件放进历史六特性集成树 `673a84f` 的独立工作区，使用独立 target：
复合测试 6/6、完整 `joyword-docx` 160/160、该 crate 全目标 clippy（`-D warnings`）、
测试文件 rustfmt 与 `git diff --check` 通过。此树不含 #86 门禁及 `7a453a2` 授权加固，
因此这里只登记预验，不能替代实际合并后完整树的复验与 CI。

### 第一波实际主干合并（2026-10-23 核验）

按用户授权顺序完成下列 squash 合并；每个待合分支先纳入当时最新 main，再核验当前 head
的全部检查成功且可合并，并以精确 head 防竞争。没有强推、跳过检查或使用管理员绕过。

| 顺序 | PR | main 合并提交 |
| --- | --- | --- |
| 1 | [#86 门禁](https://github.com/willmove/joyoffice/pull/86) | `74086bb` |
| 2 | [#81 卷能力](https://github.com/willmove/joyoffice/pull/81) | `81abf63` |
| 3 | [#82 CLI 键序](https://github.com/willmove/joyoffice/pull/82) | `523a030` |
| 4 | [#83 选区授权](https://github.com/willmove/joyoffice/pull/83) | `0ba78b4` |
| 5 | [#84 栏分隔线](https://github.com/willmove/joyoffice/pull/84) | `f059278` |
| 6 | [#85 编号写回](https://github.com/willmove/joyoffice/pull/85) | `9718399` |
| 7 | [#87 格式修订](https://github.com/willmove/joyoffice/pull/87) | `85a24de` |

#83 含合并前的 `7a453a2` 授权加固，因而实际主干与早先的六特性集成树有预期代码差异。
[#83 PR 检查](https://github.com/willmove/joyoffice/actions/runs/39811714996) 的 LoongArch
首次在交叉工具链安装阶段超时，尚未运行 Rust 测试；定向重跑后成功。
#84 push 的 dependencies 状态异常也在定向重跑后恢复，未以失败或取消状态合并。

完整主干 `85a24defe724d7d870df32d6b5a0a3a4bb0ed2d7` 的
[push CI 39831168359](https://github.com/willmove/joyoffice/actions/runs/39831168359)
为 completed／success，四平台 build、两种 QEMU、unified-pdf、dependencies、provenance
共 9/9 成功。该运行的 LoongArch 首次工具链安装超时，第二次实际执行测试成功；
其他作业的成功记录一并保留。本地 main 已快进到同一 SHA，工作树干净。
这个 CI 包含七个 PR 及授权加固，但不包含本次收尾新增的复合测试和文档；收尾须独立验证。

历史分支、模拟工作区和本地报告均保留。TASKS 仍为 12 done／21 in_progress／33 planned，
没有把本轮自动通过当作 Word／WPS、GUI、打印、断电或信创真机人工验收完成。

### 收尾树本机复验（2026-10-23）

分支 `chore/wave1-closeout-20261022` 已快进到 `85a24de`，其上仅追加本次 8 份文档和
1 份复合测试文件的改动。Windows 本机使用该工作区独占的 target，串行完成：

- `cargo test --workspace --locked`：通过，包含应用适配器、授权加固与六项复合测试。
- `cargo test -p joyword-shell --features ai --test ai_toggle --locked`：通过。
- `cargo check -p joyword-engine -p joyword-model -p joyword-docx -p joyword-layout -p joyword-shell --no-default-features --locked`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`：通过。
- 独立只读审查无阻塞发现；接受修订路径补齐编号 XML 字节守恒断言后，单独复跑
  `cargo test -p joyword-docx --locked --test composite_numbering_columns_revisions`（6/6）、
  该 crate 全目标 clippy、fmt 与差异检查，均通过，不依赖此前运行是否已读取新断言。
- `check_docs.py`、`check_acceptance.py`（9 份清单）、`check_gates.py`（228 个 Rust 文件）、
  显式传入 `scripts/tests/fixtures/gate_counterexample.rs` 的反例检查，以及 38 项门禁单测均通过。
- Python 全测 107 项中仍有 1 项失败：`test_committed_bytes_and_manifest_hashes_match`，
  与上文已在干净 main 复现的 zlib-ng／stock zlib 压缩字节差异相同；未重生成夹具，未降低断言。

本节是收尾树的本机证据，不把前一节主干 CI 冒充收尾 PR 的 CI。收尾 PR 仍须通过自身的
跨平台检查后才合并；本轮没有新增 GUI、Word／WPS、打印、断电或信创真机人工验收。

### 收尾 PR 合并与第二波基线（2026-10-23）

上述收尾随后以 [PR #88](https://github.com/willmove/joyoffice/pull/88) 合并，提交为
`ec8c8070288b8e5cedfe0608dbccf7fa4475fdaf`。其 push CI
[39839238527](https://github.com/willmove/joyoffice/actions/runs/39839238527) 与 PR CI
[39839249585](https://github.com/willmove/joyoffice/actions/runs/39839249585) 均 9/9 成功；合并后
main CI [39843589125](https://github.com/willmove/joyoffice/actions/runs/39843589125) 也是 9/9 成功，
没有重跑、跳过检查或借用历史组合树的结果。协调者另行核对 PR 已合并、主工作区干净，
本地 main 与 origin/main 一致。

第二波四条开发分支及独立审查工作区统一从该 SHA 出发，分别处理 T104 生产恢复双代、
T204 PDF 书签内部跳转、T202 章节 PAGE、T402 有界替换提案与只读预览。每路独占源码范围和
构建 target，任务／验收证据由协调者集中维护。各切片独立审查及 CI 不替代四路组合树验证；
切片完成也不据此把整个 T104／T204／T202／T402 标为 done。

### T402 有界替换与只读预览切片（2026-10-23）

[PR #89](https://github.com/willmove/joyoffice/pull/89)，源码 `43d02b3`，仅修改
`crates/joyword-agent` 内的 7 个文件。`ReplaceRequest` 只允许替换整个非空授权选区；
`ReplaceProposal` 的原 Grant、runtime identity、替换文本与说明均私有。文本前后各最多
24 KiB、说明最多 4 KiB、ASCII ID 最多 128 字节；runtime 生命周期内最多 256 个 ID，
失败与预览不占号，满额拒绝而不淘汰。登记与旧提交 admission 表分离，不调用提交 hook，
不增加提交计数。每次 `preview_replace` 重新校验完整原授权与新快照，公开直调和 dispatch
都先检查工具白名单；预览不调用 Provider，也不改变文档。

独立审查在指定源码上完整审读 7 文件，用独占 target 复跑 `cargo test -p joyword-agent --locked`
（47 项既有、15 项新增、1 项 compile-fail）、该 crate 全目标 clippy（`-D warnings`）、fmt 与
差异检查，均通过且无阻塞发现。并发 duplicate 与串行容量边界已有测试；并发争抢最后槽位是
非阻塞补测建议，不把未执行的测试写成已有证据。授权失效前已拥有的预览是时点快照，宿主须
撤下显示并重新请求，不承诺追溯撤回已读取数据。截至本节记录，PR CI 和最终合并待完成。

本切片不含 engine 事务提交／撤销、AI 作者修订、DOCX 修订往返或真实宿主／Provider 接线；
旧 `ChangeProposal.apply` 仍返回延后实现。无新增 GUI、Word／WPS 或真机人工验收。

### 第二波并行切片与组合验证（2026-10-23）

第一波七项 PR 与收尾 PR（[#88](https://github.com/willmove/joyoffice/pull/88)，合并提交
`ec8c807`）之后，从同一 SHA 分出四路并行开发与一路独立审查。本节记录实际结果，
不含未执行的验收。

**T104 生产恢复双代（[#90](https://github.com/willmove/joyoffice/pull/90)，任务仍 `in_progress`）**：
两个发布槽 `recovery-a.jwr`／`recovery-b.jwr` 交替写入，每代带 `JWREC002` 头与 CRC32，
写入经临时文件、flush、sync、读回校验与原子改名后才发布，发布前一代始终保留。
回退规则：最新槽损坏时回退到另一槽，两槽同代内容不同或都损坏时拒绝，损坏槽不覆盖、
不重编号，墓碑式显式丢弃让旧请求失效。旧 `JWREC001` 单文件格式只读兼容，迁移期间
不改写旧文件。文档加密拒绝写明文副本，超预算输入不落文件。保存 N 完成时编辑记录 N+1
仍为 dirty，重启可恢复到 N+1。`joyword-shell` 263 项与应用 65 项测试通过。
独立审查在独占 target 上复跑同组测试与全目标 clippy、fmt，无阻塞发现。
macOS 上的一次失败经查为真实平台差异而非用例问题：BSD 与 macOS 的 `flock` 锁属于文件
而非描述符，关闭描述符不释放进程持有的锁，std 的 `File` 析构只关描述符、不发 `LOCK_UN`，
于是同一进程的下一次取锁被自己的旧锁拒绝（`WouldBlock`），而 Linux 与 Windows 随最后一个
句柄关闭即释放，所以只在该平台暴露。修法是显式 `unlock()` 的守卫类型，并新增
`a_released_lock_can_be_taken_again_by_the_same_process` 回归。
未做：图形界面、断电、真实磁盘满与信创真机验收；Windows 目录同步按实现记为 false。

**T204 PDF 书签内部跳转（[#92](https://github.com/willmove/joyoffice/pull/92)，任务仍 `in_progress`）**：
模型书签解析为 PDF 命名目标，锚点内链写成真实 `/S /GoTo` 动作与显式 `/D` 引用，
不用命名跳转。名称精确匹配且区分大小写；空名、控制字符名、重名、脱离锚点、
未布局位置、过期布局、非法几何与超预算都拒绝，名称永不被当作 URL。
坐标按 ISO 32000-1 §12.3.2、§12.5.6.5、§12.6.4.2 与 krilla 0.8.2、
pdf-writer 0.15.0 的锁定源码核对；krilla 接收 y 向下坐标并序列化为
`[page-ref /XYZ x (page-height-y) 0]`（缺省 zoom 序列化为 0，即保留当前缩放）。
应用侧导出复用屏幕已安装的同一批页面场景：选页导出时书签与链接的源页、目标页一起重编号，
源或目标不在选页内的链接被丢弃而不误跳。新增 14 项 `joyword-pdf` 导航测试，
含中文书签名读回与锚点缺失目标的回归。
独立审查在独占 target 上复跑 `joyword-pdf` 全套并用独立探针核对歧义判定、
UTF-8 名称预算、缺失与 cluster 内部端点拒绝、跨页源矩形、边界偏置、
同布局目标 caret 与非法字形不产生部分 PDF 字节。名称树键按 ISO 32000-1 §7.9.6
逐字节比较，检查器因此保留原始八位组并按 UTF-8 还原，修复了中文名读回乱码。
未做：外部 PDF 结构读取器复核（本机无 pypdf／pikepdf／qpdf，只有 pdftotext 可用）、
DOCX 书签导入、PDF 结构标签、图形界面与 Word／WPS 对照。

**T202 章节 PAGE 编号（[#91](https://github.com/willmove/joyoffice/pull/91)，已合并，任务仍 `in_progress`）**：
见 [TASKS](TASKS.md) 的 T202 条目。独立审查复跑 426 项与 918 项测试、全目标 clippy
与 fmt，无阻塞发现；选章规则与 Word 的差异、跨页拒绝、多级 restart 未覆盖等
如实登记为已声明假定与未覆盖项。该 PR 的两套 CI 各 9/9 成功，已按精确 head 合并入主干（合并提交 `2f183d8`）。

**T402 授权替换提案与只读预览（[#89](https://github.com/willmove/joyoffice/pull/89)，已合并）**：
见上文「收尾 PR 合并与第二波基线」小节。该切片已合入主干 `6c43799`。

**T104 验收清单**：2026-10-23 新增 `docs/joyword/acceptance/T104.md`，把生产恢复双代切片拆成原子准则，引用真实存在的测试名；独立撤销栈、日志校验与 undo／redo 属性测试仍标为 not-implemented，人工验收一条未做。
**组合验证**：四路精确提交合入同一验证树后，本机以该树独占 target 串行完成
特性场景 PDF 与基线逐字节相同：用同一字体生成 `crates/joyword-cli` 的 `feature_scenes` 十二个样例，与第二波之前的 `main` 逐文件比较，十二个 PDF 全部字节相同。这说明四路切片没有改变既有 PDF 输出。本机 `pdftotext` 提取到的文本很短（每个样例 7 至 22 字节），是本机 CJK 字体提取的限制，在基线上同样如此，不是本波回归。

`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets --locked -- -D warnings`
与 `cargo fmt --all -- --check`，全部通过。新增跨切片回归覆盖章节前缀与书签在同一次
布局中的一致、选页导出与全量导出字节相同、锚点无目标时不阻断导出、
无书签文档的导出字节与既有路径相同。人工验收、Word／WPS 对照、双阅读器与信创真机未做。

### T402 授权替换的单事务接受（2026-10-23）

[PR #94](https://github.com/willmove/joyoffice/pull/94)。#89 的 `propose_replace`／`preview_replace` 保持只读。本切片只加接受路径，T402 状态仍是 `in_progress`，进度计数不改。

接受发生在 `joyword-agent` 的 `ToolSession::accept_replace`。生产 crate 继续不依赖 engine、model 或网络；宿主实现 `DocumentCommit`（`&self`，可与会话端口是同一对象），在 `authorize` 回调成功之后才改文档。engine 导出 `apply_ai_text_replace`：一次 `Origin::Agent`、`UndoGroup::Standalone` 事务，`tracked_author` 为 `JoyWord AI`。该作者与会话用户作者分开，也不与用户输入合并撤销。文档的 `trackRevisions` 可以仍是关闭；这两处作者常量分属两个 crate，集成测试断言字符串相同。模型没有新字段，沿用已有 `TextRevision.author`。普通 `Origin::Agent` 事务在未设置 `tracked_author` 时仍不产生修订。

ID 在重读文档之前占用。已接受或正在接受返回 `AlreadyApplied` 且零写入；重新校验失败则退回 `Proposed`，同一次提案可以再试一次，成功后不能再试。`authorize` 是写入前的线性化点：门控已关闭则零写入；写入已经完成之后关闭，该次提交保留并可撤销。`allowed_commits` 与 Provider 计数不变；`applied_replacements` 只记录成功接受的次数。诊断的 `kind()` 与 `Debug` 只报告长度、版本和静态标签。

坐标与 #89 的快照相同：正文最终视图按段落用 `\n` 连接。替换必须落在同一段落的一段连续可见标量上。空替换是一次跟踪删除。命令是一条 `DeleteRange`，非空时再加一条 `InsertText`，同属一个事务。

本机证据：`cargo test -p joyword-agent --locked --offline` 74 项（库内 7、contract 18、scoped 22、预览 15、接受 11、compile-fail 1）；`cargo test -p joyword-engine --locked --offline` 94 项（其中 `tests/ai_replace.rs` 6 项）；`cargo clippy -p joyword-agent --all-targets --locked --offline -- -D warnings` 与 engine 同口径 clippy；`cargo fmt --all -- --check`；`python3 scripts/check_docs.py`；`python3 scripts/check_gates.py`（236 个 Rust 文件）。未跑整工作区 `cargo test --locked`，也未跑 shell。

仍留给后续切片：宿主界面接线、真实 Provider、DOCX／Word 修订往返、AIGC 文件级标识（`aigc::write_label` 仍是评估未完成）、旧 `ChangeProposal::apply`、跨段范围、替换文本里的控制字符（制表符除外）、行分隔符与对象标记、最终视图不连续的选区（中间有隐藏修订或行内对象）、正文里的非段落块，以及人工验收。预览仍允许展示这些被接受路径拒绝的文本，因为预览不落盘。

### T402 跨段替换与段落标记修订（2026-10-23 第三切片）

无 PR，按用户指令直接并入 main（分支 `feature/t402-cross-paragraph-docx-20261023`）。在 #94 的单事务接受之上补齐 T402 两个明确未做项：跨段有界替换与 AI 修订的 DOCX 往返。T402 状态仍是 `in_progress`，进度计数不改（12 done／22 in_progress／32 planned）。

**模型**：`joyword-model` 的 `Paragraph` 新增 `mark_revision: Option<TextRevision>`——段落标记自身的跟踪修订（`w:pPr/w:rPr/w:del`，§17.13.5.12 与 §17.13.5.14 的空元素形态）。只有 `RevisionKind::Delete` 会被生产代码写入；`w:ins` 标记在导入侧保持保留 gap。`split_off` 把标记修订随段落标记移到尾半段；`append` 不触碰标记（接受路径显式继承被吸收段的标记）。`canonical_dump` 的段落头新增 `markdel=…#id author date`，因此 `semantic_difference` 能看见标记修订，不再可能被静默丢掉。

**engine 跨段跟踪删除**：`revision::delete_tracked` 不再把不同段落一律拒为 `RevisionBoundary`。同一容器内的跨段 `DeleteRange` 现在按段规划：首段后缀、中间整段、末段前缀全部标 Delete（沿用 `plan_tracked_delete` 的作者规则：自己的 Insert 物理移除、自己的 Delete 复用同一 id、他人内容拒绝），并把首段到末段前一段的段落标记逐个盖上 Delete 修订；整条命令共享一个修订 id，与既有单段语义一致。段落本身保留，接受时才物理合并——`resolve_marks` 的接受把段与下一兄弟段合并（合并段保留首段身份与属性，继承被吸收段自己的待决标记，同一 id 的链式标记一次收掉），拒绝清除标记、换行回来。`AcceptAllRevisions`／`RejectAllRevisions` 全量处理；`AcceptRevision` 对不可合并的标记（容器末尾，只有导入可造）整条拒绝而非猜测。容器边界（表格、内容控件、跨单元格）仍拒绝：`RevisionBoundary` 只剩表格／内容控件语义，跨故事改报 `CrossContainer`。`current_revision` 在段尾光标处能看到待决标记修订。用户路径因此也解除 T301 的单段限制（有自动测试），AI 路径与用户路径同一套规则。

**AI 跨段替换**：`apply_ai_text_replace` 的 `locate` 重写为两阶段：先在"段落最终文本按 `\n` 连接、已删除标记的段落间不加分隔符"的投影上把全局范围映射到首末两个位置（起点可落在分隔符上＝纯标记删除、终点可在分隔符后＝末段空跨度），再按是否跨界分别校验——单段要求一段连续可见标量；跨界要求首段后缀连续、中间段无行内对象、末段前缀从位置 0 连续。隐藏的 Delete／MoveFrom 修订是被结构性覆盖的对象：自己的复用，他作者的保留不动（自动测试固定该行为），真正拒绝的只有行内对象（修订导出本就拒绝带行内对象）。替换文本仍必须是单行。`body_final_paragraphs` 改为按删除标记把相邻段合并成组，与 agent 快照消费方一致；后续第二次替换在合并后的投影上进行（有测试）。

**agent**：`accept` 的 `single_paragraph` 放宽为"非空且不越过最终视图文本末尾"；结构拒绝仍由宿主/engine 报告。内存宿主 `History` 的 `commit_replace` 改为对整篇最终视图拼接后裁剪再按 `\n` 拆回：覆盖组塌缩为一段、保留首段标题级别、聚合被覆盖段的隐藏内容（对应 engine 合并段保留 runs），未触碰段逐字保留。engine 宿主无改动（`apply_ai_text_replace` 直接支持全局坐标）。

**DOCX 往返**：导入侧 `w:pPr/w:rPr/w:del`（恰好 id／author／可选 date、无子元素）经 `run_with_revision` 捕获并类型化，文档不因此锁定；`w:ins`、move 标记、多修订、带子元素或缺 id 仍是 gap＋整体保留（测试逐项固定）。导出侧 `patch_paragraph` 把"标记修订发生变化"并入 revised 判定；`rewrite_revisions` 复制原始 `w:pPr` 字节后注入 `<w:rPr><w:del …/></w:rPr>`（无 pPr 时合成整个 pPr；已有 rPr 时作为其首个子元素注入；自闭合标签展开；无 rPr 时按 CT_PPr 顺序插在 `w:sectPr`／`w:pPrChange` 之前），模型已解决时从原字节剥离。导入时已类型化且未变化的标记保持原始字节不动。样式改写与属性重写两条会合成 pPr 的路径在有待决标记时拒绝（与未解决格式修订同一守卫风格）。对照器 `compare.rs` 的 `rebuild` 现在携带 run 修订、格式修订、`props_revision` 与 `mark_revision`，关闭了"往返校验把修订差异规范化掉"的既有漏洞（有反例测试）。`joyword-docx` 新增 dev 依赖 `joyword-engine`（登记进 `check_gates.py` 的 `DEV_LAYERS`，注释写明用途），使测试可以：导入→engine 会话执行一次真实 AI 跨段替换→导出→重导入→断言语义一致、作者＝`JoyWord AI`、删除标记与删除／插入修订齐全、最终视图投影一致。

**已知边界（如实记录）**：布局仍逐段排版——最终视图里已删除标记的相邻段在屏幕上渲染为独立行（含空行），引擎模型、投影与 DOCX 都是精确的，合并渲染留待后续切片（AI 路径尚未接入 shell 界面，用户路径在开启修订时跨段删除会触发该渲染近似）；`w:ins` 标记修订不类型化；替换文本不支持换行（需要插入标记机制）；跨容器（表格单元格之间、正文与单元格之间）的跟踪删除仍拒绝。

**本机证据（Windows 参考环境，分支 HEAD）**：`cargo test --locked --offline` 998 项全过（engine `ai_replace` 12 项＋`revisions` 11 项、agent `replace_commit` 13 项、docx `mark_revisions` 7 项均含新增）；`cargo clippy --workspace --all-targets --locked --offline -- -D warnings` 干净；`cargo fmt --all -- --check` 干净；`cargo check -p joyword-engine -p joyword-model -p joyword-docx -p joyword-layout -p joyword-shell -p joyword-agent --locked --offline --no-default-features` 通过（AI 仍默认关闭、`joyword-agent` 生产依赖为空）；`python scripts/check_gates.py` ok（237 个 Rust 文件）；`python scripts/check_docs.py`、`python scripts/check_acceptance.py` 通过；`python -m unittest discover -s scripts/tests` 107 项中 106 过，唯一失败是已登记的环境性问题（本机 Python 3.14.7 链接 zlib-ng，DOCX-FEAT 夹具压缩字节哈希与 CI 的 stock zlib 不同，干净 main 同样失败，见 T401 证据）。`Cargo.lock` 新增一行（docx 的 dev 依赖边）。未做：GUI／Word／WPS 人工验收、宿主界面接线、真实 Provider、AIGC 标识、信创真机。

### T204 DOCX 书签导入（2026-10-23，[#95](https://github.com/willmove/joyoffice/pull/95)）

T204 仍是 `in_progress`，进度计数不改。本切片只补 #92 留下的 DOCX 书签导入：正文成对的 `w:bookmarkStart`／`w:bookmarkEnd` 成为模型 `AnnotationKind::Bookmark`，名称原样、区分大小写，起点 `Bias::After`、终点 `Bias::Before`。既有 `navigation_from_document` 因此能把它们写成 PDF 命名目标，锚点内链写成 `/S /GoTo`。内置标题样式 1–9 的大纲书签路径不变，裸 `outlineLvl` 仍不成书签。

拒绝规则与 PDF 名称检查同一顺序：超过 2048 字节、trim 后为空、任一控制字符、重名、已接受满 512 个。ECMA-376 重名保留先出现的书签。空名、控制字符、脱离或颠倒的锚点、跨故事、列书签、不透明表格、页眉页脚和批注故事里的书签，以及未知属性或子内容，只记警告并且不插入。插入它们会让整份 PDF 导出失败，或让布局 caret 解析不到。XML 1.0 属性规范化把字面的制表符、换行和回车变成空格；`&#9;` 这类字符引用仍是被引用的字符。只含该字符的名称在 trim 后为空，夹在其他字符中则是控制字符，两者都不插入。分类函数仍单独拒绝无法出现在良构 XML 1.0 属性里的控制字符。

标记字节继续作为不支持片段保留，所在段落保持锁定。未改动导出复制原包。改名、改锚点，或在仍有锁定段落时编辑其他段落，导出都拒绝，避免静默丢掉标记。本切片不写回书签。超链接的直接书签子元素仍属于简单子集，链接可以盖上，段落在盖上之后锁定；书签包在 `w:r` 里面时，该超链接不进入简单子集。

`joyword-docx` 不依赖 `joyword-pdf`。导入侧的 512 与 2048 是公开常量，CLI 测试断言它们等于 PDF 侧同名预算。

证据：`crates/joyword-docx/tests/bookmarks.rs` 五项与 `apps/joyword-cli/tests/bookmark_goto.rs` 的 `an_imported_bookmark_is_a_pdf_goto_named_destination`。未做：DOCX 书签写回，页眉页脚、批注、不透明表格和列书签成为 PDF 目标，外部 PDF 结构读取器，PDF 结构标签，图形界面，Word／WPS 对照，真实打印与系统打印对话框。

### T204 DOCX 书签写回（2026-10-24）

T204 仍是 `in_progress`，进度计数不改。本切片只补 #95 留下的正文 DOCX 书签写回。已接受的正文 `w:bookmarkStart`／`w:bookmarkEnd` 不再锁定所在段落。未改动的导出仍逐字节复制原包。编辑后，宿主段落没变且锚点没变时，标记字节留在原处；宿主被改写或锚点仍是可表示的正文范围时，用导入时的标记字节按模型偏移拼回，不新造部件、不改 `w:id`、不改 `w:name`、保留 `displacedByCustomXml`。重导入得到同样的名称和范围。

拒绝与 #95 的导入政策对齐，不静默丢掉标记：改名、脱离、颠倒、偏置不是导入时的 `Bias::After`／`Bias::Before`、锚点落到页眉页脚批注或非正文顶层段落、新造书签，都拒绝写出。列书签、缺 id、不透明表格、页眉页脚和批注里的书签仍然不是模型书签，所在段落继续锁定，因此任何编辑导出都是 `Restricted`，标记字节留在原包里。

证据：`an_edited_export_writes_body_bookmarks_back`、`bookmark_write_back_refuses_unrepresentable_shapes`、`header_and_comment_bookmarks_still_refuse_an_edit`，以及 `restricted_signed_and_locked_documents_refuse_structure_edits` 里书签段落的拆分保留与合并拒绝。未做：页眉页脚、批注、不透明表格和列书签成为 PDF 目标，外部 PDF 结构读取器，PDF 结构标签，图形界面，Word／WPS 对照，真实打印与系统打印对话框。

### T204 页眉页脚与批注书签目标（2026-10-24）

T204 仍是 `in_progress`，进度计数不改（12 done／22 in_progress／32 planned／0 blocked）。本切片只补 #95 留下的页眉、页脚和批注书签：这些故事里成对的 `w:bookmarkStart`／`w:bookmarkEnd` 成为模型 `AnnotationKind::Bookmark`，名称规则与正文相同（原样、区分大小写、trim 后为空、控制字符、超过 2048 字节、重名保留先出现的、已接受满 512 个都拒绝）。`w:id` 按部件配对，不按整个包配对，所以正文和页眉可以都用 `1`。起点仍是 `Bias::After`，终点仍是 `Bias::Before`。

只导入还不够。`navigation_from_document` 用布局 caret 的起点当命名目标。页眉页脚原先画了字形，但 caret 索引只有正文；批注会让整篇布局拒绝。现在页眉页脚在第一次绘制时把 caret 记进辅助索引，重复出现的页眉不会因为 `Bias::After` 跳到最后一页。本节引用了、当前页型却不画的变体（未开 `titlePg` 的 First）在第 0 页拟合插入点，不画字形，也不追加 `StoryExceedsMargin`。空段记一个高度为 0 的插入点，不把后面的行往下推。批注文字仍然不画：该 story 的每个偏移都使用批注锚点的起点 caret，GoTo 因此落在被批注的正文、页眉或页脚上。游离批注没有这个点，导入记 `BookmarkAnchorInvalid` 并且不插入，免得一个书签让整份 PDF 失败。脚注、尾注和文本框仍是 `BookmarkOutsideBody`。几何预算把辅助 caret 行和正文行一起计入。

写回留到后面的切片。这些段落继续锁定。未改动的导出仍逐字节复制原包。任何编辑在写回之前就是 `Restricted`，所以标记既不会从原包里消失，也不会被拼进 `document.xml`。`comment::envelope` 允许一个没变的非正文书签通过。锚点变了之后仍必须是可写回的正文范围；正文书签的宿主段落消失时，即使存下来的位置字节没变，也报 `a bookmark changed`。列书签和不透明表格仍不是模型书签，也不是 PDF 目标。

带批注的文档现在可以排版并导出 PDF，画面上仍只有正文和页眉页脚。没有批注的文档不因为辅助索引多画字形。

证据：`header_footer_and_comment_bookmarks_import_with_body_rules`、`story_bookmarks_refuse_bad_names_duplicates_columns_and_detached_comments`、`a_header_bookmark_past_the_shared_budget_is_refused`、`header_and_comment_bookmarks_still_refuse_an_edit`、`header_and_footer_carets_stay_on_the_first_painted_page`、`a_comment_caret_is_the_anchor_and_comment_text_is_not_painted`、`an_unpainted_first_header_and_an_empty_header_still_resolve_carets`、`header_footer_and_comment_bookmarks_are_goto_destinations`。未做：页眉页脚与批注书签写回，不透明表格和列书签成为 PDF 目标，外部 PDF 结构读取器，PDF 结构标签，图形界面，Word／WPS 对照，真实打印与系统打印对话框。当时「这些段落保持锁定、编辑即 `Restricted`」的测试已由下一节的写回切片取代。

### T204 页眉页脚与批注书签写回（2026-10-24）

T204 仍是 `in_progress`，进度计数不改（12 done／22 in_progress／32 planned／0 blocked）。本切片补 #97 留下的页眉、页脚和可定位批注书签写回，规则与 #96 的正文书签写回相同。已接受的 `w:bookmarkStart`／`w:bookmarkEnd` 不再锁定所在段落。未改动的导出仍逐字节复制原包。编辑后，宿主段落没变且锚点没变时，标记字节留在原部件原处；宿主被改写，或锚点仍是该部件里可表示的范围时，用导入时的标记字节按模型偏移拼回。正文标记仍只进 `document.xml`。页眉、页脚和批注标记写回各自的部件，不发明进 `document.xml`。不新造部件、不改 `w:id`、不改 `w:name`、保留 `displacedByCustomXml`。`w:id` 继续按部件配对，所以正文和页眉可以都用 `1`。批注段落的文字叶子会就地改写，书签再按偏移拼回 `comments.xml`。重导入得到同样的名称和范围。书签拼回时会把 run 拆开；往返比对把页眉、页脚和批注里相邻且格式相同的文字并回一段，与正文已经采用的规则相同。

拒绝与导入政策对齐，不静默丢掉标记：改名、脱离、颠倒、偏置不是导入时的 `Bias::After`／`Bias::Before`、锚点落到另一个部件、新造书签，都拒绝写出。批注不再可定位（锚点脱离正文、页眉或页脚）时，书签也不再可写回。列书签、缺 id、不透明表格和游离批注里的书签仍然不是模型书签，所在段落继续锁定，因此任何编辑导出都是 `Restricted`。

证据：`an_edited_export_writes_story_bookmarks_back`、`story_bookmark_write_back_refuses_cross_part_and_bad_shapes`，以及既有的 `an_edited_export_writes_body_bookmarks_back`、`bookmark_write_back_refuses_unrepresentable_shapes`。未做：不透明表格和列书签成为 PDF 目标，外部 PDF 结构读取器，PDF 结构标签，图形界面，Word／WPS 对照，真实打印与系统打印对话框。

### 2026-10-26 Mac GUI 会话：T109／T110 部分补验

main `5efe06a` 调试构建、隔离 test profile、系统键盘事件驱动。要点：120,000 段样例打开期间观察到「正在打开…只读，键入的文字会保留」状态并在打开后确认缓冲键入落到新文档；Cmd+S 三按钮提示（默认另存副本）与损失清单、确认替换后原件被替换并可重开、隔离标记文档的损失清单（macOS 上 quarantine 可观测不列为损失）与默认另存副本均经当场哈希与内容核对。关闭提示 `close.dirty_after_save` 文案已捕获，但其脚本化场景（保存作业在途时键入再关闭）被一次焦点丢失打断，未干净重放。工作目录在会话中途被外部回退，截图档案大部分丢失（仅 `gui/11-prompt.png` 存档），读图结果以文字记入 `validation/20261026/gui/summary.json`。T109／T110 保持 `in_progress`。
