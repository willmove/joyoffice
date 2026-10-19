# JoyWord 当前实施与验证记录

记录日期：2026-10-18（Word 参考环境落地后更新）。这是源码工作区的实施状态，不能替代 SPEC 的目标或宣称所有路线图已经完成。文档基线为 v0.3，工程版本为 0.0.1。

## 进度速览

- **当前阶段**：M0／M1 技术预览；T000、T002 与 T111 已完成（2026-10-17，见下文），M0 退出条件与 M1 发布门槛均未完成。T200 的样式基础、多级编号、制表位、查找替换、格式刷、样式面板，以及项目符号、级别标签、列表对齐、主题 tint／shade 和表格样式粗体底纹边框切片已落地，任务仍 `in_progress`；T201 的合并与拆分、垂直合并视觉跨行、跨页拆行、表头重复、表级边距、单元格 tcMar、double／dotted／dashed／thick 边框、有主题时的主题色边框，以及百分比宽度和 fixed／autofit 已落地，任务仍 `in_progress`；T202 的页面几何、显式分页符、页眉页脚三种变体、正文与页眉页脚页码域、`pgNumType`、下一页／连续／奇偶分节和等宽分栏已落地，任务仍 `in_progress`；T203 的行内图片、简单超链接、安全 HTML／RTF 粘贴与插入图片按钮已落地，拖放和浮动图未做，任务仍 `in_progress`；T204 的文件菜单导出 PDF 与打印到文件已消费屏幕上的同一 PageScene，系统打印对话框未验证，任务仍 `in_progress`；T205 的功能区、状态栏、字体与段落对话框和第二切片的标尺拖拽已落地，任务仍 `in_progress`；T207 已把有证据的测量接入排版入口，任务仍 `in_progress`；T209 撤销组继续进行中；T210 的 inspect／text／render／convert 均已实现但人工验收未完成；T211 的应用日志、panic hook 与测试 profile 已接入，崩溃后检查日志的人工验收未做。完整统计、逐项勾选与状态口径见 [TASKS 进度总览](TASKS.md#进度总览)。
- **已落地的主要能力（不代表所在任务已全部完成）**：真实 DOCX 会话、中文文本编辑、基础格式与文本剪贴板、另存／重开、原件保存的元数据观测与确认路径、Windows 原件原子替换与稳定导出重绑定、统一排版场景的 PDF／CPU 光栅及无头 PNG／PDF 输出、可见页缓存、打开／保存并发控制和恢复日志。
- **主要未完成项**：Windows 原生 GUI 原件保存确认与真实 ACL／断电／非 NTFS 验收、WPS 参考 PDF、信创真机验证、T200 剩余的按字体名换面、甲乙超过 10、中文计数超限、罗马数字超出 1..=3999、字母重复超过 64、bar／middleDot／heavy、编号部件改写、非自动颜色绘制、正则查找与全角半角折叠、T201 剩余的嵌套表、vAlign、trHeight、表格属性对话框、列宽拖拽、首列末列与纵向条纹的排版和合同表格人工验收、T202 剩余的图形界面验收、widowControl、不等宽栏、`nextColumn`、章节页码、页眉编辑界面与页面设置写回、T203 剩余的拖放、浮动图、表格内超链接和 Word／WPS 粘贴对照、T204 剩余的双阅读器人工检查、真实打印、系统打印对话框、结构标签和仅书签锚点的文档内跳转、T205 剩余的 Word 字数对话框对照、视图选项进入布局快照键（排版没有这些选项，三个开关保持禁用）、下划线与非自动颜色的页面绘制，以及只用功能区完成报告排版的人工验收、T207 剩余的字符网格／中西文间距／adjustRightInd／字体替代链及 Word／WPS 版式对照、T211 崩溃后人工检查日志，以及其余 M2–M9 的审阅／AI 等路线图工作。
- **信创真机验收策略（2026-10-18 用户决策）**：有条件时再补验，不阻塞开发、任务依赖、阶段验收或发布。T005 改为 `in_progress`，已完成的构建与 QEMU 自动验证证据保留，真机仍标“未验证／待补验”。本条取代下文历史记录中因缺信创真机而保持 `blocked` 的安排；其他未完成项的门槛不变。
- **T207（进行中，2026-10-18）**：`joyword-layout::cjk` 的测量函数已接到 `layout_document`／`wrap_mixed`。等线名称表命中时，窄于整形结果的 R7 宽度进入页面场景；纯中文行 `both` 等分余量，段末行不拉伸；单倍行距仅对 R4 已测家族使用 1.3×win 跨度并在未关闭吸附时对齐 312 twips。未验证配对、非等线标点、非纯中文两端对齐、黑体与未具名字体的单倍行距、中西文间距和 `adjustRightInd` 保持原行为或拒绝。模型不读 `w:docGrid`。人工 Word／WPS 版式对照未做，不标完成。细节见文末「T207 排版入口接入」。

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
- W09（T103 健壮性实验）：experiments/docx-robustness 独立 workspace，提供 opc／import／roundtrip 三目标 harness、27 份原创种子、固定种子变异、最小化与有界 campaign 驱动。未发现生产缺陷；不是覆盖引导 fuzz，也未接入 CI。固定的生产目标随集成改为 bcf435c；PR #9 修改 crates 后需重新指定，见 [W09 报告](../coordination/reports/W09.md)。
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
| 命令注册表 | 稳定命令 ID、本地化标签、平台快捷键、菜单分组；`invoke` 是菜单、快捷键、界面按钮和功能区的唯一执行入口，先查可用状态（如缩放上下限）再执行；校验重复 ID、各平台快捷键冲突与缺失标签；只登记已实现命令。T205 第一切片把开始／插入／布局／视图功能区接到这些命令 | 命令搜索；编辑标记、隐藏文字与域代码的实际绘制（三个开关已登记，因排版没有对应选项而禁用） |
| 本地化 | `key[one]`／`key[other]` 复数形式按 CLDR 基数规则选择（中文仅 other，英文 one／other），`{name}` 插值与 `{{`／`}}` 转义；校验语法、重复、未知或缺失复数类别、未闭合花括号，以及跨语言的键与占位符一致 | 更多语言 |
| 偏好设置 | 作者姓名／缩写（缩写可自动推导）、度量单位、自动保存间隔、界面语言、新文档纸型（按语言给默认值）、最近文件；`schema` 迁移（0→1）、非法值逐项回退、损坏文件回退默认并另存原件、较新版本只读不覆盖、未知键原样写回、临时文件加重命名的原子保存 | 设置界面；修订作者接入 T300 |
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
7. T103、T110：只读导入、保守导出、新副本落盘和默认格式段落的拆分／合并已经接入。原件保存在未确认时列出不能保留或不能观测的元数据且不改源字节；显式确认且观测成功后，用 rustix 1.1.5 的 `RenameFlags::EXCHANGE`（macOS 上为 `RENAME_SWAP`）替换，并核对权限、所有者、macOS 创建时间与可复制的扩展属性。观测失败拒绝替换。macOS ACL 只观测、不回写。含隔离标记文档的图形确认、覆盖引导 fuzz 和 T107 的 Word／WPS 正文核对仍未完成，两项都保持 `in_progress`／`planned`。在 T107 通过前不提供真实用户文档的编辑保存承诺。
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
- CI 将 W09 固定 27 个种子 × 3 个目标的 bounded smoke 纳入四平台作业；Windows 使用显式 `.exe` 路径以便读取二进制哈希。smoke 未完成完整矩阵即失败，不再将总时间预算耗尽视为成功；campaign 的有界停止契约保持原样。新增回归同时覆盖完整、提前停止和 campaign 情况。该 smoke 不等于覆盖引导 fuzz。
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
- 未覆盖：Retina/多显示器 DPI、断电恢复、T110 GUI 替换路径、T109 长只读窗口、WPS 参考 PDF（本机通道故障）、信创真机（非阻塞待补验）、覆盖引导 fuzz。

### T207 排版入口接入（2026-10-18）

`layout_document` 与 `wrap_mixed` 开始调用 `joyword-layout::cjk` 的测量函数。屏幕与 PDF 仍只读同一份 `PageScene`：压缩后的 advance 与两端对齐间隙写进字形的 `x`／`advance`，PDF 内容流沿用相邻字形 `x` 之差。布局仍不改文档。

接入范围按已记录的证据等级收紧：

- **R7**：名称表（name ID 16，否则 ID 1）解析为等线时，仅当 `punctuation_advance` 成功且结果窄于整形 advance 时替换该一字一字形簇。Measured、Scaled、Inferred 都沿用函数自己的标记。全宽结果不改写整形宽度，因此 R2 压力样例里跟汉字的孤立标点保持原断行。未验证配对（含行尾无跟随标点、ASCII 标点）和其他字体保持整形宽度。
- **R3**：`w:jc` = both 且全文为汉字加非 ASCII 禁则／引号时，调用 `justify_intercharacter`。非末行余量等分到字间并抵满版心；段末行间隙为 0。单字行函数返回未验证，不加边缘间隙。拉丁字母、ASCII 标点、不能一字一字形对齐的行，以及 `distribute`，返回不支持，不把左对齐当成两端对齐成功。证据仍是 Inferred。
- **R4**：段落单倍行距（auto 240）且字体名称为宋体、仿宋、楷体、等线或微软雅黑时，行框为 `single_line_height`（1.3×该脸的 win 跨度×字号，Inferred）。`snapToGrid` 不是显式 false 时吸附到实测 312 twips；显式 false 只用自然行高。黑体不在拟合集合内。其他字体、其他行距、未设置行距时继续用 `SingleFontProfile.leading`。模型没有 `w:docGrid`，不读取其他 `linePitch`。
- 未接入：中西文自动间距宽度、`adjustRightInd`、字符／行单位的缩进（函数仍只做调用方单位的百分之一换算）、字符网格、CJK 字体替代链。禁则仍是既有字符集。

自动验证（Rust 1.98.1，本环境 Linux x86_64）：`cargo fmt --all -- --check`、`cargo clippy -p joyword-layout --all-targets --locked -- -D warnings`、`cargo test -p joyword-layout --locked`（79 项，含新增 `cjk_layout` 9 项：引号压缩、连排半宽、行尾不压缩、未验证标点保持整形宽度、压力样例在等线名称门开关下断行不变、纯中文两端对齐抵满版心且末行不拉伸、单字行不加边缘间隙、拉丁／distribute 仍拒绝、单倍行距吸附与关闭吸附、未测字体的单倍行距拒绝）。`cargo test -p joyword-pdf -p joyword-render --locked` 通过（PDF 5 项、render 25 项）。名称门测试把拉丁夹具的 Windows 家族名改成 DengXian，只证明调用条件；期望宽度是纯函数的 em 分数和该脸自己的 win 跨度，不是等线字节的新测量，也不是 Word 2024 的重新导出。

未覆盖：真实等线／宋体／雅黑字节上的断行与行高、`tests/fixtures/reference/word2024/breaks/` 的 Word PDF 未在本环境重抽、Word／WPS 人工版式对照、字符网格与 `w:docGrid` 的其他间距、中西文间距、adjustRightInd、字体替代与缺字诊断链。T207 保持 `in_progress`。

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
- **未覆盖**：没有启动 GUI、制造崩溃、再打开日志核对无正文泄漏。人工验收未做，T211 保持 `in_progress`。未在 macOS／Windows 上复跑。没有把文件 sync 说成断电后目录项一定可见。W09 harness 按原契约重锚到 `a3cec13b3cea9fb4f6e98f6158cdeae01fbe2688`（本任务改 `crates/` 的提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境重跑固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

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
- **未覆盖（当时）**：主题 tint／shade、按字体名加载面、表格样式解析、边框底纹、级别 `w:rPr`、字母超过 26、甲乙超过 10、中文计数超过 99999 或为 0、罗马数字超过 3999、bar／middleDot／heavy、居中／右对齐／两端对齐与列表或制表位同用、编号部件改写、非自动颜色绘制、PDF 斜体剪切、正则查找、全角半角折叠、格式刷省略的 hint 与具名复杂文种、整段链接字符样式仍走既有应用规则、已有样式的段落在不改变样式 id 时不能刷直接格式，以及人工验收。其中项目符号、级别标签、字母超过 26、列表对齐、tint／shade 和表格样式的粗体底纹边框已由第四切片补上，其余仍未做。没有新增外部包。没有启动图形界面。W09 harness 重锚到 `8eb55cc450176b288858fa398252e3aac5d546ad`（本任务改 `crates/` 的提交；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境重跑固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁只把既有的 `unicode-segmentation` 1.13.3 记成引擎的直接依赖，无新包。

### T200 项目符号、tint 与表格样式（2026-10-19）

这一刀补 T200 当时未覆盖的项目符号、级别标签字号与粗体、字母超过 26、列表或制表位上的对齐、主题 tint／shade，以及喂给 T201 排版的表格样式。T200 保持 `in_progress`。没有新增外部包。没有启动图形界面。非自动颜色和下划线仍不绘制。

- **项目符号**：`numFmt` 为 bullet 时，标签是 `lvlText` 把 `%%` 换成 `%` 之后的字面量。`%1` 留在标签里，不当计数。计数仍前进。`lvlPicBulletId` 存下来，标签记 `NumberingProblem::PictureBullet`，不发明字形。级别 `w:rPr` 的半磅字号、粗体和具名 ascii／hAnsi／eastAsia 只作用于标签。字号或粗体与正文不同时，标签单独成一个字形 run，以免 PDF 把同一段文字画两次。字体名是 Symbol 或 Wingdings 时，U+F020–U+F0FF 按公开表映射后再用调用者提供的面整形，不加载另一张面。未映射的字符原样留下。Wingdings 2／3 与 Webdings 不映射。
- **字母与罗马数字**：下标 `(n-1)%26`，重复次数 `(n-1)/26+1`。27 为 aa，54 为 bbb。重复超过 64 记为不能绘制。规范示例没有上限，64 是本切片的界限。罗马数字仍是 1..=3999（`ROMAN_MAX`），超出不发明上横线或 `MMMM`。
- **对齐**：列表几乎都有悬挂缩进，所以「有缩进就拒绝居中」不再挡住列表。居中和右对齐只平移带簇的正文，标签留在悬挂位置，行宽设成帧宽，后面的行放置不再加一次偏移。两端对齐只拉伸纯中文正文，拉丁文仍拒绝「inter-character justification」。制表符或自定义非清除制表位保持绝对停靠。只有缩进、没有列表也没有自定义制表位时仍拒绝，错误串仍是 “justification with indent, numbering or tabs”。段落对话框里 justify 加缩进的拒绝没有改。
- **主题色**：`themeTint`／`themeShade` 是两位十六进制。shade 为 `C * byte / 255`，tint 为 `C * byte / 255 + (255 - byte)`，半入向上取整 `(c * factor + 127) / 255`。条款例子：tint 字节 153 把 80 变成 150；50% shade 的字节 128 或 127 把 150 变成 75。两者都有时先 shade 再 tint。结果写成 `Color::Rgb`。导出仍拒绝主题色，不靠丢掉 tint 来改写 run。排版仍拒绝绘制非自动颜色。
- **表格样式**：`tblStyle` 沿 `basedOn` 从根到叶叠整表格式，再按 band、首行、末行叠条件格式。本切片的属性是 run 粗体、底纹和 `nil`／`single` 边框。粗体是指定值：run 没有直接粗体时强制开或关，不是 toggle 翻转。直接单元格边框、直接底纹和直接 run 粗体优先。横向条纹一行一条。首行外观打开时第 0 行不进条纹；末行外观打开时末行不进条纹并盖过条纹。下标 0 是 band1。`tblLook` 的 `w:val` 按 §17.4.56 的位解释，同名属性盖过位。元素省略时用应用默认：首行、末行和横向条纹开，纵向条纹关，两列关。这是假定，不是 Word 观察。元素存在而属性省略则为假。首列、末列和纵向条纹只存储并在改写时写回，不参与排版。未改的表仍复制原字节。改过且 look 有值的表写出 `1`／`0` 属性；look 为空时不发明 `tblLook`。样式部件不改写。主题色边框和底纹仍不解析。有表时增量排版仍是整篇重排，页与全量一致。
- **命令**：`Command::ApplyBullet` 在同一次事务里 `ensure_bullet` 再写级别 0 的 `numPr`。已有无图片的 0 级 bullet 就复用，否则新建抽象编号和实例。新标签是 U+2022、制表后缀、起始缩进 720 twip、悬挂 360 twip。这些距离是 JoyWord 预设，不是 Word 测量。一次撤销恢复编号库和段落。`list.apply` 的 `num` 与 `bullet` 都可选；有 bullet 时优先，否则仍要 `num`，两者都缺的失败信息是 `num`。功能区 `start.list_bullet` 紧挨在 `start.list_remove` 之前，固定参数 `bullet=1`，图标 `list-bullet`。目录下限从 40 升到 41。新建编号定义之后的导出仍拒绝；套用包里已有的项目符号只改 `numPr`，编号部件字节不变。
- **缓存**：段落缓存键哈希标签字号和粗体。只改级别字号或粗体时，该段未命中缓存，无关段落命中，增量页与全量相同。
- **未覆盖**：按字体名换面、甲乙超过 10、中文计数超过 99999 或为 0、罗马数字超出 1..=3999、字母重复超过 64、bar／middleDot／heavy、编号部件改写、非自动颜色与下划线的字形绘制、PDF 斜体剪切、正则查找、全角半角折叠、格式刷省略的 hint 与具名复杂文种、整段链接字符样式仍走既有应用规则、已有样式的段落在不改变样式 id 时不能刷直接格式、Wingdings 2／3 与 Webdings、只有缩进时的居中／右对齐／两端对齐、首列／末列与纵向条纹的排版、主题色底纹，以及人工验收。无 tint／shade 的主题色边框、单元格 `tcMar`、`nil`／`single` 以外的边框和百分比宽度已由 T201 第三切片补上；嵌套表仍未做。合并 origin/main 的页码域、分节与等宽分栏之后，W09 harness 重锚到 `547c37466710a9bbef48c3d9d1832f78c545b0a4`（该提交的 `crates/` 同时含项目符号、tint／shade、表格样式和 T202 页码域；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

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
- **未覆盖**：嵌套表、`vAlign`、`trHeight`（模型里有字段，导入遇到它们仍整表不透明，排版不用它们）、表格属性对话框、列宽拖拽，以及制作合同表格的人工验收。表格样式见 T200 第四切片，本切片不另做 `tblStyle` 解析。没有启动图形界面。没有新增外部包。没有 Word 观察。合并 origin/main 的表格样式之后，W09 harness 重锚到 `99fd20b912644d8d9dc36a26925fca98c2483c85`（该提交的 `crates/` 同时含单元格边距、边框样式、百分比宽度和 T200 表格样式；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境重跑固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

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
- **未覆盖**：Word 字数对话框对照；导航窗格与审阅栏；视图选项进入布局快照键（排版没有这些选项，不能把禁用开关说成已经切换显示）；下划线与非自动颜色的页面绘制；Exact 小于基线时画布仍会报错，对话框不把 Exact 悄悄抬到基线；GUI 人工验收未做。没有新增依赖。合并 `origin/main` 的行内图片之后，W09 harness 曾锚到 `347d44aebbf3d7f34e58ad88828f3ca7c031d0a7`（该提交的 `crates/` 同时含字体／段落对话框与行内图片）。随后与表格合并拆分再次合入，当时锚在 `2e886f16801984c21b60490c770825732a6d6179`。再并入超链接与安全粘贴之后，当前锚点见上一节 T203 记录，不放宽 `git diff <SHA> -- crates`。

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
- **未覆盖**：两个独立阅读器的人工检查；真实打印机；系统打印对话框；不可打印区域作为用户可见选择；结构标签；仅书签锚点的文档内跳转；审阅标记视图打印；份数在文件里重复页面。CPU 光栅与打印预览是既有路径，本切片不把它们说成新完成。没有新增外部包版本。Cargo.lock 只为 joyword-pdf 记下已有的 flate2 1.1.10 直接边。合并 origin/main 的页码域、分节、等宽分栏、超链接与安全粘贴，并让 PDF 页框跟随各节已存储的宽高之后，W09 harness 曾锚到 `4c54d2829263983a43f74afee54ee8bacd700f93`。再合并 origin/main 的项目符号、主题 tint／shade 与表格样式之后，锚点改到 `eae9e9b3dc5303ff5d2eceb5d759fbdb7da26fd2`（该提交的 `crates/` 同时含这些改动；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。

