# JoyWord 宿主任务状态

2026-11-11：新增三产品共享界面主题，Word 将原风格归为「青岚」，在「文件 → 选项」提供青岚、素雅、晴空、暖砂的亮暗八个预设。兼容旧偏好，选择即时生效并保存；主题不修改文档内容。[主题说明](../../joyoffice/THEMES.md) · [本机验证与边界](../../joyoffice/validation/20261111-themes/README.md)

2026-11-09 补齐 ODT、OTT、FODT 往返的标题字重与字号、段前段后和行距、图片锚点与偏移、单元格四边内边距及逐边线型、线宽与颜色。修复 Writer 重存时嵌入在图片节点内的空段落被当成正文、导致图片和后续表格下移的问题。ODF 文件从 JoyWord 的有效版式生成，表格按合并边框模型写出。[ODF 1.3 XML 规范](https://docs.oasis-open.org/office/OpenDocument/v1.3/os/part3-schema/OpenDocument-v1.3-os-part3-schema.html)

本机 Windows LibreOffice Writer 26.8.2.2 对两份样本的三种格式各做两轮往返，检查 XML、图片字节和 Writer 渲染。主样本保留了 14 pt 粗体标题、继承的 115% 行距、段前段后、内嵌 PNG 的位置和尺寸、浮动 PNG 的锚点与单侧绕排、表格合并及四边内边距。实线、虚线、点线与等分双线的颜色和渲染线宽一致。标题字形总宽缩短约 0.64 pt，表格文字纵坐标变化约 0.06 pt。第二份样本确认，不等宽双线的两道线与间隙会被均分，浮动图片两侧不等的绕排留白会取平均，图片坐标保留。独立页锚、旋转图片和自定义绕排轮廓未验。Windows 构建、严格 Clippy 和 formats 的 306 项测试通过。产品全量检查仍停在改动前已复现的 `joyword-layout` 分页断言。[LibreOffice Writer 转换过滤器](https://help.libreoffice.org/latest/en-GB/text/shared/guide/convertfilters.html)

2026-11-09 JoyWord 的桌面与 CLI 可打开 FODT，导入正文、内嵌图片、表格及现有 ODF 转换覆盖的版式。Windows 实屏打开 JoyWord 导出的 FODT，修改正文并从「文件 → 导出 → 平面 OpenDocument 文本」写出副本，经 LibreOffice 保存后用 JoyWord 重开确认修改保留。普通「保存」仍使用 DOCX 副本流程。FODT 支持最多 64 MiB 的 UTF-8 XML，外链图片不会被读取。[ODF 独立 XML 与内嵌数据](https://docs.oasis-open.org/office/OpenDocument/v1.3/os/part3-schema/OpenDocument-v1.3-os-part3-schema.html)

Windows 上用 LibreOffice Writer 26.8.2.2 对 ODT、OTT、FODT 各做两轮往返。两页样本的 PNG 字节与显示尺寸、表格正文与跨列合并、Letter 横向纸张、四边页边距和显式分页均保留。修复窄表在 Writer 中被拉伸至正文宽度及无边框表格出现网格。对比 Writer 的实际渲染，标题字号和字重、段间距、图片位置与单元格内边距仍有差异。验证范围为此样本，复杂表格边框和浮动图片的精确还原尚未验证。[LibreOffice Writer 转换过滤器](https://help.libreoffice.org/latest/en-GB/text/shared/guide/convertfilters.html)

Windows 产品构建、格式检查和严格 Clippy 通过，formats、engine、shell 和桌面 UI 共 1,583 项测试通过。产品全量检查停在 `joyword-layout` 的 `a_page_ends_when_a_multiple_spaced_lines_text_no_longer_fits`，改动前的 `0d762000` 在本机复现同一失败。

2026-11-09 修复旧版 DOC 打开兼容性。常规复合文件解析失败时，只对整扇区填零、且没有任何非空小流使用的单扇区 MiniFAT 尝试兼容读取。回退只在内存视图中调整文件头。Windows 用桌面活动会话和 CLI 核对附件的两段正文，另存 DOCX 后重开核对一致，原件 SHA-256 一致。Windows 实屏通过「文件 → 打开 → 浏览本地文件」选择原始 `textutil-minimal-test.doc`，第 1 页的两段正文均正常显示。合成回归覆盖仍有小流使用损坏 MiniFAT 时的拒绝行为。[MS-CFB §2.4](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/c5d235f7-b73c-4ec5-bf8d-5c08306cd023)

2026-11-09「文件 → 导出」增加独立格式入口，覆盖参考图的 13 种格式，并保留 EPUB 和 PDF/A-2b。两种 Word XML 分别写出 Flat OPC 和 Word 2003 XML。PNG 保存当前可见页。OTT 使用模板包类型，FODT 使用单文件 XML 与内嵌图片，沿用现有 ODT 转换范围。导出使用当前编辑快照并拒绝覆盖。Windows 已实屏写出 PDF、Word 2003 XML、OTT、FODT 和 PNG，含图片与表格的 ODT、OTT、FODT 样例通过 ODF 1.3 结构校验。[ODF 包规范](https://docs.oasis-open.org/office/OpenDocument/v1.3/os/part2-packages/OpenDocument-v1.3-os-part2-packages.html) · [ODF XML 规范](https://docs.oasis-open.org/office/OpenDocument/v1.3/os/part3-schema/OpenDocument-v1.3-os-part3-schema.html)

2026-11-09「文件」页按用户参考图调整为左侧导航与右侧工作区，使用 JoyWord 浅色和深色配色。开始页展示可编辑模板和最近文档，新建页提供完整模板列表。Windows 实屏已验证模板新建、最近文档重开、窄窗换行与滚动、关闭文档、Esc 返回和中英文切换。macOS、Linux 和实际打印未实测。

2026-11-09 Windows P1 续验 JW-515。认证控制新增字面查找、单页和页宽适配，实屏确认查找高亮与画布缩放。TCP 与 MCP 回归覆盖密钥、文档版本和文件访问边界。通用界面控制仍开放。[本机验证](../../joyoffice/validation/20261109-p1-followup/README.md)

2026-11-09 Windows P1 实屏通过正文命中列表、第二项定位、选区内替换、单步撤销及一级大纲跳转。修复「更多分栏」误清空未改动的必填值，默认本节的分栏表单已通过 GPUI 回归、实屏保存和独立 XML 读回。JW-406／407 的其余范围仍开放。[验收记录](../../joyoffice/validation/20261109-v012/README.md#p1)

2026-11-09，JoyWord v0.1.2 在 Windows x64 补验旧日志提示及已丢弃日志抑制、偏好迁移和真实崩溃恢复。原生 PDF/A-2b 经独立校验通过。重新运行旧新版 CLI 差分及五类复杂对象保留回归。JW-806、JW-808、JW-812 保持部分验证。[本机验证及剩余条件](../../joyoffice/validation/20261109-v012/README.md)

2026-11-08「视图」功能区对照：现场比较本机 Word for Mac 16.114.4，按视图／沉浸式／显示／缩放／窗口／宏重排，接通屏幕标尺、网格、标题导航、单页与页宽适配、共享缩放和可退出的专注模式，格式标记移到开始页。未接入的排版视图、多页并排和多窗口能力明确禁用。产品检查 3,614 项通过、2 项既有忽略；最后 Esc 输入法组合优先级调整后，UI crate 17 项、严格 Clippy 和产品构建复验通过。验证与边界见[改动及实屏证据](VIEW-UI.md)。

2026-11-08 布局焦点崩溃复核：用户报告与上一轮 PID 93446 的原生验收崩溃为同一事件，定位为数值外框与内部输入重复申报 AccessKit 焦点。单一焦点修复已保留，新增外框焦点语义与四框聚焦／步进回归，并修正步进后 Enter 的焦点交接。产品检查 3,612 项通过、2 项既有忽略；macOS 原生四框连续操作、错误草稿与保存读回复验通过。详见[报告分析与修复证据](LAYOUT-FOCUS-CRASH.md)。

2026-11-08「布局」功能区对照：按本机 Word for Mac 16.114.4 重排页面设置、段落数值框及对象排列，补齐菜单和真实选择窗格，保留原有完整表单。数值草稿与菜单绑定选区，修复原生数字框重复焦点断言；产品检查 3,610 项通过、2 项既有忽略，macOS 输入、撤销、另存重开及独立 XML 已复验。实际对象层级、手动断字、完整 Word 菜单与其他平台验收仍开放。详见[改动与验证边界](LAYOUT-UI.md)。

2026-11-08「插入」功能区对照：现场比较本机 Word for Mac 16.114.4，按页面／表格／插图／媒体／链接／批注／页眉页脚／文本／符号重排，接通现有命令、图标面板和 10 × 8 表格选择器，未实现能力明确禁用。修复原生保存复验暴露的小栈后台线程溢出，新增真实 DOCX 保存读回回归；产品检查 3,607 项通过、2 项既有忽略。macOS 原生插入、撤销、另存重开及浅深色／英文显示已按范围复验；完整 UI、图库、读屏和其他平台仍开放。详见[改动、截图与验证边界](INSERT-UI.md)。

2026-11-06。以下记录 JoyOffice 中的实际进展。验收条件沿用 [R0–R8 任务账本](migration/docs/joyword/TASKS.md)，旧产品覆盖范围见 [M0–M9 任务](../TASKS.md)。导入前的计数和结果保留在各自基线。

## R8

2026-11-06 本地整合后修复 shell 示例的冗余转换，并将测试构造/断言 lint 例外限定到已有测试模块。`joyword-shell` 全目标严格 Clippy 与 73 项测试通过，生产 deny 规则保持不变；未进行本批真实设备验收或发布。[验证与剩余阻碍](../../joyoffice/validation/20261106-local-safety-integration/word-clippy-followup.md)

| 任务 | 宿主状态 | 证据或剩余条件 |
| --- | --- | --- |
| JW-801 可恢复基线 | 部分验证 | 已在独立工作树复现 `00fb0687` 并生成旧日志。另有 Windows 旧 R8 便携包和隔离的回退演练。替换前 GUI 及正式安装器待验。 |
| JW-802 目录和名称 | 已验证 | JoyOffice 只构建一套 JoyWord 桌面和 CLI，均使用当前编辑会话。产品边界门禁和 [来源记录](../../../licenses/joyoffice/JOYWORD-R8-SOURCE.json) 可复查。 |
| JW-803 公共库 | 部分验证 | 使用宿主 UI 和 OPC 预检。DOCX 的专用包处理及 CFB、DrawingML、OMML 和字体服务仍待逐项评估。 |
| JW-804 依赖和 feature | 部分验证 | 根锁与 GPUI 补丁已接入。Windows 已复跑默认 JoyWord 产品门禁，并与 Deck 验证、Sheet 自动化一起启用可选 Agent 跑全仓测试。各平台的 feature 组合继续分别归档。 |
| JW-805 构建测试和打包 | 部分验证 | 产品 xtask、CI、覆盖引导 fuzz、固定字体金样、独立 PDF 读回和旧 CLI 子进程回归已接入。EPUB 子进程回归和独立 EPUBCheck 复验已通过。旧完整编辑行为矩阵待迁移。 |
| JW-806 数据迁移与回退 | 部分验证 | Windows 原生旧日志打开、已丢弃日志抑制、英文偏好迁移和强制结束后恢复另存均通过。V1/V2/V3 差分与五类复杂对象合成语料已复跑。更广真实对象语料及正式安装后的数据回退仍开放。 |
| JW-807 三产品联合回归 | 部分验证 | Windows 续验启用 JoyDeck 验证、JoySheet 自动化和 JoyWord Agent，工作区 5,965 项通过、3 项既有忽略。[P1 续验](../../joyoffice/validation/20261109-p1-followup/README.md#自动验证) MCP 回归使用独占临时目录。Linux 安全属性用例按系统实际列举的名称断言，Linux 原生复跑仍开放。 |
| JW-808 功能与协议并集 | 部分验证 | Windows v0.1.2 的 19 份 DOCX schema 1 报告与旧 CLI 一致。Windows 原生 EPUB、PDF/A-2b 导出及各自独立验证通过。认证桌面控制覆盖跳页、数值缩放、字面查找和单页／页宽适配。打印预览与系统打印框已打开。取消回传、队列提交、出纸、其余界面操作及 Agent Provider／凭据仍开放。 |
| JW-809 桌面包及实际操作 | 部分验证 | Windows 候选包在仓库外运行，活动文档控制与 MCP 读回通过，便携回退后旧 GUI 可启动。新增打印和 Agent 入口的实屏操作待复验。 |
| JW-810 移除重复实现 | 待完成 | 保留只读旧格式适配。公共基础和协议并集需完成核对后再清理。 |
| JW-811 交接与维护入口 | 已更新 | 宿主约定、R8 交接、状态页及验证记录已关联。最终移除迁移期重复实现后需同步入口。 |
| JW-812 替换及回退演练 | 部分验证 | Windows 实际发布的 0.1.1 和本机 0.1.2 Release 便携候选包通过坏包拒绝、切换失败复原、候选控制和 MCP、主动回退及旧版读回。正式安装、关联注册、卸载、签名及跨平台回退仍待验。 |
| JW-899 宿主替换验收 | 开放 | 宿主构建与部分原生流程已验证。必保留功能和分发验收仍有未完成条件。 |

R8 各平台、包及本轮自动验证见 [Windows P0 验证](VALIDATION.md#windows-p0-字段兼容与便携回退2026-11-06) 与 [验证记录](VALIDATION.md)。旧 CLI 的命令、范围和差距见 [交接兼容表](../../joyoffice/JOYWORD-R8-HANDOFF.md#已迁入的旧-cli-接口)。

### JW-806 异步恢复的文档身份隔离（2026-11-06）

修复 `Reader::recovery_tick` 只比较修订号的问题。恢复另一份草稿会切换文档身份，而新旧文档都可能处于修订 1；此前外壳因此未刷新布局或清除旧原件路径，旧日志的原样保存缓存也可能被另一份文档复用。现在同时核对文档身份，切换后清除旧保存来源、旧快照与旧恢复提示，并刷新显示。

两项回归使用真实恢复工作线程和磁盘快照：修复前均失败，分别确认旧原件路径残留，以及当前草稿为 `Recovered draft, not the old document` 时另存内容却为 `Legacy checkpoint`。修复后，恢复草稿不能沿用旧路径作原件保存，另存重开内容与当前文档一致，旧原件和旧日志保持不变。Linux x64、Rust 1.98.1 下通过本批完整 `joyword-shell` 63 项测试、生产库 Clippy、全仓格式及文档检查。未执行桌面恢复面板和真实崩溃操作，JW-806/812 仍为部分验证。

### JW-806 偏好配置读取失败保护（2026-11-06）

修复活动偏好文件读取失败时仍允许把默认值写回的问题。只有确认文件不存在才允许新建；无法读取、超限及悬空符号链接使用默认值但保持只读，并返回 `Unreadable` 原因。活动配置读取与旧配置迁移统一为 256 KiB 上限，读取增长超过上限也拒绝；保存前验证序列化大小，避免写出下次无法读取的配置。原有损坏文件备份、未知字段保留和新 schema 只读契约不变。

Linux x64、Rust 1.98.1 下 11 项偏好测试通过，包括不可读文件、目录错误、悬空链接、恰好达到上限、超限读取和超限保存不改原件；本机以非 root 用户运行，不可读文件用例实际覆盖权限拒绝。完整 `joyword-shell` 67 项测试、生产库 Clippy、全仓格式及文档检查通过。本轮未执行桌面偏好面板、Windows/macOS 权限场景或正式安装回退，JW-806/812 仍为部分验证。

### JW-806 Linux 旧日志对象安全回归（2026-11-06）

在 Linux x64 容器安装 Rust 1.98.1 后，直接运行 `Reader::recover_legacy` 验证混合媒体 DOCX 的 V2/V3 日志。新增反例覆盖 V2 删除或插入内联对象标记、合并不同表格单元格：均拒绝，原 DOCX、日志和既有锁文件字节不变，目录未新增文件。V3 精确匹配时另存副本与快照逐字节一致，正文失配时拒绝恢复。原有 V2 多处编辑回归增加中文与 emoji，保存重开后正文和媒体保持一致。

`cargo test --locked -p joyword-shell` 在本轮通过 64 项测试（含 4 项恢复测试）；未修改生产恢复逻辑。本轮覆盖 headless Rust 路径，不替代桌面旧日志交互、其他复杂对象差分、真实崩溃、正式安装和文件关联回退，JW-806/812 保持部分验证。

全仓 `cargo fmt --all --check`、文档完整性检查及生产库 `cargo clippy --locked -p joyword-shell --lib -- -D warnings` 通过。额外尝试 `cargo clippy --locked -p joyword-shell --all-targets -- -D warnings` 未通过：未修改的测试模块存在 `unwrap_used` 等告警，`r8_render_goldens` 示例有冗余 `f64::from`。这超出当前 JoyWord xtask 的生产 `--lib --bins` lint 范围，本批不扩大到清理无关测试和示例。

### JW-808 旧 CLI 差分门禁补强（2026-11-06）

`scripts/joyword/verify-r8-compat.py --baseline` 对普通语料也强制比较完整 schema 1 报告，诊断字段变化、增加或缺失均失败；此前普通语料只要求正文一致，其他差异仅记录。报告以仓库相对路径区分同名语料，并记录旧 CLI 的 SHA-256；未提供基线时 `baseline_cli` 为 null，不代表旧新版差分通过。字段专项使用同一比较规则。

`scripts/tests/test_joyword_compat.py` 通过替代 CLI 响应运行真实语料比较入口，覆盖正文相同但遗漏对象、保留片段、只读状态变化及字段增删的反例。该文件由既有四平台 CI 的脚本测试发现。该批次本地通过 4 项测试；当时容器尚未安装 Rust 工具链或构建产品二进制，未重跑真实旧新版 CLI、原生界面、正式安装及文件关联回退，JW-806/808/812 状态保持部分验证。

### JW-812 回退验证脚本补强（2026-11-06）

`scripts/joyword/verify-r8-rollback.py` 将验收断言改为始终执行的检查。此前 Python 优化模式会移除 `assert`，连同其中的认证拒绝请求、编辑命令和回退后 CLI 重开操作一起跳过，不能用该模式的输出证明这些步骤通过。现在失败会抛出带原因的错误，候选验证失败仍通过既有 `finally` 路径停止进程并恢复旧目录。

`scripts/tests/test_joyword_rollback.py` 在临时目录验证候选校验失败和候选目录缺失时的旧目录恢复，用替代控制响应验证认证异常拒绝与进程清理，并在 `python -O` 子进程重跑认证反例。完整 Windows 验证在其他系统上会在创建输出前明确拒绝；平台无关的脚本回归可在 Linux 执行。5 项本地测试通过；这不是 Windows 安装、注册关联、真实候选程序或原生界面验收，JW-806/812 仍为部分验证。

### 五批 P0 本地组合验证（2026-11-06）

在本地 `fix/joyword-p0-integration` 合入以上五批修改，代码无冲突；状态文档保留各批证据。组合代码通过 72 项 shell 测试、161 项 Python 脚本测试（2 项跳过）、生产库 Clippy、全仓格式及文档检查。整合后的 CLI 已构建并运行真实文件的打开、编辑、保存重开、拒绝覆盖、PNG/PDF、邮件合并、格式交换损失确认、EPUB 和页面设置验证。64 个损坏 DOCX 中 21 个解析、43 个正常拒绝，无 panic。Poppler 独立读回 PDF 中的编辑标记；未运行 qpdf 结构验证或独立 PDF/A 合规验证。

代码提交、五批来源、命令和产物 SHA-256 见 [组合验证记录](evidence/p0-linux-integration.json)。整合分支仅在本地，未创建 PR、未触发远程 CI、未合入 main；原分支历史保留，无强推。Windows/macOS 实屏、真实崩溃、正式安装与关联回退、旧新版 CLI 二进制差分仍待验，不据此关闭 JW-806/808/812。

## R0–R7 待完成范围

| 阶段 | 当前宿主进展 | 仍需关闭的验收范围 |
| --- | --- | --- |
| R0–R2 基线、外壳与阅读器 | 来源已固定，当前产品从宿主构建。CLI 打开和导出有真实文件及固定字体证据。 | 完整功能和协议并集、来源同机对照、原生字形边界、全套冻结语料。 |
| R3 编辑与恢复 | 单一 Session 执行键盘编辑、保存及恢复，已有 DOCX 厂商 XML 保存回归。JW-306 接入 GPUI 图片粘贴，使用插图的输入预算和事务。 | 图片及剪贴板实屏、宿主三端输入、真实崩溃恢复与复杂旧日志差分。 |
| R4 常用编辑 | JW-406／407 已接入选区查找替换、大纲、高级分节分栏。JW-408 接入富文本粘贴。JW-409／413／414 提供页眉页脚、PAGE 字段、格式标记和 Windows 打印预览，并有保存重开及 PDF 栅格回归。 | 查找、版式、Office／浏览器粘贴、页眉页脚和预览实屏，标尺、其余视图选项和非 Windows 预览。 |
| R5 引用、审阅与格式交换 | JW-509 接入 EPUB 导出和差异确认。JW-514 接入可选本地提案。JW-515 的认证活动文档控制包含跳页、数值缩放、字面查找和单页／页宽适配。macOS 上已有 Word/WPS 代表性双向往返。 | EPUB 阅读器、WPS 专项交换、保护与密码、复杂故事审阅、其他格式的损失确认、校对与 RTL、多窗口、Agent Provider 和其余桌面控制范围。 |
| R6 性能 | macOS 千页 CLI 和键盘到 GPUI 提交已有测量。Windows 补测千页 CLI 全流程 5 轮。 | 物理呈现、原生绘制对比、跨对象编辑、同条件基线和平台性能预算。 |
| R7 平台与分发 | Windows、macOS 和 WSLg 开发验证分别记录。JW-705 接入 Windows 打印入口。JW-708 增加 Windows 便携换版与回退流程。 | Windows 本轮 IME／读屏／高 DPI，macOS IME／VoiceOver，Linux 原生显示协议、输入和无障碍，实屏打印取消、真实队列、其他平台打印、安装、关联、签名及卸载。信创真机按宿主约定后补。 |

系统集成进展见 [Windows 系统集成与回退复验](VALIDATION.md#windows-系统集成与回退复验2026-11-05)。此前的格式与旧 CLI 证据分别见 [Windows 格式与编辑复验](VALIDATION.md#windows-格式与编辑复验2026-11-05) 和 [Windows 兼容性复验](VALIDATION.md#windows-兼容性复验2026-11-05)。

### JW-812 Linux CLI 隔离回退（2026-11-06）

新增 Linux CLI-only 换版验证入口，实测旧版与当前 main CLI 的失败复原、成功换版后回退、V3 快照逐字节恢复和旧版读回；5 个隔离原文件及 26 个语料文件未改变。验证范围不包含发布包、安装器、关联和真实窗口，不将 JW-812 标为完成。详见[剩余任务与实测证据](../../joyoffice/validation/20261106-remaining-work/README.md)。
