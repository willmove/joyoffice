# JoyWord 实施任务

版本 0.3 · 2026-10-17。需求以 [SPEC](SPEC.md) 为准，验收以 [QUALITY](QUALITY.md) 为准。状态证据见 [IMPLEMENTATION](IMPLEMENTATION.md)。

## 任务规则

- 状态仅为 planned、in_progress、blocked、done。没有验证证据不能标 done。
- 状态前的复选框由检查器与状态绑定：只有 `done` 使用 `[x]`，其余状态一律使用 `[ ]`。
- 需求编号、前置依赖、输入、产物（输出）、错误路径、自动验证、人工验收与完成证据是必填字段。
- 任务的程序性前置与阶段发布门槛分别检查；可提前开展独立任务，但不能提前宣称阶段通过。
- 2026-10-18 用户决策：信创真机验证有条件时补验，不阻塞开发、任务依赖、阶段验收或发布。T005 及 T700–T703 的信创真机子项明确记为“未验证／待补验”；仅因这些子项未完成的前置任务不连带阻塞下游，其他实现与验证要求照常检查，不把待补验记为 done。
- 状态只陈述本仓库实际工作；后续路线图不等于本次全部实施完成。
- M0→M1→M2→M3 为主线；M4／M5 可在依赖满足后分别推进；M6 汇合到 M7。M8 是 1.0 之后的扩展，M9 仅为下一轮规格入口。
- 前置只写技术依赖；阶段发布顺序由门槛任务（T107、T206、T304、T403、T704）串联，M0–M7 的工作任务不以其他阶段的门槛任务为前置；M8、M9 任务以 T704 为前置，表示在 1.0 之后开展。M0–M7 的任务都必须在 T704 的前置闭包内（检查器校验）。

## 进度总览

- [x] **已完成（`done`）**：任务要求的实现、自动验证和人工验收均已有证据。
- [ ] **未完成（`in_progress`／`planned`／`blocked`）**：即使已有部分代码或测试，只要整个任务尚未完成就不打勾；已完成部分与剩余项见各任务的“完成证据”。

| 阶段 | 已完成 | 进行中 | 未开始 | 阻塞 | 合计 |
| --- | ---: | ---: | ---: | ---: | ---: |
| M0 | 4 | 2 | 0 | 0 | 6 |
| M1 | 7 | 4 | 1 | 0 | 12 |
| M2 | 2 | 9 | 1 | 0 | 12 |
| M3 | 0 | 4 | 1 | 0 | 5 |
| M4 | 0 | 3 | 1 | 0 | 4 |
| M5 | 0 | 1 | 4 | 0 | 5 |
| M6 | 0 | 4 | 2 | 0 | 6 |
| M7 | 0 | 0 | 5 | 0 | 5 |
| M8 | 0 | 0 | 9 | 0 | 9 |
| M9 | 0 | 0 | 2 | 0 | 2 |
| 总计 | 13 | 27 | 26 | 0 | 66 |

T400 于 2026-10-21 改为 `in_progress`。T302 第一切片也是 `in_progress`。2026-10-22 六分支波次之后 T301 与 T401 也由 `planned` 改为 `in_progress`；2026-10-23 第二波的 T402 有界替换提案与只读预览进入独立审查，状态改为 `in_progress`。所以 2026-10-23 各波次后上表为 12 done／22 in_progress／32 planned／0 blocked；2026-10-26 T001 收口后为 13 done／21 in_progress／32 planned／0 blocked（见下文该段）。任务总数仍是 66，切片交付不等于整个任务完成。2026-10-23 第二波四路并行切片落地：T104 生产恢复双代、T204 PDF 书签内部跳转、T202 章节 PAGE 编号与 T402 有界替换提案与只读预览，四项各自进入 `in_progress`，没有一项被标成 `done`；详细证据见各任务的完成证据与 [验收清单](acceptance/README.md)。2026-10-23 第三切片（无 PR，按用户指令直接并入 main）：T402 跨段有界替换与段落标记删除修订（`w:pPr/w:rPr/w:del`）落地，同时补齐 AI 修订的 DOCX 往返与对照器修订携带；T301 的跨段跟踪删除限制随之解除（表格／内容控件边界仍拒绝）。布局的合并渲染（最终视图中删除标记的相邻段仍逐段排版）是如实记录的已知边界，留待后续切片。

**当前判断：项目处于 M0／M1 技术预览阶段。** T000、T001、T002、T004（M0）与 T100、T101、T102、T105、T106、T108、T111（M1）已完成：三端参考环境的固定工具链与可启动产物已核对、cargo-deny 在 CI 与 Mac 本机通过；中文输入（含系统 IME 与字素编辑）、基础格式、系统剪贴板、另存与重开、长文档滚动缩放等路径已有自动测试，并分别在 Windows 参考环境与 2026-10-18 Mac GUI 会话（用户在场＋屏幕自动核对）复验；Word 2024 与 WPS 12.1.29181 双固定参考环境已建立并读回核对（WPS 参考 PDF 因本机导出通道故障待补）。剩余：T003 的 Retina／多显示器（硬件不具备）、T005 的信创真机专项（按 2026-10-18 用户决策为非阻塞待补验）、T109 的慢打开只读与保存中关闭提示、T110 的精确 ACL／命名流与非 NTFS／网络盘，以及 T103 的 WPS 生成样例与 Word／WPS 人工交换（行为特征语料扩展 fuzz 与 2026-10-20 的代码覆盖引导 fuzz 已有自动证据，见下文验证波次收口；T103 仍 `in_progress`）。T107 在前置 T109、T110 完成前保持 `planned`，M1 发布门槛未通过；M2 的 T200（样式继承解析、主题字体／颜色、可撤销的样式应用、多级编号与制表位、字素查找替换、格式刷、样式面板、`style.list`／`style.apply`／`list.apply`／`list.level`／`list.remove`／`find.next`／`find.replace`／`find.replace_all`／`format.copy`、样式指定与段落 numPr 导出，以及排版与段落缓存消费；按字体名换面、罗马数字 0 与负数、超过 256 个标量的编号标签、bar／middleDot／heavy、编号部件改写和人工验收未做；甲乙超过 10 按天干重复、中文计数覆盖 0 与非负 i32、正数罗马数字超出 3999、字母重复超过 64 均在 256 个标量预算内绘制，主题色文字按方案 sRGB 绘制并在改写 run 时写回 themeColor；项目符号、级别标签字号与粗体、字母超过 26、列表与制表位上的居中／右对齐／两端对齐、主题 tint／shade，以及表格样式的粗体、底纹和边框已在第四切片落地，全角半角折叠与正则查找已在查找折叠切片落地，正则替换的 `$1`..`$n` 与 `$$` 已在捕获组切片落地）、T201（合并与拆分、垂直合并的视觉跨行、跨页拆行、表头重复、表级边距、单元格 tcMar、nil／single／double／dotted／dashed／thick 边框、有主题时的主题色边框，以及百分比宽度和 fixed／autofit、单元格 vAlign、行 trHeight，以及表格属性对话框已落地；一层嵌套表、首列末列与纵向条纹排版，以及样式部件 firstCol／lastCol／band1Vert／band2Vert 读入已有自动测试；列宽拖拽、第二层嵌套和合同表格人工验收未做）、T202（页面几何、显式分页符、页眉页脚三种变体、正文与页眉页脚的 PAGE／NUMPAGES／SECTIONPAGES、pgNumType、下一页／连续／奇偶分节和等宽分栏已落地；widowControl、页眉编辑、页面设置写回、页眉页脚部件写回和不等宽分栏已有自动测试；图形界面验收、栏分隔线、`nextColumn`、章节页码和 Word 对照未做）、T203（行内图片、简单超链接、安全 HTML／RTF 粘贴与插入图片按钮已落地；拖放与表格内超链接已落地并有自动测试；页面绝对 wrapNone 浮动图、页眉页脚行内图片和页面相对 wrapSquare 已有自动测试；页面相对 wrapTight／wrapThrough 按锚点范围近似为方形绕排；wrapTopAndBottom、衬于文字下方、非页面相对位置和 Word／WPS／浏览器粘贴未做）、T204（文件菜单导出 PDF 与打印到文件消费屏幕已安装的同一 PageScene；标题、Producer、子集字体 ToUnicode、分节纸型、图像 XObject、标题样式书签，以及 http／https／mailto 的 URI 链接注释有自动测试；仅书签锚点不写成跳转；系统打印对话框未验证，UOS 上 qpdfview 与 WPS PDF 双阅读器已补验）、T205（功能区、状态栏、字体与段落对话框、标尺缩进与制表位拖拽、单线／双线／点线下划线与 sRGB 绘制、以及编辑标记／隐藏文字／域代码三个视图开关已落地；UOS 三开关与缩放已补验并修复一处栅格断言；Word 字数对话框对照、导航窗格、审阅栏和只用功能区完成报告排版的人工验收未做）、T207（有证据的测量已接入排版入口，人工对照未做）、T208（段落缓存与增量重排已接入编辑路径，百页首段／末段人工验收未做）、T209 撤销组（键入、IME 与粘贴已做 Windows GUI 分步撤销；拖动／自动更正／审阅模式边界未做）、T210（五个原创样例的 inspect／text／render／PDF 已有自动测试且两次输出逐字节相同，并已用 Windows GUI 的 DOCX-MIN-001 实际窗口与 CPU render 对照）已完成，T211（应用日志、panic hook 与测试 profile 已接入，2026-10-19 银河麒麟与统信 UOS `kill -9` 崩溃后日志完好且无正文泄漏的人工验收已执行）已完成；M3 的 T300 文字与移动修订进行中（页边标记、审阅命令、单元格文字修订、成对移动和页眉页脚标记已有自动测试；Word／WPS 人工交换未做），T302 批注线程第一切片进行中（只有模型和 DOCX 自动测试，见下方 2026-10-21 说明），T301 段落与运行格式修订也在进行中（跨段跟踪删除已解除限制，表格／内容控件边界仍拒绝），M3 其余任务（T303、T304）尚未开始；M4 的 T400、T401、T402 已有进行中的 AI 切片（Provider 关闭基础、选区授权与受限工具、有界替换提案与跨段修订），门槛 T403 与 M5–M9 尚未开始，不能据此宣称 1.0 或完整 Word 兼容已完成。 2026-10-19 银河麒麟桌面版 V10 SP1（x86_64）真机会话补充验证并登记九项发现：工具链与 SwiftShader 软件渲染窗口启动、fmt／clippy／各 crate 测试通过（joyword-shell 2 项失败为本机 KYSEC 扩展属性与内核 5.4 mtime 粒度所致）、WPS 麒麟预装版打开 JoyWord 导出副本读回一致、kill -9 恢复与崩溃日志无泄漏（T211 据此完成）、CLI PDF 双独立阅读器读回一致、部分 GUI 人工验收通过；同时发现混合格式整画布拒绝（已由 `da5f192` 修复并复验）、功能区项目符号按钮缺 `bullet` 参数、X11 无菜单栏且 Ctrl+Shift+E 被 UKUI 全局快捷键抢占、文件打开对话框不出现与 Fcitx 未接入 GPUI X11，详见 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json)。

本轮验证更新（2026-10-19）：Windows 基线 652 项无头测试、38 项应用适配器测试、恢复实验与固定健壮性矩阵已复跑；独立 PDF 读回发现并修复合成粗体文字重复，五样例光栅对照发现并修复亚像素下划线消失。用户确认 GUI 时段后，Windows GUI 完成真实系统 IME、系统剪贴板、连续打字/IME/粘贴撤销、默认另存、显式替换、连续保存和 DOCX-MIN-001 窗口/CPU render 对照。T210 的自动与人工验收均有证据，状态改为 `done`；T211 随后也因麒麟与 UOS 的真实 `kill -9` 会话完成；计数更新为 12 done／17 in_progress／37 planned。完整证据、缺口与后段 Computer Use 管道故障见 [本轮验证审计](VALIDATION-20261019.md)。

统信 UOS 补充验证（2026-10-19）：在 UOS Desktop 20 Professional（x86_64，KVM 海光 C86，软件 EGL + SwiftShader Vulkan）完成构建、窗口、中文剪贴板编辑、缩放、视图开关、`kill -9` 恢复、CLI PDF 与 qpdfview／WPS PDF 双阅读器检查；Fcitx 未接入 GPUI X11，文件打开/保存对话框不可达。补验中发现并修复隐藏文字切换触发的 tiny-skia 栅格断言（亚 2 px 分数坐标矩形），新增回归测试；裸启动默认 Vulkan 无可用设备、GL surface 不兼容时，应用现在自动寻找 SwiftShader／lavapipe ICD 并带环境重启一次，实测无需手工设置 `VK_ICD_FILENAMES` 即可出窗。环境、自动测试差异、截图和日志见 [UOS 报告](../../tests/fixtures/reference/uos/uos_report.json)。

2026-10-20 文档核对：上面进度总览里有几处「未做」已被后续合并取代，任务状态和计数不改（仍是 12 done／17 in_progress／37 planned／0 blocked）。T201 的一层嵌套表和首列、末列、纵向条纹排版已有测试；仍未做列宽拖拽、更深嵌套、从样式部件读入这四条条件，以及合同表格人工验收。T202 的 widowControl、页眉编辑、页面设置写回、页眉页脚部件写回和不等宽分栏已有测试；仍未做图形界面验收、栏分隔线、`nextColumn`、章节页码和 Word 对照。T203 的页面相对 wrapSquare 已有测试；仍未做 wrapTight／Through／TopAndBottom、衬于文字下方、非页面相对位置和 Word／WPS／浏览器粘贴。T300 已超过第一切片：页边标记、审阅命令、单元格文字修订、成对移动和页眉页脚标记已有测试；Word／WPS 交换仍未做。T200 查找折叠切片里「`$1` 不展开」已被捕获组切片取代。没有任何 M2 或 M3 任务同时具备自动验证和人工验收，因此没有一项改为 `done`。原子准则见 [验收清单](acceptance/README.md)。本段里 T201「从样式部件读入这四条条件」和 T203 的 wrapTight／Through 两项，随后由 [#67](https://github.com/willmove/joyoffice/pull/67) 取代：前者已读入，后者近似为方形绕排（不是轮廓绕排），见下一段。

2026-10-20 验证波次收口：五个验证切片已合入 main，分别是 S4 验收清单与功能指南（[#61](https://github.com/willmove/joyoffice/pull/61)，`1bae1231`）、S2 T103 覆盖引导 fuzz（[#62](https://github.com/willmove/joyoffice/pull/62)，`8a12e6dd`）、S5 场景无界面排练（[#63](https://github.com/willmove/joyoffice/pull/63)，`e615d308`）、S1 页面场景金样（[#64](https://github.com/willmove/joyoffice/pull/64)，`0c80d8bb`）和 S3 特性样例（[#65](https://github.com/willmove/joyoffice/pull/65)，`f0ccc659`）。证据见 [覆盖引导 fuzz](verification/t103-coverage-fuzz.md)、[场景排练](verification/scenario-rehearsals.md)、[页面场景金样](verification/golden-scenes.md)和[特性样例](verification/feature-fixtures.md)。这些都是自动证据，不是人工验收；任务状态和计数不改（仍是 12 done／17 in_progress／37 planned／0 blocked）。收口时验证文档报告的已知缺口，五条已由 [#67](https://github.com/willmove/joyoffice/pull/67)（main `caf84147`）修复：T201 样式部件里的首列、末列和纵向条纹条件现在读入模型；CPU 光栅接受编号加制表位或点线引导符的行；T300 的 `text` 投影不再输出删除和移出的文字；T203 的 `wrapTight`／`wrapThrough` 不再让排版拒绝、编辑导出不再返回 `Restricted`，但只按锚点范围近似为方形绕排，不是真正的轮廓绕排；T202 对已有正文段落插入页码后可以导出，但只限无行内对象段落里的无格式域，带格式域、超链接里的域和原有行内对象的段落仍拒绝。T203 四条图片关系另报 `not-imported` 的诊断不在 #67 范围内，已由 [#69](https://github.com/willmove/joyoffice/pull/69) 修复（资源已导入的图片关系不再另报，未消费的关系仍报告）。修复说明见 [验证波次缺口修复](verification/verify-wave-fixes.md)。这些仍是自动证据，任务状态和计数不改。收口记录与 main `f0ccc659` 上的整套 CI 等价复跑见 [IMPLEMENTATION](IMPLEMENTATION.md)「2026-10-20 验证波次收口」。

2026-10-21 文档同步（对照 main `f7cbeadd`，当时任务状态与计数不改，仍是 12 done／17 in_progress／37 planned／0 blocked）：[#71](https://github.com/willmove/joyoffice/pull/71)（`e8c47764`）把 W09 生产目标从手工重锚的固定提交改为当前检出已提交的 `crates/` 树 id（`git rev-parse HEAD:crates`）；`.github/workflows/ci.yml` 与 `.github/workflows/fuzz-nightly.yml` 去掉 `fetch-depth: 0`。[#70](https://github.com/willmove/joyoffice/pull/70)（`f7cbeadd`）把 2026-10-19 银河麒麟桌面版 V10 SP1 修复会话的机器侧证据落进 main：`tests/fixtures/reference/kylin/` 的截图、`min002_export.pdf` 与更新后的 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json)（含逐项 status、`fix_session` 与 `main_landing`）。该分支的代码没有整体合入；等价改动此前已由 PR #44、#46 与 `4bf14f2` 落到 main，分支变体与 main 变体的差异见 [IMPLEMENTATION](IMPLEMENTATION.md)「麒麟缺陷修复会话的机器侧证据」。这两项都不是人工验收关闭，没有任务改为 `done`。阶段口径在该同步点是：M0／M1 仍是技术预览，M1 多数任务已完成，T107 在 T109、T110 完成前保持 `planned`；M2 九项进行中并已有大量自动覆盖，T210 与 T211 为 `done`，门槛 T206 仍 `planned`；M3 当时只有 T300 `in_progress`；M4 四项均未开始。

2026-10-21 T302 第一切片开始：状态从 `planned` 改为 `in_progress`。当时计数改为 12 done／18 in_progress／36 planned／0 blocked；T400 随后也改为进行中，当时上表为 12 done／19 in_progress／35 planned／0 blocked。这一刀只有批注模型和 DOCX 导入导出的自动测试，没有 Word／WPS 人工交换，不标 `done`。见 [IMPLEMENTATION](IMPLEMENTATION.md)。

2026-10-21 T203 上下型绕排：页面相对、位于文字前面的 `wrapTopAndBottom` 已有自动测试。正文与图片带垂直相交的行整行放到带下方，不排到两侧。原绕排 XML 照原样写回。这是 JoyWord 假设，不是 Word 观察。真正的轮廓绕排、衬于文字下方、非页面相对位置和 Word／WPS／浏览器粘贴仍未做。状态保持 `in_progress`。计数不改。

2026-10-22 六分支并行波次：T110 的 Windows 卷能力观测、T200 的 `numbering.xml` 写回、T202 的栏分隔线与 `nextColumn`、T301 的段落与运行格式修订、T401 的选区授权与受限工具这五个实现分支，加上 review 分支的 CLI JSON 键序修复，曾在集成分支 `chore/integration-wave-20261022` 组合验证；这六项与 T111 门禁加固 [#86](https://github.com/willmove/joyoffice/pull/86) 现已全部合入 `main` `85a24de`，该主干 CI 九项全绿（2026-10-23 核验）。T301 与 T401 由 `planned` 改为 `in_progress`：两者已有真实实现和自动测试，但没有 Word／WPS 往返、图形界面、信创真机或打印的人工验收，按任务规则不能标 `done`。计数改为 12 done／21 in_progress／33 planned／0 blocked，任务总数仍是 66。这一波没有任何任务改为 `done`。各切片的实现说明见 [IMPLEMENTATION](IMPLEMENTATION.md)，T200 与 T202 的原子准则见 [验收清单](acceptance/README.md)。

2026-10-27 T500 开始：M5 第一个任务由 `planned` 改为 `in_progress`（计数随之改为 13 done／22 in_progress／31 planned／0 blocked，任务总数 66）。第一切片恢复带栏分隔线文档的稳定页尾复用（T202 记录的已知性能缺口），并补上百页混合分节长文档、长文档取消与屏幕／PDF 几何对照的自动证据；没有人工验收，不标 `done`。见 T500 的完成证据与 [IMPLEMENTATION](IMPLEMENTATION.md)。

2026-10-26 T001 收口：核对发现 TASKS 此前记录的两个缺口中，Windows 腿其实已有证据——2026-10-19 Windows GUI 会话（[验证审计](VALIDATION-20261019.md)）已核对 rustc／cargo 1.98.1，并以隔离测试 profile 启动 `JoyWord · Preview` 完成 16 步截图，fmt／clippy／652＋38 项测试通过，只是该机缺 cargo-deny；「Windows 腿仍未做」一句出自同日麒麟会话的提交 `8b7f9c2`，两条会话并行，Windows 证据合入 main 后没有归并回 T001。真正缺的 cargo-deny 本机复核于本日在 Mac 参考机完成（cargo-deny 0.20.2，`--locked --all-features`，公告库 `21d909e4`，advisories／bans／licenses／sources 四项 ok），并在 main `5efe06a` 复核工具链、fmt 与不启用 `dev-runtime-shaders` 的应用构建，该提交 CI [40280336418](https://github.com/willmove/joyoffice/actions/runs/40280336418) 九项全绿。T001 由 `in_progress` 改为 `done`，计数改为 13 done／21 in_progress／32 planned／0 blocked，任务总数仍是 66。本次只改文档，未改代码与依赖；M0 退出条件中的信创真机专项仍属 T005 非阻塞待补验，不因 T001 完成而改变；[2026-10-19 验证审计](VALIDATION-20261019.md) 关于 T001「Windows 参考环境仍缺」的两处表述是当日快照，以本段为准。

## 阶段与任务

## M0：工程与关键验证

退出条件：真实 Mac 窗口、中英文显示／IME、统一字体场景 PDF、三端构建、信创构建与可用环境中的渲染退路验证及语料框架全部通过；信创真机专项待补验，不阻塞退出。

### T000：规格落盘与追踪

- 输入：用户批准的 JoyWord 计划、仓库 MIT 许可与已核对的公开资料。
- [x] 状态：`done`
- 需求：`QUALITY-001`、`QUALITY-002`、`QUALITY-008`
- 前置：无
- 产物：八份专题文档、入口、状态和需求—任务映射。
- 错误路径：来源失效、文档冲突、漏映射必须显式诊断。
- 自动验证：文档检查器验证 ID、依赖图、本地链接、需求覆盖和任务字段。
- 人工验收：按 README 阅读一遍主工作流，检查已确认决策均保留。
- 完成证据：2026-10-17 文档检查器通过（84 个需求、66 个任务、依赖无环、阶段一致、M0–M7 均在 T704 前置闭包内、12 份 Markdown、10 个登记用例、本地化键一致、9 个 Actions 按 commit SHA 固定）；人工验收已执行——按 [README](README.md) 复核阅读顺序、解释优先级、状态规则与 v0.3／v0.2 变更记录中的已确认决策（信创进入 1.0、DOC 导入属 1.0、WPS 原生格式导入、M8／M9 以 T704 为前置、PROVENANCE 禁止条款恢复），逐条核对无丢失。审查与验证细节见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T001：工具链、依赖锁与三端 CI

- 输入：架构依赖边界、宿主环境清单、Rust／GPUI 版本及各平台构建要求。
- [x] 状态：`done`
- 需求：`PLATFORM-001`、`QUALITY-002`
- 前置：`T000`
- 产物：固定 Rust 工具链和按 ADR-010 选定的 GPUI 来源；Cargo.lock；三端构建工作流（Actions 按 commit SHA 固定）；依赖许可与安全公告检查；环境检查记录。
- 错误路径：缺少 Xcode／Metal 工具链（Xcode 26 起需单独下载）、Linux 系统库、Windows SDK 时显示对应原因，不假称平台不支持。
- 自动验证：三端执行 locked build、fmt、clippy、核心测试和 cargo-deny；开发 shader 与发行路径分开。
- 人工验收：在三台参考环境核对工具链及可启动产物；CI 配置不算成功运行证据。
- 完成证据：三端 CI 运行 38481758247（提交 ac40043）全部通过：三端 fmt、40 项核心测试、clippy 与不启用开发着色器的应用构建，以及 cargo-deny；Actions 按 SHA 固定。2026-10-17 本机 Xcode 27.1 下不启用 `dev-runtime-shaders` 的普通构建已通过。GPUI 来源（ADR-010）的屏幕字形核对已在 Mac 执行（宋体衬线渲染确认，见 T003）；2026-10-18 Mac 参考环境腿已核对（Rust/Cargo 1.98.1、Xcode 27.1、macOS 27.0.1 arm64，`target/debug/joyword` 多次启动含 kill -9 后重启恢复）。2026-10-19 Linux 真机参考环境腿已在银河麒麟桌面版 V10 SP1（x86_64）核对：rustup 安装 1.98.1（minimal + clippy + rustfmt，与工具链文件一致），`cargo build -p joyword --locked`（不启用 dev-runtime-shaders）成功并启动窗口；fmt、全工作区 clippy `-D warnings`、各 crate 测试在本机通过（joyword-shell 2 项失败为本机环境特性，见 T110 与 kylin_report.json）。2026-10-19 另在统信 UOS Desktop 20 Professional（x86_64，海光 C86 虚拟机）核对第二条 Linux 参考环境腿：rustup 1.98.1（minimal + clippy + rustfmt）；因 UOS 的 `libfontconfig1-dev` 与已装 freetype 版本冲突，补装 g++/cmake 与 xcb/xkbcommon 开发包后，添加本地 `/usr/local/lib/pkgconfig/fontconfig.pc` 与 `libfontconfig.so` 链接（或使用 `RUST_FONTCONFIG_DLOPEN=1`），随后裸 `cargo build -p joyword --locked` 成功并启动窗口，fmt、clippy、文档检查与门禁通过，未引入仓库依赖。Windows 参考环境腿已于 2026-10-17 与 2026-10-19 核对（Windows 11 build 26300，x86_64；`rustc 1.98.1 (48a229cea)`、`cargo 1.98.1 (797e8a9bc)`；不带特性标志的 `cargo build` 编出 `joyword`，`JoyWord · Preview` 窗口以隔离测试 profile 启动并完成 16 步截图操作；`cargo test --locked` 652 项、`cargo test --locked -p joyword` 38 项、全工作区 clippy `-D warnings` 与 fmt 通过；当时该机未装 cargo-deny），见 [2026-10-19 验证审计](VALIDATION-20261019.md) 与 [Windows 证据](validation/20261019/evidence.json)。cargo-deny 本机复核已于 2026-10-26 在 Mac 参考机完成：cargo-deny 0.20.2（与 CI 固定版本一致）执行 `cargo deny --locked --all-features check`，RustSec 公告库 `21d909e4`（2026-10-24），advisories／bans／licenses／sources 全部 ok；同日在 main `5efe06a` 复核 Rust/Cargo 1.98.1、Xcode 27.1（27A9275）、macOS 27.0.1（26A434），`cargo fmt --all -- --check` 与不启用 `dev-runtime-shaders` 的 `cargo build -p joyword --locked` 通过，该提交的 CI 运行 [40280336418](https://github.com/willmove/joyoffice/actions/runs/40280336418) 九项作业全部通过（四平台构建、ARM64／LoongArch64 QEMU 测试、unified-pdf、dependencies、provenance）。三台参考环境的工具链与可启动产物均已核对，自动验证与 cargo-deny 的 CI 及本机证据齐备，T001 标记 `done`；信创真机专项属 T005，Retina／多显示器属 T003，均不在本任务范围。见 [IMPLEMENTATION](IMPLEMENTATION.md)「T001 收口」与 [UOS 报告](../../tests/fixtures/reference/uos/uos_report.json)。

### T002：GPUI 原生窗口与页面投影

- 输入：固定 GPUI API、物理单位约定和中英文工程样例文字。
- [x] 状态：`done`
- 需求：`PLATFORM-001`、`PLATFORM-003`、`LAYOUT-001`
- 前置：`T000`
- 产物：JoyWord 工程验证窗口、A4 页面、缩放、中英文资源和正常退出。
- 错误路径：窗口／GPU 初始化失败（含无 Vulkan 的 Linux、无独立 GPU 的虚拟机）返回诊断；不创建假编辑或保存按钮。
- 自动验证：构建 app；检查物理单位换算与缩放不改变页面尺寸。
- 人工验收：启动窗口，切换语言、缩放、关闭；确认明确标注 M0 工程样例。
- 完成证据：自动验证（构建、物理单位换算与缩放测试、三端 CI 构建）通过；2026-10-14 用户在 macOS 上运行 `cargo run -p joyword --features dev-runtime-shaders --locked` 人工确认中英文切换、缩放和正常关闭均正常，窗口标注为 M0 工程样例。输入和 PDF 不属于此任务完成证明。见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T003：字体、IME 与统一 PDF 技术验证

- 输入：按 ADR-010 选定来源的 GPUI 输入／字形 API、许可明确的固定字体（OFL，如思源宋体或 Noto CJK 与 Carlito）、原创中英混排及组合输入样例。
- [ ] 状态：`in_progress`
- 需求：`LAYOUT-001`、`LAYOUT-002`、`LAYOUT-009`、`PLATFORM-004`、`EDIT-002`
- 前置：`T001`、`T002`
- 产物：固定字体字节／face 的整形、屏幕与 PDF 定位验证，按字体字节取得 FontId 的验证或补丁，以及 IME 验证程序。
- 错误路径：字体映射不一致、缺字、surrogate 范围非法、组合取消和字形 API 缺口有明确记录。
- 自动验证：同一字形场景几何比较；Unicode 范围和组合态测试；PDF 文本抽取。
- 人工验收：系统拼音提交／取消、候选框、一次撤销、多显示器 DPI；PDF 独立阅读器检查。
- 完成证据：整形—页面场景—PDF 链路已实现（joyword-layout、joyword-pdf）：同一字体字节整形并写出可检索 PDF，独立读回验证。W02 独立 macOS 探针以 GPUI 0.2.2 最小可见性补丁验证外部字形绘制，协调者复验实验测试与已有截图；C06 已实现实际 FontKey 的 CPU 场景消费者并接入只读窗口，离线 PNG／PDF 光栅位置对照通过，见 [C06 报告](../coordination/reports/C06-CPU-CANVAS.md)。2026-10-17 PDF 独立阅读器检查已在 Windows 参考环境执行：`document_flow` 场景 PDF（2 页 32 行，Noto Serif SC VF）经 xpdf `pdftotext` 与 pdfplumber（pdfminer.six，独立实现）双阅读器逐页文字一致、pdfplumber 行首偏差 0.000 pt，两阅读器与场景互相一致，见 [IMPLEMENTATION](IMPLEMENTATION.md)。2026-10-18 Mac GUI 会话补验：屏幕字形经截图判读确认为宋体衬线（画布加载系统 Songti.ttc，与 CPU/PDF 同一份字体字节，未落入无衬线回退）；系统拼音候选框贴近插入光标（用户实测）；PDF 双独立阅读器核对已于 2026-10-17 在 Windows 完成。Retina 与多显示器 DPI 未验证（本机仅一台 1920×1080 1x 显示器），T003 保持 `in_progress`，见 [IMPLEMENTATION](IMPLEMENTATION.md) 和[W02 审查](../coordination/reports/W02-REVIEW.md)。 2026-10-19 Windows GUI 补验：真实系统中文输入法以 `nihao` 组成、候选窗跟随光标，候选 1 为“你好”；空格提交，Esc 取消无文档变化，提交后一次 Ctrl+Z 删除两个字。2026-10-19 统信 UOS Desktop 20 Professional 补充：系统 Fcitx 4.2.9.32.49 与搜狗输入法处于活动状态，但 GPUI X11 窗口不接入 IME——物理键入 `nihao` 直接以 ASCII 进入文档，无候选框；中文只能经系统剪贴板粘贴（粘贴与状态栏计数通过）。PDF 仍由同一字体字节写出，qpdfview 与 WPS PDF 两个独立阅读器均可检索（见 T204）。Retina、多显示器 DPI 与 UOS IME 仍未验证，T003 保持 `in_progress`。

### T004：原创语料与验证框架

- 输入：需求清单、规范元素列表、原创文本，以及固定参考环境（Word 为 Word 2024 永久版，观测记录绑定确切 build；中文互操作对照固定版本 WPS）和字体。
- [x] 状态：`done`
- 需求：`QUALITY-001`、`QUALITY-003`
- 前置：`T000`
- 产物：case registry、原创文本、来源信息、预期行为、语义／几何比较接口，以及固定 Word 导出 PDF 的字形坐标抽取与行／页断点比较工具；排版行为规则目录随实测建立。编辑行为登记表把每条规定行为写成“操作 → 固定 Word 观测记录（版本、样例 ID、日期、截图散列）→ 测试名”；外部项目的已固定规则只作待验证假设登记，不直接当作规则。
- 错误路径：缺失字体／参考结果必须标未验证；非法清单和未知需求 ID 拒绝。
- 自动验证：清单格式、文件散列、ID 唯一性和需求引用检查；最小样例可重复生成；检查器校验行为登记表引用的测试名存在。
- 人工验收：审查样例无敏感内容；确认原始文字与正式 DOCX 夹具区别。
- 完成证据：清单与检查器、模型语义比较、PDF 行／页断点比较工具（CI 端到端：相同排版通过、改变换行被检出）、5 个确定性生成的原创最小 DOCX 样例已落地；2026-10-17 编辑行为登记表落地：[behavior_registry.json](behavior_registry.json)（schema 1，7 条假设条目均绑定现有测试名），`check_docs.py` 校验唯一 ID、假设不携带观测／规则必须带完整 Word 观测字段、引用的测试名在工作区真实存在，并有 10 项单元测试（`scripts/tests/test_behavior_registry.py`）。2026-10-18 固定 Word 参考结果已建立：Word 2024 永久版（家庭版 2024，build 16.0.20430.20182，Windows 11 x86_64）COM 只读打开 6 个格式样例并导出 PDF（禁链接更新与宏、位图字体替代关闭），逐样例保存 PDF、文本读回与 pdfplumber 几何抽取记录于 `tests/fixtures/reference/word2024/`，清单的参考档案置为 established；Word 实测字体：默认中文为等线（DengXian），DOCX-MIN-002 的显式样式解析为宋体＋Times New Roman；DOCX-MIN-005 报保护类型 3（只读限制）且强制修订开启。2026-10-18 排版规则目录建立首批实测条目（[layout_rules.md](layout_rules.md) R1–R9）：同页框同度量下 Word 与 JoyWord 断行逐行一致（两处对照，行位偏差 ≤1 pt）。2026-10-18 人工验收已执行：逐份审查 4 个文本种子与 6 个 DOCX 夹具，均为原创虚构内容（无个人信息、凭据或真实地址；注入测试样例明确标注为测试数据），原始 UTF-8 种子与 joyword-corpus 确定性生成的正式 DOCX 夹具角色区分明确。2026-10-18 WPS 固定参考环境建立（Mac）：WPS Office for Mac 12.1.29181（build 29181）打开 6 个格式夹具与 e2e 编辑副本，全选复制读回逐段一致（WPS 复制在中西文边界插半角空格，去空格归一后逐字一致；基线为包内 document.xml 独立解析；当日两轮独立复现，现存读回文件与 [wps_report.json](../../tests/fixtures/reference/wps/wps_report.json)）。参考 PDF 导出通道当日本机故障（系统“存储为PDF”与 WPS“输出为PDF”均静默无产物，上午曾成功、下午不可复现，无 sudo 无法排查），PDF 参考与几何抽取待环境修复后补建，如实记录。T004 人工验收（审查样例、种子与夹具区分）已于 2026-10-18 执行，标记 `done`，见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T005：信创平台技术验证

- 输入：统信 UOS 与银河麒麟桌面版（x86_64、ARM64、LoongArch64）测试机或镜像、Rust 目标支持说明及 GPUI 渲染后端候选。
- [ ] 状态：`in_progress`
- 需求：`PLATFORM-009`、`PLATFORM-001`
- 前置：`T001`、`T002`
- 产物：ARM64 与 LoongArch64 构建记录、目标系统 glibc／ABI 核对、GL 与 CPU 渲染启动结果、Fcitx 中文输入与系统字体清单，以及供 ADR-010 选型的结论。
- 错误路径：旧世界 LoongArch ABI、缺少 Vulkan／GL 驱动或输入法框架时记录原因与替代方案，不以 x86_64 结果代替。
- 自动验证：交叉构建与目标机冒烟启动；无 GPU 渲染路径的启动与帧时间记录。
- 人工验收：有条件时在至少一台国产 CPU 与 GPU 的机器上启动窗口、输入中文并缩放；真机专项为非阻塞待补验项。
- 完成证据：无界面 crate 已在 CI 中交叉编译到 aarch64 与 loongarch64（新世界 ABI，glibc 2.39 交叉库）并在 QEMU 用户态通过测试；原生 ARM64 runner 构建完整应用。本环境无统信 UOS、银河麒麟、旧世界 LoongArch 与 Fcitx，真机仍未验证，QEMU 不能代替真机。2026-10-18 按用户决策改为 `in_progress`，真机专项待有条件时补验，不阻塞下游或发布。2026-10-19 银河麒麟桌面版 V10 SP1（x86_64，Intel Xeon 虚拟机、无 Vulkan 硬件）部分验证：GPUI 窗口经机内 SwiftShader Vulkan 与软件 EGL 成功启动并完整渲染（中文界面、页面画布、功能区），文字与页面场景消费同一份 Noto CJK 字体字节；Fcitx 4 在运行但 GPUI X11 窗口不接入 IME（拼音直通、无候选框），中文输入经系统剪贴板粘贴可用；系统字体清单含 Noto Sans/Serif CJK 与思源系（见 kylin_report.json）。本机不是国产 CPU+GPU 硬件，真机专项仍为待补验，T005 保持 `in_progress`。2026-10-19 统信 UOS Desktop 20 Professional（x86_64，内核 4.19，glibc 2.28，海光 C86 7390 四 vCPU 的 KVM 虚机，无 Vulkan 硬件）完成另一信创环境腿：安装 Rust 1.98.1 与 g++/cmake/xcb/xkbcommon 开发包，并在 UOS 的 fontconfig 开发包版本冲突下补本地 `/usr/local/lib/pkgconfig/fontconfig.pc` 与 `libfontconfig.so` 链接（或 `RUST_FONTCONFIG_DLOPEN=1`）后，裸 `cargo build -p joyword --locked` 成功；GPUI 窗口经机内 SwiftShader Vulkan ICD 与软件 EGL 启动，中文界面、页面和功能区完整渲染，字体与页面场景消费同一份 Noto CJK 字节；裸启动在默认 Vulkan 无可用设备、GL surface 不兼容时自动寻找软件 ICD 并重启一次，无需手工 `VK_ICD_FILENAMES`；Ctrl+=／Ctrl+0 的 80%→90%→100% 缩放实测页宽按比例变化，编辑、系统剪贴板中文粘贴与撤销可用。Fcitx 4 与搜狗输入法在运行但窗口不接入 IME，拼音直通；Ctrl+O 无文件选择框，Ctrl+S／Ctrl+Shift+S 无保存对话框，GUI 打开／保存／导出在本机不可达。上述为 x86_64 虚机且无 GPU，不能代替国产 CPU+GPU 真机专项；T005 保持 `in_progress`，详见 [UOS 报告](../../tests/fixtures/reference/uos/uos_report.json)。

## M1：最小真实编辑器

退出条件：中文输入—编辑—DOCX 保存—退出—重开；支持子集、失败恢复与不可信文件基线均验证（T107）。

### T100：类型化文档模型

- 输入：架构数据契约、故事区域列表、Unicode 与原始包保留要求。
- [x] 状态：`done`
- 需求：`EDIT-001`、`STYLE-001`、`FORMAT-002`、`REVIEW-006`
- 前置：`T000`
- 产物：story、block（含 SDT 透明容器）、稳定标识、经基准选定的文本存储、样式（按规范名称识别内置样式）、资源，以及跨段域、书签和批注共用的标注范围与原始单位保留。
- 错误路径：无效树、重复 ID、样式环和不合法范围返回有界错误。
- 自动验证：模型不变量、Unicode 边界、结构序列化、原始保留信息与快照结构共享开销测试。
- 人工验收：加载原创正文／表格结构样例，检查不支持节点被保留而非忽略。
- 完成证据：joyword-model 与文本存储基准已落地，自动验证项均有测试并在三端 CI 通过。2026-10-17 人工验收已执行：模型层 `contract_structure_preserves_unsupported_nodes` 加载原创合同结构（表格网格／横向纵向合并、锁定 SDT、`m:oMathPara` 不透明块、`w:object` 行内保留对象）验证不支持节点经结构与快照往返后保留而非忽略（13 项模型测试通过）；DOCX 导入层 `joyword-docx/tests/import.rs` 验证真实包中 `w:tbl` 等未知元素导入为带保留句柄的不透明块（20 项通过）；会话层 DOCX-MIN-003 验证未知内容保留、locked 非空且 engine 整篇只读、原包字节不变（8 项通过）。三处均在本机复验，见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T101：事务与位置映射

- 输入：合法文档快照、带版本的 Command、选区和范围标注。
- [x] 状态：`done`
- 需求：`EDIT-004`、`EDIT-005`、`REVIEW-006`
- 前置：`T100`
- 产物：原子 apply、单调版本、ChangeSet、PositionMap 和权限入口。
- 错误路径：过期版本、跨范围、无效命令不能提交半次修改。
- 自动验证：失败原子性、位置映射组合、分裂／合并／移动及撤销性质测试。
- 人工验收：跨段编辑后检查选择、书签、批注锚点和撤销结果。
- 完成证据：joyword-engine 已实现原子事务、单调版本、ChangeSet、PositionMap、权限入口与快照撤销，16 项测试含参考模型随机对照。2026-10-17 人工验收已执行：以生产会话与输入组件（文档画布所绑定的同一 `Session`／`TextInput` 代码路径，非 GUI 壳层）复核跨段编辑后的选择、书签与批注锚点及撤销——`bookmarks_and_comments_follow_cross_paragraph_edits_and_undo`、`selection_is_mapped_and_restored_by_undo`、`undo_and_redo_restore_exact_states_with_monotonic_versions`、`inline_objects_and_formatting_round_trip_through_undo` 及输入层选择恢复测试全部通过（本机复验：engine 39 项、text_input 8 项），见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T102：输入、选择与基础格式

- 输入：文档会话、PositionMap、平台输入事件及布局命中数据。
- [x] 状态：`done`
- 需求：`EDIT-001`、`EDIT-002`、`EDIT-003`、`STYLE-001`、`PLATFORM-004`
- 前置：`T101`、`T105`、`T106`
- 产物：GPUI 文档画布输入、选择、字符及段落基础格式、组合态 overlay。
- 错误路径：IME 取消、焦点切换、非法 UTF-16 范围和渲染陈旧结果正确处理。
- 自动验证：字素簇编辑、组合提交／取消、命中和 selection 映射测试。
- 人工验收：输入中文／emoji／组合字符，鼠标与键盘选择后编辑并一次撤销。
- 完成证据：C07 已实现 engine 事务驱动的会话内纯文本编辑、UTF-16 投影、独立组合预编辑、字素移动／删除、选择／撤销及 GPUI EntityInputHandler；统一画布绘制光标／选择／标记并拒绝陈旧结果，关闭提供不保存／取消。新增输入性质、预算、只读 DOCX、真实排版／CPU 和几何测试，见 [C07 报告](../coordination/reports/C07-TEXT-INPUT.md)。`Office` 命令路径已能对选区加粗、段落居中，并经复制／粘贴回写文字；`m1_edit_save` 覆盖中文编辑、格式、剪贴板、另存与重开。2026-10-18 GUI 人工验收执行（用户在场操作＋恢复日志字节核对）：系统拼音提交（数字键选词）、Esc 取消（无残留、撤销栈无多余步）、候选框贴近光标、一次撤销撤掉整次提交；emoji 全家福（ZWJ 组合）粘贴一次退格整组删除（宋体无 emoji 字形，画布缺字占位为预期，原文码点保留）；`é`（U+00E9 预组合，恢复日志字节核对）与组合式字素光标一步跨过；鼠标拖选跨行与键盘扩展选择、替换并一次撤销。期间发现并修复 GUI 剪贴板缺陷（复制/粘贴原只走应用内字符串，不接系统剪贴板，跨应用粘贴无反应；已加 GPUI 读写桥接并实测双向通过，提交 e9d3d1c）。T102 标记 `done`，见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T103：DOCX 最小语义与保留读写

- 输入：ECMA／微软公开规范、原创 OPC 样例、文档模型和资源预算。
- [ ] 状态：`in_progress`
- 需求：`FORMAT-001`、`FORMAT-002`、`FORMAT-003`、`FORMAT-004`、`QUALITY-005`
- 前置：`T100`、`T004`
- 产物：OPC、关系图、正文段落和基础格式；兼容模式写入与保留；ImportResult、ExportPlan、未知保留；解析资源预算与 fuzz 目标；锁文件检测与保留元数据的原子保存。
- 错误路径：损坏包、未知嵌套、失效关系、超预算和不安全编辑只读／拒绝，原件保持；受保护或强制修订的文档在对应能力实现前只读打开或明确提示。
- 自动验证：规范最小文件、无编辑复制、编辑往返、未知部件散列、失败保存、元数据保留和 fuzz 冒烟测试。
- 人工验收：打开自建与固定 Word／WPS 样例，编辑支持区域、保存、重新打开。
- 完成证据：已有格式中立 OPC 与 joyword-docx 声明子集只读导入（普通段落、基础属性、规范样式、兼容模式、保护与未知内容保留），W01 切片经集成复验及两项拒绝边界修正。C01 增量接入 shell 的 DOCX 会话：只读或 locked 非空时由 engine 整篇拒绝编辑，原包和诊断随会话保留；编辑／撤销后拒绝作为未编辑原包复制。C02 已增量实现版本绑定的保守 ExportPlan 与现有段落、现有格式边界内文字修改的字节导出，并在原预算下重新导入验证；结构／格式／对象改动、受限与签名包编辑明确拒绝。C03 进一步提供已验证计划到新文件的同目录临时写入／sync／回读／不覆盖发布及版本收据，故障与并发测试通过。第二轮 W05 增量支持默认格式纯文本正文段落的拆分、合并、插入与删除导出，并按身份对齐、节边界与保留片段字节复核，见 [W05 报告](../coordination/reports/W05.md)。W07 提供源身份、Office／JoyWord 锁文件检测与冲突预检；save_original 因 ExportPlan 缺少来源绑定仍拒绝原件替换，见 [W07 报告](../coordination/reports/W07.md)。W09 独立健壮性 harness 与固定种子变异未发现生产缺陷，但不是覆盖引导 fuzz，也未接入 CI，见 [W09 报告](../coordination/reports/W09.md)。第三轮：W10 为 ExportPlan 增加来源字节绑定（`origin_bytes()`，私有字段不可伪造）；W11 实现写入前的来源比对与平台能力矩阵，公开 `save_original` 在所有平台仍于写入前返回 ReplacementUnavailable；W12 支持 pPr／rPr 可证明完全映射段落的结构导出（原样复制源属性元素字节，不能证明即拒绝），见 [W10](../coordination/reports/W10.md)、[W11](../coordination/reports/W11.md)、[W12](../coordination/reports/W12.md) 报告。其后的替换增量补上读回确认、未确认发布保留备份与元数据恢复核对，Unix 所有权与 macOS 创建时间转为可保留，见 [W07 替换增量](../coordination/reports/W07-REPLACE-INCREMENT.md)。2026-10-18 Word 人工验收的 Word 一侧已执行：Word 2024（build 16.0.20430.20182）打开 JoyWord 导出副本并追加一段另存，JoyWord 重新导入 Word 写出的文件——正文逐段一致、可编辑（unrestricted，无 locked）、Word 新增部件（theme/fontTable/webSettings）与未知属性按保留诊断记录不丢弃，`export_plan` 对该 Word 文件成功（0 警告），产物在 `tests/fixtures/reference/word2024/interop/`。同日补充 Word 生成文件导入矩阵：6 个格式样例经 Word 重存（完整原生部件）后逐一导入——正文逐段一致，MIN-001/002/003/SAMPLE-001 可编辑（unrestricted），MIN-004 外部超链接按 locked 保留，MIN-005 保护设置保留为整篇只读，`export_plan` 对全部 6 个 Word 文件成功；Word 写出的 theme/fontTable/webSettings/customXml 部件按 NotImported 保留诊断记录。WPS 一侧、覆盖引导 fuzz 未完成，见 [IMPLEMENTATION](IMPLEMENTATION.md)、[集成审查](../coordination/reports/WAVE01-REVIEW.md)、[C01 报告](../coordination/reports/C01-DOCX-SESSION.md)、[C02 报告](../coordination/reports/C02-DOCX-EXPORT.md)和 [C03 报告](../coordination/reports/C03-DOCX-COPY.md)。 2026-10-19 Windows 基线补验：固定健壮性矩阵 81/81（43 ok、38 rejected）及 Rust 3 项、Python driver 12 项（1 项 POSIX 专属断言跳过）通过；无覆盖反馈、Windows 无 Job Object 硬内存上限，覆盖引导 fuzz 仍未完成。矩阵在本轮生产修复前执行，绑定与边界见 [验证审计](VALIDATION-20261019.md)。 2026-10-19 Windows GUI 打开、编辑、另存并三次显式替换 DOCX-MIN-001 隔离副本，CLI 读回中文正确；受保护 DOCX-MIN-005 的 inspect 返回 `read_only=true`。 WPS 一侧的打开—复制—读回已在两处执行：Mac（2026-10-18，WPS 12.1.29181）与 2026-10-19 银河麒麟 V10 SP1 的 WPS Office 12.1.2.1129.AK.preload.sw（麒麟预装版）——打开 JoyWord 导出的编辑副本（DOCX-SAMPLE-001 段首插入「校验」）正常，Ctrl+A 复制读回与包内正文去空白归一后逐字一致（`tests/fixtures/reference/kylin/wps_readback.txt`、`kylin_report.json`）；本机 WPS 的 PDF 导出通道同样静默失败。2026-10-19 T103 专属会话（kimi/t103-coverage-fuzz）：W09 harness 按既有契约重锚到 `8b7f9c2b5e5b03c0ee6ac9a56c550a8e715f4d32`（2026-10-19 验证轮 crates 树；随后只改 experiments 与本文档），固定矩阵复跑 81／81（43 ok、38 rejected，无 crash／timeout／invariant／protocol，地址空间限制探测 `enforced=true`）。harness schema-1 JSON 新增 `features` 字段（只含净化后的阶段事实：目标、状态、拒绝类别令牌、枚举变体名与尺寸桶，不含正文、路径或部件散列），driver 新增 `fuzz` 子命令：以 harness 自报特征的新颖度做语料扩展（新颖输入加入变异语料，最新优先＋平方偏置调度），硬预算覆盖 wall clock、迭代数、每子进程超时、RLIMIT_AS 地址空间（POSIX）、1 MiB 输入上限、64 KiB 输出上限、语料条数与语料字节（32 MiB），全程无网络（外部关系目标永不解析、driver 不建连接）。固定种子 20261019、2000 次迭代、512 MiB 地址空间上限的 evidence campaign：2000 次执行，发现 62 个行为特征、32 次新颖事件，语料 27→59，0 异常（1524 rejected／476 ok）。rebase 到统信 UOS 渲染提交 `39193936ccc532ae546dd163b6c244b28994be07` 后，`crates/`（`joyword-render` 亚 2 px 分数矩形整像素吸附）相对 `8b7f9c2b5e5b03c0ee6ac9a56c550a8e715f4d32` 有差异，按同一契约把绑定改到该提交；随后只改 experiments 与本文档。本环境在新绑定上复跑：固定矩阵 81／81（43 ok、38 rejected，`matrix_complete=true`，地址空间限制 `enforced=true`）；同一 evidence campaign 再次为 2000 次、62 个特征、32 次新颖事件、语料 59、0 异常（1524 rejected／476 ok）。验证：Rust 3 项、Python driver 17 项测试通过（新增反馈提取、语料上限与确定性、异常退出、真实子进程端到端），fmt／clippy 干净，四平台 CI 在 smoke 后增加 60 秒有界 fuzz（固定种子 200 次迭代）。如实声明：这不是代码覆盖引导 fuzz——未安装任何覆盖引擎，summary 的 `coverage_feedback` 保持 false，反馈信号是 harness 行为特征（阶段事实新颖度），不是源码行覆盖；代码覆盖引导 fuzz 仍属未完成（需 C08 工具／版本／许可审查）。WPS 生成的固定样例、完整 Word／WPS 人工交换与代码覆盖引导 fuzz 仍缺，T103 保持 `in_progress`，见 [IMPLEMENTATION](IMPLEMENTATION.md)、[集成审查](../coordination/reports/WAVE01-REVIEW.md)、[C01 报告](../coordination/reports/C01-DOCX-SESSION.md)、[C02 报告](../coordination/reports/C02-DOCX-EXPORT.md)和 [C03 报告](../coordination/reports/C03-DOCX-COPY.md)。
- 2026-10-20 文档核对：行为特征 fuzz 已有测试，覆盖引导 fuzz 仍未做，验收清单写明 pending, separate PR。打开自建与固定 Word／WPS 样例的人工验收未关闭。状态保持 `in_progress`。见 [验收清单](acceptance/T103.md)。
- 2026-10-20 验证波次收口：S2（[#62](https://github.com/willmove/joyoffice/pull/62)，`8a12e6dd`）接上代码覆盖引导 fuzz。反馈来自插桩二进制的 LLVM 源码区域计数器；稳定版 1.98.1 不写分支计数器，所以不是分支覆盖。固定种子 20261020、60 次变异，没有 crash、timeout、invariant 或 protocol。夜间工作流是 `.github/workflows/fuzz-nightly.yml`，见 [T103 覆盖引导 fuzz](verification/t103-coverage-fuzz.md)。验收清单里的覆盖引导 fuzz 一行改为 partial。WPS 生成的固定样例和 Word／WPS 人工交换仍未做。状态保持 `in_progress`。
- 2026-10-23 生产恢复双代切片（[#90](https://github.com/willmove/joyoffice/pull/90)，任务仍 `in_progress`）：两个发布槽 `recovery-a.jwr`／`recovery-b.jwr` 交替写入，每代带 `JWREC002` 头与 CRC32，写入经临时文件、flush、sync、读回校验与原子改名后才发布，发布前一代始终保留。最新槽损坏时回退到另一槽；两槽同代内容不同或都损坏时拒绝，且不改写损坏槽。旧 `JWREC001` 单文件格式只读兼容，迁移期间不改写旧文件。文档加密拒绝写明文副本，超预算输入不落文件。保存 N 完成时编辑记录 N+1 仍为 dirty，重启可恢复到 N+1。独立审查在独占 target 上复跑 `joyword-shell` 263 项与应用 65 项测试、全目标 clippy 与 fmt，无阻塞发现。macOS 上的一次失败经查为平台锁语义差异：BSD 与 macOS 的 `flock` 锁属于文件而非描述符，关闭描述符不释放进程持有的锁，std 的 `File` 析构只关描述符而不发 `LOCK_UN`，于是同一下一次取锁被自己的旧锁拒绝，而 Linux 与 Windows 随最后一个句柄关闭即释放。已改为显式 `unlock()` 的守卫类型，并新增 `a_released_lock_can_be_taken_again_by_the_same_process` 回归。未做：图形界面、断电、真实磁盘满与信创真机验收；Windows 目录同步按实现记为 false。

### T104：撤销、本地日志与恢复

- 输入：已提交事务、文档快照、正式保存版本与可注入失败的存储适配。
- [ ] 状态：`in_progress`
- 需求：`EDIT-005`、`FORMAT-004`、`QUALITY-004`
- 前置：`T101`、`T103`
- 产物：独立撤销栈、日志校验、周期快照、dirty 版本与恢复选择。周期快照必须包含 PackageBacking 与原包基线所需信息，不能只含模型；加密文档不落明文副本；快照写入经临时文件、校验与原子改名，并保留上一代有效副本。
- 错误路径：磁盘满、截断、外部修改、保存期间继续编辑和日志迁移失败不丢原件。
- 自动验证：故障注入、有效前缀恢复、保存版本竞争、undo／redo 属性测试。
- 人工验收：强制结束测试进程后恢复，比较最后完整事务和正式保存文件。
- 完成证据：生产恢复日志在 `joyword-shell` 的 `storage::recovery`：提交写入原包基线与已提交文本，校验后原子改名；加密包拒绝且不写明文。`Office` 在编辑提交后写恢复日志。子进程经 `FILE_OPEN` 与 `EDIT_INSERT` 提交后被终止，父进程用 `Office::recover_from` 把该提交放回会话。窗口启动时把目录设为 `JOYWORD_RECOVERY_DIR`，否则用偏好目录下的 `recovery`；已有日志则恢复进会话，示例文档不会盖掉它。2026-10-17 起 `m1_concurrency_recovery` 已在 Windows 参考环境原生通过（3 项，含子进程终止后的恢复与加密包拒绝），macOS 复验见 [IMPLEMENTATION](IMPLEMENTATION.md)。2026-10-18 GUI 强制结束验证执行：对正在编辑的窗口提交两笔事务（QMARK、K1）后 `kill -9`，重启后恢复的最后完整事务含两笔插入且顺序正确（截图与日志双证）。断电场景未验证，T104 保持 `in_progress`。 2026-10-19 银河麒麟复验通过：对含未保存修改的窗口 `kill -9`，日志完好且无正文泄漏；同一测试 profile 重启后日志记录 `recovery.restored kind=document count=1`，状态栏显示「会话文档」，正文为最后完整事务状态（`kylin-gui-recovery-restored.png`）。断电场景仍未验证，T104 保持 `in_progress`。 2026-10-19 Windows 原生基线全测及独立 recovery-journal 实验 15 项通过；仍未做断电验收，未把故障注入计为断电，见 [验证审计](VALIDATION-20261019.md)。2026-10-19 统信 UOS Desktop 20 Professional 复验通过：固定 `JOYWORD_RECOVERY_DIR` 后粘贴并提交哨兵 `UOS-RECOVERY-MARK`，`kill -9` 终止窗口；日志无哨兵正文，重启日志记录 `recovery.restored kind=document count=1`，状态栏显示「会话文档」，正文恢复到最后一笔提交（`tests/fixtures/reference/uos/uos-gui-recovery-restored.png`）。断电场景仍未验证，T104 保持 `in_progress`。
- 2026-10-26 撤销属性与日志校验切片：`joyword-engine/tests/undo_properties.rs` 以自研 xorshift64* 固定种子（4 个）驱动随机编辑事务，覆盖全量撤销回到初始快照、撤销 k 重做 k 精确还原、撤销后新编辑永久截断重做、被拒事务（陈旧版本、Agent 关闭、前置条件、非法文字、越界、倒序、未知节点、多命令半途失败）不动文档与双栈、段落身份保持且版本只增不重用，失败打印种子与命令日志；独立撤销栈经 `two_sessions_have_independent_undo_stacks` 与 `saving_and_journaling_do_not_clear_the_undo_stack`（保存副本与恢复日志写入不清空、不增加撤销步）；日志校验经 `tampered_slots_fall_back_to_the_valid_prefix_or_fail_closed`（逐位翻转、任意截断、头前／尾后垃圾回退上一有效代且写入失败即关闭、损坏槽不覆盖）、`generation_order_selects_across_slots_not_by_file_name`（按代次不按槽名选择、同代同内容去重）与 `only_verified_bytes_reach_the_target_slot`（发布前解码校验，未验证字节不进槽、无遗留临时文件）。无生产代码改动；周期快照与断电人工验收仍未做，T104 保持 `in_progress`。

- 2026-10-27 周期快照收口切片（任务仍 `in_progress`，进度计数不变）：核对「周期快照」产物项与现状：2026-10-26 的生产恢复双代日志每次编辑提交都写一份完整代——原包基线（`PackageBacking` 的 `original_bytes`，不是只有模型）加全文文本——经临时文件、flush、sync、读回校验与原子改名发布，发布前一代始终保留在另一槽，加密包拒绝写明文；这正是任务要求的周期快照形态，粒度为每次提交（严于定时）。本切片补上两点：其一，审计 `host.rs` 的命令分发——全部 33 个会改文档的命令分支（编辑、格式、样式、列表、分页与页面设置、查找替换、格式刷、表格增删改、审阅接受／拒绝／跟踪）在提交后调用 `journal_active`，非修改命令（查看、选择、视图切换、复制）不写日志；其二，新增端到端性质测试把它钉住。证据：`crates/joyword-shell/tests/t104_periodic_snapshots.rs` 的 `every_committed_edit_publishes_a_complete_periodic_snapshot`——经命令注册表依次驱动插入、全选、加粗、居中、分页符、全部替换、插表、开跟踪、插入、全部拒绝共九次修改与一次非修改，每步断言：代次恰加一、快照文本等于当前全文、快照基线等于原包字节、版本字段非空；全选不产生新代；九次提交后两个槽文件并存（上一代保留）。剩余未做：断电场景人工验收（不做故障注入冒充）；周期快照按本实现即为逐提交粒度，无更粗的定时器需求。T104 保持 `in_progress`。见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T104 周期快照收口」。

### T105：基础排版引擎

- 输入：文档快照、T003 验证的字体身份与整形链路、页面设置和 Unicode 分析。
- [x] 状态：`done`
- 需求：`LAYOUT-002`、`LAYOUT-003`、`LAYOUT-009`
- 前置：`T100`、`T003`
- 产物：四槽字体选择与整形、段落断行（含基础禁则）、单节分页、命中测试、光标与选区几何、LayoutSnapshot 与屏幕 PageScene。
- 错误路径：缺字体、超长无断点文本、过高对象和陈旧快照有界诊断，不阻塞输入。
- 自动验证：固定字体断行／分页样例、命中往返、快照键与版本、缩放不改变几何。
- 人工验收：打开原创中英文样例，滚动、缩放并核对光标、选区与分页。
- 完成证据：W03 已增量实现单字体 LTR、显式合法断点的行拟合切片，含候选行重整形、EMU 几何、源范围映射、预算及超宽诊断；7 项新单元测试和 8 项真实字体集成测试在集成复验中通过。C04 已增量接入 Unicode 15.0 默认自动断点，与整形簇求交并共享同一请求预算；新增 8 项真实字体测试通过。C06 已把只读文档快照和 CPU 场景图像接入窗口（新画布 GUI 尚待验收），见 [C06 报告](../coordination/reports/C06-CPU-CANVAS.md)。C05 已增量实现单节预览分页、版本／内容／字体／profile 快照键、命中／光标／跨页选区与多页 PDF 链路，19 项新增排版测试及 PDF 集成测试通过，见 [C05 报告](../coordination/reports/C05-LAYOUT-SNAPSHOT.md)。第二轮 W06 增量接受 keep_next／keep_lines 并做有界分页，原有几何接口不变，新增 12 项测试，见 [W06 报告](../coordination/reports/W06.md)。`wrap_mixed` 按四槽选字体，缺字改用 fallback，并用 `adjust_cjk_break` 挪开会把收尾标点放到行首的断点；本机宋体对照见 layout 测试。2026-10-17：`m1_scene_pdf` 对同一次 `layout_document` 的两次结果比较页面、行、光标与选区，缩放只走屏幕投影，字形、光标和选区换算回点后不超过 0.01 pt；`joyword_pdf::render` 用同一份字体字节，PDF 内容流里的文本矩阵与场景字形原点不超过 0.01 pt。`layout_document` 对含东亚或复杂文字的段落调用 `wrap_mixed`、`slot_for` 和 `adjust_cjk_break`，窄页上的句号不会被排到行首。2026-10-18 固定 Word 2024 实测对照：同字体（等线）同字号同固定行距同页框下，合同样例（2 页 24 行）与标点压力样例（2 页 27 行）的断行与 JoyWord 逐行一致、行位偏差 ≤1 pt（[规则目录 R2](layout_rules.md)，产物在 `tests/fixtures/reference/word2024/breaks/`）；同日实测登记 R3（两端对齐余量分配到字间、段末行不拉伸）、R4（单倍行距机制破解：自然行高 ≈1.3×win 跨度×字号，行距吸附到 312 twips 行网格整数倍）、R5（孤行控制强制拆分负结果，已核实写入 XML）、R6（keepNext 正对照）；标点压缩分类规则已实测（R7：引号 0.36em、连排半宽、孤立全宽）、keepLines 负结果（R8）、悬挂标点观测（R9）；2026-10-18 界面滚动缩放验收执行（Mac GUI，屏幕像素测量）：100%↔150% 页宽 466→699 px（比例 1.4998）、行集合与分页不变；30000 段大文档滚动（数百页当量）、200% 缩放（页宽比例 ≈2.02）、深位置插入、回滚顶部无错乱、无画布错误。T105 标记 `done`（混合格式的 Word 对照属 T207、widowControl 属 T202，如实登记为未实测/未实现），见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T106：应用外壳、命令注册表与偏好

- 输入：GPUI 窗口、事务入口、平台菜单与文件对话框接口及本地化资源。
- [x] 状态：`done`
- 需求：`EDIT-001`、`PLATFORM-002`、`PLATFORM-008`
- 前置：`T002`、`T101`
- 产物：命令注册表（名称、可用状态、快捷键、执行入口；元数据含是否修改文档与类型化参数说明，程序化调用参数齐全时直接执行、缺参数时返回待输入结果而非弹框）、菜单、快捷键、文件对话框、最近文件、多文档会话与未保存提示、偏好设置，以及支持插值和复数的本地化资源。
- 错误路径：未实现命令不出现；取消关闭保留会话；偏好损坏时回退默认值并提示。
- 自动验证：菜单、快捷键与 CLI 指向同一命令；关闭流程、偏好迁移和本地化键一致。
- 人工验收：在 Mac 上用菜单和快捷键新建、打开、切换、关闭多个文档，修改作者名后重启核对。
- 完成证据：joyword-shell 已实现命令注册表（唯一执行入口、可用状态、平台快捷键、菜单分组与冲突校验）、带插值与 CLDR 复数的本地化、偏好设置（迁移、损坏回退与备份、新版本只读、原子写入）和多文档未保存／关闭流程；应用的菜单、快捷键与按钮已改由注册表驱动。第三轮 C09 已把绑定打开接入应用外壳：`open_source` 成为持有 DocxSession 的绑定会话，另存只走 `save_export_copy`，绑定路径与 dirty 状态跟随已打开文件，锁文件创建／释放归会话生命周期，见 [main 集成记录](../coordination/reports/MAIN-INTEGRATION-20261016.md)。文件对话框、最近文件界面与多窗口随 T103 的打开／保存接入。W07 的源身份与冲突预检已随 C09 接入文件命令：`open_path` 经 `BoundDocx::open_default`（内含 `open_source` 的身份核对与锁冲突预检），保存版本跟踪由 `start_save`／`finish_save` 票据实现（会话版本不符不写文件、不清 dirty；2026-10-17 Windows 原生复验）。命令已带 `modifies_document` 与类型化参数；缺参数返回 `PendingInput`，参数齐全则执行。`Office` 覆盖最近文件、多文档切换和未保存关闭的取消。无头读回命令仍属 T210，不在本任务。`file.open_recent` 与 `document.switch` 有命令测试。窗口持有 `Office`，关闭时 `choice=cancel` 保留会话。文件对话框的图形人工验收未完成，T106 不标 done。2026-10-18 GUI 人工验收执行（Mac）：菜单触发原生打开/另存面板（经 Cmd+Shift+G 直达路径完成打开与另存，另存建议名 copy-big.docx 按源文件名派生）；最近文件菜单列出并切换多个文档（恢复日志基线字节数随切换佐证 23535/23514/1356）；未保存关闭提示默认回车=取消保留会话、显式“放弃修改”才关闭；修改作者名（手写 preferences.conf）后重启再正常退出，值原样保留未被清掉。T106 标记 `done`，见 [IMPLEMENTATION](IMPLEMENTATION.md)。2026-10-17：启动示例改为真实 DOCX 包会话（DOCX-SAMPLE-001，正文逐行取自 cjk-contract.txt），未打开文件时另存同样可用；另存在保存期间继续编辑走后台 `SaveCopyJob`，结果按会话纪元与版本落地。命令被禁用或失败时在窗口显示原因，不再静默。2026-10-19 Linux 入口：GPUI `set_menus` 在 Linux 不绘制菜单栏，门户 FileChooser 在 UKUI/X11 上可以不出现且无诊断。Linux 窗口增加文件／编辑条（打开、保存原件、另存副本、导出 PDF、撤销、重做、复制、粘贴），打开／另存／导出改为窗口内路径栏，并在有 zenity、qarma 或 kdialog 时提供浏览。macOS／Windows 的原生对话框与主快捷键未改。证据 `linux_bar_covers_open_save_and_export`、`typed_open_requires_a_file_and_save_requires_a_parent_directory`。这不改变 T106 已完成的 Mac 菜单验收，也不把 Linux 图形点击算成新的人工验收。

### T108：画布可见页光栅与缓存

- 输入：现有 CanvasPipeline 排版／光栅两段流水线、页面场景与投影约定、当前全部页面共用像素预算的实现。
- [x] 状态：`done`
- 需求：`LAYOUT-015`
- 前置：`T105`
- 产物：光栅阶段改为视口±N 页的增量作业；缓存键为布局键＋页序号＋投影；每帧光栅预算与远离视口淘汰；超高页按固定网格分块；像素上限只约束缓存规模，不限制文档页数；选区、光标与查找高亮作为叠加层，不触发重新光栅。
- 错误路径：缓存键遗漏依赖导致陈旧像素必须有反例测试；预算超限排队而非整次失败；导出与打印不读屏幕缓存。
- 自动验证：2 倍屏 100%／200% 缩放下 100 页文档逐页可显示；滚动只光栅进入视口的页；淘汰后重建像素一致。
- 人工验收：长文档中交替滚动、缩放与编辑，无画布错误且响应正常。
- 完成证据：`joyword-render::NeighborhoodCache` 按视口邻域光栅，每页使用自身像素预算；100 页、2 倍缩放下远页可出图。应用侧 `CanvasPipeline` 按可见页及前后预取页排队，`raster_page` 一次画一页，超出像素预算时先丢掉远离视口的页。光标叠加由 `raster_page` 经 `joyword_render::paint_caret_overlay` 画到单独缓冲，不改该页像素，其他页不带光标。人工长文档滚动未做，不标 done。2026-10-18 补验：30000 段文档交替滚动、100%→200% 缩放、深位置编辑与回滚顶部，无画布错误、页面不错乱、单页提示未出现。T108 标记 `done`，见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T109：打开与保存的并发安全

- 输入：C09 绑定打开会话、W07 冲突预检、engine 版本与脏版本绑定机制。
- [ ] 状态：`in_progress`
- 需求：`EDIT-001`、`EDIT-004`
- 前置：`T102`、`T103`
- 产物：打开读取期间编辑器处于只读“打开中”状态，新文档就绪按版本复核后才替换当前会话；保存副本与原件保存作为携带会话身份与版本的后台任务，计划、校验与 IO 在后台完成，结果按身份与版本应用。
- 错误路径：放弃修改后、新文件就绪前键入的内容不无提示丢弃；保存期间继续输入不阻塞、不误清未保存状态；切换或关闭后迟到结果不影响新会话。
- 自动验证：版本复核、迟到结果丢弃、保存期间脏版本绑定的竞争测试。
- 人工验收：慢速打开大文档期间输入与取消；保存期间连续编辑后关闭提示正确。
- 完成证据：`file.open` 经 `begin_open`／`accept_open`。窗口打开在版本门关闭前保持只读，期间输入保留。`finish_save` 只在版本仍匹配时写另存并清 dirty；过期票据不写字节。另存副本经 `save_copy_job` 在后台写出取作业时的版本，保存期间的后续输入仍为未保存，迟到结果不改另一份文档。见 `m1_concurrency_recovery` 与 `save_copy_job`。2026-10-18 人工补验（Mac GUI）：另存大文档副本落盘后立即继续输入，关闭提示正确识别“保存开始后的修改仍未保存”；打开确认后立即键入的内容出现在最终文档（打开→就绪边界输入不丢失）。长时间“打开中”只读窗口未稳定观察到（本机导入大文档约 1 秒内完成），如实记录。2026-10-18 后续增量：`SaveOriginalJob` 在后台完成导出计划、重导入验证、来源复查与 IO；保存后的 `ExportBinding` 保留稳定节点 ID 和撤销历史并更新 XML 定位，迟到结果按 DocumentId 路由。`original_save_job` 验证保存期间继续输入、连续保存、撤销后再保存和跨文档路由。Windows 原生 GUI 的连续编辑与关闭提示尚待人工验收，T109 保持 `in_progress`。 2026-10-19 Windows GUI 完成默认另存副本、显式替换、连续第二次保存、保存后撤销及撤销后再保存；每轮均由独立 CLI 读回，证明保存绑定可连续使用。慢速打开期间的只读窗口与保存进行中关闭提示仍未验证，T109 保持 `in_progress`。2026-10-21 自动补强：`StepGate` 在打开与保存作业入口阻塞到测试放行，不靠睡眠。慢打开期间除 `edit.insert` 外的编辑与页面命令保持只读，插入与平台键入进入打开缓冲且不改当前文档，`accept_bound` 后才写入新文档；过期接受或打开失败时缓冲写回仍在的文档。保存作业在途且编辑版本新于作业版本时，关闭与退出共用 `close_prompt`，提示 `close.dirty_after_save`；作业版本与当前版本相同则仍提示尚未落盘。取消关闭保留会话。身份与版本路由测试仍覆盖迟到结果。见 `t109_open_save_gate`。Windows 与 macOS 的慢打开只读窗口和保存进行中关闭对话框没有做人工验收，T109 保持 `in_progress`。
- 2026-10-26 Mac GUI 部分补验（任务仍 `in_progress`）：在 main `5efe06a` 的调试构建与隔离 test profile 上，以系统键盘事件驱动 GUI。120,000 段（约 6531 页）样例经 Cmd+O 打开期间，状态栏显示「正在打开…文件就绪前编辑器为只读，键入的文字会保留。」且旧文档保持显示；在只读窗口内键入的 `DURING-OPEN` 在打开完成后出现在新文档第一行开头——打开缓冲、版本门与 `accept_bound` 路径在 GUI 上得到直接观察（此前只有 `t109_open_save_gate` 自动测试）。打开期间没有可用的取消入口，如实记录。关闭提示：一次保存流程从 large.docx 发起后，Cmd+W 弹出的对话框正文为 `close.dirty_after_save` 文案「“large.docx”在保存开始后又有修改，这些修改不在这次保存中。」，按钮为 保存／不保存／取消，默认 保存——但本次「保存作业在途时键入再关闭」的脚本化序列被一次焦点丢失（其他应用抢焦点）打断，该提示是在前一轮保存流程中捕获的，尚未按干净脚本重放，不据此关闭人工验收。观察记录与唯一存档截图见 [验证记录](validation/20261026/gui/summary.json)；工作目录在会话中途被外部回退，其余截图档案丢失，读图结果以文字形式记录在 summary 中。T109 保持 `in_progress`。

- 2026-10-27 打开取消入口切片（任务仍 `in_progress`，进度计数不变）：补上 2026-10-26 Mac GUI 会话记录的缺口「打开期间没有可用的取消入口」。`Office` 新增 `cancel_open` 与 `cancelled_open` 代次门：取消丢弃未就绪的新文档会话并返回打开期间缓冲的键入，由既有 `keep_unopened_keystrokes` 路径写回仍在的当前文档（与过期／失败打开同一语义，键入不无提示丢弃；无当前文档时显式报错而不是丢弃）；取消后的迟到结果被 `accept_open`／`accept_bound`／`finish_open` 以「代次 ≤ 已取消代次即拒绝」拒绝，不影响当前会话、不清 dirty；取消后再打开仍正常。命令注册表新增 `file.open_cancel`（无必填参数；`modifies_document` 为真——缓冲写回是正常可撤销编辑，注册表注释写明；无菜单槽与快捷键，仅打开在途时可用，其余时刻 `Availability::Disabled("file.open.not_opening")`）；应用侧注册 `CancelOpen` 动作并绑定，打开中的状态栏提示追加「按 Esc 取消」，`escape_panel` 在无其他面板时把 Esc 路由到取消（en／zh-CN 各补 3 键）。证据：`t109_open_save_gate.rs` 新增 `cancelling_a_slow_open_discards_it_and_keeps_keystrokes_on_the_current_document`（StepGate 阻塞打开→键入→取消：当前文档保留且含键入、源文件字节不变、迟到 `accept_bound` 被拒、再次打开成功并保留新键入、非在途时命令不可用）与 `cancelling_with_no_current_document_reports_instead_of_dropping_keystrokes`（无当前文档时取消显式报 "no document" 而不是静默丢键入）；应用侧 `registry_commands_all_have_actions_and_bindings` 扩展覆盖新命令。未做：Windows／macOS GUI 上慢打开期间按 Esc 的人工验收、信创真机；保存中关闭对话框的干净脚本化重放仍未做。T109 保持 `in_progress`。见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T109 打开取消入口」。

### T110：原件保存的元数据观测与用户选择路径

- 输入：W10 来源绑定、W11 能力矩阵、各平台发布步骤、替换增量的读回确认、SPEC §5 的用户选择路径。
- [ ] 状态：`in_progress`
- 需求：`FORMAT-004`
- 前置：`T103`
- 产物：各平台 ACL、扩展属性、来源标记与创建时间的观测实现（经审计的平台接口或按 ADR-009 精确固定的依赖）；不可保留或不可观测项在替换前列出，默认另存副本，用户显式确认才替换原件；以平台交换或替换语义（RENAME_SWAP／RENAME_EXCHANGE／ReplaceFileW）处理最后检查与改名之间的竞争窗口。
- 错误路径：观测失败默认拒绝替换；确认与拒绝都留下诊断；新增依赖须经 cargo-deny 并记录来源。
- 自动验证：逐项元数据的保留与拒绝测试、锁与外部修改检测、确认替换与另存副本两条路径。
- 人工验收：普通本地文档保存原件并重开；含 ACL 或隔离标记的文档在替换前列出损失项且默认仍为另存。
- 完成证据：`save_original` 在未确认时不改源字节并列出不能保留或不能观测的元数据；`save_original_with_choice(Confirm)` 只在显式确认后替换；`save_default_copy` 写新副本。macOS／Linux 沿用 rustix 1.1.5 交换改名及 displaced inode 复核。2026-10-18 后续 Windows 增量：精确固定安全 Win32 包装 `winsafe` 0.0.19（kernel、MIT），用 `ReplaceFileW` 且不使用 IGNORE_MERGE／ACL 标志；替换前保留硬链接原件，操作时刻备份再核对卷序列号／文件 ID 与精确字节，竞争写者被替换时恢复并拒绝。创建时间和 `Zone.Identifier` 已验证保留，ownership／完整 ACL／其他命名流仍在确认清单中，不宣称逐项观测。`file.save` 已接入菜单和 Ctrl／Cmd+S：后台 Decline 预检，原生提示列出损失，默认另存副本，确认后才后台替换；保存后继续输入和切换文档不会误清 dirty 或落到其他会话。`windows_save_original`、`original_save_job` 及存储层故障／竞争测试通过；Windows 原生 GUI 确认操作、真实 ACL 文档和断电／非 NTFS／网络盘验收未完成，T110 保持 `in_progress`。 2026-10-19 银河麒麟发现两项：其一，KYSEC 为每个文件挂 `security.ksip`／`security.boxadm` 扩展属性且 `security.ksip` 用户态不可写回，确认替换按契约返回 `ReplacementUnavailable[UnpreservableMetadata(ExtendedAttributes)]`——拒绝路径正确、默认另存副本不丢数据，但替换原件在麒麟（及同类的 SELinux 系统）上对普通文件必然不可用，宜把内核管理的 `security.*` 命名空间排除出可保留集合（对应测试 `confirmed_replacement_copies_a_user_extended_attribute` 在本机失败）；其二，内核 5.4 的 mtime 粒度为 jiffy，stat→open 身份守卫对同长度快速改写存在盲区（`identity_change_between_stat_and_open_is_not_a_baseline` 在本机失败）。其余 45 项 shell 测试在本机通过，细节见 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json)。 2026-10-19 Windows GUI 截图确认风险提示逐项列出 ownership、ACL、扩展属性／其他命名流不可观测，默认按钮为另存副本；默认路径未改原文件散列，显式确认路径可保存并读回。创建时间与 `Zone.Identifier` 保留，SID owner 和 ACE 内容相同，但 raw SDDL 的 DACL 从 `D:` 变为 `D:AI`，因此不宣称精确 ACL 标志保留。其他命名流、真实 ACL 边界、非 NTFS／网络盘未验，T110 保持 `in_progress`。2026-10-19 统信 UOS Desktop 20 Professional 补充：`confirmed_replacement_copies_a_user_extended_attribute` 在本机通过（与麒麟 KYSEC 的 `security.*` 拒绝不同）；但内核 4.19 的 jiffy 粒度使 `identity_change_between_stat_and_open_is_not_a_baseline` 与 `hard_link_alias_writes_are_detected_through_the_stamped_path` 两项失败——同长度快速改写／硬链接别名写入在同一个时间片内不改变 mtime/ctime。该盲区仍按未完成记录，不把测试失败改判为通过，见 [UOS 报告](../../tests/fixtures/reference/uos/uos_report.json)。Linux 元数据切片：内核管理的 `security.*`（KYSEC `security.ksip`／`security.boxadm`、SELinux `security.selinux`）只记入观测名单，不进入必须回写集合，也不再因 `setxattr` 的 EPERM 让确认替换返回 `UnpreservableMetadata(ExtendedAttributes)`；`user.*`（含 `user.security.*`）仍必须复制。stat 与 open 之间按同一读取预算复读内容，粗粒度 mtime／ctime 下的同长度改写不再成为基线（`identity_change_between_stat_and_open_is_not_a_baseline` 与粗时间戳用例）。相同字节的删除重建在 inode 号被复用且时间戳未跨 jiffy 时仍无法与原对象区分（`FS_IOC_GETVERSION` 能看到 generation，但本仓库禁止 unsafe，rustix 1.1.5 只以 unsafe ioctl 暴露它），相关测试改为跨过一个时钟刻度后再断言。精确 ACL、其他命名流、非 NTFS／网络盘与信创真机复验仍未完成，T110 保持 `in_progress`。
- 2026-10-22 Windows 卷能力观测切片（任务仍 `in_progress`）：在不新增依赖、不使用 unsafe（仓库 `unsafe_code = forbid`）的前提下落地，改动仅限 `crates/joyword-shell/src/storage/metadata.rs`，`winsafe` 仍是锁文件里的 0.0.19。`observe_volume` 经 kernel feature 的 `GetVolumePathNameW`／`GetVolumeInformationW`／`GetDriveTypeW` 观测源卷的文件系统名、驱动类型与能力标志，存进以原生基元表示的 `VolumeFacts`；`classify_volume` 是纯函数，先判驱动类型（`DRIVE_REMOTE` 网络共享、`DRIVE_UNKNOWN`／`DRIVE_NO_ROOT_DIR` 不可识别），再判文件系统名（只有 `ntfs`／`refs` 通过，大小写无关），最后逐项核对 `FILE_VOL_PERSISTENT_ACLS`／`FILE_VOL_NAMED_STREAMS`／`FILE_VOL_SUPPORTS_HARD_LINKS`（即 Win32 的 `FILE_PERSISTENT_ACLS`／`FILE_NAMED_STREAMS`／`FILE_SUPPORTS_HARD_LINKS`），六种 `VolumeRejection` 各有可定位诊断文本。Windows 的 `observe_platform` 接入后，不支持的卷以 `ErrorKind::Unsupported` 的观测失败呈现，`save_original` 即便显式确认也拒绝替换（走 `ObservationFailed`），默认另存副本路径不受影响。查证结论写进模块文档：`winsafe` 0.0.19 没有暴露 `GetFileSecurityW`／`GetSecurityInfo`／`ConvertSecurityDescriptorToStringSecurityDescriptorW`，也没有 `FindFirstStreamW`／`GetFileInformationByHandleEx(FileStreamInfo)`，所以 raw SDDL（含 `D:`／`D:AI`／`D:PAI` 标志）与除 `Zone.Identifier` 外的命名流无法用已审计依赖在进程内观测，二者按 FORMAT-004 契约保持 `UnobservableMetadata`，在替换前列出、需显式确认，`Zone.Identifier` 的保留行为未退化。本机 Windows 参考环境（NTFS）另做了一次进程外 PowerShell + `ReplaceFileW` 实证：继承 `D:AI` 与受保护 `D:PAI` 两种 DACL 下替换前后 `Get-Acl .Sddl` 完全相同，`::$DATA`、自定义流与 `Zone.Identifier` 全部存活且内容不变，因此先前记录的 `D:`→`D:AI` 差异并非 `ReplaceFileW` 固有；但因为进程内无法观测，仍不宣称精确 ACL 标志保留，这次实证也不是仓库测试。证据（全部 `#[cfg(windows)]`）：`classify_volume_accepts_a_local_ntfs_volume_with_every_required_capability`、`classify_volume_refuses_network_and_unrecognized_drive_types`、`classify_volume_refuses_non_ntfs_filesystems`、`classify_volume_refuses_each_missing_capability_flag`、`observe_volume_reports_a_supported_volume_for_the_temp_directory`（对本机 NTFS temp 卷做一次真实观测）、`volume_rejection_messages_are_distinct_and_non_empty`。切片内自动验证通过：`cargo test --locked`（CI 口径）、`cargo test --locked -p joyword-shell`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`python scripts/check_docs.py`、`python scripts/check_gates.py`（含反例）、`python scripts/check_acceptance.py`。未做的人工验收：Word／WPS 往返、Windows 原生 GUI 的确认操作与损失清单交互、真实 FAT／exFAT／网络共享卷上的端到端替换拒绝（本机无此类卷可挂载，分类分支只以合成 `VolumeFacts` 覆盖）、真实带 ACL 文档的替换、断电场景、信创真机与打印，全部未执行；非 Windows（macOS／Linux rustix）路径未在本机验证，改动对其为纯增量、未改语义；不支持卷的硬拒绝也还没有接进 `file.save` 的原生损失提示（需改共享 UI 与语言包，超出切片边界）。因此不标 `done`，T110 保持 `in_progress`。
- 2026-10-26 卷拒绝接入 file.save 原生提示切片（任务仍 `in_progress`，进度计数不变）：补齐 10-22 切片明确留下的缺口——不支持卷的硬拒绝此前只在 `save_original` 内部以 `ObservationFailed` 拒绝，`file.save` 的 Decline 预检提示没有区分它与可确认的元数据损失。现在 `storage::metadata::VolumeRejection` 成为所有平台可用的纯数据枚举（六种：网络卷、未知驱动类型、非 NTFS/ReFS 文件系统、不保留 ACL、不支持命名流、不支持硬链接），Windows 观测路径把它作为 payload 放进 `io::ErrorKind::Unsupported` 的 `io::Error`（可定位文本不变），`SaveOriginalError::volume_rejection()` 从 `ObservationFailed` 里取回结构化原因；非 Windows 平台不产生这种错误，行为零变化。应用层 `input_ui.rs` 新增纯函数 `OriginalPromptKind`：`ConfirmableLosses(blockers)` 走原来的损失清单对话框并提供「确认替换」按钮；`VolumeRefused(reason)` 用新标题 `file.original.volume_prompt` 与明细 `file.original.volume_detail`（含路径和按六种原因分别本地化的 `file.original.volume.*` 文案），按钮只有「取消」与「另存副本」，默认仍是另存副本，不再提供必然失败的确认替换；`volume_refusal_decision` 把其余任何回答都视为取消。语言包 en／zh-CN 各加 8 个键（parity 通过）。证据：`a_volume_refusal_survives_inside_the_observation_error`（payload 可取回、普通观测失败不被误判、外层包装不穿透）、`windows_volume_rejected_carries_the_structured_refusal`（`#[cfg(windows)]`，六种拒绝经真实观测辅助函数都带结构化 payload）、`a_volume_refused_precheck_surfaces_the_structured_reason_and_keeps_the_source`（`storage/original.rs`：预检以结构化原因拒绝、源文件字节不变、无遗留临时文件）、`a_volume_refused_precheck_reports_replacement_unavailable_not_confirmable_losses`（`tests/m1_save_original.rs`：预检结果里是卷拒绝而不是可确认损失）、`volume_refused_precheck_prompts_replacement_unavailable_and_defaults_to_copy` 与 `volume_refusal_entries_exist_in_both_locales`（`apps/joyword`：六种原因都映射到无确认按钮的提示、默认另存副本、两种语言键齐全）。`scripts/panic_baseline.txt` 追加 8 条，全部位于 `storage/original.rs` 的 `#[cfg(test)]` 模块（T111 第一类测试代码），生产位点零增。本机：`cargo test --locked` 1037 项全过、workspace clippy `-D warnings` 零告警、fmt 干净、`check_docs.py`／`check_gates.py`／`check_acceptance.py` 通过。未做：真实 FAT／exFAT／网络共享卷上的端到端人工操作（本机无此类卷，分类仍以合成事实覆盖）、Windows 原生 GUI 上该对话框的人工验收、Word／WPS 往返、精确 ACL／其他命名流观测（仍受 winsafe 0.0.19 接口面限制）、断电与信创真机。
- 2026-10-26 Mac GUI 部分补验（任务仍 `in_progress`）：同一会话中验证了「普通本地文档保存原件并重开」与「含隔离标记的文档在替换前列出损失项且默认仍为另存」。Cmd+S 的原生提示为三按钮（取消／另存副本（默认）／确认替换原件），默认高亮为另存副本，损失清单列出「无法观测所有权、无法观测 ACL、无法观测扩展属性/其他命名流」，提示期间源文件哈希不变；点击「确认替换原件」后 plain.docx（DOCX-MIN-001 副本）被替换——哈希由 `7f234d…` 变为 `019935…`、写入的 `T110OK` 标记出现在 document.xml、状态栏显示「已保存原件 …」，干净退出后重启同一文件，第一行显示 `T110OK…`（替换与重开闭环）。带 `com.apple.quarantine` 的副本上，损失清单为「无法观测所有权、无法观测 ACL」——隔离标记在 macOS 上可观测也可保留，因此不列为损失，原件哈希与隔离标记全程未变；按默认「另存副本」经原生存储面板写出含 `QMARK` 标记的副本。注：工作目录在会话中途被外部回退，替换后的字节与副本文件未能归档（当场哈希与内容核对见 summary.json），唯一存档截图为提示框布局 `gui/11-prompt.png`。真实 ACL 边界、非 NTFS／网络盘、断电与 Windows GUI 确认操作仍未做，T110 保持 `in_progress`。

### T111：工程门禁自动化

- 输入：ARCHITECTURE §1 的层表与禁止依赖、AI 专题的核心 crate 边界、现有 clippy 与 CI 门禁。
- [x] 状态：`done`
- 需求：`QUALITY-010`
- 前置：`T001`
- 产物：逐 crate 启用 no-panic deny lint（现存 unwrap／expect／panic 清理为返回错误或带注释的局部豁免）；engine 事务外层 panic 守卫覆盖命令执行、不变量检查与 ChangeSet 生成；全命令与全事务恶意输入测试（panic 计数断言为零，覆盖守卫内路径）；依赖方向与未登记 crate 自动检查；文件系统访问白名单源码扫描；以上全部进入 CI。
- 错误路径：lint 违规、守卫外 panic、未登记依赖、白名单外的文件系统调用均使 CI 失败。
- 自动验证：注入 panic 的恶意输入测试先失败后通过；分层与白名单检查各有反例测试。
- 人工验收：逐条审查 lint 豁免清单与门禁配置。
- 完成证据：`scripts/check_gates.py` 已进入 CI（`.github/workflows/ci.yml`）：分层依赖、文件系统白名单、panic 基线（`scripts/panic_baseline.txt`）与反例；engine `apply` 有 panic 守卫（`catch_unwind` 恢复文档／策略／撤销栈并返回 `TxError::Interrupted`），注入 panic 后文档与版本不变。2026-10-17 人工验收已执行：基线 513 条逐条归类审查（测试／示例代码；生产代码均为“先校验后使用”的内部不变量、资源耗尽防护、静态输入或启动装配错误、测试注入钩子四类之一），门禁配置（LAYERS、FS_ALLOW、扫描规则、CI 接入）逐项核对，本机 gates 与 counterexample 全部通过；审查中发现 `check_gates.py` 在 GBK 默认编码的 Windows 上读取基线失败，已改为显式 UTF-8（CI 行为不变）。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。
- 2026-10-22 门禁加固跟进（[#86](https://github.com/willmove/joyoffice/pull/86)，分支 `chore/gate-hardening-20261022`，只改 `scripts/`，未跑 cargo）：本条如实记录加固之前门禁的实际强度，并更正上面 2026-10-17 记录里的两处数字。层检查此前一条真实依赖也没有读到——`dependencies()` 的 `DEP_RE` 只认 `name = "..."` 内联写法，而本仓库几乎所有内部依赖都写成点号形式 `name.workspace = true`，13 个 manifest 里只解析出 1 个内部依赖，因此 ARCHITECTURE §1 的层表在本次之前基本没有强制力，唯一可靠触发的路径是新 crate 未登记；现在 `dependency_tiers()`（`DEP_KEY_RE` 取代 `DEP_RE`）认全三种写法（`name.workspace = true`、`name = { ... }`、`[dependencies.name]`）以及 `[target.'cfg(...)'.dependencies]`／`.dev-dependencies`／`[build-dependencies]`（build 依赖按普通层约束），普通依赖严格照 `LAYERS` 校验、不留基线容忍，dev 依赖照 `LAYERS ∪ DEV_LAYERS` 校验，`DEV_LAYERS` 是显式登记的许可表（六对：cli→corpus、docx→corpus、layout→engine、layout→render、pdf→engine、render→pdf，每条注释指明需要它的 manifest 行），键与值都必须已在 `LAYERS` 里，写错不能悄悄放宽；实测所有 `[dependencies]` 与 target 专属依赖边本来就合法，加固没有改任何 `Cargo.toml`。no-panic 基线双向漂移：删掉 12 条指向已不存在代码的条目（陈旧条目本身就是绕过，同文件同文本再出现会直接放行），补录 19 条被旧"集合差"语义藏住的真实位点（19 条全部位于 `#[cfg(test)]` 模块体内，属 T111 归类的第一类测试代码，生产位点一条未增），净 −12／+19，`scripts/panic_baseline.txt` 由 554 行变为 561 行；上面记的"基线 513 条"是 2026-10-17 的数字，此后在加固之前就已漂到 554。`check_panic()` 改为按 `(文件, 代码文本)` 的 `Counter` 多重集比较，报出两侧计数与当前全部行号，行号漂移仍容忍；死代码 `extra = sorted(set(current) - set(expected))` 删除；`--write-baseline` 与检查共用 `gated_panic_sites()`／`panic_scope()`，另加 `--update-baseline` 只追加缺额、既有条目字节不变，全量重写与可审阅的增量补录两种模式从此可区分。`#[cfg(test)]` 截断此前会丢掉第一个标记之后的全部生产代码：本仓库把 `cfg(test)` 用在 `use` 语句、结构体字段和单个 fn 上，不只是测试模块，例如 `apps/joyword/src/document_canvas.rs:6` 是一条 `#[cfg(test)] use`，旧的 `split("#[cfg(test)]")[0]` 把该文件 1945 行里的 1939 行整个丢掉，共 44 个文件被截断；现在 `test_module_ranges()` 只认下一非属性行为 `mod NAME {`／`mod NAME;` 的 `#[cfg(test)]`（含 `cfg(all(test, ...))`），并在 `mask_rust()`（注释与字符串／字符／原始字面量挖空、偏移保持）的输出上做花括号配对，字面量里的花括号不能挪动边界，其余行由 `production_lines()` 全扫。`PANIC_RE` 增加 `todo!`／`unimplemented!`（`assert!`／`assert_eq!`／`debug_assert!` 按指示不加，它们是合法的"先校验后使用"守卫，会把基线冲垮）；含 `allow` 子串就跳过整行的绕过已关闭，改为 `lint_suppressed()`：只有同行或紧邻上一行的 `#[allow(...)]`／`#[expect(...)]` 且点名 panic 族 lint（`unwrap_used`、`expect_used`、`panic`、`unreachable`、`todo`、`unimplemented`、`indexing_slicing`）才抑制，比原任务描述更严，`#[allow(dead_code)]` 不再挡住 `unwrap`；这两项当天在 `crates/`＋`apps/` 里各命中 0 处，属关闭绕过而非清点违规。文件系统门禁的 `FS_RE` 补上 `OpenOptions::new`、`File::open`／`create`／`open_options`、`read_dir`、`DirBuilder`、`remove_dir[_all]`、`hard_link`、`symlink`／`symlink_file`／`symlink_dir`／`symlink_metadata`、`canonicalize`、`read_link`、`std::process::Command`／`Command::new`、`env::set_var`／`remove_var`（匹配调用与 import 形式，文档注释里的散文不误触），`"tests/" in shown` 子串判定换成 `is_test_path()`（`tests` 路径分量或文件名 `tests.rs`）与 `is_example_or_bin()`；加固后的扫描在 main `dd5d0dd` 上只多出一个真实的文件系统违规者 `apps/joyword/src/linux_files.rs`（第 11 行 `use std::process::Command;`、第 198 行 `Command::new(&argv[0])`）——Linux 文件对话框回退：gpui-pre-linux 0.3.9 的 XDG portal 提示在 UKUI/X11 上可能挂住，所以该模块在窗口内问路径并 spawn zenity／qarma／kdialog，与已在白名单里的 `input_ui.rs`（spawn 进程打开 URL）同类，且不能移进 `joyword-shell`（该 crate 不得依赖 GPUI）；协调者据此把它加进 `FS_ALLOW`，理由记在 `scripts/check_gates.py` 的白名单注释里。**这条白名单新增仍待项目负责人批准。**反例与测试：`scripts/tests/fixtures/` 新增 4 个反例夹具，`scripts/tests/test_check_gates.py` 新增 38 个测试；加固后 `python -m unittest scripts.tests.test_check_gates` 38 项全通过，同一套测试对着加固前的脚本（`git show dd5d0dd:scripts/check_gates.py`）是 failures=14、errors=14，即 38 项里 28 项失败。`.github/workflows/ci.yml` 未改：第 48 行自动发现新测试，第 46 行的旧反例调用不变，旧反例仍触发层、文件系统与 panic 三项。加固分支上 `python scripts/check_gates.py` 输出 `gates ok (216 rust files)`（该分支基于 main `dd5d0dd`，不含本次波次的代码）。未做与未覆盖：`FS_ALLOW` 是人工审阅的安全边界，本次只加上面一条且待批准；`panic_sites()` 仍扫描 `#[cfg(test)]` 模块体（T111 已把这些位点计入基线，排除会把基线砍半并削弱门禁，属另一次策略决定）；`check_panic()` 仍是单向的（基线可以比树里多），只清掉当天已陈旧的 12 条并用测试钉住"不得再有陈旧条目"；`examples/` 与 `src/bin/` 仍在文件系统门禁之外（既有行为，保留）；加固不改 `docs/`，所以 ARCHITECTURE §1 那句"由自动检查固化"仍未说明 dev 依赖边由单独审阅的 `DEV_LAYERS` 许可表管辖，这条文档跟进本次未做。T111 的 `done` 本次不改：任务产物要求的门禁都已进入 CI，2026-10-17 的人工审查也确实执行过；但那次审查是在层检查读不到任何真实依赖、`#[cfg(test)]` 之后不扫、含 `allow` 子串即跳过的前提下做的，其"分层与白名单检查各有反例测试"的强度低于当时记录，本条即为更正，是否据此重开 T111 由项目负责人判断。

### T107：M1 发布门槛

- 输入：T102–T106 与 T108–T111 的产物、原创中文样例，以及固定 Word 与 WPS 生成的 DOCX 样例。
- [ ] 状态：`planned`
- 需求：`EDIT-001`、`EDIT-004`、`FORMAT-004`、`QUALITY-004`、`QUALITY-005`、`QUALITY-006`
- 前置：`T102`、`T103`、`T104`、`T105`、`T106`、`T108`、`T109`、`T110`、`T111`
- 产物：中文输入—编辑—保存 DOCX—退出—重开的端到端报告、不可信文件预算与 fuzz 冒烟报告、首个性能冒烟基线。
- 错误路径：任何静默数据丢失、绕过文档保护或无界资源消耗阻断阶段；参考样例缺失不按通过计。
- 自动验证：端到端脚本、保存故障注入、fuzz 冒烟及 20 页样例的打开与输入延迟。
- 人工验收：在 Mac 上完成 M1 工作流，并用 Word 与 WPS 打开保存结果。
- 完成证据：2026-10-17 自动部分已跑：`m1_edit_save` 连续两次通过，覆盖中文编辑、加粗、居中、剪贴板、另存重开和未知部件，受保护文档拒绝编辑，失败的另存不改源字节。`m1_concurrency_recovery` 通过：打开期间的输入保留，过期保存不清 dirty，子进程终止后恢复最后一次提交和原包基线，加密包不写明文副本。不启用 `dev-runtime-shaders` 的 `joyword` 构建连续两次成功。首个性能冒烟基线已建立：无头可复现基准 `apps/joyword/examples/perf_smoke.rs`（release），Windows 参考环境实测——打开启动样例中位 0.099 ms（5 行小文件）、20 页样例首次整篇排版 15.997 ms、输入路径（提交＋整篇重排）中位 0.015＋16.463 ms，远低于 50 ms 输入预算；详见 [IMPLEMENTATION](IMPLEMENTATION.md)。2026-10-18 Word 正文核对已在 Windows 参考机完成；同日端到端报告组装完成（[e2e_report.md](../../tests/fixtures/reference/word2024/e2e_report.md)：打开—中文编辑—保存—退出—重开—Word 核对逐字一致，含关联恢复与性能证据）：Word 2024 永久版（家庭版 2024，build 16.0.20430.20182）COM 只读打开 JoyWord 导出的编辑副本（DOCX-SAMPLE-001 ＋段首插入"校验"），读回正文与包内段落逐段逐字一致（144 字符、1 页），`protection_type` 无保护。WPS 一侧在 Mac 上被要求打开过另存 DOCX（进程运行，无读回）；本 Windows 环境未装 WPS。在 Mac 上完成 M1 工作流并用 WPS 核对的人工验收未完成，T107 保持 `planned`。2026-10-18 WPS 一侧已执行（Mac）：WPS 12.1.29181 打开 JoyWord 另存的 e2e 编辑副本并全选复制读回，与包内正文逐字一致（去中西文空格归一）；Mac 上 M1 工作流（打开—中文编辑—另存—退出—重开—kill -9 恢复）经 GUI 会话多轮验证。前置 T109/T110 完成前 T107 保持 `planned`。

## M2：日常办公

退出条件：含样式、编号、表格、图片、页眉页脚和页码的报告；中文基础排版对照通过；富文本粘贴、PDF 与打印可用（T206）。

### T200：样式、编号与编辑便利

- 输入：段落及字符属性、样式继承图、多级编号和用户查找条件。
- [ ] 状态：`in_progress`
- 需求：`STYLE-002`、`STYLE-003`、`STYLE-004`、`STYLE-005`、`EDIT-006`、`EDIT-007`
- 前置：`T102`、`T103`
- 产物：样式面板、继承、主题、多级编号、制表位、查找替换与格式刷。
- 错误路径：样式循环、编号冲突、替换跨修订或只读范围显式拒绝。
- 自动验证：样式解析、编号重启、批量替换原子性和 Word 往返测试。
- 人工验收：制作多级标题与列表，格式刷、查找替换和撤销。
- 完成证据：2026-10-18 样式基础切片已落地，任务保持 `in_progress`。已覆盖：按 ECMA-376 §17.7.2／§17.7.3 解析 `docDefaults`、`basedOn` 链和段落／字符直接格式（字符 toggle 显式关闭或翻转，段落开关取最近指定值）；`w:link` 只用于应用配对，不作为第二层属性；表格与编号样式保存但不解析；主题 `fontScheme`／`clrScheme` 的具体字体名与 sRGB（无 tint／shade）。引擎 `ApplyStyle` 是一次可撤销事务（行为登记 `STYLE-APPLY-UNDO`）。外壳命令 `style.list`（编辑菜单）与 `style.apply` 经命令注册表执行。导出在既有拒绝规则内写回 `w:pStyle`／`w:rStyle`，不改写样式部件；未知样式、错误类型、断裂链和超出子集的直接格式拒绝。排版消费解析后的字号、粗体、斜体、对齐、间距和分页开关，并把请求的拉丁／东亚字体名记在字形上，整形仍用调用者提供的面。T208 段落缓存在样式定义变化时按 `basedOn`／`link` 闭包失效，增量结果与全量一致。证据：`theme_font_and_colour_resolve_when_the_scheme_has_them`、`applying_a_paragraph_style_is_one_undoable_transaction`、`style_list_and_apply_go_through_the_command_registry`、`style_assignments_export_and_unknown_styles_are_rejected`、`min_002_recognises_builtin_styles_by_name_and_keeps_mode_15`（DOCX-MIN-002 解析为宋体 + Times New Roman、标题 16 pt 粗体、正文 10.5 pt）、`resolved_styles_set_size_weight_and_requested_families`、`a_style_definition_change_refits_dependents_and_reuses_the_rest`。2026-10-18 编号与制表位切片继续落地，任务仍 `in_progress`。已覆盖：编号部件的 `abstractNum`／`num`／`lvl`／`lvlOverride`／`startOverride` 进入模型；`numStyleLink`／`styleLink` 保留但不跟随；段落与样式上的 `ilvl`／`numId`（`numId` 0 取消样式列表）；按文档顺序计算计数，遵守 `lvlRestart` 与 `startOverride`。可绘制的格式为 decimal、lowerLetter／upperLetter（1..=26）、lowerRoman／upperRoman（1..=3999）、chineseCounting 与 chineseLegalSimplified（1..=99999）、ideographTraditional（1..=10）和 `none`。其余 `ST_NumberFormat`（含 bullet）给出不支持诊断，不猜测标签。标签按级别缩进、悬挂和后缀（tab／space／nothing）排入页面场景，屏幕与 PDF 共用。制表位来自段落、样式和级别，对齐 left／center／right／decimal，前导符 none／dot／hyphen／underscore；缺省间隔取 `w:defaultTabStop`，缺省 720 twip。bar、middleDot、heavy 与相对位置拒绝。引擎 `SetNumbering`／`SetNumberingLevel` 一次可撤销（`LIST-APPLY-UNDO`、`LIST-LEVEL-UNDO`、`LIST-REMOVE-UNDO`）。外壳 `list.apply`（编辑菜单，级别 0）、`list.level`、`list.remove`（编辑菜单）经注册表执行。导出只在既有拒绝子集内改写段落 `w:numPr`，不改写 `numbering.xml`；改编号定义、默认制表位，或在带制表位的段落上改编号，都拒绝。T208 缓存键含标签与默认制表位间隔；前面项的计数变化使后面的项与全量一致，无关段落命中缓存。语料中没有 Word 2024 编号样例，测试用合成包和内存文档。证据：`counters_restart_only_the_level_that_names_the_trigger`、`chinese_counting_and_legal_follow_place_value`、`applying_a_list_is_one_undoable_transaction`、`changing_a_list_level_is_one_undoable_transaction`、`removing_a_list_is_one_undoable_transaction`、`numbering_and_tab_stops_import_and_export_keeps_the_numbering_part`、`chinese_numbering_labels_are_prefixed_and_unsupported_formats_are_diagnosed`、`a_left_tab_places_the_following_glyph_at_the_stop`、`a_list_counter_change_refits_later_items_and_reuses_an_unrelated_paragraph`、`list_apply_level_and_remove_go_through_the_command_registry`。2026-10-18 查找替换、格式刷与样式面板切片继续落地，任务仍 `in_progress`。已覆盖：引擎在快照上按 Unicode 扩展字素匹配，大小写按整个字素比较，不拆开组合字符；ß 不匹配 SS。整词把 CJK 字素当作边界，因此「中」可在「中国」中命中，「cat」不命中「category」。匹配不跨行内对象，可跨 run。命中上限 256，超出后截断并停止扫描。空查询与超长查询拒绝。替换由既有删除与插入组成，从后往前，一次事务可撤销，替换文本不再被搜索。内容锁定、保留属性和保留行内对象的命中计入跳过次数，不改那些内容。只读会话仍可查找，替换返回只读错误。外壳 `find.next`（编辑菜单，Cmd/Ctrl+F）、`find.previous`、`find.replace` 与 `find.replace_all` 经注册表执行。格式刷复制可导出的段落样式、对齐和直接字符格式；主题色、主题字体、隐藏、语言、保留标记、非半磅字号，以及 ascii 与 hAnsi 具名不一致，在复制时拒绝。提示字体和具名复杂文种字体不进入刷子，也不因此拒绝。改变样式 id 的目标须通过样式指定子集；只刷直接格式时目标须是空白段落。一次撤销恢复刷入前的格式。`format.copy`（编辑菜单）、`format.paint` 与 `format.cancel` 经注册表；重复刷由外壳保持刷子。样式面板列出文档样式及解析后的粗体、斜体、半磅字号、颜色和具名字体，点击走 `style.apply`，并标出插入点的段落样式与字符样式。表格与编号样式只列出，点击仍被引擎拒绝。证据：`grapheme_case_and_cjk_word_boundaries`、`a_match_does_not_cross_an_inline_and_does_cross_runs`、`replace_all_is_one_undo_and_skips_locked_and_preserved`、`replacement_does_not_search_again_and_rejects_a_separator`、`search_still_runs_when_replacement_is_read_only`、`hit_count_is_bounded`、`an_emoji_sequence_is_one_cluster`、`painting_a_format_is_one_undoable_transaction`、`painter_refuses_theme_colour_and_a_preserved_target`、`direct_paint_refuses_a_paragraph_that_already_has_a_style`、`find_replace_and_format_painter_go_through_the_command_registry`。行为登记 `FIND-REPLACE-UNDO`、`FORMAT-PAINT-UNDO`，来源是假设，没有 Word 观察。`unicode-segmentation` 1.13.3 成为引擎的直接依赖，版本仍是锁文件中的既有钉扎，没有新的外部包，也没有正则。没有启动图形界面。2026-10-19 第四切片继续落地，任务仍 `in_progress`。已覆盖：`numFmt` 为 bullet 时标签是 `lvlText` 在 `%%` 换成 `%` 之后的字面量，`%1` 不再当计数；计数仍前进。`lvlPicBulletId` 记在级别上，标签拒绝为 `NumberingProblem::PictureBullet`，不发明字形，计数仍前进。级别 `w:rPr` 的半磅字号、粗体和具名 ascii／hAnsi／eastAsia 字体用于标签：字号或粗体与正文不同时标签单独成一个字形 run；字体名是 Symbol 或 Wingdings（大小写不敏感）时，把 U+F020–U+F0FF 按公开编码表映射到 Unicode 再用调用者提供的面整形，不按字体名换面。Wingdings 2／3 与 Webdings 不映射。lowerLetter／upperLetter 按规范示例重复：下标 `(n-1)%26`，重复 `(n-1)/26+1`，27 为 aa、54 为 bbb；重复超过 64 仍不可绘制。罗马数字仍只画 1..=3999（`ROMAN_MAX`），不发明上横线或 MMMM。列表或自定义制表位上的居中、右对齐和两端对齐不再整段拒绝：居中和右对齐只移动正文簇，标签留在悬挂位置；两端对齐只拉伸纯中文正文，拉丁文仍是既有的「inter-character justification」拒绝；制表符保持绝对停靠，不再被居中挪动。只有缩进、没有列表也没有自定义制表位时，居中／右对齐／两端对齐仍拒绝。主题色的 themeTint／themeShade 按 ECMA-376 §17.3.2.6 的线性 sRGB 计算（半入向上取整）；两者都有时先 shade 再 tint，这是规范未写明时的假定，不是 Word 的 HSL 偏差。解析后写成 Rgb。非自动颜色仍不绘制。表格样式经 `basedOn` 从根到叶解析整表格式，再按带、首行、末行叠条件格式；本切片至少含 run 粗体、底纹和边框。粗体是指定层：run 没有直接粗体时强制开或关，不是 §17.7.3 的翻转。直接单元格边框、直接底纹和直接 run 粗体优先。横向条纹一行一条；首行外观打开时第 0 行不参与条纹，末行外观打开时末行不参与并盖过条纹。条纹下标 0 是 band1。`w:tblLook` 按 §17.4.56 的位和属性解析，属性盖过位；元素省略时用应用默认（首行、末行、横向条纹开，纵向条纹关，两列关），这是假定，不是 Word 观察；元素存在而属性省略则为假。首列、末列和纵向条纹只存储并导出，不参与排版。省略 `tblLook` 的表在改写时不发明该元素；改过且 look 有值的表写回 1／0 属性。未知标记（改用 `w:tblOverlap` 作为不透明夹具）仍整表不透明。主题色边框和底纹仍不解析。`list.apply` 的 `num` 与 `bullet` 都可选，给出 bullet 时优先；两者都缺则失败信息为 `num`。bullet 在引擎事务里 `ensure_bullet`：已有无图片的 0 级项目符号就复用，否则在内存中新建 U+2022、制表后缀、起始 720 twip／悬挂 360 twip（JoyWord 预设，不是 Word 测量）。一次撤销恢复编号库。新建定义后的导出仍拒绝，因为不改写 `numbering.xml`；套用文档里已有的项目符号只改段落 `numPr`，可在既有规则内导出。功能区 `start.list_bullet` 在取消列表之前，调用 `list.apply` 且 `bullet=1`，图标 `list-bullet`。目录下限升到 41。标签字号和粗体进入段落缓存键，增量与全量一致；有表时仍整篇重排。证据：`bullets_use_lvl_text_and_picture_bullets_advance_without_a_glyph`、`symbol_bullet_and_wingdings_shapes_follow_the_published_bytes`、`tint_and_shade_follow_the_linear_examples`、`table_styles_layer_bands_under_first_and_last_rows`、`centered_and_right_lists_move_only_the_body_and_label_metrics_split_the_run`、`justified_lists_stretch_chinese_body_text_and_tabs_keep_their_stops`、`a_label_size_or_bold_change_misses_the_cache_and_matches_a_full_layout`、`table_styles_paint_bands_and_direct_formatting_wins`、`tbl_look_bits_and_attributes_round_trip_after_a_cell_edit`、`list_apply_bullet_is_one_undo_and_the_ribbon_sends_it`。未覆盖：按字体名换面、甲乙超过 10、中文计数超过 99999 或为 0、罗马数字超出 1..=3999、字母重复超过 64、bar／middleDot／heavy、编号部件改写、非自动颜色与下划线的字形绘制、PDF 斜体剪切、格式刷省略的 hint 与具名复杂文种、整段链接字符样式仍走既有 `ApplyStyle` 规则、已有样式的段落在不改变样式 id 时不能刷直接格式、Wingdings 2／3 与 Webdings、只有缩进时的居中／右对齐／两端对齐、首列／末列与纵向条纹的排版、主题色底纹，以及人工验收。无 tint／shade 的主题色边框、单元格 `tcMar`、`nil`／`single` 以外的边框和百分比宽度已由 T201 第三切片补上；嵌套表仍未做。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。2026-10-19 查找折叠切片继续落地，任务仍 `in_progress`。已覆盖：查找新增全角半角折叠选项 `fold_width`，打开后 U+FF01..=U+FF5E 减 0xFEE0、U+3000 落成普通空格再比较，"ABC" 命中 "ＡＢＣ"、"123" 命中 "１２３"、半角空格命中全角空格，汉字不受影响；折叠表是文档化假定，不是 Word 观察，大小写仍按整字素折叠（ß 不扩成 SS）。查找新增正则选项 `regex`，模式用锁文件既有的 `regex` 1.13.1 编译（gpui-pre 快照线已带入，非新外部包版本），只走线性时间匹配、无回溯面；引擎按段落把宽度折叠后的字素簇拼成干串、行内对象换成 U+FFFC 占位，命中映回标量偏移后，拆字素簇、盖住行内对象或零宽的命中一律跳过，整词沿用既有字素边界；模式编译失败返回 `InvalidPattern`，替换文本始终是字面量、`$1` 不展开，锁定与保留段落仍只计入跳过。外壳 `find.next`／`find.previous`／`find.replace`／`find.replace_all` 参数表新增可选布尔 `width` 与 `regex`，`EditRequest` 相应增项并经注册表执行；应用查找栏暂未加新勾选项，只透传命令参数。证据：`fullwidth_forms_fold_to_halfwidth`、`width_folding_combines_with_case_folding`、`regex_find_matches_patterns`、`regex_skips_zero_width_and_cluster_splitting`、`regex_never_crosses_an_inline_object`、`regex_width_folding_applies_to_the_haystack`、`regex_hit_count_is_bounded`、`regex_replace_is_one_undo_and_skips_locked`、`find_width_and_regex_flags_go_through_the_command_registry`。未覆盖：正则替换的捕获组引用、图形界面查找栏的折叠／正则勾选项、与 Word 查找选项逐项对照的人工验收；`apps/joyword` 仅做参数透传适配。本切片不改 W09 harness。 2026-10-19 银河麒麟 GUI 部分人工验收通过：双击按字素选中、整段加粗、查找「English」并替换为「ENGLISH」（状态栏「1 处匹配」）、格式刷取源后应用到目标段（状态栏「已应用格式」）、一次 Ctrl+Z 撤掉整组打字与逐项撤销均正常。两项缺陷：其一，早期构建上对段内单字加粗使整画布以 `Unsupported { feature: "mixed character formatting" }` 拒绝渲染（撤销可恢复），同日 `da5f192` 改为按 run 跨度绘制后在本机复验通过；其二，功能区「项目符号」按钮仍报「输入未完成：num」——应用侧 `execute_input` 的 `LIST_APPLY` 分支只读 `num`、忽略 `bullet`（shell 层已有 bullet 分支但应用未走），待修。截图与细节见 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json)（`kylin-gui-format-painter-applied.png`、`kylin-gui-mixed-format-canvas-error.png`、`kylin-gui-bullet-button-num-error.png`）。2026-10-19 应用入口修补：`execute_input` 的 `LIST_APPLY` 在有 `bullet` 时调用 `apply_bullet`（与 shell host 一致，同时给出 `num` 时 bullet 优先），缺参失败信息仍是 `num`。证据 `ribbon_bullet_reaches_the_document_without_num`。麒麟窗口未重跑，T200 保持 `in_progress`。2026-10-19 查找栏勾选切片继续落地，任务仍 `in_progress`。查找栏在区分大小写和全字匹配之后增加「全半角」与「正则」勾选（英文 Match width／Regex；外壳语言包还没有 `find.width`／`find.regex`，缺键时用上述回退）。勾选打开后，`find.next`／`find.previous`／`find.replace`／`find.replace_all` 经既有参数 `width`／`regex` 送出 `true`；两者都关时这两个布尔为 `false`，与省略时一样不折叠、不按正则。功能区查找按钮仍只绑定查询、大小写和整词，应用在参数缺 `width`／`regex` 时补上当前勾选，已经显式给出的值保持不变。半角查询在勾选全半角后命中全角形式；勾选正则后 `\d+` 能替换数字，非法模式 `(` 走既有 `InvalidPattern`，应用侧收到 `Rejected("query is not a valid pattern")`，正文不变。证据：`find_bar_checkboxes_set_width_and_regex_on_every_find_command`、`find_bar_width_hits_fullwidth_and_regex_rejects_invalid_patterns`。没有启动图形界面。人工点选验收与信创真机仍未做。2026-10-19 no-panic 门禁把 `main.rs` 测试模块里的 `unwrap` 计为新 panic 点，上述两项检查改放到 `apps/joyword/src/tests.rs`（门禁按文件名跳过），行为不变，T200 仍 `in_progress`。2026-10-19 正则替换捕获组切片继续落地，任务仍 `in_progress`。已覆盖：正则命中带上 `FindHit::captures` 后，`find.replace`／`find.replace_all` 展开 `$0`（整段命中）与 `$1`..`$n`；`$$` 变成一个 `$`；`$` 后的连续 ASCII 数字是一个组号，所以 `$10` 不是 `$1` 再加 `0`。组号超出模式（`(a+)(b+)` 的 `$9`）或数字放不进 `usize` 时，`$` 和数字原样留下，不拒绝整次替换。没参加匹配的可选组换成空串。非正则替换不展开，`$1` 与 `$$` 仍是字面量。插入的是段落原标量：大小写不敏感时保留原文大小写，全半角折叠时 `$1` 仍是全角。展开结果仍受替换长度上限和普通文字检查，失败则整次计划不写入。锁定与保留命中仍跳过，多次展开放进一次事务，一次撤销全部恢复。单次替换的选区按展开后的标量长度。没有新的外部包，没有启动图形界面。证据：`regex_replace_expands_groups_and_leaves_unknown_dollars`、`regex_replace_keeps_original_case_and_width`、`regex_replace_skips_locked_and_preserved_and_undoes_once`、`regex_capture_replacement_goes_through_the_command_registry`。行为登记 `FIND-REPLACE-CAPTURE`，来源是假设，没有 Word 观察。人工点选验收与信创真机仍未做。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。
- 2026-10-20 文档核对：查找折叠切片里「替换文本始终是字面量」只描述该切片；捕获组切片已展开 `$0`、`$1`..`$n` 与 `$$`。进度总览已写明捕获组落地。按字体名换面、甲乙超过 10、超限编号、bar／middleDot／heavy、编号部件改写、主题色文字和人工验收仍未做。状态保持 `in_progress`。见 [验收清单](acceptance/T200.md)。
- 2026-10-20 验证波次收口：S5 排练 `t200_headings_lists_painter_and_regex_undo_round_trip` 走命令注册表完成标题样式、两级列表、格式刷、正则 `$2-$$-$1` 替换、逐步撤销和另存重开，见 [场景排练](verification/scenario-rehearsals.md)。S1 金样 `t200-numbering-tab` 与 S3 样例 DOCX-FEAT-T200-NUM 锁定编号标签和点线制表位，见 [页面场景金样](verification/golden-scenes.md)与[特性样例](verification/feature-fixtures.md)。已知缺口已修复（[#67](https://github.com/willmove/joyoffice/pull/67)，main `caf84147`）：CPU 光栅原先以 `InvalidGeometry` 拒绝编号加制表位的行；现在正文从标签后的制表位开始，光栅允许笔位向前跳过制表间隙和点线引导符余量，样例的 `render` 成功，光栅金样已重新 bless，见 [验证波次缺口修复](verification/verify-wave-fixes.md)。人工验收仍未做。状态保持 `in_progress`。
- 2026-10-21 编号超限与主题色文字切片继续落地，任务仍 `in_progress`。已覆盖：lowerLetter／upperLetter 仍按下标 `(n-1)%26`、重复 `(n-1)/26+1`，重复次数从 64 放到标签预算 256 个 Unicode 标量（65 个 a、256 个 a 可绘制，257 次重复不可绘制）；0 与负数仍不可绘制。正数罗马数字在 `ROMAN_MAX`（3999，古典三次重复 `MMMCMXCIX`）之后沿用同一加法表，4000 为 `MMMM`，不使用上横线；0 与负数仍不可绘制；标签将超过 256 个标量时停止分配并记为不可绘制。`ideographTraditional` 超过 10 时按天干重复：11 为甲甲、12 为乙乙、21 为甲甲甲。这是 JoyWord 假定，不是 §17.18.59 示例里的句子，也不是 Word 观察；0 与超过预算仍不可绘制，不退回十进制。`chineseCounting` 与 `chineseLegalSimplified` 覆盖每一个非负 `i32`：0 为同一内部码位 U+96F6「零」，按四位一组继续到万和亿；跳过的组，或更高组之后不足 1000 的组，插入一个零；十作为整个数字的首字时省略前导一（10 为十，100000 为十万，100010 为十万零一十，10000 仍为一万）。负数不可绘制。`i32::MAX` 为二十一亿四千七百四十八万三千六百四十七。主题色文字在方案里有对应项时，解析把 tint／shade 收成 Rgb 后绘制（先 shade 再 tint，沿用第四切片的假定）；缺方案项时段落拒绝。改写过的 run 写出 ECMA-376 §17.3.2.6 要求的 `w:val`（计算后的 sRGB）以及 `w:themeColor`，有 tint／shade 时再写两位大写十六进制的 `w:themeTint`／`w:themeShade`。重新导入恢复 `Color::Theme`，不改写主题部件。缺方案项的导出拒绝，原因是 `theme colour has no scheme entry`。格式刷仍拒绝主题色。表格底纹仍拒绝主题色。编号部件仍不改写。bar／middleDot／heavy 仍拒绝。按字体名换面未做。证据：`letters_roman_and_ideographs_follow_the_repeat_and_the_label_budget`、`chinese_counting_and_legal_follow_place_value`、`numbering_edges_place_labels_past_the_old_caps`、`theme_colour_text_paints_the_resolved_srgb`、`theme_colour_text_round_trips_when_the_run_is_rewritten`。没有启动图形界面。没有 Word 观察。W09 自 `e8c4776` 起按检出的 `crates/` 树标识绑定，本切片不重锚 SHA，也没有在本机重跑固定矩阵 smoke。状态保持 `in_progress`。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。
- 2026-10-22 `numbering.xml` 写回切片（[#85](https://github.com/willmove/joyoffice/pull/85)，任务仍 `in_progress`）：导出侧新模块 `crates/joyword-docx/src/export/numbering.rs` 把追加到内存 `NumberingStore` 的编号定义确定性写回 `word/numbering.xml`。`plan` 在原库与当前库相等时返回 `None`，part 字节原样复制并在 preserved 列表报告；否则 `appended()` 只接受"追加式"差异——旧的 abstractNum／num 必须是新序列逐元素的前缀且 `unmodeled` 标志一致，被修改、被删除或 `unmodeled` 变化的既有定义一律以 `existing numbering definitions are not rewritten` 拒绝，part 绝不重建。既有 part 走 `patch_part`：解析 `word/numbering.xml`，按 ECMA-376 §17.9.17 的元素顺序（`abstractNum*` → `num*` → `numIdMacAtCleanup?`）把新条目的标记拼在最后一个既有 abstractNum／最后一个既有 num 之后、其他尾随子元素之前，除这两个插入点外没有任何字节移动（测试删去插入的标记后与原 part 逐字节相同），自关闭根元素也能处理；`nsid`、`lvlJc`、`numIdMacAtCleanup` 等未建模子元素与 `numStyleLink`／`styleLink` 原样保留。缺 part 时 `create_part` 新建 `word/numbering.xml`（含 XML 声明），`extend_relationships` 只在 `word/_rels/document.xml.rels` 追加一条 numbering 关系（rId 取现有最大数字加一，追加在 `</Relationships>` 之前），`extend_content_types` 只在 `[Content_Types].xml` 追加一条 Override；两者都在超链接与批注阶段产出的字节上继续追加，关系追加发生在超链接之后、`comment::apply` 之前。写出的级别子集按 §17.9.11 顺序：`start`、`numFmt`、`lvlRestart`、`suff`（仅在不等于 tab 时写）、`lvlText`、`pPr`（按 §17.17.86 把 tabs 写在 ind 之前）、`rPr`（按 §17.18.74 的 rFonts＜b＜sz），`w:num` 按 §17.9.14–15／§17.9.19 写 `abstractNumId` 再写 `lvlOverride*`（`startOverride` 为空元素，替换级别为 `lvl`）；级别制表位的全部 7 种对齐与 6 种引导符都能写出并往返。对链接定义、未建模条目或级别、图片项目符号、Theme 字体、超出子集的段落／字符属性、非 twips 的缩进与制表位、非半磅的字号、XML 属性规范化无法往返的控制字符与 U+FFFE／U+FFFF、指向不存在 abstract id 的实例、空格式记号，导出以可定位的 `ExportError::Unsupported` 显式拒绝，不猜测也不假装成功。`export.rs` 的改动是增量式的：`numbering_changed` 参与 OriginalPackage 提前返回的判定，`numbering_delta` 计入 `projected` 与 `grown` 两处预算投影，重导入校验显式比较编号库（`canonical_dump` 不含它），`check_envelope` 里"`before.numbering() != after.numbering` 即整体拒绝"的旧规则被删除，改由 `export::numbering` 的追加式差异承担，因此 `list.apply bullet=1` 新建定义后的导出不再被一律拒绝；段落 `w:numPr` 的改写规则本切片未动。模型侧只加 `NumberSuffix::token()`（§17.18.88 的 `w:suff` 记号），`NumberFormat::token` 已是 `pub`。证据：`crates/joyword-docx/tests/numbering_writeback.rs` 六项——`appended_bullet_definitions_are_inserted_without_touching_other_numbering_bytes`、`a_new_numbering_part_gets_a_relationship_and_a_content_type_override`、`an_unchanged_numbering_store_keeps_the_part_verbatim_and_preserved`、`restoring_the_previous_store_keeps_the_package_unchanged`（直接恢复上一份模型快照后导出与原包字节相同，不执行 engine 撤销）、`modified_or_unrepresentable_numbering_changes_are_refused`、`numbering_level_tabs_write_every_align_and_leader`；端到端要求（`ensure_bullet` 追加定义 → 导出写出 → 重导入复原编号库并画出 `1.`／`•`／`startOverride` 的 `4)`）在模型与 docx 层覆盖，没有新增引擎测试（`Command::ApplyBullet` 就是 `ensure_bullet` 加 numPr 赋值，撤销沿用引擎既有的快照恢复）。切片内自动验证通过：`cargo test --locked`（CI 口径）、`cargo test --locked -p joyword`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`check_docs.py`、`check_gates.py`（含反例）、`check_acceptance.py`；`python -m unittest discover -s scripts/tests` 有 1 项环境性失败（`test_committed_bytes_and_manifest_hashes_match`：本机 Python 3.14.7 链接 zlib-ng 1.3.1，级别 6 的 deflate 字节与 CI 的 stock zlib 不同，5 个 DOCX-FEAT 夹具的压缩字节哈希因此不一致，解压后的成员名、内容与 CRC 完全相同；在干净的 main 上同样失败，与 `core.autocrlf` 无关，`.gitattributes` 已把 `*.docx` 标为 binary）。可选的制表位引导符一项只是部分关闭：编号部件写出侧已能写出并往返全部 6 种引导符与 7 种对齐，但排版侧仍拒绝 `Bar` 对齐（`crates/joyword-layout/src/list_layout.rs`）与 `MiddleDot`／`Heavy` 引导符，段落制表位的导出拒绝也仍在 `export/structure.rs` 的 `geometry_supported`／`tabs_markup` 里；`heavy` 没有规范字形，发明一个会违反不猜测原则，所以 bar／middleDot／heavy 的端到端支持仍未做，不按完成记。人工验收全部未做：Word／WPS 打开含追加编号定义的导出件并往返对照、图形界面里点项目符号后保存重开、信创真机、打印均未执行。T200 保持 `in_progress`。

### T201：基础表格编辑

- 输入：合法段落模型、表格网格定义、行列选择和 DOCX 表格规范样例。
- [ ] 状态：`in_progress`
- 需求：`LAYOUT-005`、`STYLE-004`
- 前置：`T200`
- 产物：真实网格、行列增删、基础合并拆分、边框／底纹及表格内选择。
- 错误路径：不规则合并、结构破坏和跨只读对象编辑不能产生非法模型。
- 自动验证：网格不变量、内容归属、合并逆操作和 DOCX 读写样例。
- 人工验收：制作合同表格，插行、合并、拆分、撤销、保存重开。
- 完成证据：2026-10-18 第一切片已落地，任务保持 `in_progress`。已覆盖：`w:tbl` 仅在完全可表示的子集内成为类型化表（`tblGrid` 的 twip 列宽、`tblW` 的 auto／dxa、`gridSpan`、`vMerge`、`tcW`、`nil`／`single` 边框、`clear` 底纹、仅含 `pStyle`／`jc` 与 `b`／`i`／`sz`／sRGB 的单元格段落）。缺网格、修订、嵌套表和其他未知标记整表保持不透明。`w:tblLook` 在 T200 第四切片进入模型，不再单独把表锁成不透明，不分配标识，也不改写未知 XML。省略的 `vMerge` `w:val` 按 ECMA-376 §17.4.84 记为 `continue`；省略的边框 `sz` 记为 4、颜色记为 `auto`。导出仍走既有拒绝规则：未改的表复制原字节；改过的类型化表整表重写；含锁定节点（含不透明表）的编辑导出返回 `Restricted`，不生成丢掉该表的包。引擎 `InsertTable`／`InsertTableRow`／`DeleteTableRow`／`InsertTableColumn`／`DeleteTableColumn` 各是一次可撤销事务（行为登记 `TABLE-INSERT-UNDO`、`TABLE-ROW-UNDO`、`TABLE-COLUMN-UNDO`，来源是假设，没有 Word 观察）。行列上限 1..=32；不能删最后一行或最后一列；带垂直合并的表拒绝增删行，带水平跨列、垂直合并或 `gridBefore`／`gridAfter` 的表拒绝增删列。新表列宽取节内容宽度的 twip 等分（A4 中文页为 4153），否则每列 1440 twip。新单元格是空段加四边 `single`、`sz` 4、颜色 `auto`。外壳 `table.insert`（编辑菜单，默认 2×2）、`table.row.insert`、`table.row.delete`、`table.column.insert`、`table.column.delete`、`table.select` 经命令注册表执行。单元格选择覆盖该格全部段落；跨单元格删除仍是 `CrossContainer`。排版把单元格段落按网格跨度减去左右各 108 twip 的可用宽度放入同一 `PageScene`，边框和 sRGB 底纹画成矩形，屏幕与 PDF 共用；有表时增量排版改为整篇重排。证据：`grid_invariants_reject_a_short_row_and_an_orphan_merge`、`a_represented_table_is_typed_and_an_unknown_sibling_stays_opaque`、`editing_a_cell_round_trips_and_keeps_merge_state`、`a_simple_grid_table_round_trips_after_a_cell_edit`、`an_edited_paragraph_beside_an_opaque_table_is_refused`、`inserting_a_table_is_one_undoable_transaction`、`inserting_and_deleting_a_row_is_one_undoable_transaction`、`inserting_and_deleting_a_column_is_one_undoable_transaction`、`column_and_row_edits_refuse_merged_structure`、`cell_selection_covers_one_cell_and_cross_cell_delete_is_refused`、`two_cells_place_glyphs_at_different_x_and_paint_borders`、`incremental_layout_of_a_table_matches_a_full_layout`、`table_commands_go_through_the_command_registry`、`column_commands_refuse_a_merged_table`。2026-10-18 第二切片继续落地，任务仍 `in_progress`。已覆盖：`MergeTableCells`（向右 `gridSpan`、向下 `vMerge`）和 `SplitTableCell` 各是一次可撤销事务（行为登记 `TABLE-MERGE-UNDO`、`TABLE-SPLIT-UNDO`，来源是假设，没有 Word 观察）。向右合并拒绝没有右邻、任一格已有 `vMerge`、保留标记、非段落内容或内容锁定；成功时去掉右邻、跨度相加、两端都是 twip 宽度时相加宽度、右框改用右邻的右框，并把右邻的段落接到后面。向下合并必须从 `None` 或 `Restart` 开始，下一行同一列起点且同一跨度的格子还不是合并的一部分；内容段落上移，继续格至少留一个空段。拆分先解开垂直合并且不重分文字，没有垂直合并时才把 `gridSpan` 拆成若干跨度 1 的格子，文字留在第一格。带合并的表仍拒绝增删行列。排版把 Restart 与随后同一列、同一跨度的 Continue 画成一个视觉跨行：继续格的文字不绘制、也没有插入点几何；顶框在第一片、底框在最后一片、左右框每片都画。默认可在行内按行拆到下一页；`cantSplit` 为真时，当前页放不下但新页放得下就整行移到下一页，行高超过页正文则记一次 `RowTallerThanPage` 并留在一页上溢出。表头重复只取行首连续 `header` 前缀，且前缀高度不超过页正文；前缀更高时记一次 `HeaderTallerThanPage` 且不重复。非前缀的 `tblHeader` 仍保存并导出，但不重复。省略的 `tblCellMar` 用本切片假定的左右各 108 twip、上下 0，这不是 Word 观察；元素存在时省略的边按 ECMA-376 §17.4.43 为 0。`cantSplit` 与四边 `tblCellMar` 进入导入导出，未改的表仍复制原字节。有表时增量排版仍是整篇重排，页和诊断与全量一致。外壳 `table.merge.right`、`table.merge.down`、`table.split` 在编辑菜单，不进功能区。证据：`merging_cells_is_one_undoable_transaction`、`splitting_a_cell_is_one_undoable_transaction`、`a_vertical_merge_spans_rows_and_hides_continue_text`、`a_splittable_row_breaks_across_pages_and_cant_split_moves_whole`、`a_cant_split_row_taller_than_the_page_is_noted_and_kept`、`a_leading_header_repeats_and_a_later_header_does_not`、`a_header_taller_than_the_page_is_not_repeated`、`an_explicit_zero_left_margin_shifts_the_glyph_left_of_the_default`、`incremental_layout_of_a_split_table_matches_a_full_layout`、`cell_margins_cant_split_and_header_round_trip_after_an_edit`、`merge_and_split_go_through_the_command_registry`。2026-10-19 第三切片继续落地，任务仍 `in_progress`。已覆盖：单元格 `tcMar`（ECMA-376 §17.4.68）整组覆盖表级 `tblCellMar`，元素存在而某边省略时该边为 0；`w:start`／`w:end` 在没有 `bidiVisual` 的子集里记成左／右，与 `left`／`right` 同时出现则整表不透明。边框在 `nil`／`single` 之外接受 `double`、`dotted`、`dashed`、`thick`（§17.18.2 的这一子集），厚度都是 `w:sz`／8 磅，省略 `sz` 仍为 4；`thick` 不另乘宽度。`w:themeColor` 没有 tint／shade 时记成主题槽；排版在主题方案里有该槽时画对应 sRGB，没有该槽时保留厚度但不画墨。`tblW`／`tcW` 的 `pct` 是五十分之一百分比（§17.18.90）：表宽相对页心宽度（§17.4.63），跨度 1 的单元格百分比相对表宽（§17.4.71）。省略的 `tblLayout` 按 §17.4.52 当作 autofit，只在首选宽度宽于页心时缩小，不按内容加宽；显式 `fixed` 不缩小。`jc` 为 §17.4.28 的 left／center／right，`tblInd`（§17.4.50）只在最终对齐为左时生效。未改的表仍复制原字节；改过的类型化表整表重写。有表时增量排版仍是整篇重排：行会跨页、表头会重复，按正文段落后缀复制对不上全量页，缓存与全量的页相等测试仍在。证据：`cell_margins_borders_and_percentage_widths_round_trip_after_an_edit`、`a_cell_margin_overrides_the_table_margin`、`double_borders_paint_two_bands`、`an_unresolved_theme_border_keeps_no_ink`、`fixed_layout_does_not_shrink_and_centre_uses_the_leftover`、`omitted_layout_shrinks_only_when_wider_than_the_content`、`incremental_layout_of_a_table_matches_a_full_layout`、`incremental_layout_of_a_split_table_matches_a_full_layout`。未覆盖：嵌套表、`vAlign`、`trHeight`（模型里有字段，导入遇到它们仍整表不透明，排版不用它们）、表格属性对话框、标尺或表格边框上的列宽拖拽，以及制作合同表格的人工验收。表格样式的粗体、底纹和边框见 T200 第四切片；首列、末列与纵向条纹仍不参与排版。没有启动图形界面。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。 2026-10-19 银河麒麟 GUI 会话确认：X11 窗口不渲染菜单栏且功能区无表格控件，`table.*` 命令在 Linux GUI 无入口（macOS 菜单栏不受影响），插行／合并／拆分的图形人工验收在 Linux 上无法执行，记 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json)。2026-10-19 第四切片继续落地，任务仍 `in_progress`。已覆盖：`w:vAlign` 的 top／center／bottom（ECMA-376 §17.4.83）；`both` 与省略 `w:val` 仍整表不透明。`w:trHeight`（§17.4.80，`ST_HeightRule` §17.18.37）的 `auto`／`atLeast`／正的 `exact`：省略 `hRule` 记为 `auto`，`auto` 排版时忽略 `w:val`；省略 `w:val` 记为 0；`atLeast` 拒绝负数；`exact` 缺少或非正的 `w:val` 仍整表不透明，不发明零高度。没有这些元素的表仍按原子集成为类型化表。排版在内容高度之后应用行高：`atLeast` 只抬高，`exact` 改成指定 twip；放不进最终单元格的整行盒不绘制，不把字形切成半行。垂直对齐在边框和边距之内的剩余空间上平移：居中取一半，底对齐取全部；内容已经高于单元格时保持顶对齐，没有按未测量的 Word 裁切把文字拉到单元格上方。`SetTableProperties` 一次写入表宽、对齐、表级 `tblCellMar` 和每个单元格的直接边框，一次撤销（行为登记 `TABLE-PROPERTIES-UNDO`，假设，没有 Word 观察）。表宽不改 `tblGrid`。边框写的是 `tcBorders`，因为 `tblBorders` 不在类型化子集里。对话框取消不调用命令；空变更不开启事务。功能区插入页新增 `table.properties`，无 `commit` 时打开对话框。证据：`valign_and_row_height_round_trip_after_an_edit`、`at_least_centers_the_line_exact_omits_it_and_auto_ignores_val`、`setting_table_properties_is_one_undoable_transaction`、`a_cancelled_table_dialog_does_not_open_a_transaction`、`table_properties_go_through_the_command_registry`。未覆盖：嵌套表、列宽拖拽、首列／末列与纵向条纹（样式解析仍只有行带，本切片不扩 T200）、以及制作合同表格的人工验收。没有启动图形界面。 2026-10-19 嵌套表第一自动切片继续落地，任务仍 `in_progress`。已覆盖：单元格里再嵌一层、且内外表都落在既有类型化子集的 `w:tbl`（ECMA-376 §17.4.64）成为类型化表。第二层嵌套，或内表带有既有子集之外的标记（`tblOverlap`、缺网格、`vAlign` `both`、非正的 exact `trHeight`、主题 tint，以及原先就会拒绝的其余标记），整张外表保持不透明并锁定；旁边的段落编辑仍返回 `Restricted`，不生成丢掉该表的包。排版把内表放进父单元格内容框：内表坐标相对该框，字形和边框加上父格内容横坐标；内表是父行里一块不可拆的高度，不单独跨页，也不单独重复表头。有表时增量排版仍与全量页一致。外格或内格的文字插入各是一次可撤销事务。网格不变量仍拒绝孤立的垂直合并。含嵌套表的单元格合并返回 `UnsupportedCell`。插入点在内表时，结构命令仍不是表编辑位点。未改的嵌套包复制原字节；改过的类型化嵌套整张外表重写，内表随既有 `table_markup` 写出。`table_metrics` 不改接口：嵌套排版把父格内宽传进去，省略的 `tblLayout` 缩到该宽度，显式 `fixed` 不缩。证据：`a_one_level_nested_table_is_typed_and_round_trips`、`unknown_or_deeper_nesting_stays_opaque_and_locks`、`a_nested_table_places_glyphs_inside_the_parent_cell`、`incremental_layout_of_a_nested_table_matches_a_full_layout`、`a_table_nested_two_levels_deep_is_refused`、`editing_outer_or_inner_cell_text_is_one_undoable_transaction`。仍未覆盖：第二层及更深的嵌套、内表已在子集外的标记、内表自己的跨页拆行和表头重复、内表插入点上的行列与合并命令、含嵌套表的单元格合并与拆分、列宽拖拽、首列／末列与纵向条纹，以及制作合同表格的人工验收。没有启动图形界面。没有新增外部包。没有 Word 观察。rebase 到 main `30a76d8583e2ca130b3a0f8ed2b9a6b3f51aab88`（T202 squash）之后，W09 harness 改锚到 `bf91e67d1393064475711478973cd64cd65de662`（该提交的 `crates/` 含一层嵌套表，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。先前锚点 `83c2751cdc9a5c241ebd289a589b4517e009479e` 上的固定矩阵为 81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。固定矩阵尚未在新锚点上重跑。这仍不是覆盖引导 fuzz。独立锁无新包。再 rebase 到 main `26fd7ce2a87440395dddf94cb19c2569c6186669`（T200 find-bar squash，未改 `crates/`）之后，W09 harness 改锚到 `0386738678d03341e03d4cb710d466472e1e2c96`（该提交的 `crates/` 含一层嵌套表，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `02a49a41e800004949857b1065cb33661b6eee3d`（T300 squash）之后，W09 harness 改锚到 `4a54bc81ae1b8f70f213c9425bbab16a3f8f1b4d`（该提交的 `crates/` 含一层嵌套表，以及 T300 的成对移动与 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。


- 2026-10-19 首列、末列与纵向条纹排版继续落地，任务仍 `in_progress`。已覆盖：ECMA-376 §17.7.6 的层叠（整表，然后纵向条纹、横向条纹、首行、末行、首列、末列；后一层盖过同一属性）。纵向条纹一列一条，下标 0 是 band1。单元格取网格起点（含 `gridBefore`），不按 `gridSpan` 拆开。首列外观打开时第 0 列不进纵向条纹，末列外观打开时最后一列不进纵向条纹，和首行末行排除横向条纹一样。`noVBand` 关闭纵向条纹。省略的 `tblLook` 仍用既有应用默认（纵向条纹关，两列关）。粗体是指定值，不是 §17.7.3 的 toggle。直接单元格边框、直接底纹和直接 run 粗体优先。四角单元格不做。有表时增量排版仍是整篇重排，页与全量一致。`w:tblStylePr` 的 `firstCol`／`lastCol`／`band1Vert`／`band2Vert` 这一刀不扩 OOXML 导入：样式部件里只有这些条件时，样式仍按原来记为不完整并保留原字节，排版只画已经放进 `TableCondition` 的条目。`tblLook` 的首列、末列和纵向条纹位本来就会存并在改写时写回。证据：`table_styles_layer_vertical_bands_under_columns_and_rows`、`table_styles_paint_first_and_last_columns_and_vertical_bands`。未覆盖：把这四条条件从样式部件读进模型、四角单元格、主题色底纹、列宽拖拽、合同表格人工验收。没有启动图形界面。没有 Word 观察。MS-OE376 记载 Word 把横向条纹放在纵向条纹之前、把首末行放在首末列之前；本切片按 ECMA-376 的顺序，不采用那条偏差。rebase 到 main `5704bf6226cde228d37e21034c9a8d84b51eb5f7`（T200 捕获组 squash）之后，W09 harness 重锚到 `ce1ef0c3f79be63992f974d8aedd8c5161e9bcfb`（该提交的 `crates/` 含首列、末列与纵向条纹的排版，以及 T200 的正则替换捕获组；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。
- 2026-10-20 文档核对：一层嵌套表和首列、末列、纵向条纹排版已有自动测试。进度总览把这两项写成未做已经过时。列宽拖拽、第二层嵌套、从样式部件读入纵向条件，以及合同表格人工验收仍未做。状态保持 `in_progress`。见 [验收清单](acceptance/T201.md)。
- 2026-10-20 验证波次收口：S5 排练 `t201_contract_table_edits_undo_and_reimport` 覆盖插表、插行、插列、删行、表属性、向下合并、拆分、向右合并、撤销和另存重开。S1 金样 `t201-banded-nested-table` 与 S3 样例 DOCX-FEAT-T201-TABLE 覆盖横向条纹和一层嵌套表。已知缺口已修复（[#67](https://github.com/willmove/joyoffice/pull/67)，main `caf84147`）：样式部件 `w:tblStylePr` 的 firstCol、lastCol、band1Vert、band2Vert 现在读入模型并在页面上填充，样式部件仍原样写回；`vertical_column_bands_are_imported_from_table_style_pr` 不再忽略，见 [验证波次缺口修复](verification/verify-wave-fixes.md)。合同表格人工验收仍未做。状态保持 `in_progress`。

### T202：页面与页眉页脚

- 输入：段落分页属性、物理页面设置、分节及页眉页脚 story。
- [ ] 状态：`in_progress`
- 需求：`LAYOUT-004`、`LAYOUT-012`
- 前置：`T102`、`T103`
- 产物：分页符、页面设置、页眉页脚（首页不同、奇偶页不同、链接到前一节）、页码域、基础分节及孤行等段落分页约束。
- 错误路径：无限 keep 链、过高段落和未稳定页码有界诊断。
- 自动验证：段落分页、孤行、页眉链接、分页几何及保存样例。
- 人工验收：改变页边距与方向，检查页码、页眉及缩放后分页。
- 完成证据：2026-10-18 第二切片进行中，未做人工验收，不标 done。已落地：显式 `w:br w:type="page"`、纸张大小／页边距／方向经命令写入当前节并进入屏幕与 PDF 共用的 PageScene、下一页分节的页框、页眉页脚三种 story（`titlePg` 在该节首页画 First；`evenAndOddHeaders` 按与 `PAGE` 相同、从 1 起的页码画偶数页 Even、奇数页 Default，首页仍计入奇偶但首页不同优先；省略的同类型引用继承前面最近一节的显式 story，显式空 story 不继承也不改用另一种）、页眉页脚内 `PAGE`／`NUMPAGES`（可带 `\* MERGEFORMAT`）。过高 keep 链记 `KeepTallerThanPage` 后继续，页码绘制改变页数时返回“布局未稳定”。自动测试：`pages`（分页符、页框、三种页眉页脚、继承与空 story 截断、增量与全量一致）、`default_header_and_footer_import_page_fields_without_cached_results`、`a_header_on_the_page_scene_is_the_pdf_page`、`first_and_later_headers_share_one_page_scene_with_pdf`、`page_setup_and_break_go_through_the_command_registry`。2026-10-18 第三切片继续落地，任务仍 `in_progress`。已覆盖：正文和页眉页脚的 `PAGE`／`NUMPAGES`／`SECTIONPAGES`；`fldSimple` 与 `fldChar` begin／instrText／separate／end 导入为一个 `PageField`，缓存不进入正文。排版按节的 `pgNumType` 格式化 `PAGE`，`NUMPAGES` 与 `SECTIONPAGES` 用十进制；不能格式化，或页码反馈不收敛且每处正文域都有缓存时，改画缓存并记 `PageFieldCached`。`fmt` 与 `start` 进入模型，奇偶页眉按显示页码。`w:type` 的 nextPage（省略即此）、continuous、evenPage、oddPage 参与分页：偶数或奇数节最多补一页空白，空白页属于前一节；`start` 与奇偶不一致时仍显示 `start` 并记 `PageNumberParity`。连续节在纸张、页边距和等宽栏一致时留在当前页并保持栏号，否则记 `ContinuousPageSplit` 并从下一页开始。等宽 `w:cols`（2..=45 栏、间距省略为 720 twip、无 `w:col`、`equalWidth` 不为假、`sep` 不为真）按内容宽度均分，整数余数留在末栏右侧，各行按栏宽拟合；栏分隔符推进下一栏，单栏时相当于分页。不等栏、`sep`、`nextColumn`、`chapStyle`／`chapSep` 整节保留，排版拒绝。多栏中的表拒绝。页码域与分页符、图片、编号或制表位同段时拒绝。编辑菜单 `page.number` 插入 `PAGE`，一次撤销（行为登记 `PAGE-NUMBER-INSERT-UNDO`，假设，没有 Word 观察）。未改的域段落导出复制原 XML；插入域后的导出沿用既有内联拒绝。全量与增量对正文域文档的页文本和注释一致。证据：`a_body_page_field_uses_the_displayed_number`、`an_unformattable_page_field_falls_back_to_its_cache`、`section_pages_count_pages_opened_by_that_section`、`next_even_odd_and_continuous_breaks_place_sections`、`equal_columns_and_a_column_break_share_one_page`、`a_table_in_newspaper_columns_is_refused`、`page_fields_import_without_treating_the_cache_as_body_text`、`section_break_page_numbers_and_equal_columns_are_imported`、`an_unchanged_field_package_keeps_its_markup_and_an_inserted_field_is_refused`、`inserting_a_page_number_is_one_undoable_transaction`。W09 重锚到 `aa2f97faacd447c43aa4987a095ed94e1c494e83`：固定矩阵 81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真，地址空间限制 `enforced=true`；这不是覆盖引导 fuzz。2026-10-19 第四切片继续落地，任务仍 `in_progress`。已覆盖：`widowControl`（ECMA-376 §17.3.1.26 `w:widowControl` 为真时，段内分页尽量保持至少两行同在页顶或页底；高于页框的段落仍拆分，行为与 layout_rules R5／R8 的 Word 负结果独立登记）；页面设置写回（`w:pgSz`／`w:pgMar`／`orient` 子集，节其余属性与 `sectPr` 保留片段不变时 patch `document.xml`，否则拒绝）；页眉／正文输入焦点（`page.edit.header`／`page.edit.footer`／`page.edit.body` 与布局功能区「页眉」「正文」按钮，经 `TextInput` 与引擎 `Position.story` 编辑并可撤销）。证据：`widow_control_keeps_two_lines_together_at_a_page_break`、`page_setup_margin_change_round_trips_in_export`、`page_setup_change_exports_through_the_docx_plan`、`header_story_edits_and_undo_through_the_engine_path`。仍未做：页眉／页脚部件 XML 写回、其他域开关、页码域与编号／制表位／图片同段、不等宽栏、`nextColumn`、章节页码、连续节若全程留在前一节打开的页上则 `SECTIONPAGES` 不计该页、状态栏仍是物理页序、图形界面验收。 2026-10-19 银河麒麟 GUI 部分验收通过：布局选项卡横向／纵向切换即时重排，插入分页符后状态栏正确变为「第 2 页，共 2 页」并滚动到新页，撤销恢复单页；页眉编辑与页面设置写回在 Linux GUI 未复验。截图 `tests/fixtures/reference/kylin/kylin-gui-page-break-2-pages.png`。
- 2026-10-19 页眉页脚部件写回（任务仍 `in_progress`）：编辑过的页眉／页脚 story 经 `ExportPlan` 写回对应的 `word/header*.xml`／`word/footer*.xml`；新的外部超链接关系 id 只追加到该部件已有的 `word/_rels/headerN.xml.rels`（或 footer），不新建关系部件，也不改写 `word/_rels/document.xml.rels` 的既有字节。未改的 story 部件仍按原字节复制。首页／偶数页与默认页眉各自写回被编辑的那一份；显式空 story 仍不继承默认页眉。无部件、跨部件、结构增删、页码域改写、共享部件或缺少关系部件时拒绝，不发明不支持的页眉标记。空的自关闭 `w:hdr` 只接受纯文本并写进同一部件。证据：`edited_header_and_footer_text_round_trips_and_keeps_relationships`、`a_body_edit_copies_an_unchanged_header_part`、`first_and_even_writeback_keeps_the_other_part_and_an_empty_story`、`an_empty_header_part_gains_plain_text_without_a_new_part`、`a_page_field_header_is_refused_instead_of_dropping_the_field`、`a_header_table_is_kept_when_the_paragraph_text_changes`、`header_link_text_keeps_the_relationship_and_a_new_target_is_appended`、`adding_a_header_paragraph_is_refused`。W09 harness 重锚到 `cbfa1d62a10f74cd0a9a73529ab0aed65ef2d24b`（该提交改 `crates/`；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。未做：图形界面验收、不等宽栏、`nextColumn`、章节页码、Word 对照。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。
- 2026-10-19 不等宽分栏（任务仍 `in_progress`）：`w:cols` 在 `equalWidth` 为假、`sep` 不为真、且 2..=45 个 `w:col` 各自带正的 dxa `w:w`（`w:space` 省略为 0）时进入 `UnequalColumns`。`sep` 为真、缺宽度、只有一栏或未知子元素的节保持不透明，导出仍含原来的 `w:cols`。排版按各栏宽度与栏后间距放置字形；`w:br w:type="column"` 推进一栏，最后一栏之后换页。有分栏符时后文按该栏宽度拟合。多栏节里的表仍拒绝。屏幕与 PDF 共用同一次 PageScene，文本矩阵与字形原点不超过 0.01 pt。不等宽栏规格进入段落缓存键，增量与全量页面一致。证据：`unequal_columns_place_glyphs_at_different_x_and_a_break_advances`、`a_table_in_unequal_columns_is_refused`、`unequal_columns_import_and_a_separator_or_missing_width_stays_preserved`、`unequal_columns_share_one_page_scene_with_pdf`。rebase 到 main `5704bf6226cde228d37e21034c9a8d84b51eb5f7`（T200 捕获组 squash）之后，W09 harness 重锚到 `675c9b82e3ee16dc225166d94953af1f7331906a`（该提交的 `crates/` 含不等宽栏的导入与排版，以及 T200 的正则替换捕获组；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。该树上固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `cd087577a67cf5efb9164de15cde166ba7da028e`（T201 首列、末列与纵向条纹 squash）之后，W09 harness 改锚到 `3ffb58dbf7e8a8987068ff61b425901dd749a42a`（该提交的 `crates/` 含不等宽栏的导入与排版，以及 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `a6fe0cf6cacd360f1e000aa26ee705a0804e5eb8`（T300 页眉页脚修订装饰 squash）之后，W09 harness 改锚到 `89e32568cb93d2c74f03fe72df5b73b89a3e8b79`（该提交的 `crates/` 含不等宽栏的导入与排版，以及 T300 的页眉页脚修订装饰和 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。未做：图形界面验收、栏分隔线、`nextColumn`、章节页码、Word 对照。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。
- 2026-10-20 文档核对：widowControl、页眉编辑、页面设置写回、页眉页脚部件写回和不等宽分栏已有自动测试。进度总览把 widowControl、页眉编辑和页面设置写回写成未做已经过时。图形界面验收、栏分隔线、`nextColumn`、章节页码和 Word 对照仍未做。状态保持 `in_progress`。见 [验收清单](acceptance/T202.md)。
- 2026-10-20 验证波次收口：S5 排练 `t202_margins_orientation_page_numbers_and_zoom` 覆盖页边距、方向、页眉页码、格式标记与缩放不改分页、另存重开。S1 有等宽栏、不等宽栏与分栏符、连续与偶数页分节、首页／偶数／默认页眉四组金样；S3 样例是 DOCX-FEAT-T202-PAGES。已知缺口已修复，有限制（[#67](https://github.com/willmove/joyoffice/pull/67)，main `caf84147`）：对已有正文段落执行 `page.number` 后，导出原先以 `paragraph formatting, source or inline objects changed` 拒绝；现在原段落没有行内对象、新增的是无格式 PAGE／NUMPAGES／SECTIONPAGES 域时写成 `w:fldSimple`，排练改为保留正文页码保存并在重开后断言。带格式的域、超链接里的域和原有行内对象的段落仍拒绝，复杂域仍不支持，见 [验证波次缺口修复](verification/verify-wave-fixes.md)。图形界面验收、栏分隔线、`nextColumn`、章节页码和 Word 对照仍未做。状态保持 `in_progress`。
- 2026-10-22 栏分隔线与 `nextColumn` 切片（[#84](https://github.com/willmove/joyoffice/pull/84)，任务仍 `in_progress`）：`w:cols/@w:sep`（ECMA-376 §17.6.4）从 not-implemented 变为可用。模型侧 `Section` 新增 `column_separator: bool`（`Document::new` 里默认 false）；导入侧 `read_columns` 用既有 `props::on_off` 类型化解析 `sep`，等宽（`w:num`）与不等宽（`w:col` 子元素、`equalWidth="0"`）两种形态都认，非法值（如 `sep="maybe"`）仍照原样落进 preserved／opaque 通道。排版侧新增 `paint_column_separators`：每个栏间空隙画一条竖直 `SceneRect`，1 pt（`Emu::PER_POINT`）黑色细线，位置取前一栏右边缘与后一栏左边缘的中点（等宽与不等宽都成立），纵向从页面正文框的 `geometry.top` 到 `geometry.bottom_edge()`；不足两栏的节什么都不画。规范只记录 `sep` 打开，既不定线宽也不定精确位置，所以"1 pt 细线、空隙居中"是 JoyWord 假定，不是 Word 观察，代码注释里同样写明。该通道挂在 `finish_stories` 的第一行，那是全量、混合与增量排版的唯一汇合点，所以增量与全量按构造一致，也不存在第二套几何；屏幕、PDF（`joyword-pdf` 遍历 `scene.rects`）与 CPU 光栅（`joyword-render`）消费同一批矩形，这三个 crate 一行未改。缓存上 `column_separator` 折进 `LayoutKey.content`（SHA-256）的节指纹，标志变化时快照缓存失效；它有意不进入段落拟合缓存键 `column_mix`，因为分隔线不改变行的拟合。导出往返是免费的：`rewrite_sectpr_page` 除 `pgSz`／`pgMar` 外逐字节拷贝 sectPr 子元素，`w:sep` 自然存活；再导入的相等校验由 `export/compare.rs` 的 `same_section_envelope` 承担，该函数增加了一个 `column_separator` 比较项。`w:type w:val="nextColumn"`（§17.6.22 `ST_SectionMark`，含义是"本节从当前页的下一栏开始"，不是域也不是 `w:br`）新增 `SectionBreak::NextColumn`：导入在 body.rs 的 `w:type` 分支识别，排版新增 `SectionEdge::NextColumn`——纸张框与分栏几何不变且当页已有墨迹时经 `advance_column` 在当前页下一栏续排（`advance_column` 在末栏之后本就换页，单栏流因此自然退化为新起一页），页码连续，该页仍归打开它的那一节（页眉页脚与分隔线不变）；纸张或分栏不同、或当页为空时退化为连续式的页拆分并发出既有的 `LayoutNote::ContinuousPageSplit`（复用既有注记，避免改动 `apps/joyword-cli/tests/golden_scenes.rs` 里的穷尽匹配）。新增 12 个测试，全部在新文件里：`crates/joyword-layout/tests/columns.rs` 六项——`equal_column_separator_paints_one_centered_rule`、`unequal_column_separator_paints_a_rule_per_gap`、`separator_off_paints_no_rule`、`incremental_layout_matches_full_with_separators`、`next_column_section_break_advances_one_column`、`next_column_with_different_columns_splits_the_page`；`crates/joyword-docx/tests/column_separator.rs` 五项——`equal_column_separator_imports_typed_and_round_trips`、`unequal_column_separator_imports_typed`、`separator_false_or_omitted_is_not_flagged`、`invalid_separator_value_stays_preserved`、`next_column_section_break_imports_typed_and_round_trips`；`crates/joyword-pdf/tests/column_separator.rs` 一项——`the_column_separator_rule_is_in_the_pdf_page_scene`（分隔矩形出现在交给 `joyword_pdf::render` 的场景里，且渲染字节与关闭分隔线的同一文档不同，直接证明细线经共用 PageScene 到达 PDF）。已知限制如实记录：连续节或 `nextColumn` 节与打开该页的节共页时，页眉页脚与分隔线都跟随打开该页的节（与既有 Continuous 行为一致），所以在页中途才打开 `sep` 的节不会在已经打开的那一页上画线；归给前一节的奇偶补白页也带前一节的分隔线；分隔矩形是在组装之后追加的，增量的 `suffix_copies` 快路径要校验 `page.rects.len()` 相等，因此带分隔线的文档失去后缀复制复用、退回完整放置（结果正确，略慢），增量输出仍等于全量输出；分隔线纵向取整个正文框高度，不按最高那一栏的文字裁切，规范未写明，属 JoyWord 假定。`chapStyle`／`chapSep` 章节页码本切片明确未做：章节号需要一个与标题编号互锁的章号来源，而编号写回属 T200、当时正在并行分支上进行，现在实现只能发明未经测试的互锁，因此二者仍留在 preserved／opaque 通道不变。附带一处名称过时：`crates/joyword-docx/tests/import.rs` 的 `unequal_columns_import_and_a_separator_or_missing_width_stays_preserved` 里两个 `w:sep` 用例已移进 `column_separator.rs` 成为更强的正向断言，该测试现在只覆盖不等宽栏的类型化导入、等宽栏与三种仍在子集外的形态（缺宽度、`equalWidth="0"` 而无 `w:col`、只有一个 `w:col`），名字里的"a separator … stays preserved"已不描述它覆盖的内容；改名要动 `crates/`，超出本次文档范围，只在 [验收清单](acceptance/T202.md) 里更正引用。切片内自动验证通过：`cargo test --locked`（CI 口径）、`cargo test --locked -p joyword`、`cargo test --locked -p joyword-layout -p joyword-pdf -p joyword-render`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all -- --check`、`check_docs.py`、`check_gates.py`（含反例）、`check_acceptance.py`；`python -m unittest discover -s scripts/tests` 有 1 项环境性失败（`test_committed_bytes_and_manifest_hashes_match`，本机 Python 3.14.7 的 zlib-ng 1.3.1 与 CI 的 stock zlib 在级别 6 deflate 字节上不同，5 个 DOCX-FEAT 夹具哈希因此不一致，解压后的成员与 CRC 相同；在干净 main 上同样失败，与 `core.autocrlf` 无关）。人工验收全部未做：图形界面里看分隔线与 `nextColumn` 续排、Word／WPS 对照（含线宽与位置的假定核对）、信创真机、打印，均未执行。T202 保持 `in_progress`。

- 2026-10-23 章节 PAGE 切片（[#91](https://github.com/willmove/joyoffice/pull/91)，任务仍 `in_progress`）：`w:pgNumType` 的 `chapStyle`（ECMA-376 §17.6.12，1..=9 的内置标题级别）与 `chapSep`（ECMA-376 `ST_ChapterSep`：hyphen／period／colon／emDash／enDash，其中 hyphen 是 U+2011 非断行连字符）类型化进模型；MS-OI29500 §17.6.12 明确「标准未规定 chapSep 默认值，Word 用 hyphen」，本实现取该默认并忽略没有 chapStyle 的 chapSep。章节前缀只作用于 PAGE 域，NUMPAGES／SECTIONPAGES 不加。章节号取实际渲染出的编号标签原样（含多级与字面格式，不含其制表与空格后缀），不从正文文字里猜数字；只用规范的内置标题样式名（`heading 1` 到 `heading 9`），不实现 ECMA 的上一级回退。选章规则是已声明的 JoyWord 假定：本物理页首个匹配标题用于整页，否则取前方最后一个同级标题；Word 按域所在位置取当时生效的章节，两者可能不同，因此本切片不声称 Word／WPS 视觉一致。跨物理页的 PAGE 段落明确拒绝（`chapter PAGE paragraph spans pages`），不发布过期前缀。章节前缀进入段落缓存键与字段反馈，宽度变化会反馈进分页，全量与增量布局一致。非法 `pgNumType` 仍整节保留、无关修改时原包逐字节不变、直接改模型章节设置在导出时拒绝。新增 12 项测试：`crates/joyword-layout/tests/pages.rs` 八项、`crates/joyword-docx/tests/column_separator.rs` 两项、`crates/joyword-layout/src/cache.rs` 两项。独立审查在独占 target 上复跑 model／docx／layout 共 426 项与工作区 918 项测试、全目标 clippy（-D warnings）与 fmt，无阻塞发现。人工验收与 Word／WPS 对照未做；多级编号 restart 与章节标签的交互沿用既有编号存储，本切片未新增测试。
### T203：图片与富文本剪贴板

- 输入：系统剪贴板多种 MIME／平台格式、图片资源及用户粘贴策略。
- [ ] 状态：`in_progress`
- 需求：`LAYOUT-006`、`CLIP-001`、`CLIP-002`、`CLIP-003`、`EDIT-003`、`EDIT-010`
- 前置：`T101`、`T200`
- 产物：行内图片、超链接插入编辑与确认后打开、拖放、纯文本／HTML／RTF／内部片段及粘贴策略；富剪贴板格式由 platform 层实现（GPUI 仅提供文本与图片）。
- 错误路径：畸形富文本、远程资源、过大图片和 ID 冲突不执行活动内容。
- 自动验证：清理、资源去重、样式编号映射、单次撤销与剪切失败原子性。
- 人工验收：Word／WPS／浏览器双向复制列表、表格、图片，比较粘贴选项。
- 完成证据：2026-10-18 第一切片与第二切片已落地，任务保持 `in_progress`。第一切片已覆盖：`w:drawing`／`wp:inline` 的单个内部光栅 blip（以及 `w:pict` 的 `v:imagedata`）在字节、边长和像素预算内成为图片资源；`r:link`、外部目标和 OLE／SVG／EMF／WMF 不取回、不执行，诊断不回显 URL；超宽 PNG 在解码像素前拒绝并锁定段落。编辑插入按内容哈希与媒体类型去重（导入仍按部件保持独立）。`InsertImage` 与 `edit.insert_image` 是一次可撤销事务；系统剪贴板优先接纳图片字节，图片全部被拒时不回退成文字，文件路径不打开。行内图片进入屏幕与 PDF 共用的 `PageScene`，PDF 写出图像 XObject，CPU 光栅绘制同一矩形。未改动的图片段落保持原 XML；新增资源或改写图片段落显式拒绝，不改写媒体部件。第二切片已覆盖：简单 `w:hyperlink`（仅 `r:id` 与／或 `w:anchor`，子节点为文字或制表符）成为 run 上的链接；外部目标只存储、不取回；`javascript`、`data`、`file` 可以导入但 Ctrl+点击（macOS 上还有 Command+点击）不打开；允许的方案只有 `http`、`https`、`mailto`。`SetHyperlink` 设置、修改或清除链接，每次一次撤销。导出在链接签名不变时保留包装；链接改变时改写段落，并只在写出新 id 时向 `word/_rels/document.xml.rels` 追加关系；写不出的链接显式拒绝，不静默丢掉。工具提示等复杂超链接仍保持并锁定。HTML／RTF 粘贴只留下粗体、斜体、下划线、段落和有依据的列表，剥掉脚本、样式、对象、图片和远程资源；超限拒绝，过深退回纯文本。图片后的文字在剩余宽度够时留在同一行。插入图片按钮弹出文件框后走已有 `edit.insert_image`。选中恰好一张图片时，复制写入 GPUI 图片剪贴板。行为登记在图片三条之外增加 `LINK-EDIT-UNDO`、`LINK-OPEN-ALLOWLIST`、`RICH-PASTE-STRIP`（来源是假设，没有 Word 观察）。证据另见 `an_external_hyperlink_imports_without_being_fetched`、`javascript_and_file_targets_import_and_are_not_openable`、`an_anchor_imports_and_is_not_a_url`、`a_tooltip_stays_preserved_and_locks_the_paragraph`、`editing_link_text_keeps_the_target_and_a_new_target_is_appended`、`removing_a_hyperlink_does_not_leave_the_wrapper`、`min_004_records_external_targets_without_accessing_them`、`set_edit_and_remove_a_hyperlink_are_separate_undo_steps`、`openable_link_refuses_schemes_outside_the_allowlist`、`html_keeps_bold_italic_underline_and_drops_active_content`、`rtf_keeps_formatting_and_drops_objects_pictures_and_field_instructions`、`oversize_markup_is_refused_and_deep_markup_falls_back_to_plain_text`、`pasted_html_becomes_formatted_runs_and_one_undo`、`a_grounded_list_paste_uses_the_existing_numbering_instance`、`text_after_an_inline_image_stays_on_the_same_line`、`a_hyperlink_is_an_underline_in_the_page_scene`、`a_selected_image_exposes_its_bytes_for_the_clipboard`。未覆盖：拖放、浮动 `wp:anchor`、页眉页脚图片绘制、表格内超链接与图片的产品验收、富文本剪贴板的平台专有格式（GPUI 只提供文本与图片，文本里的 HTML／RTF 靠嗅探）、以及 Word／WPS／浏览器双向粘贴的人工对照。没有启动图形界面，因此没有实际拉起浏览器。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。 2026-10-19 静态产物核对发现 DOCX-MIN-004 的 CPU 链接下划线在 96 DPI 消失；矩形改用覆盖率抗锯齿与专用矩形光栅器，新增五种亚像素位置回归。未新增剪贴板／拖放或 GUI 人工验收，见 [验证审计](VALIDATION-20261019.md)。2026-10-19 第三切片已落地，任务保持 `in_progress`：表格单元格内的简单超链接端到端打通——导入接受与正文同一子集的 `w:hyperlink`（复杂包装仍整表保留只读）；改写过的表按 run 写出 `w:hyperlink`（一个模型 run 一个 `w:r`，相邻不合并；关系复用或追加）；引擎 `SetHyperlink` 在单元格选区设置／修改／清除、每步一次撤销；命中测试与 Ctrl+点击打开、页面场景 0x0563C1 下划线均覆盖单元格。本地图片文件可拖入文档：落点经 `Window::mouse_position()` 设为插入点后按路径顺序插入，复用 `insert_image_file` 的全部检查（普通文件、32 MiB 字节上限、字节嗅探拒绝 URL／UNC 形态与活动内容），被拒路径跳过、每张图片各自一次撤销、全部被拒时报第一个拒绝原因；固定 GPUI 0.3.9 仅 Windows／Wayland 携带拖放路径，X11 后端不发路径、处理器不触发，如实记录。浮动 `wp:anchor` 排版仍未做（layout 无锚点／绕排概念，本切片不新建该子系统）。证据：`a_table_cell_hyperlink_round_trips_after_a_cell_edit`、`a_new_hyperlink_target_in_a_cell_appends_a_relationship`、`an_anchor_only_cell_hyperlink_round_trips`、`a_complex_hyperlink_keeps_the_table_opaque`、`a_hyperlink_inside_a_cell_is_set_edited_and_cleared_with_undo`、`a_hyperlink_inside_a_table_cell_is_set_edited_removed_and_openable`、`a_hyperlink_in_a_table_cell_is_an_underline_in_the_page_scene`、`dropped_image_paths_insert_in_order_and_skip_refused_files`；行为登记新增 `LINK-CELL-EDIT-UNDO`、`IMAGE-DROP-LOCAL`（假设，无 Word 观察）。未覆盖：浮动图、页眉页脚图片、拖放与表格内图片的 GUI 人工验收、Word／WPS／浏览器双向粘贴对照、平台专有剪贴板格式。 2026-10-19 第四切片已落地，任务保持 `in_progress`。简单浮动 `wp:anchor` 在既有图片预算内成为页面绝对坐标：`behindDoc` 缺省或为假，直接子元素是 `wp:wrapNone`，且没有 `wp:wrapSquare`、`wp:wrapTight`、`wp:wrapThrough`、`wp:wrapTopAndBottom`；位置要么是 `simplePos` 为真时的 `wp:simplePos` x／y，要么是 `positionH` 与 `positionV` 的 `relativeFrom="page"` 且只有非负 `wp:posOffset`（不超过 100 英寸）。该矩形进入屏幕与 PDF 共用的 PageScene。负偏移、超界、`behindDoc`、非页面相对、`wp:align` 以及上述绕排保持原 XML 并锁定段落，诊断为 `Unsupported`，不把字节放进模型资源。`r:link`、OLE、SVG、EMF、WMF 仍不取回；图片诊断的关系标识不回显 URL。页眉与页脚 story 里已导入的行内图片画在对应条带上，并进入该页 PDF。超大与链接 blip 仍拒绝并锁定。证据：`a_page_anchor_imports_and_wrap_or_remote_targets_stay_preserved`、`an_anchor_refuses_svg_emf_wmf_and_ole_without_fetching`、`a_header_and_footer_inline_image_imports_with_the_part`、`a_page_anchor_is_a_positioned_rectangle_shared_with_the_cpu`、`a_header_and_footer_image_paint_on_their_bands`、`a_floating_anchor_and_a_header_image_share_pdf_and_cpu_geometry`。行为登记新增 `IMAGE-ANCHOR-PAGE`、`IMAGE-STORY-PAINT`（假设，没有 Word 观察）。仍未覆盖：`wrapSquare`、`wrapTight`、`wrapThrough`、`wrapTopAndBottom`、`behindDoc`、非 `page` 的 `relativeFrom`、`wp:align`、负偏移、表格内图片、拖放与页眉页脚图片的 GUI 人工验收、Word／WPS／浏览器双向粘贴对照、平台专有剪贴板格式。没有启动图形界面。rebase 到 main `30a76d8583e2ca130b3a0f8ed2b9a6b3f51aab88`（T202 squash）之后，W09 harness 改锚到 `6ecf8139ec7db904f2c38621da01dd1faa18b6be`（该提交的 `crates/` 含页面绝对 wrapNone 浮动图与页眉页脚图片绘制，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。先前锚点 `12884fa55b989852160ec31a598cba64f726c108` 上的固定矩阵为 81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。固定矩阵尚未在新锚点上重跑。这仍不是覆盖引导 fuzz。独立锁无新包。再 rebase 到 main `26fd7ce2a87440395dddf94cb19c2569c6186669`（T200 find-bar squash，未改 `crates/`）之后，W09 harness 改锚到 `e6a146b43d244a80be84d83412d7af18315ffacb`（该提交的 `crates/` 含页面绝对 wrapNone 浮动图与页眉页脚图片绘制，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `02a49a41e800004949857b1065cb33661b6eee3d`（T300 squash）之后，W09 harness 改锚到 `ceda23592802ca91f76170be0680a0ce62468dc2`（该提交的 `crates/` 含页面绝对 wrapNone 浮动图与页眉页脚图片绘制，以及 T300 的成对移动与 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。再 rebase 到 main `e51cdb86de5ef6cea54d2010b20b7dadfe97a3b9`（T201 nested-tables squash）之后，W09 harness 改锚到 `a78b79af422933491b4df10f845845633aba108b`（该提交的 `crates/` 含页面绝对 wrapNone 浮动图与页眉页脚图片绘制，以及 T201 的一层嵌套表、T300 的成对移动与 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。 2026-10-19 第五切片已落地，任务保持 `in_progress`。页面相对、非负偏移、位于文字前面的 `wp:wrapSquare` 在既有图片预算内成为页面坐标图片。距离先取 `wp:wrapSquare` 的 distT／distB／distL／distR，缺省再取锚点同名属性，再缺省为 0（不是 Word 的 0.125 英寸）。`wrapText` 省略、`bothSides` 或 `largest` 记为较宽一侧，宽度相同用左侧；`left`／`right` 只用该侧，文字不拆到方形两侧。正文行在放置时避开矩形加距离后的方形；`wrapNone` 仍不占行宽，文字可以穿过矩形。屏幕、PDF 与 CPU 共用同一矩形。`wrapTight`、`wrapThrough`、`wrapTopAndBottom`、`behindDoc`、非 `page` 的 `relativeFrom`、`wp:align`、负偏移、负距离、未知 `wrapText`、绕排子元素、非零 `wp:effectExtent`，以及同时出现的 `wrapSquare` 与 `wrapNone`，保持原 XML 并锁定段落。`r:link` 不取回，图片诊断不回显 URL。证据：`a_wrap_square_page_anchor_imports_and_other_wraps_stay_preserved`、`a_wrap_square_keeps_text_outside_the_rectangle_and_wrap_none_does_not`、`a_wrap_square_rectangle_is_shared_by_pdf_and_the_cpu`。行为登记新增 `IMAGE-WRAP-SQUARE`、`IMAGE-WRAP-SQUARE-PLACE`（假设，没有 Word 观察）。仍未覆盖：两侧同时排字、锚点段之前已经放下的行、表格单元格、页眉页脚方形参与正文绕排、非零 effectExtent 的放置、负偏移、非页面相对、GUI 人工验收、Word／WPS 观察。没有启动图形界面。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。 rebase 到 main `5704bf6226cde228d37e21034c9a8d84b51eb5f7`（T200 捕获组 squash）之后，W09 harness 改锚到 `edd6480d4805af517c056d21ad14a66a327ec807`（该提交的 `crates/` 含页面相对 wrapSquare，以及 T200 的正则替换捕获组；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。该树上固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `cd087577a67cf5efb9164de15cde166ba7da028e`（T201 首列、末列与纵向条纹 squash）之后，W09 harness 改锚到 `bb6454922d904ac5bfa026477d5c0ac66bbc4b8e`（该提交的 `crates/` 含页面相对 wrapSquare，以及 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `a6fe0cf6cacd360f1e000aa26ee705a0804e5eb8`（T300 页眉页脚修订装饰 squash）之后，W09 harness 改锚到 `8309c9e22fdb57fe869fefe82b61eb7fb4f21165`（该提交的 `crates/` 含页面相对 wrapSquare，以及 T300 的页眉页脚修订装饰和 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `5310fabb612bb3a9495fc5b123b77a8f3346266d`（T202 不等宽分栏 squash）之后，W09 harness 改锚到 `9026d230e7e4d34349c3c4f5567f231b9f4d4c52`（该提交的 `crates/` 含页面相对 wrapSquare，以及 T202 的不等宽分栏、T300 的页眉页脚修订装饰和 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。
- 2026-10-20 文档核对：页面相对 wrapSquare 已有自动测试。进度总览里「绕排与人工对照未做」的绕排已经过时。wrapTight、wrapThrough、wrapTopAndBottom、衬于文字下方、非页面相对位置，以及 Word／WPS／浏览器双向粘贴仍未做。状态保持 `in_progress`。见 [验收清单](acceptance/T203.md)。
- 2026-10-20 验证波次收口：S1 金样 `t203-wrap-none` 与 `t203-wrap-square` 锁定页面绝对 wrapNone 与页面相对 wrapSquare 的场景、PDF 和光栅。S3 样例 DOCX-FEAT-T203-IMAGES 当时确认 wrapTight 原样保留并锁定段落，编辑导出返回 `Restricted`，未编辑包字节不变。该缺口已修复，有限制（[#67](https://github.com/willmove/joyoffice/pull/67)，main `caf84147`）：页面相对的 `wrapTight`／`wrapThrough` 按锚点范围近似为方形绕排，样例现在可排版、渲染并接受文字修补，drawing 原绕排 XML 照原样写回；`wp:wrapPolygon` 不跟随，这不是真正的紧密型或穿越型轮廓绕排，见 [验证波次缺口修复](verification/verify-wave-fixes.md)。图片关系另报 `not-imported` 已修复（[#69](https://github.com/willmove/joyoffice/pull/69)）：四张图仍是图片资源，这四条关系不再另报；未消费的图片关系和未引用的自定义部件仍报告。真正的轮廓绕排、wrapTopAndBottom、衬于文字下方、非页面相对位置和 Word／WPS／浏览器粘贴仍未做。状态保持 `in_progress`。
- 2026-10-21 上下型绕排切片已落地，任务保持 `in_progress`。页面相对、非负偏移、位于文字前面的 `wp:wrapTopAndBottom` 在既有图片预算内成为页面坐标图片。距离只取 `distT`／`distB`（先 wrap 元素，缺省再取锚点，再缺省为 0，不是 Word 的 0.125 英寸）。`distL`／`distR` 不打开侧向空隙。与图片带垂直相交的正文行整行移到带下方，并回到当前栏框左缘；带上方的行留在原处。没有字形的锚点行不因此下移，也不把文字排到图片两侧。`wrapSquare` 仍把字形留在矩形外的一侧。屏幕、PDF 与 CPU 共用同一矩形。原绕排 XML 在未改写该段时照原样写回。`behindDoc`、非 `page` 的 `relativeFrom`、`wp:align`、负偏移、负距离、`wrapText`、绕排子元素、非零 `wp:effectExtent`、同时出现的多种绕排，以及远程链接，保持原 XML 并锁定段落。证据：`a_wrap_top_and_bottom_page_anchor_imports_and_its_markup_survives_an_edit`、`a_wrap_top_and_bottom_pushes_overlapping_lines_below_and_square_stays_beside`、`a_wrap_top_and_bottom_rectangle_is_shared_by_pdf_and_the_cpu`。行为登记新增 `IMAGE-WRAP-TOP-BOTTOM`、`IMAGE-WRAP-TOP-BOTTOM-PLACE`（假设，没有 Word 观察）。`IMAGE-ANCHOR-PAGE` 不再把 wrapTopAndBottom 记为拒绝。仍未覆盖：真正的轮廓绕排、衬于文字下方、非页面相对位置、锚点段之前已经放下的行、表格单元格、页眉页脚上下型参与正文绕排、两侧同时排字、Word／WPS／浏览器粘贴。没有把 T203 标成 done。见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T204：统一 PDF 与平台打印

- 输入：稳定 PageScene、字体嵌入许可、链接书签及平台打印设置。
- [ ] 状态：`in_progress`
- 需求：`LAYOUT-009`、`FORMAT-008`、`PLATFORM-007`
- 前置：`T003`、`T201`、`T202`
- 产物：PageScene 完整页面输出、可选文本 PDF、平台打印层（GPUI 无打印接口）与预览，以及 PageScene 的 CPU 光栅后端。
- 错误路径：布局未稳定、字体不可嵌入、打印取消或设备失败不报告成功。
- 自动验证：页面几何一致、PDF 文本／链接、纸型和打印范围测试。
- 人工验收：两个独立阅读器检查 PDF；真实打印或平台打印到文件。
- 完成证据：2026-10-19 第一切片已落地，任务保持 `in_progress`。文件菜单 `file.export_pdf` 与 `file.print` 使用屏幕已经安装的同一 `PageScene`（页范围只切片，不再分页）；CLI `convert` 在自己的一次分页之后调用同一个 `render_export`。Producer 固定为 JoyWord；标题来自 core props 的 `dc:title`；不写 CreationDate／ModDate，也不开 XMP。子集字体带 ToUnicode，`pdftotext -enc UTF-8` 在有该程序时抽出文字（未设置 `JOYWORD_REQUIRE_TEST_FONT` 且程序缺失时跳过，不把跳过说成已抽出）。分节纸型成为各页 MediaBox；场景图片成为图像 XObject；书签只来自内置标题样式 1–9（含 `basedOn` 链），普通段的 `outlineLvl` 不成书签。落盘是写入、sync、回读、硬链接，不覆盖已有文件。无头打印到文件在 CI 可测；份数只记在收据上，文件仍是一份 PDF。审阅标记 `markup`／`original` 拒绝。macOS／Windows／Linux 的系统打印对话框按 cfg 编译并返回未验证，不调用系统 API。结构由 joyword-pdf 内与写出器不共享对象的读取器核对（ISO 32000-1 经典交叉引用或交叉引用流；FlateDecode 复用已钉扎的 flate2 1.1.10，没有新外部包版本）。证据：`two_renders_match_and_carry_no_clock`、`heading_styles_become_bookmarks_and_a_bare_outline_level_does_not`、`each_section_keeps_its_page_box`、`an_image_is_an_xobject`、`a_classic_cross_reference_file_reports_structure_without_the_writer`、`an_https_link_is_a_uri_annotation_and_a_script_is_not`、`pdftotext_reads_the_scene_text`、`publish_verifies_bytes_and_refuses_an_existing_path`、`print_to_file_records_copies_once`、`the_native_dialog_is_not_verified_and_creates_no_file`。未做：两个独立阅读器的人工检查、真实打印机、系统打印对话框、结构标签、仅书签锚点的文档内跳转、审阅标记打印，以及把不可打印区域做成用户可见选择。CPU 光栅与打印预览是既有路径，本切片不另称完成。没有启动图形界面。见 [IMPLEMENTATION](IMPLEMENTATION.md)。 2026-10-19 银河麒麟补充：`joyword-cli convert`（Noto Serif CJK face 2）对 JoyWord 导出的编辑副本出 1 页 PDF，poppler `pdftotext -enc UTF-8` 与 pdfminer.six 两个独立读取器逐行读回一致（5 行正文），结构核对内容流／ToUnicode／Producer=JoyWord，产物 `tests/fixtures/reference/kylin/joyword_e2e_cli.pdf`。图形入口在本机不可达：X11 无菜单栏，且 UKUI 桌面把 Ctrl+Shift+E 全局分配给文本编辑器（按键被系统抢占，XTEST 实测复现），Ctrl+Shift+S 亦无反应——文件菜单导出 PDF 与另存副本的快捷键路径在该默认桌面配置下不可用，见 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json)。系统打印对话框与真实打印机仍未验证。 2026-10-19 独立 PDF 验证发现并修复合成粗体文字重复：偏移副本改画轮廓，仅第一次绘制提供可检索文字；新增 Poppler raw 读回单次文字回归。五个 DOCX-MIN 样例经 Poppler、pdfplumber、pypdf 核对文字；这是自动读回，两个 GUI 阅读器与系统打印人工验收仍未做，见 [验证审计](VALIDATION-20261019.md)。 2026-10-19 Windows 对五个 PDF 完成 Poppler、pdfplumber、pypdf 读回和 Poppler 光栅对照；本轮后续 GUI Computer Use 管道中断，未完成两个 GUI 阅读器与系统打印对话框检查，T204 保持 `in_progress`。2026-10-19 统信 UOS Desktop 20 Professional 完成两个独立 GUI 阅读器人工检查：`joyword-cli convert`（Noto Serif CJK face 2）对 DOCX-SAMPLE-001 出 1 页可检索 PDF，Poppler `pdftotext -raw -enc UTF-8` 五段逐字一致；qpdfview 0.4.17 显示该页并在搜索框高亮「远山」；WPS PDF 12.1.2.1128 显示同一页并报告「精确查找 (1)」。系统打印对话框与真实打印机仍未验证，T204 保持 `in_progress`，证据见 [UOS 报告](../../tests/fixtures/reference/uos/uos_report.json)。2026-10-19 Linux 入口：`file.export_pdf` 主快捷键仍是 `cmd-shift-e`／`ctrl-shift-e`；Linux 额外注册 `ctrl-e`，避免 UKUI 占用 Ctrl+Shift+E，也不改 Windows。文件条上的导出 PDF 与窗口内路径栏不依赖该和弦。证据 `export_pdf_keeps_desktop_chords_and_adds_a_linux_alternate`、`linux_export_keeps_the_primary_chord_and_adds_ctrl_e`。Fcitx 仍未接入（GPUI X11 无 IME）。
- 2026-10-20 文档核对：页面几何、PDF 文本与链接、打印范围和失败路径已有自动测试。UOS 报告记录了 qpdfview 与 WPS PDF，不关闭两个独立阅读器的人工验收。真实打印和系统打印对话框仍未做。状态保持 `in_progress`。见 [验收清单](acceptance/T204.md)。
- 2026-10-20 验证波次收口：S1 金样 `t204-paper-and-link` 锁定两种纸型的页盒（与场景差不超过 0.01 pt）和唯一的 https 链接；CI 作业 unified-pdf 另用 `scripts/verify_feature_pdf.py` 以 pdftotext 回读特性场景 PDF。双阅读器人工检查、真实打印和系统打印对话框仍未做。状态保持 `in_progress`。

- 2026-10-23 PDF 书签内部跳转切片（[#92](https://github.com/willmove/joyoffice/pull/92)，任务仍 `in_progress`）：模型书签解析为 PDF 命名目标，锚点内链写成真实 `/S /GoTo` 动作与显式 `/D` 引用，不用命名跳转。名称精确匹配且区分大小写；空名、控制字符名、重名、脱离锚点、未布局位置、过期布局、非法几何与超预算都拒绝，名称永不被当作 URL。坐标按 ISO 32000-1 §12.3.2、§12.5.6.5、§12.6.4.2 与 krilla 0.8.2、pdf-writer 0.15.0 的锁定源码核对。应用侧导出复用屏幕已安装的同一批页面场景：选页导出时书签与链接的源页、目标页一起重编号，源或目标不在选页内的链接被丢弃而不误跳。名称树键按 ISO 32000-1 §7.9.6 逐字节比较，检查器因此保留原始八位组并按 UTF-8 还原，修复了中文名读回乱码。锚点目标不在本文档书签中时该链接被跳过并计数，不阻断整份导出；这是因为 DOCX 的书签与其锚点链接分处不同部件且只导入链接。独立审查在独占 target 上复跑 `joyword-pdf` 全套并用独立探针核对歧义判定、UTF-8 名称预算、缺失与 cluster 内部端点拒绝、跨页源矩形、边界偏置、同布局目标 caret 与非法字形不产生部分 PDF 字节。未做：外部 PDF 结构读取器复核（本机无 pypdf／pikepdf／qpdf）、DOCX 书签导入、PDF 结构标签、图形界面与 Word／WPS 对照。
- 2026-10-23 DOCX 书签导入切片（[#95](https://github.com/willmove/joyoffice/pull/95)，任务仍 `in_progress`，进度计数不变）：正文里成对的 `w:bookmarkStart`／`w:bookmarkEnd`（ECMA-376 §17.13.6.1／§17.13.6.2）导入为模型书签，名称原样保留并区分大小写，范围用引擎书签的起点 `Bias::After`、终点 `Bias::Before`，供既有 PDF 命名目标与 `/S /GoTo` 解析。`displacedByCustomXml` 不改变 XML 位置。空名或仅空白、控制字符、超过 2048 字节的名称、重名（ECMA 保留先出现的那个）、缺端点、顺序颠倒、跨故事、列书签（`colFirst`／`colLast`）、不透明表格里的书签、页眉页脚与批注故事里的书签、未知属性或子内容，以及超过 512 个已接受书签，都只记警告并且不插入，避免一个坏书签让整份 PDF 导出失败。标记字节仍作为片段保留并锁定所在段落；未改动的导出仍是原包逐字节复制。改名、改锚点或编辑其他段落都会拒绝写出，以免丢掉标记。本切片不写回书签。内置标题样式 1–9 的大纲书签路径不变，裸 `outlineLvl` 仍不成书签。锚点对不上已导入书签时仍跳过并计数。证据：`a_body_bookmark_keeps_its_name_and_range`、`bookmark_names_keep_case_and_reject_empty_duplicates_and_overlong_values`、`detached_column_and_opaque_table_bookmarks_are_not_destinations`、`the_pdf_bookmark_budget_keeps_the_first_names`、`an_imported_bookmark_is_a_pdf_goto_named_destination`。当时拒绝丢掉标记的测试已由 2026-10-24 写回切片取代。未做：页眉页脚／批注／不透明表格／列书签成为 PDF 目标、外部 PDF 结构读取器复核、PDF 结构标签、图形界面、Word／WPS 对照、真实打印与系统打印对话框。
- 2026-10-24 DOCX 书签写回切片（任务仍 `in_progress`，进度计数不变）：#95 已经导入的正文成对书签可以写回。未改动的包仍逐字节复制。宿主段落和锚点都没变时标记字节留在原处；段落被编辑或锚点仍是可表示的正文范围时，按模型偏移拼回原来的 `w:bookmarkStart`／`w:bookmarkEnd` 字节（保留 `w:id`、名称和 `displacedByCustomXml`），重导入得到同样的名称和范围。改名、脱离、颠倒、非正文锚点、新造书签都拒绝，不静默丢掉。列书签和页眉／批注里的书签继续锁定所在段落，编辑导出为 `Restricted`。已接受的正文书签不再锁定段落。证据：`an_edited_export_writes_body_bookmarks_back`、`bookmark_write_back_refuses_unrepresentable_shapes`、`header_and_comment_bookmarks_still_refuse_an_edit`。未做：页眉页脚／批注／不透明表格／列书签成为 PDF 目标、外部 PDF 结构读取器复核、PDF 结构标签、图形界面、Word／WPS 对照、真实打印与系统打印对话框。
- 2026-10-24 页眉页脚与批注书签目标切片（任务仍 `in_progress`，进度计数不变，仍是 12 done／22 in_progress／32 planned／0 blocked）：页眉、页脚和可定位批注里成对的 `w:bookmarkStart`／`w:bookmarkEnd` 按 #95 的正文规则导入为模型书签。名称原样、区分大小写；空名或仅空白、控制字符、超过 2048 字节、重名（先出现的保留，跨故事也算）、缺端点、顺序颠倒、跨部件拆开的同一 `w:id`、列书签、不透明表格，以及已接受满 512 个，都只记警告并且不插入。`w:id` 在每个部件里配对，正文、页眉、页脚和批注可以共用同一个数字。游离批注没有页面插入点，书签记 `BookmarkAnchorInvalid` 而不插入，避免整份 PDF 导出失败。脚注、尾注和文本框仍是 `BookmarkOutsideBody`。布局把页眉页脚的 caret 记在该 story 第一次绘制的那一页（重复页眉不会跳到最后一页）；本节引用了但当前页型不画的变体（例如未开 `titlePg` 的 First）在第 0 页留下插入点，不画字形，也不因此多一条越界注释。批注文字不绘制，该 story 的每个偏移都落在批注锚点的起点上，所以 GoTo 跳到被批注的正文、页眉或页脚。既有 `navigation_from_document` 因此能解析这些名称；几何预算把辅助 caret 行和正文行加在一起。未改动的导出仍逐字节复制原包。这些段落保持锁定，所以任何编辑仍是 `Restricted`，标记不会被丢掉，也不会被发明进 `document.xml`。本切片不写回页眉、页脚和批注里的书签字节。带批注的文档现在可以排版并导出 PDF（只画正文和页眉页脚）。证据：`header_footer_and_comment_bookmarks_import_with_body_rules`、`story_bookmarks_refuse_bad_names_duplicates_columns_and_detached_comments`、`a_header_bookmark_past_the_shared_budget_is_refused`、`header_and_comment_bookmarks_still_refuse_an_edit`、`header_and_footer_carets_stay_on_the_first_painted_page`、`a_comment_caret_is_the_anchor_and_comment_text_is_not_painted`、`an_unpainted_first_header_and_an_empty_header_still_resolve_carets`、`header_footer_and_comment_bookmarks_are_goto_destinations`。#95 的 `detached_column_table_and_header_bookmarks_are_not_destinations` 已改名为 `detached_column_and_opaque_table_bookmarks_are_not_destinations`，仍拒绝列书签和不透明表格。未做：页眉页脚与批注书签写回、不透明表格和列书签成为 PDF 目标、外部 PDF 结构读取器复核、PDF 结构标签、图形界面、Word／WPS 对照、真实打印与系统打印对话框。
- 2026-10-24 页眉页脚与批注书签写回切片（任务仍 `in_progress`，进度计数不变，仍是 12 done／22 in_progress／32 planned／0 blocked）：#97 已经导入的页眉、页脚和可定位批注书签可以写回，规则与正文书签写回相同。未改动的包仍逐字节复制。宿主段落和锚点都没变时，标记字节留在原部件原处；段落被编辑或锚点仍是该部件里可表示的范围时，按模型偏移拼回原来的 `w:bookmarkStart`／`w:bookmarkEnd` 字节（保留 `w:id`、名称和 `displacedByCustomXml`）。正文标记只进 `document.xml`；页眉、页脚和批注标记写回各自部件，不发明进 `document.xml`。`w:id` 仍按部件配对。批注文字叶子就地改写后再拼书签。改名、脱离、颠倒、偏置变化、跨部件移动和新造书签都拒绝，不静默丢掉。批注不再可定位时书签也不写回。列书签、缺 id、不透明表格和游离批注继续锁定所在段落，编辑导出为 `Restricted`。已接受的页眉、页脚和批注书签不再锁定段落。证据：`an_edited_export_writes_story_bookmarks_back`、`story_bookmark_write_back_refuses_cross_part_and_bad_shapes`。未做：不透明表格和列书签成为 PDF 目标、外部 PDF 结构读取器复核、PDF 结构标签、图形界面、Word／WPS 对照、真实打印与系统打印对话框。
- 2026-10-26 列与不透明表格书签目标切片（任务仍 `in_progress`，进度计数不变，仍是 12 done／22 in_progress／32 planned／0 blocked）：映射段落里带 `colFirst`／`colLast` 的成对书签按普通范围书签导入为模型书签，名称原样、区分大小写；所在段落保持锁定，因为列属性不写回，编辑导出仍是 `Restricted`，未改动导出仍逐字节复制原包。只在未映射标记里观察到的成对书签（不透明表格、块级内容控件、页眉页脚的非段落子元素）没有文本范围可锚定：良构成对且名称合法的，以前置段落末尾（块在故事开头则用第一个段落开头）的点目标插入模型书签，标记字节留在保留标记内、不设 source，写回不会移动它们，并保留 `BookmarkAnchorUnmapped` 作为「锚点为合成」的信息性诊断。不成对、顺序颠倒、未知属性、列标记、空名／控制字符／超预算、重名和超 512 个的仍然只诊断不插入，避免污染整份 PDF 导出。半对半映射的同一 `w:id` 仍记 `BookmarkAnchorInvalid` 并锁定段落。既有 `navigation_from_document` 与布局 caret 不改动即可解析这些名称。证据：`column_and_opaque_table_bookmarks_are_destinations`、`an_opaque_table_at_the_body_start_anchors_the_first_paragraph`、`bad_opaque_table_bookmark_pairs_stay_refused`、`story_bookmarks_refuse_bad_names_duplicates_and_detached_comments`（原 `story_bookmarks_refuse_bad_names_duplicates_columns_and_detached_comments`），以及 `apps/joyword-cli/tests/bookmark_goto.rs` 的 `column_and_opaque_table_bookmarks_are_goto_destinations`。未做：外部 PDF 结构读取器复核、PDF 结构标签、图形界面、Word／WPS 对照、真实打印与系统打印对话框。
- 2026-10-26 错误路径切片（任务仍 `in_progress`，进度计数不变，仍是 12 done／22 in_progress／32 planned／0 blocked）：布局未稳定、字体不可嵌入不再报告成功，也不留下已发布文件。joyword-pdf 新增 `PdfError::UnembeddableFont`（与 `MissingFont` 分开）：写页前逐面检查 OS/2 `fsType`——Restricted License 按 `fsType & 0xF == 2` 读，与 krilla text/cid.rs 一致，无 OS/2 的 face 视为可嵌入——再由 krilla `Font::new` 接收；任一拒绝则整份导出不产出字节，finish 阶段的子集化失败报字体原因。应用侧导出与打印到文件共用的 publish 入口 `run_pdf` 现在先校验 `ensure_current`：布局未稳定或过期（菜单同一条件 `file.export.unstable`／布局未稳定）以「layout unstable」拒绝，场景字节不会写出；CLI `convert` 对 `LayoutError::Unstable` 与不可嵌入字体分别明确拒绝，坏路径不留文件。布局不收敛的检测本身仍在 layout（`run_layout_passes` 返回 `LayoutError::Unstable` 不发布临时页），本切片只接导出／打印侧。证据：`a_restricted_license_font_refuses_export_with_no_bytes`、`the_restriction_reads_the_low_nibble_like_krilla`、`an_allowed_embedding_bit_still_exports`（joyword-pdf，改 OS/2 fsType 的 fixture 字节），`an_unembeddable_font_refuses_export_and_publishes_nothing`、`export_and_print_refuse_without_a_settled_layout_and_publish_nothing`（apps/joyword，共用 publish 入口无头复排），`convert_refuses_a_restricted_license_font_and_writes_no_pdf`、`convert_still_exports_with_an_embeddable_font`（joyword-cli 真实进程）；不收敛检测仍由 `repeated_feedback_is_unstable_and_unpublished` 覆盖。未做：两个独立阅读器人工检查、真实打印、系统打印对话框、PDF 结构标签、图形界面、Word／WPS 对照。
- 2026-10-27 外部 PDF 结构读取器复核切片（任务仍 `in_progress`，进度计数不变，仍是 12 done／22 in_progress／32 planned／0 blocked）：「文档内跳转」不再只由 joyword-pdf 内部读取器自证。新增外部 qpdf CLI 结构读取器复核（`qpdf --json=2`；设 `JOYWORD_REQUIRE_QPDF=1` 时要求跑通（unified-pdf apt 安装 qpdf 的 CI 步骤另跟），本机无 qpdf 或未设该变量时跳过，与 `pdftotext`／`JOYWORD_REQUIRE_TEST_FONT` 同一 skip-vs-require 模式；只加 qpdf 一个 apt 包与 serde_json 一条 dev-dependency 边，lock 无新包）。`crates/joyword-pdf/tests/navigation.rs` 新增 `an_external_reader_sees_named_destinations_and_resolves_goto_actions`：模型书签（ASCII `first`、非 ASCII「目录」、无链接的 `unused`）、两条锚点 GoTo 与一条 URI 链接同页导出，qpdf 侧断言命名目标树按键字节可见（qpdf v2 JSON 把可打印字符串标 `u:`、其他字节串标 `b:<hex>`，按 ISO 32000 字节比较还原，与内部 `name_key` 契约一致）、每个 `/S /GoTo` 注释的 `/D` 解析为 `[page /XYZ x y 0]` 且命中同名命名目标数组与正确页面对象、坐标与内部读回一致、URI 动作原样。`apps/joyword-cli/tests/bookmark_goto.rs` 的正文、页眉页脚批注、列与不透明表格三个导入场景在同一份字节上追加同一外部复核并与内部 `inspect_navigation` 逐名逐链对照。证据：`an_external_reader_sees_named_destinations_and_resolves_goto_actions`，以及现在内含 qpdf 外部断言的 `an_imported_bookmark_is_a_pdf_goto_named_destination`、`header_footer_and_comment_bookmarks_are_goto_destinations`、`column_and_opaque_table_bookmarks_are_goto_destinations`。未做：锚点目标不在本文档书签中时链接仍被跳过而不阻断导出、PDF 结构标签、图形界面、Word／WPS 对照、真实打印与系统打印对话框、两个独立阅读器人工检查（UOS 报告的 qpdfview／WPS PDF 仍是人工行）。

### T205：功能区、标尺与状态栏

- 输入：命令注册表、样式与段落属性、视图选项和本地化资源。
- [ ] 状态：`in_progress`
- 需求：`PLATFORM-003`、`EDIT-009`
- 前置：`T106`、`T200`
- 产物：开始、插入、布局、视图等功能区中的已实现命令，字体与段落对话框（含中文字号），标尺（缩进、制表位），状态栏（页码、字数），编辑标记／隐藏文字／域代码显示切换；功能区图标以代码绘制并纳入资产署名门禁；功能目录逐项绑定需求 ID 与证据等级（存在／单元测试／语料／参考对照），只统计有证据的项。
- 错误路径：未实现命令不出现；视图切换不改变文档数据；对话框取消不产生事务。
- 自动验证：功能区与注册表一致、字数统计规则（中文按字，与固定 Word 样例的对话框数字对照）、视图选项进入布局快照键、对话框到命令的映射；功能目录地板测试只升不降。
- 人工验收：只用功能区和对话框完成报告排版，切换编辑标记与缩放。
- 完成证据：2026-10-18 第一切片已落地。开始、插入、布局、视图功能区只列出注册表里已有的命令，按钮经 `Registry::invoke_with` 执行。状态栏用当前布局的插入点页码和会话正文的字数／字素。水平标尺显示解析后的缩进和制表位。图标是 `chrome.rs` 里的 16×16 原创格子图。同日第二切片已落地，任务保持 `in_progress`。`format.font` 与 `format.paragraph` 在确认时各提交一次可撤销事务；取消、空选区或空命令列表不开启事务。字体家族同时写入 ascii 与 eastAsia，空家族保持原值；字号接受半点小数或中文号（初号至八号，含小初至小七；二号 22 pt、三号 16 pt、四号 14 pt、六号 7.5 pt、七号 5.5 pt）。粗体、斜体、单下划线与自动色或 sRGB 在导出子集内写入。下划线和非自动颜色在第二切片会导出；页面场景当时拒绝绘制，并在应用后记 `dialog.font.paint`。主题色拒绝。段落写入对齐、左右缩进、首行或悬挂（不能同时写）、段前段后和行距。居中、右对齐或两端对齐与会移动文字的缩进同时出现时拒绝；`distribute` 拒绝；`snapToGrid` 为真且行距不是省略或 Auto(240) 时拒绝。字符单位仍不能换算。行距 Auto(n≠240) 把 profile 行距按 n/240 整数截断，Exact 与 AtLeast 用绝对 twip，未与 Word 行网格对照；Auto(240) 的既有路径不改。标尺可拖首行、正文左缘、右缩进和制表位；拖动生成的默认位会新增 Left／None 直接制表位；样式继承来的自定义制表位拒绝改写。移动不足 3 像素不提交。三个视图开关在第二切片已登记且 `modifies_document` 为假，当时因为排版没有这些快照选项而禁用。功能目录地板在第二切片升到 39，T203 的插入图片按钮再把它升到 40，T200 的项目符号按钮再把它升到 41，新增控件证据等级为 `unit`。证据：`chinese_sizes_follow_the_office_point_table`、`a_cancelled_font_dialog_does_not_open_a_transaction`、`font_apply_is_one_undo_and_an_empty_selection_is_not`、`paragraph_indent_spacing_and_line_are_one_undo`、`justify_with_indent_is_refused_and_the_document_stays`、`first_line_drag_keeps_the_body_edge`、`dragging_a_generated_stop_adds_a_direct_left_stop`、`inherited_style_tabs_are_detected`、`underline_and_srgb_do_not_record_a_paint_refusal`、`view_toggles_flip_without_a_document`、`auto_line_spacing_scales_the_profile_leading`、`direct_indent_spacing_and_tabs_round_trip_and_survive_a_font_change`，以及第一切片已列出的目录与功能区测试。2026-10-19 第三切片已落地，任务仍 `in_progress`。页面场景绘制单线、双线和点线下划线以及 sRGB 文字颜色；CPU 光栅和 PDF 共用这些矩形与填充。下划线位置和粗细优先读 OpenType `post`（y 向上，负值在基线之下），厚度非正时用 `OS/2.yStrikeoutSize`，两者都没有时厚度为 `unitsPerEm/20`（至少 1）且顶边在基线下一倍粗细。双线的第二笔和点线的间隔重复这一笔，不是 Word 测量。其他下划线样式和主题色仍拒绝，不再记录 `dialog.font.paint`。格式标记、隐藏文字和域代码进入布局快照键。格式标记（段落标记、制表箭头、空格点）是灰色非文档叠层，不改变断行。`w:vanish` 的显示或省略会改变排版。域代码显示指令，域结果显示分隔符之后的文字；没有分隔符时结果投影省略整段域。正文和页眉页脚的 PAGE／NUMPAGES／SECTIONPAGES 在域代码打开时显示这三个名字，否则仍显示计算后的页码。三个功能区开关可用，且不修改文档。每种选项的增量排版与完整排版页面相同。字数测试对照 EDIT-009 的手算样例「你好，Word 2024 的字数：don't count 12.5。」（汉字 5、西文词 3、数字 2、合计 10），不是 Word 对话框。证据：`single_underline_and_srgb_are_on_the_shared_scene`、`double_and_dotted_repeat_the_same_stroke`、`dash_underline_stays_unpainted`、`formatting_marks_do_not_change_line_breaks`、`hidden_text_changes_layout_and_the_cache_matches`、`field_codes_swap_instruction_and_result`、`field_codes_replace_body_page_fields_and_match_the_cache`、`underline_metrics_prefer_post_then_strikeout_then_em`、`srgb_text_and_its_underline_paint_the_run_colour`、`a_non_black_run_changes_the_pdf_bytes`、`word_count_matches_the_documented_edit_009_rule`、`view_toggles_flip_without_a_document`、`underline_and_srgb_do_not_record_a_paint_refusal`。未做：与固定 Word 字数对话框的数字对照、导航窗格与审阅栏、粗线／虚线／波浪线／按字下划线、主题色、隐藏段落标记导致的段落合并、跨段域、斜体剪切下划线，以及只用功能区和对话框完成报告排版的人工验收。见 [IMPLEMENTATION](IMPLEMENTATION.md)。 2026-10-19 银河麒麟 GUI 部分验收：开始功能区经 `Registry::invoke_with` 执行加粗／查找替换／格式刷（见 T200），布局选项卡页面设置即时生效（见 T202），状态栏字数与缩放（Ctrl+=／Ctrl+0）正常；字体／段落对话框与「只用功能区完成报告排版」的完整人工验收仍未执行。 2026-10-19 CPU 亚像素矩形改用专用覆盖率光栅器，回归五个亚像素位置的细下划线可见性；未新增功能区、Word 字数或真实窗口验收，见 [验证审计](VALIDATION-20261019.md)。2026-10-19 统信 UOS GUI 补验：格式标记、隐藏文字、域代码三个视图开关均可切换，格式标记画出灰色空格点与段落标记且不改变断行；缩放 80%→90%→100% 与 Ctrl+0 复位正常。补验中发现切换隐藏文字可在 debug 构建触发 tiny-skia `hairline_aa` 断言（1–2 px 宽的分数坐标矩形），已把亚 2 px 宽矩形吸附到整像素并新增 `narrow_fractional_rects_keep_their_pixels_without_panicking` 回归；修复后三个开关连续切换无崩溃。仍缺 Word 字数对话框对照、导航窗格、审阅栏与「只用功能区完成报告排版」的人工验收，T205 保持 `in_progress`，证据见 [UOS 报告](../../tests/fixtures/reference/uos/uos_report.json)。
- 2026-10-20 文档核对：功能区与注册表、字数规则（EDIT-009 手算，不是 Word 对话框数字）、视图快照键、对话框映射和功能目录地板已有自动测试。与固定 Word 样例对话框数字对照，以及只用功能区完成报告排版，仍未做。状态保持 `in_progress`。见 [验收清单](acceptance/T205.md)。
- 2026-10-24 P0／P1 桌面界面切片：标题区整合文件与快速访问命令；开始页改为字体／字号／颜色、段落和样式库分组，插入页增加 1×1 至 8×8 表格网格，布局页改用下拉预设；样式和查找替换改为按需侧栏，格式与 Linux 路径输入改为模态对话框；状态栏改为单行并增加可拖动／键盘缩放滑块。Ribbon 按实际窗口宽度分完整、紧凑和溢出三档，控件增加悬停、焦点、按下与 tooltip。纸张在宽视口居中，命中、选区、IME 与标尺共用页面原点，标尺按正文页边距和缩放对齐并显示数字刻度。段落缩进、间距、单元格边距与边框粗细以磅显示并转换回命令单位。功能目录地板升至 60；`ribbon_density_has_full_compact_and_overflow_tiers`、`ruler_uses_the_same_page_origin_and_zoom_as_the_canvas`、`zoom_slider_clamps_to_the_supported_range`、`formatting_dialog_converts_points_back_to_command_units` 等自动回归通过，IME 居中几何断言同步更新。T205 仍为 `in_progress`，不以此替代只用功能区完成报告、固定 Word 对照及三端人工验收。

### T206：M2 发布门槛

- 输入：M2 任务产物、原创报告语料和固定 Word／WPS 参考环境。
- [ ] 状态：`planned`
- 需求：`LAYOUT-004`、`LAYOUT-008`、`LAYOUT-009`、`CLIP-001`、`QUALITY-006`
- 前置：`T107`、`T201`、`T203`、`T204`、`T205`、`T207`、`T208`、`T209`、`T210`、`T211`
- 产物：含样式、编号、表格、图片、页眉页脚和页码的报告往返与 PDF／打印报告，以及中文基础排版对照结果。
- 错误路径：参考环境或字体缺失不按通过计；行／页断点超出 QUALITY 阈值的样例逐项登记。
- 自动验证：报告语料往返、几何比较、PDF 文本抽取和性能冒烟。
- 人工验收：在 Mac 上按报告工作流完成制作、跨应用粘贴、导出 PDF 与打印。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T207：中文基础排版

- 输入：ECMA-376 与 MS-OI29500 的网格条款、中文 Word／WPS 默认文档、固定字体与 Word 参考结果。
- [ ] 状态：`in_progress`
- 需求：`LAYOUT-008`、`LAYOUT-002`
- 前置：`T105`、`T004`
- 产物：文档网格（行／字符）吸附、字符与行单位的缩进和间距、禁则字符表、中西文自动间距、标点压缩、adjustRightInd 及相关兼容标志的支持子集；两端对齐在纯中文行字间分配余量；四槽字体与主题东亚字体（a:ea）落地；CJK 字体替代表（宋体、仿宋、楷体、黑体、等线、微软雅黑到各平台实际字体）与替代／缺字诊断；行高在 hhea／OS/2 typo／win 三套度量间的选择以固定 Word 实测为准。
- 错误路径：字符单位依据未确认、字体缺失或网格参数非法时给出诊断并标未验证，不猜测为通过。
- 自动验证：中文默认文档和公文网格样例的行／页断点比较、单位换算、禁则与间距单元测试。
- 人工验收：打开中文 Word 与 WPS 生成的合同和报告，对照参考 PDF 检查断行与分页。
- 完成证据：仅当前切片，非完整任务完成。2026-10-18 纯函数切片仍在：R7 只接受逐字表中的连排配对和汉字跟随类别（等线 12 pt 已测配对为 Measured，其他字号为 Scaled，汉字类别外推为 Inferred；ASCII、空格、行尾和未测标点跟随返回未验证）。R4 的 1.3×win 跨度×字号与 312 twips 网格吸附为 Inferred。R3 等分余量为 Inferred，段末行不拉伸。字符／行单位只做百分之一换算（SuppliedBasis）；中西文间距宽度与 adjustRightInd 仍返回未验证。禁则沿用既有字符集。六种 CJK 家族只做同名匹配，默认等线与主题东亚字体仍只是查询助手，不是字体加载链。同日排版入口接入：`layout_document`／`wrap_mixed` 在名称表解析为等线时，把窄于整形结果的 R7 宽度写入页面场景（屏幕与 PDF 共用该场景）；全宽与未验证配对保持整形宽度。纯中文行的 `both` 调用 `justify_intercharacter`，非末行抵满版心，段末行和单字行不拉伸；`distribute` 仍拒绝。2026-10-21 起中西文混排的 `both` 行只在中日韩字符两侧和空格处分配余量，拉丁词内部不拉开，没有可分配间隙的行保持左对齐（Inferred，见 [R3](layout_rules.md)）。单倍行距（auto 240）仅在宋体、仿宋、楷体、等线、微软雅黑上使用 R4 行框，未显式关闭 `snapToGrid` 时吸附到 312 twips；其他字体与其他行距规则保持调用方行框。2026-10-21 起 `w:docGrid` 进入模型：`lines`／`linesAndChars` 的正 `linePitch` 替换单倍行距的 312 twips 默认吸附，证据仍是 Inferred；`charSpace` 与 `snapToChars` 不改变行高；非正 `linePitch` 整节保留并拒绝排版。自动验证见 [IMPLEMENTATION](IMPLEMENTATION.md)。Word／WPS 人工对照未做，T207 保持 `in_progress`。

### T208：段落缓存与增量重排

- 输入：engine 的 ChangeSet、现有全量排版入口、ARCHITECTURE §6.2 的缓存键约定。
- [ ] 状态：`in_progress`
- 需求：`LAYOUT-010`
- 前置：`T101`、`T105`
- 产物：段落布局缓存（内容指纹＋解析样式＋可用宽度＋FontKey＋profile 基础键，页码／脚注编号／绕排区等外部依赖只在段落确实依赖时入键）、ChangeSet 驱动的失效、样式变更按受影响样式闭包失效、分页传播的稳定停止条件、整形器数据按 FontKey 缓存。
- 错误路径：缓存键遗漏依赖产生陈旧结果必须有反例测试；迭代不收敛返回“布局未稳定”诊断，不把临时结果当正式输出。
- 自动验证：增量结果与全量结果逐页一致；末段编辑的整形次数显著少于全量；首段编辑仍正确传播。
- 人工验收：百页文档分别编辑首段与末段，观察响应与分页正确性。
- 完成证据：2026-10-18 段落布局缓存与增量重排已接入 `layout_document` 共用的拟合／分页，并由应用编辑路径调用。缓存键是内容指纹、解析样式、可用宽度、FontKey 与 profile 基础项；页码、脚注编号和绕排只在段落依赖字段指令时入键。样式变更按 basedOn／link 闭包失效，不整表清空。分页从最早失效段重放，后缀页边界与上一快照一致时停止拷贝。整形器 `ShaperData` 按 FontKey 缓存，最多 8 个面。不收敛返回“布局未稳定”，不发布临时页。缓存默认 32 MiB，单条超预算不入库，超出则淘汰最久未用。自动验证见 [IMPLEMENTATION](IMPLEMENTATION.md)。百页文档分别编辑首段与末段的人工验收未做，T208 保持 `in_progress`。 2026-10-19 银河麒麟尝试受挫：文件打开对话框在 X11 上不出现（见 T204 同源的 [kylin_report.json](../../tests/fixtures/reference/kylin/kylin_report.json) 缺陷），百页文档无法经 GUI 装入，该项人工验收在 Linux 上暂被阻塞，待打开对话框修复或替代入口后补做。 2026-10-19 Windows release 无头 20 页／725 段冒烟：增量输入重排中位 8.492 ms、每次整形 1 段；首段改动 8.557 ms、整形 1 段，对照全量 725 段。未测百页 GUI 与实际呈现延迟，不标 done，见 [验证审计](VALIDATION-20261019.md)。 2026-10-19 生成 4000 段／160000 字符原创长文档；CLI 全文排版超过 64 页硬预算并明确拒绝输出，但未在 GUI 编辑首段与末段，也未获得可复核的精确页数，T208 保持 `in_progress`。

### T209：撤销组与输入来源区分

- 输入：现有快照撤销、SPEC §6 的撤销组约束、组合输入与拖动手势路径。
- [ ] 状态：`in_progress`
- 需求：`EDIT-005`
- 前置：`T102`
- 产物：事务层撤销组（连续打字合并为一步，组合输入一次提交、拖动手势、自动更正各自成一步，边界不跨作者、审阅模式与版本冲突）；输入层区分键入与粘贴两类来源，粘贴文本中的段落分隔不触发按键语义；行为登记表中的撤销条目逐条有测试。
- 错误路径：合并跨越边界或来源混淆均有回归测试。
- 自动验证：撤销组合并性质测试、行为登记表撤销条目存在性校验。
- 人工验收：连续打字、IME 提交、拖动与粘贴混合操作后逐步撤销，每步粒度符合预期。
- 完成证据：2026-10-18 分支 `m2/t209-undo-groups` 实现事务层与输入层切片。engine `Transaction::undo_group`（Standalone／Typing／Deleting）只在同类、`Origin::User`、起始选区等于栈顶结束选区、组未封口且未满 `MAX_UNDO_GROUP_STEPS`（100）时并入栈顶；撤销、重做、独立提交与 `Session::seal_undo_group` 封口，失败或空事务不影响组，panic 守卫一并回滚组状态。shell `InputSource`（Keyboard／Paste／Command）与 `replace_from`：仅键盘、非组合提交、无段落分隔、作用于当前选区的文本为 Typing，`delete` 为 Deleting；回车、粘贴（含多段）、IME 提交（unmark 或组合中 insertText）、菜单插入、恢复与打开期间缓冲输入均为独立步骤；选区实际改变时封口。host 与应用的 EDIT_PASTE／EDIT_INSERT 已接入对应来源。engine 10 项（含 48 种子×160 步随机性质测试）与 shell 8 项测试通过，行为登记表新增 10 条 `EDIT-UNDO-*`，check_docs 校验其存在。未覆盖：拖动手势与自动更正尚未实现，届时以 Paste／Command 来源接入；审阅模式边界待 T300 调用 `seal_undo_group`；协作多作者未建模（仅区分 User／Agent／System）；系统 IME、拖动与粘贴混合的人工验收未做，T209 保持 `in_progress`。验证见 [IMPLEMENTATION](IMPLEMENTATION.md)。 2026-10-19 Windows GUI 实测三次物理按键 `abc` 合并为一步、系统 IME 提交“你好”为一步、一次 Ctrl+Z 删除整次提交、系统剪贴板粘贴为独立一步，保存后撤销也只撤掉保存后的文字。拖动、自动更正和审阅模式边界仍未实现／验收，T209 保持 `in_progress`。
- 2026-10-20 验证波次收口：S5 排练 `t209_keyboard_paste_command_and_ime_undo_one_step_at_a_time` 覆盖键盘输入、退格、粘贴、命令插入和两次 IME 提交各自成步的逐步撤销、全部重做和另存重开，见 [场景排练](verification/scenario-rehearsals.md)。拖动和自动更正仍未实现，输入法候选窗等人工验收不由排练替代。状态保持 `in_progress`。

### T210：无头读回与转换 CLI

- 输入：ARCHITECTURE §3 的 CLI 契约、DocxSession 与 PageScene 的只读接口。
- [x] 状态：`done`
- 需求：`QUALITY-003`
- 前置：`T103`、`T105`
- 产物：inspect（带 schema 版本的 JSON、显式读取投影声明与截断标志）、text、render（CPU 光栅页面 PNG）、convert（仅已支持子集）子命令；输出确定可复现。
- 错误路径：未支持的转换明确拒绝，不提供假成功入口；截断必须带标志，不静默省略。
- 自动验证：对 5 个原创样例输出稳定 JSON 与 PNG，同一输入两次运行逐字节相同。
- 人工验收：检查 inspect 的投影与截断行为符合声明，render 输出与窗口显示一致。
- 完成证据：2026-10-18 inspect／text 已有 5 个原创样例的稳定投影测试。后续同轮增量已实现 `render` 与 `convert --to pdf`：真实 DOCX 只读导入后经同一 `layout_document` PageScene，分别由 `joyword-render` 写 CPU PNG、`joyword-pdf` 写可检索 PDF，不建立第二套分页；显式 `--font PATH:FACE`，页码／DPI／字体／输入／页数／输出预算有界。输出先在同目录临时文件写入、sync 和回读，再以不覆盖目标的硬链接发布并复核；输入永不修改，其他转换目标拒绝。Windows 宋体真实进程测试验证 PNG／PDF 两次运行逐字节相同、重复目标拒绝和稳定 JSON 报告；原有 inspect／text 契约回归通过。`pdftotext -enc UTF-8` 已独立读回宋体 PDF 的全部中英文（默认输出编码会丢中文，验证必须显式指定 UTF-8）；回归测试固定该读回。同日后续切片使 DOCX-MIN-001..005 都能经 CLI `render` 与 `convert --to pdf`：settings 只在存在没有模型字段的子元素时整段保留，正文不透明块跳过并在 schema 1 报告里计为 `omitted_blocks`。文泉驿微米黑进程测试对五个样例各跑两次，PNG／PDF 与报告 JSON 逐字节相同，MIN-003 的 `omitted_blocks` 为 1，输入字节不变，`--to html` 与覆盖已有输出仍以退出码 2 拒绝。render 与窗口显示的人工一致性验收未做，T210 保持 `in_progress`。命令及预算见 [IMPLEMENTATION](IMPLEMENTATION.md)。 2026-10-19 Windows 宋体 face 0 五样例各两次 inspect／render／PDF 确定性、输入不变与显式截断已复验；独立读回揭示并修复粗体重复文本，CPU／PDF 对照揭示并修复细下划线消失。真实窗口对照尚待 GUI 时段确认，不能标 done，见 [验证审计](VALIDATION-20261019.md)。 2026-10-19 Windows GUI 打开 DOCX-MIN-001 隔离副本，80% 缩放时正文与 CLI CPU render 同为单行；按页面宽度归一后文字水平边界差 ≤2.23 px、垂直边界差 ≤6.32 px（截图边界误差，不宣称像素相同）。图像见 `validation/20261019/gui/01-open-local.png` 与 `validation/20261019/DOCX-MIN-001/cpu.png`，机器可读对照见 `validation/20261019/gui/summary.json`。结合五样例逐字节确定性、inspect 投影与截断退出码 3，自动与人工验收均完成，状态改为 `done`。

### T211：应用日志与崩溃诊断

- 输入：应用与 shell 的现有错误路径、QUALITY-007 的隐私约束。
- [x] 状态：`done`
- 需求：`QUALITY-007`
- 前置：`T106`
- 产物：按启动轮转、单文件封顶的应用日志；panic hook 记录位置与回溯并对可能的正文内容脱敏；隔离偏好与日志目录的测试 profile 开关。
- 错误路径：日志不得含文档正文、凭据、修订或批注内容；崩溃后仍保留上一份日志。
- 自动验证：测试断言日志不出现样例正文；轮转、封顶与脱敏测试。
- 人工验收：崩溃后检查日志可用且无正文泄漏。
- 完成证据：2026-10-18 已接入 `joyword-shell::diag` 与 `apps/joyword` 启动路径。每次启动以 `File::create_new` 新建日志，默认单文件 256 KiB、保留 5 份；新文件头 sync 之后才删除更早的超额文件。panic hook 记录位置与按行脱敏的回溯，stderr 只写固定句。`JOYWORD_PROFILE=test` 把偏好和日志放到与用户配置目录不同的根。集成测试覆盖样例正文、修订、批注和凭据不出现在日志中，以及轮转、封顶、脱敏、profile 隔离和真实 panic hook。命令见 [IMPLEMENTATION](IMPLEMENTATION.md)。 2026-10-19 检查隔离 profile 的 2 份 GUI 日志，正文、IME 和保存哨兵词均未出现；日志中的诊断字段已脱敏。同日银河麒麟完成真实崩溃人工验收：对正在编辑的窗口 `kill -9`，日志文件完好、`grep` 文档正文关键词（标题、双方名称、金额、替换词）零命中，重启后日志可读且会话恢复（见 T104）；自动验证与人工验收均已完成，状态改为 `done`。

## M3：专业审阅

退出条件：合同在 Word 和 JoyWord 间交换，接受／拒绝、撤销和批注锚点正确。

### T300：文字与移动修订

- 输入：编辑事务、原始和目标内容、审阅作者及当前修订集合。
- [ ] 状态：`in_progress`
- 需求：`REVIEW-001`、`REVIEW-003`、`REVIEW-004`
- 前置：`T101`、`T103`
- 产物：插入／删除／移动、作者时间、分组、四种视图（排版随投影变化）和局部处理。
- 错误路径：交叉范围、已有修订内编辑和不完整移动对有明确语义。
- 自动验证：原始／最终投影、接受／拒绝、undo 与文件往返性质测试。
- 人工验收：在 Word 与 JoyWord 交换文字修订，逐项和批量接受／拒绝。
- 完成证据：2026-10-19 第一切片进行中，任务不打勾。已落地：`w:ins`／`w:del`／`w:moveFrom`／`w:moveTo` 与四类移动范围标记按 id／author／date 进入类型化修订；`rPrChange` 等 T301 种类仍整篇只读。`trackRevisions` 单独不再只读，文档保护仍然只读。布局视图 `markup`／`final`／`original` 进入 `LayoutKey` 与段落缓存；最终带标记对插入画下划线和作者色、对删除画删除线，最终／原始投影在整形前去掉隐藏文字。接受／拒绝单条与全部是可撤销事务；`Origin::User` 在跟踪打开时打字和删除产生修订，独立事务（含 `SetTrackRevisions`）封口打字组；Agent 不产生修订。导出按 ExportPlan：未改段落复制原字节，改过的修订段落重写，`trackRevisions` 只改设置部件。自动测试：`text_revisions_round_trip_and_tracking_is_a_settings_patch`、`tracked_changes_make_the_document_read_only`、`user_tracking_accept_and_reject_undo_and_a_mode_switch_seals_typing`、`revision_views_change_pagination_text_and_match_the_incremental_cache`、`track_revisions_alone_stays_editable`。2026-10-19 第二切片：注册表命令 `review.track`／`review.accept`／`review.reject`／`review.accept_all`／`review.reject_all`／`review.view.markup`／`review.view.final`／`review.view.original` 无必填参数，菜单和功能区可直接调用；`review.author` 读写偏好 `author.name`，空白在 `Origin::User` 创建修订时仍落为 JoyWord。Markup 在共享 PageScene 的左页边为有修订的行画一条作者色条，最终／原始投影不画。已类型化的单元格段落可跟踪插入和删除，接受／拒绝沿用同一事务；纯 `w:ins`／`w:del` 可导入导出，格式与表格结构修订仍整表只读。自动测试：`review_commands_use_the_registry_and_the_author_preference`、`markup_paints_a_margin_bar_beside_a_changed_line`、`tracked_text_in_a_typed_cell_uses_the_author_and_accepts`、`a_plain_cell_insertion_and_deletion_round_trip`。2026-10-19 第三切片：成对 `moveFrom`／`moveTo` 按 `w:name` 作为一次修订。接受或拒绝任一端的修订 id 同时处理两端（接受留下目标并去掉来源，拒绝反过来）；标记范围内的普通文字、插入和删除在单次处理时保留。全部接受后的正文等于最终投影，全部拒绝后的正文等于原始投影；撤销恢复两端文字、修订 id 和四枚范围标记。移动 run 被标记切开，或一侧里出现另一种移动，返回 `RevisionConflict`，文档不变；不完整或交叉的移动对同样拒绝。切换 `trackRevisions` 仍封口打字组。只改 `w:trackRevisions` 时导出只补设置部件、不改写正文，移动对保留；改写含移动的段落后两端和 `w:name` 仍在；按接受结果改写后不再留下半对标记。Markup 为移动来源画删除线、为目标画下划线，并在页边画作者色条；最终／原始投影不画。自动测试：`accept_all_and_reject_all_restore_the_move_projections`、`accepting_either_move_end_resolves_the_pair_and_leaves_other_revisions`、`a_move_run_that_crosses_its_marker_is_refused`、`incomplete_and_crossed_moves_leave_the_document_unchanged`、`switching_tracking_seals_a_typing_group_beside_a_move`、`a_settings_patch_keeps_the_move_pair`、`rewriting_a_move_paragraph_keeps_both_ends`、`an_accepted_move_exports_without_leaving_half_the_pair`、`move_markup_paints_both_ends_and_the_projections_do_not`、`review_commands_accept_a_paired_move_and_a_settings_patch_keeps_it`。rebase 到 main `30a76d8583e2ca130b3a0f8ed2b9a6b3f51aab88`（T202 squash）之后，W09 harness 改锚到 `d6588d052ff8fb898127c437f3558842a03d3770`（该提交的 `crates/` 含上述移动配对，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。先前锚点 `9ca3fead40ba70ba9707150e38bf143f89f0db0b` 上的固定矩阵为 81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。固定矩阵尚未在新锚点上重跑。这仍不是覆盖引导 fuzz。再 rebase 到 main `26fd7ce2a87440395dddf94cb19c2569c6186669`（T200 find-bar squash，未改 `crates/`）之后，W09 harness 改锚到 `e3dca7a9d7ecf45c5ec9e108a3dc2f153628a31f`（该提交的 `crates/` 含上述移动配对，以及 T202 的页眉页脚部件写回；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。固定矩阵尚未在该树上重跑。未做：页眉修订装饰、Word／WPS 人工交换、T301。见 [IMPLEMENTATION](IMPLEMENTATION.md)。
- 完成证据（2026-10-19 页眉页脚装饰切片，任务仍不打勾）：页眉和页脚故事里的 `w:ins`／`w:del`／`w:moveFrom`／`w:moveTo` 画在共享 PageScene 的页眉页脚带上。插入和移动目标用下划线与作者色，删除和移动来源用删除线，有修订的行在左页边画作者色条。最终／原始投影在整形前去掉隐藏文字，并且不画这些装饰。上一则证据里的「未做页眉修订装饰」是第三切片结束时的状态。接受或拒绝页眉故事里的修订仍是一次可撤销事务：全部接受后页眉等于最终文字，全部拒绝后等于原始文字，一次撤销恢复修订 id。带图片的页眉或页脚段落仍由图片拟合摆放，不另画修订装饰。没有改 `pages.rs`。没有做 T301，也没有做 Word／WPS 人工交换。自动测试：`header_and_footer_markup_paints_and_projections_omit_it`、`accepting_or_rejecting_a_header_revision_is_one_undo`。既有正文修订测试 `markup_paints_a_margin_bar_beside_a_changed_line`、`move_markup_paints_both_ends_and_the_projections_do_not`、`revision_views_change_pagination_text_and_match_the_incremental_cache` 仍通过。rebase 到 main `5704bf6226cde228d37e21034c9a8d84b51eb5f7`（T200 捕获组 squash）之后，W09 harness 改锚到 `23fb2faf872b5bd5a3b103e2b33867a9b033068f`（该提交的 `crates/` 含上述页眉页脚修订装饰，以及 T200 的正则替换捕获组；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。该树上固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。再 rebase 到 main `cd087577a67cf5efb9164de15cde166ba7da028e`（T201 首列、末列与纵向条纹 squash）之后，W09 harness 改锚到 `b59c8d4c8f429ff2fa6e5e26290d368e107f76e9`（该提交的 `crates/` 含上述页眉页脚修订装饰，以及 T201 的首列、末列与纵向条纹排版；随后只改 experiments 与本记录，不放宽 `git diff <SHA> -- crates`）。本环境固定矩阵 smoke：81／81，43 ok、38 rejected，无 crash／timeout／invariant／protocol，`matrix_complete` 为真；地址空间限制探测 `enforced=true`。这仍不是覆盖引导 fuzz。独立锁无新包。见 [IMPLEMENTATION](IMPLEMENTATION.md)。
- 2026-10-20 文档核对：进度总览「第一切片进行中（简单页边标记、壳层命令、表格内修订和 Word／WPS 人工交换未做）」已经过时。页边标记、审阅命令、单元格文字修订、成对移动和页眉页脚标记已有自动测试。Word／WPS 交换仍未做。四种视图目前只有标记、最终、原始。没有 M3 任务改为 `done`。状态保持 `in_progress`。见 [验收清单](acceptance/T300.md)。
- 2026-10-20 验证波次收口：S5 排练 `t300_tracked_edits_views_and_revision_round_trip` 只覆盖 JoyWord 一侧：正文与页眉跟踪编辑、三种视图、逐项与批量接受／拒绝及撤销、导出与再导入。S1 有标记、最终、原始三份金样；S3 样例是 DOCX-FEAT-T300-REVIEW。已知缺口已修复（[#67](https://github.com/willmove/joyoffice/pull/67)，main `caf84147`）：`text` 投影元数据声明排除删除和移出，原先标准输出仍含这些文字；现在投影跳过最终视图里不可见的 run，黄金文件已重新 bless，见 [验证波次缺口修复](verification/verify-wave-fixes.md)。Word／WPS 交换仍未做。状态保持 `in_progress`。
- 2026-10-26 简单标记切片（REVIEW-003 第四种视图）：`RevisionView` 新增 `Simple`，投影与最终完全一致（`visible_in` 同 Final），正文与页眉页脚都不画行内修订装饰（无删除线、无下划线、无作者色 run），唯一标记是左页边作者色条：`annotate` 在简单视图为含修订段落的每一行记一条注释供 `margin_bar` 取色，绘制走与最终相同的普通路径再加色条；页眉页脚 `decorate_stories` 在简单视图只补色条、保留已放置的最终 run。`LayoutKey` 与段落缓存把 Simple 作为独立判别（0..3），与 Final 可见文字相同也不会复用无注释的拟合。命令注册表新增 `review.view.simple`（无必填参数、不改文档），功能区审阅视图组加入「简单标记」按钮并按当前视图显示按下态，`en`／`zh-CN` 补 `command.review.simple` 与 `status.review.simple`，特性目录 floor 60→61。自动测试：`simple_view_paints_the_margin_bar_without_inline_marks`、`simple_markup_owns_its_cache_slot_across_a_view_switch`、`header_and_footer_markup_paints_and_projections_omit_it`（扩展）、`revision_views_change_pagination_text_and_match_the_incremental_cache`（扩展）、`review_commands_use_the_registry_and_the_author_preference`（扩展）、`cycling_the_four_review_views_touches_neither_revisions_undo_nor_dirty`、`view_ribbon_commands_do_not_modify_the_document`（扩展）。Word／WPS 对照简单标记外观未做，状态保持 `in_progress`。见 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T301：格式与表格结构修订

- 输入：格式属性前后值、表格结构差异与支持区域的既有修订。
- [ ] 状态：`in_progress`
- 需求：`REVIEW-002`、`REVIEW-004`、`REVIEW-008`
- 前置：`T300`、`T201`
- 产物：属性前值、段落标记、编号、节属性和表格网格结构修订，覆盖支持的 story。
- 错误路径：非法嵌套、修订依赖冲突和丢失原始状态不得猜测恢复。
- 自动验证：格式拒绝还原、表格结构与内容守恒、局部接受合法性测试。
- 人工验收：对含表格的合同增删行、改样式并交换审阅。
- 完成证据：2026-10-22 第一切片（[#87](https://github.com/willmove/joyoffice/pull/87)）：状态由 `planned` 改为 `in_progress`，进度计数随之改为 12 done／21 in_progress／33 planned／0 blocked（与 T401 同批）。这一刀只做正文段落（表格之外）的 `w:pPrChange`（ECMA-376 Part 1 §17.13.5.29）与 `w:rPrChange`（§17.13.5.30），没有人工验收，不标 `done`。模型侧新增 `FormatRevision { id, author, date, previous_run, previous_para }`：run 上的 `format_revision` 来自 `w:rPrChange`、存变更**前**的运行属性，段落上的 `props_revision` 来自 `w:pPrChange`、存变更**前**的段落属性；`Paragraph` 增加 `stamp_format_from`、`has_run_format_revision`、`resolve_format_revision`、`resolve_all_format`，run 合并（`join_runs_at`、`push_text_marked`）把 `format_revision` 当作 run 身份的一部分，`split_off` 把段落级变更留给前半段，规范化 dump 打出 `pPrChange#id author=… date=… prev=[…]`／`rPrChange#…` 行。导入侧 `props::para_with_change`／`props::run_with_change` 捕获单个 `w:pPrChange`／`w:rPrChange` 子元素而不是把整块留白，`revision::import_format_change` 才把它类型化：要求合法的 `w:id` 与 `w:author`（日期可选、按原文保存）、至多一个正确种类的变更前属性子元素，且变更前属性必须完全可表示（不含前一样式、段落标记、节属性或嵌套修订）；子集之外一律返回 `None`，调用方退回"整块属性元素按原字节保留＋锁定所在段落＋`RevisionSubset` 诊断"，绝不静默丢弃；空 run（没有推出任何 run）上的类型化变更同样锁定段落，防止记录消失。`scan_in` 传入 `in_table` 标志，`blocks_for_review(local, in_table)` 只对 `w:tbl` **之外**的段落类型化格式变更，表格之内仍照原样让整篇只读。引擎的 `AcceptRevision`／`RejectRevision`／`AcceptAllRevisions`／`RejectAllRevisions` 复用既有命令一并解决格式修订，语义按 §17.13.5.29／.30：**接受**保留当前（变更后）的格式并丢弃记录，**拒绝**恢复记录的变更前格式（没有记录前值时用空默认），随后重新合并相邻 run；`resolve_one` 在没有文本命中时回落到只带格式修订的 id（不再误报 `UnknownRevision`），同一个 `w:id` 同时承载文本与格式修订时一条命令一并解决；锁定段落以 `CommandError::Locked` 拒绝；`current_revision` 在没有触及文本修订时也能报告插入点处的格式修订（段落级在段内任意位置生效，run 级只在其 run 上生效）。每条命令在一个 `Transaction` 内执行，撤销基于快照，一步撤销完整恢复先前状态。导出对未决格式修订实行"原字节保留，否则拒绝"，绝不合成一个把 `*Change` 抹掉的结果：`Group` 与组／元数据比较都纳入修订记录，所以未打戳的重发不可能被判为相同；`patch_paragraph` 新增五条拒绝，全部由 `after_unresolved = after.has_run_format_revision() || after.props_revision.is_some()` 守卫——与文本修订重写组合（`unresolved format revision cannot be combined with a text revision rewrite`）、改样式或改编号（`… cannot be restyled`）、段落重写（`… its paragraph cannot be rewritten`）、run 边界变化（`… its run boundaries cannot change`）、空段新增文本（`… new text cannot be added`）；已解决的修订在 `after` 里已不存在，走正常重写路径输出不含 `*Change` 的干净格式；未改动的段落仍由既有对齐机制逐字节拷贝，而 `Paragraph` 的 `PartialEq` 纳入新字段，所以丢弃一条记录永远算作一次编辑，不会留下陈旧字节的洞。**表格结构修订本切片明确不实现**：`w:tblPrChange`、`w:trPrChange`、`w:tcPrChange`、`w:cellIns`、`w:cellDel`、`w:cellMerge`（每一个仍只经 `ns::is_revision` 被引用一次），以及 `w:tbl` 子树内的任何 `w:rPrChange`／`w:pPrChange`，都让整篇文档只读（`ReadOnlyReason::Revisions`）、标记逐字节保留、任何改写式导出以 `ExportError::Restricted` 拒绝；没有任何代码路径接受、改写或丢弃它们。同样仍只读的有 `w:sectPrChange`、`w:numberingChange`、`w:tblPrExChange`、`w:tblGridChange` 与自定义 XML 修订；子集外的格式修订按"保留＋锁定段落"处理，不类型化。因此本任务产物里的段落标记修订、编号修订、节属性修订与表格网格结构修订都还没有实现。`docs/joyword/behavior_registry.json` 追加 12 条（只追加，既有条目未动）。证据：`crates/joyword-docx/tests/format_revisions.rs` 十项——`format_revisions_import_typed_with_ids_authors_dates_and_previous_properties`、`unchanged_and_settings_only_exports_keep_format_change_bytes_verbatim`、`editing_a_neighbour_keeps_the_revision_paragraph_bytes_and_reimport_matches`、`accepting_a_run_format_revision_exports_the_current_formatting`、`rejecting_a_run_format_revision_restores_the_previous_formatting`、`accept_all_and_reject_all_resolve_paragraph_and_run_format_changes`、`unresolved_format_revisions_refuse_the_five_rewrite_paths`、`out_of_subset_format_changes_are_preserved_and_lock_the_paragraph`、`table_structure_revisions_stay_preserved_and_read_only`、`the_dump_reports_typed_format_revisions`；`crates/joyword-engine/tests/format_revisions.rs` 五项——`accepting_one_format_revision_is_one_undo_step`（含未知 id 的拒绝）、`rejecting_one_paragraph_format_revision_restores_the_previous_props`、`batch_accept_and_reject_resolve_every_format_revision_in_one_undo_step`、`one_id_carrying_a_text_and_a_format_revision_resolves_both_in_one_command`、`current_revision_finds_paragraph_and_run_format_revisions`。切片内自动验证通过：`cargo fmt --all -- --check` 干净、`cargo clippy --workspace --all-targets --locked -- -D warnings` 零告警（修掉 WIP 留下的 `collapsible_if` 与 `redundant_closure` 两处）、CI 范围 `cargo test --locked` 全绿（含 15 项新测试）、`cargo test --locked -p joyword`、`python scripts/check_docs.py`、`check_gates.py`（含反例）、`check_acceptance.py`；`python -m unittest discover -s scripts/tests` 有 1 项环境性失败（`test_committed_bytes_and_manifest_hashes_match`，本机 Python 3.14.7 的 zlib-ng 1.3.1 与 CI 的 stock zlib 在级别 6 deflate 字节上不同，解压后的成员与 CRC 相同，在干净 main 上同样失败，与 `core.autocrlf` 无关）。未覆盖与未做：Word／WPS 往返交换、审阅栏与图形界面上逐项接受／拒绝、信创真机、打印，全部没有人工验收；锁定容器（表格单元格／SDT 锁）内格式修订的 `CommandError::Locked` 拒绝已实现但没有直接单测，改由 docx 侧的锁定与 `Restricted` 导出拒绝覆盖；页眉页脚与批注 story 里表格之外的格式修订走正文同一代码路径（`body::paragraph`），但没有专门的 story 级测试；段落标记 `w:rPr` 上的 `w:rPrChange`（`w:pPr/w:rPr/w:rPrChange`）经普通 `props::run` 路径退回"保留＋锁定"，行为由构造保证正确，只被子集外测试间接覆盖。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。
- 2026-10-23 跟进（T402 第三切片顺带，T301 状态与计数不变）：正文段落标记的删除修订（`w:pPr/w:rPr/w:del`）随 T402 跨段替换落地——模型 `Paragraph.mark_revision`、engine 跨段跟踪删除与接受时的物理合并（Word 语义：合并段保留首段属性并继承被吸收段自己的待决标记）、DOCX 导入类型化与导出注入／剥离，`RevisionBoundary` 的适用范围从"不跨段"收窄为"不跨表格／内容控件"。表格结构修订、`w:ins` 标记、编号与节属性修订仍不实现，整篇只读策略不变。证据见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T402 跨段替换与段落标记修订」与 `crates/joyword-engine/tests/revisions.rs`、`crates/joyword-docx/tests/mark_revisions.rs`。
- 2026-10-26 第二切片（表格属性修订，任务仍 `in_progress`，进度计数不变）：`w:tblPrChange`／`w:trPrChange`／`w:tcPrChange`（ECMA-376 Part 1 §17.13.5）类型化为模型 `TablePropsRevision { id, author, date, previous }`，分别挂在 `Table`／`Row`／`Cell` 的 `props_revision` 上，`previous` 是 `PreviousTableProps::Table(TableChangeProps)`／`Row(RowChangeProps)`／`Cell(CellChangeProps)`，字段与各级已类型化的属性子集一一对应（表：样式、宽度、布局、对齐、缩进、tblLook、单元格边距；行：gridBefore／gridAfter、tblHeader、cantSplit、trHeight；单元格：gridSpan、vMerge、tcW、tcBorders、shd、tcMar、vAlign）。导入侧 `table.rs` 的 `parse_tbl_pr`／`parse_tr_pr`／`parse_tc_pr` 接受至多一个同级 `*PrChange` 子元素：要求合法 `w:id` 与 `w:author`（日期可选按原文保存）、至多一个种类正确的变更前属性子元素，变更前属性须完全落在同一子集内且不得再嵌套变更；任何一条不满足就让整张表退回原来的不透明保留块（原字节逐字节往返、不可编辑），**不再**把整篇文档置为只读——`ns::blocks_for_review` 对这三个元素名返回 false，因为它们要么已类型化、要么处于不可改写的保留表里，不存在被静默丢弃的路径。结构修订 `w:cellIns`／`w:cellDel`／`w:cellMerge`、`w:tblGridChange`、`w:tblPrExChange` 与表内 `w:rPrChange`／`w:pPrChange` 仍让整篇只读，`table_structure_revisions_stay_preserved_and_read_only` 相应收窄为这些情形。引擎复用 `AcceptRevision`／`RejectRevision`／`AcceptAllRevisions`／`RejectAllRevisions`：`resolve_one` 在没有文本、标记与格式命中时转到表格记录，同一 id 同时承载文本与表格记录时一并解决；接受保留当前属性并丢弃记录，拒绝按记录恢复变更前属性（无变更前子元素时恢复全部缺省）；批量解决扫过表、行、单元格与单元格内嵌套表；表位于内容锁定的内容控件内时以 `CommandError::Locked` 拒绝且文档不变；`ChangeSet.tables` 记录被解决的表以触发重排与导出重写。`current_revision` 在无文本／格式修订时由内向外报告所在单元格、所在行、所在表的记录（嵌套表优先），供审阅命令定位。导出沿用"原字节保留，否则拒绝"：待决记录计入 `Table::same_content`，未改动的表逐字节复制、改邻近段落时亦然；任何需要重写该表的编辑（含改单元格文字、增删行）在 `table_markup` 以三条可定位原因拒绝（`unresolved table/row/cell property revision is kept verbatim and the table cannot be rewritten`），已解决后写出干净属性元素。规范化 dump 新增 `tblPrChange#id author=… date=…`／`trPrChange#…`／`tcPrChange#…`。`behavior_registry.json` 追加 6 条（只追加）。证据：`crates/joyword-docx/tests/table_revisions.rs` 七项——`table_property_revisions_import_typed_and_keep_the_document_editable`、`unchanged_and_neighbour_exports_keep_table_change_bytes_verbatim`、`rewriting_a_table_with_a_pending_property_revision_is_refused`、`accepting_table_property_revisions_exports_the_current_properties`、`rejecting_table_property_revisions_restores_the_previous_properties`、`out_of_subset_table_property_changes_keep_the_table_preserved_and_the_document_editable`、`an_empty_change_element_rejects_back_to_absent_properties`；`crates/joyword-engine/tests/table_revisions.rs` 五项——`accepting_one_table_property_revision_is_one_undo_step`、`rejecting_row_and_cell_property_revisions_restores_the_previous_values`、`batch_accept_and_reject_sweep_nested_tables_in_one_undo_step`、`current_revision_finds_the_innermost_table_revision_at_the_caret`、`a_table_inside_a_locked_content_control_refuses_resolution`。本机验证：`cargo test --locked` 1045 项全过、`cargo clippy --workspace --all-targets --locked -- -D warnings` 零告警、`cargo fmt --all -- --check` 干净、`check_docs.py`／`check_gates.py`／`check_acceptance.py` 通过。未做：结构修订（cellIns／cellDel／cellMerge／tblGridChange）、段落标记 `w:rPrChange`、编号与节属性修订；表格属性修订的页面标记装饰（页边色条）；带待决记录的表在编辑时引擎仍放行、只在导出拒绝（与段落格式修订一致）；Word／WPS 往返、审阅栏图形界面、信创真机与打印的人工验收全部未执行。详见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T301 第二切片：表格属性修订」与 [验收清单](acceptance/T301.md)。

- 2026-10-27 页边标记装饰测试切片（任务仍 `in_progress`，进度计数不变）：本切片补上上一条证据「未做」清单里的「表格属性修订的页面标记装饰（页边色条）」——`crates/joyword-layout/src/document.rs` 新增 `table_row_bar_colour` 并在 `paint_slice` 为每个可视行带画条，`revision_view.rs` 抽出按色绘制条形的 `margin_bar_of_colour`，`hash_table` 把待决表格属性修订折进内容指纹（接受／拒绝只删记录、不改绘制属性，指纹不动就会让缓存页继续画已解决记录的色条，与 `column_separator` 折进节指纹同理）：表级 `w:tblPrChange` 记录覆盖该表每个可视行带、行级 `w:trPrChange` 只覆盖自身行带、单元格级 `w:tcPrChange` 覆盖其所在行的行带；色条只在 Markup 与 Simple 视图绘制（Final／Original 不画）；多条记录覆盖同一行带时取最外层作者色（表＞行＞文档序首单元格，注释写明为 JoyWord 假定）；跨页表头重复的行带携带色条（重复表头的印章复制这些矩形）；嵌套表经自身排版流绘制自己的行带。自动测试：新增 `crates/joyword-layout/tests/table_revisions.rs` 五项——`markup_and_simple_paint_one_bar_per_covered_row_band`（三级记录各覆盖的行带数与同记录同色，Final／Original 零色条）、`the_outermost_record_decides_the_band_colour`（用同作者跨文档比对钉住「表＞行＞单元格」优先级，不依赖调色板分离不同作者）、`nested_tables_paint_their_own_row_bands_and_resolving_removes_the_bar`（嵌套表画自己的行带、内层记录解决后仅外层保留、无记录的表零色条）、`a_repeated_header_row_keeps_its_bar_on_later_pages`（40 行两页表：每个出现表头的页恰有一条色条）、`the_incremental_entry_matches_the_full_layout_with_bars`（增量入口与全量逐页一致含色条矩形）；新增 `crates/joyword-pdf/tests/table_revision_bar.rs` 的 `the_table_revision_margin_bar_is_in_the_pdf_page_scene`（色条矩形进入交给 PDF 的共享 PageScene，且使 PDF 字节与已解决状态不同）。已知边界：页眉页脚里的表格修订不经过该装饰通道（正文表）；色条颜色映射沿用文字修订的作者调色板。本机：`cargo test --locked` 全过、workspace clippy `-D warnings` 零告警、fmt 干净、`check_docs.py`／`check_gates.py`／`check_acceptance.py` 通过。结构修订（cellIns／cellDel／cellMerge／tblGridChange）、段落标记 `w:rPrChange`、编号与节属性修订、Word／WPS 往返与审阅栏 GUI 人工验收仍未做，T301 保持 `in_progress`。见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T301 页边标记装饰测试」。

### T302：批注线程与现代扩展

- 输入：批注范围、正文、作者、回复及传统／现代批注规范样例。
- [ ] 状态：`in_progress`
- 需求：`REVIEW-005`、`REVIEW-006`、`REVIEW-008`
- 前置：`T100`、`T101`、`T103`
- 产物：范围批注、回复、解决、重开、作者信息及现代扩展映射。
- 错误路径：删除锚点、未知扩展、孤立回复和身份缺失可见，不静默丢批注。
- 自动验证：范围变换、线程往返、解决／删除区别及未知 XML 保留。
- 人工验收：跨段批注后移动或删除正文，交换 Word 并检查线程。
- 完成证据：2026-10-21 第一切片进行中，任务不打勾，没有 Word／WPS 人工交换。类型化 `Comment` 进入 `AnnotationKind`：范围锚点、作者、日期、缩写、`w14:paraId`、回复的 `w15:paraIdParent`、`w15:done` 的解决与重开（解决不删除锚点）、`w16cid:durableId` 与 `w16cex:dateUtc`。导入依据 ECMA-376 Part 1 §17.13.4 与 [MS-DOCX] 的 commentsExtended、commentsIds、commentsExtensible。未知子元素和未匹配的 `commentEx` 保留；缺作者、孤立回复、不透明表格里的锚点保持可见并给出诊断，不把文档标成只读。导出不重写已有 `w:comment`：解决只就地改 `w15:done`，新批注才追加部件。删除锚点导出为 Detached 且不解决。引擎命令 `AddComment`、`ReplyComment`、`ResolveComment`、`ReopenComment` 可撤销；缺作者、孤立回复、未解决却重开、已解决再解决都会拒绝且文档不变。自动测试：`add_reply_resolve_and_reopen_are_undoable`、`deleting_the_range_detaches_and_does_not_resolve`、`resolving_does_not_detach`、`orphan_reply_missing_author_and_bad_reopen_leave_the_document_unchanged`、`range_comment_imports_author_date_and_cross_paragraph_ends`、`reply_thread_resolve_and_reopen_round_trip_keeps_unknown_xml`、`deleting_the_anchor_exports_detached_without_resolving`、`text_inserted_inside_a_range_stays_inside_after_export`、`orphan_reply_and_missing_author_stay_visible`、`opaque_table_comment_anchor_is_detached`、`a_new_comment_and_reply_are_written_back`。未做：图形界面气泡、people.xml 身份、commentsIds 与 commentsExtensible 对新回复的改写（这两部分保持原字节，新回复的 `durable_id` 为空）、不透明表格内的批注锚点映射、仍带范围标记的段落整段重写、页眉批注导出专项测试、Word／WPS 人工交换。见 [IMPLEMENTATION](IMPLEMENTATION.md)。

- 2026-10-27 commentsIds 与 commentsExtensible 写回切片（任务仍 `in_progress`，进度计数不变）：补上第一切片「未做」清单里的「commentsIds 与 commentsExtensible 对新回复的改写」。新批注／新回复现在同时改写四个现代扩展部件：commentsExtended（既有）、commentsIds 与 commentsExtensible（本切片），people.xml 仍不做。分配侧：`joyword-model` 新增 `allocate_durable_id(para_id)` 与共享的 `durable_id_from`（paraId 的 FNV-1a 32 位＋确定性重试，注释写明 JoyWord 假定——durableId 只需在部件内稳定且不冲突，Word 写 GUID 形态而 [MS-DOCX] 定义为 32 位整数，故写十进制）与 `is_strict_utc_date`（`YYYY-MM-DDTHH:MM:SSZ` 严格形状）；引擎 `AddComment`／`ReplyComment` 在创建时分配 durable id，且 `w:date` 已是该形状时同步镜像 `date_utc`（野外部件中 `w16cex:dateUtc` 逐字重复 `w:date`）。导出侧：`durable_plan` 一次规划全部新批注的 durable id（引擎分配值优先，未带者走同一派生；部件内既有条目的 paraId 或 durableId 冲突时显式拒绝），commentsIds 按追加式写回——既有条目字节不动，新条目插在根结束标签前，新部件附带关系与 Content-Type Override；commentsExtensible 同样追加式，且仅当每个新批注的 `w:date` 都是严格 UTC 形状时才写（否则整部件保持原字节，不发明时间戳）；两部件由同一 `durable_plan` 供值，同一 paraId 在两部件键一致。语义往返因此闭合：引擎创建的批注在导出→重导入后 durable 与 date_utc 逐字相等（规范化 dump 参与比较）。证据：`crates/joyword-docx/tests/comments.rs`——`a_new_comment_and_reply_are_written_back` 重写为经引擎命令创建（断言 ids 部件两条、extensible 含 dateUtc、重导入 durable 非空且父子关系不变）；新增 `new_comments_append_durable_ids_and_extensible_dates_without_touching_entries`（删除追加条目后与原部件逐字节相同、GUID 既有条目保留、durable 为十进制、重导入读回 durable 与 date_utc）、`a_new_comment_without_a_strict_utc_date_leaves_comments_extensible_untouched`（宽松日期：ids 仍追加、extensible 字节不变）、`a_fresh_package_without_extension_parts_gains_ids_and_extensible`（无现代扩展部件的包新建批注后获得两部件，关系与内容类型齐全，重导入 durable 一致）。未做：people.xml 身份、图形界面气泡、不透明表格内的批注锚点映射、仍带范围标记的段落整段重写、页眉批注导出专项测试、Word／WPS 人工交换。T302 保持 `in_progress`。见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T302 commentsIds 与 commentsExtensible 写回」。

2026-10-27 T303 开始：M3 最后一个未开始的实现任务由 `planned` 改为 `in_progress`（计数 13 done／22 in_progress／31 planned／0 blocked）。第一切片是无头语义比较引擎：全部接受=改稿正文、全部拒绝=原稿正文的投影不变量已有自动测试；人工验收与比较合并的其余部分仍属后续切片。见 T303 的完成证据与 [IMPLEMENTATION](IMPLEMENTATION.md)。

### T303：比较与合并

- 输入：原稿与目标稿的明确投影、语义差异及已有修订集合。
- [ ] 状态：`in_progress`
- 需求：`REVIEW-007`、`EDIT-011`
- 前置：`T300`、`T301`、`T302`
- 产物：语义比较、目标投影选择、可接受／拒绝的修订结果与合并冲突；本地版本历史的保留、浏览、比较与另存恢复。
- 错误路径：已有待决修订、无法安全表达或歧义差异要求明确处理，不猜测。
- 自动验证：A/B 比较后全部拒绝=A、全部接受=B；合并冲突及幂等性。
- 人工验收：比较两份合同并在 Word 审阅结果，检查格式和表格差异；与本地历史版本比较后另存恢复。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。
- 2026-10-27 第一切片（无头语义比较，任务仍 `in_progress`，不标 `done`）：引擎新增 `compare_documents(original, revised, author, date)`（`crates/joyword-engine/src/compare.rs`）——以正文段落文本的最长公共子序列对齐（对齐表预算 4M 格，超出显式拒绝），间隙按三类解析：仅改稿有的段落在幸存段末以跟踪插入新建（每次分节的尾段标记盖 `w:ins`，拒绝时并入后段并保持后段身份与格式）；仅原稿有的段落经跨段跟踪删除删除（标记集合为幸存段到最后被删段的前一段，接受链吸收空壳后恰好停在幸存分节处）；单删单增的间隙修剪公共前后缀后在段内做文字 diff（段落与标记不动）。多段对多段的间隙没有忠实的修订表示（一个分节必须在接受与拒绝时都消失），显式拒绝；文档开头的变化没有锚定幸存段，同样拒绝。为承载插入的段落标记，模型 `Paragraph::prepend`（镜像 `append`，保持 self 身份与格式）、引擎跟踪分节（`SplitParagraph` 在跟踪时给尾段标记盖 `w:ins`；头段标记已是 `w:ins` 时两处分节都保持插入态）与标记解决（`resolve_marks` 按类型×决议分派：接受的删除标记与拒绝的插入标记移除分节——删除保持前段身份、插入保持后段身份；容器末尾的空段插入标记拒绝时整段移除）均为本切片新增，`MarkScope` 匹配改为按 id。DOCX 侧 `w:pPr/w:rPr/w:ins` 类型化导入（恰好 id／author／可选 date 的空元素；富内容仍是保留 gap）与注入／剥离导出（`patch_mark_bytes` 按 kind 选 `w:del`／`w:ins`，剥离时两者都认）。比较输入的拒绝检查：任一侧有待决修订、移动标记、批注、表格、行内对象、脚注／尾注／文本框 story、页眉页脚文本不一致或分节数不同都显式拒绝（SPEC：不静默合并历史）。已知边界如实记录：段内 diff 保持原段格式，纯格式变化不在比较范围；比较结果含段落结构增删时 DOCX 导出尚不可用（ExportPlan 只补丁既有段落，结构改写拒绝），本切片只有段内 diff 结果的 DOCX 往返证据；原始视图对插入标记段不合并渲染（与 T402 删除标记的合并渲染同属排版已知边界）；文字级（词内）diff、格式修订比较、本地版本历史（EDIT-011）与比较合并的宿主接线是后续切片。证据：`crates/joyword-engine/tests/compare.rs` 十项——`identical_documents_compare_without_revisions`、`all_accept_reproduces_revised_and_all_reject_restores_original`、`a_deleted_paragraph_between_kept_ones_vanishes_on_accept`、`single_revisions_resolve_step_by_step`、`trailing_and_middle_deletions_absorb_the_empty_remainder`、`inserted_paragraphs_carry_inserted_marks_and_reject_joins_the_next`、`formatting_only_changes_are_a_recorded_boundary`、`one_paragraph_replaced_by_several_diffs_and_inserts`、`a_change_at_the_very_beginning_is_refused`、`pending_revisions_tables_comments_and_story_gaps_are_refused`；`crates/joyword-docx/tests/mark_revisions.rs` 三项新增——`an_inserted_paragraph_mark_imports_typed_and_round_trips`、`an_engine_created_inserted_mark_is_injected_and_a_resolved_one_is_stripped`、`an_in_paragraph_compare_result_round_trips_through_docx`（接受与拒绝双投影断言），并把 `w:ins` 从保留 gap 用例移入类型化用例（`a_malformed_or_rich_paragraph_mark_stays_a_preserved_gap`）。见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T303 第一切片：无头语义比较」。

### T304：审阅交换发布门槛

- 输入：已实现审阅能力、原创合同语料和固定版本的对照软件环境。
- [ ] 状态：`planned`
- 需求：`REVIEW-001`、`REVIEW-002`、`REVIEW-003`、`REVIEW-004`、`REVIEW-005`、`REVIEW-006`、`REVIEW-007`、`REVIEW-008`、`CLIP-003`、`QUALITY-003`
- 前置：`T301`、`T302`、`T303`、`T206`
- 产物：固定 Word／WPS／LibreOffice 审阅语料和往返报告。
- 错误路径：参考软件或字体缺失不得按跳过计通过；任何静默丢失阻断阶段。
- 自动验证：批量语义、关系、部件保留与打印投影测试。
- 人工验收：按语料执行跨软件接受／拒绝和批注回复，保存证据。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

## M4：原生 Agent

退出条件：选区授权、提案预览、AI 修订、过期拒绝和一键关闭通过。

### T400：Provider 与关闭基础

- 输入：AI 专题权限约束、生成内容标识的法律评估结论、本地／云 Provider 配置和可记录假服务。
- [ ] 状态：`in_progress`
- 需求：`AI-001`、`AI-005`、`QUALITY-007`
- 前置：`T101`
- 产物：独立 Provider、配置、系统凭据、默认关闭、管理员策略（强制关闭、Provider 白名单）及 generation 权限门。
- 错误路径：缺失密钥、断网、限流和关闭中的任务失败不影响文档。
- 自动验证：假 Provider 契约、无 AI 构建、策略禁用、启动零请求及凭据脱敏。
- 人工验收：首次启动离线编辑，配置服务后开启／关闭，检查入口与任务。
- 完成证据：进行中（2026-10-21）。独立 crate `joyword-agent` 默认关闭，含 generation 线性化点、假 Provider 契约、策略/凭据接口（操作系统后端为 stub）与启动零请求测试；shell/app 的 `ai` feature 默认不启用。`--no-default-features` 下核心编辑 crate 可构建。人工验收未做，不标完成。详见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T400 Provider 与关闭基础」。

### T401：受限工具与授权

- 输入：用户授予的选区能力、带版本快照、工具模式和恶意文本样例。
- [ ] 状态：`in_progress`
- 需求：`AI-002`、`AI-004`
- 前置：`T400`、`T100`
- 产物：选区授权、结构读取、受限检索、定位引用及工具白名单。
- 错误路径：文档提示注入、越界范围、隐藏修订和其他文件访问拒绝。
- 自动验证：最小上下文、注入语料、过期授权和能力边界测试。
- 人工验收：对合同选区提问，验证引用和实际发送内容不超范围。
- 完成证据：2026-10-22 第二切片（[#83](https://github.com/willmove/joyoffice/pull/83)）：状态由 `planned` 改为 `in_progress`，进度计数随之改为 12 done／21 in_progress／33 planned／0 blocked（与 T301 同批）。改动范围仅限 `crates/joyword-agent/**`，没有改 shell、apps、docs、scripts 或 CI；该 crate 按 LAYERS 约束不依赖任何 document／engine／network crate，所以文档访问经抽象端口完成。新增 `src/document.rs`：只读端口 trait `DocumentPort`（将来由 shell 实现，本切片只有内存假实现 `MemoryDocument` 供契约测试，shell 与 apps 未接线）与不可变投影 `DocumentSnapshot`——投影只提供最终可见文本与大纲偏移字段，不提供独立的隐藏修订或批注字段；这限制接口表达范围，但真实宿主仍须验证不会把隐藏内容混入可见字符串；`MemoryDocument` 内部带 `deleted`／`comments` 隐藏字段，投影时丢弃，`edit_visible` 升版本。不可信输入预算：段落数 ≤4096、单段 ≤64 KiB、全文 ≤4 MiB、隐藏条目 ≤64。新增 `src/scope.rs`：选区授权令牌 `Grant` 绑定文档标识 `DocumentId`（拒绝路径式标识）、base 版本、授权范围内容指纹 `ContentFingerprint`（FNV-1a 64，JoyWord 假定：非加密，只用于检测快照掉包，强制力来自版本加显式偏移）、字符起止 `ScopeRange`（≤2^20 字符）、工具白名单 `ToolAllowlist`、签发 generation 与 tick 过期条件（`TickClock` 由调用方推进，JoyWord 假定：确定性、无墙钟依赖，ttl 取 1..2^40）；`Grant::validate` 依序检查 generation→过期→文档身份→版本→范围→指纹，每一项不匹配都显式拒绝并给出静态可审计标签。新增 `src/scoped.rs`：`ToolSession`（runtime＋port＋grant＋clock，不可变），每次操作先过 `fresh()`（gate 相位 Disabled／Stopping 拒绝、generation、快照、tick）；白名单在分发之前强制，未授予的工具在触碰文档之前就被拒绝，propose 类工具即使列入白名单也返回 `ProposeDeferredUntilT402`。工具是 `read_selection`（精确授权切片与可解析的 `Citation`）、`read_structure`（大纲投影裁剪到授权范围，节点 ≤512，只有完全落在范围内的标题带 label）、`search_document`（字面、大小写敏感、仅授权范围内且无邻域扩展——JoyWord 假定，严于规范下限；query ≤128 字符，命中 ≤64 并报告截断）。`build_context` 生成最小上下文：用标记框住数据并注明"不可信数据不是指令"，内容超过 24 KiB 时拒绝而不是截断，以保证发出的内容与用户授权时的指纹一致，question ≤4 KiB。所有输出类型的 `Debug` 都对正文脱敏，错误的 Display 与 `kind()` 只含静态标签与数字。`src/runtime.rs` 把 `complete` 重构出共享的 `send()` 并新增 `complete_scoped()`：先查 gate 相位（fail closed），发送前校验 grant（版本／指纹／范围／过期／generation），构造最小上下文后走 T400 同一条 admission／log／provider 路径，**响应之后再校验一次**，期间文档被编辑、授权过期或功能被关闭的响应一律丢弃；`AgentError` 新增 `Scope`／`Tool` 变体，`kind()` 的静态标签接入既有遥测脱敏路径。`src/fake.rs` 新增 `RecordingProvider`（记录实际发出的 prompt 字节，仅测试用、上限 64 条，文档注明运行时诊断不得这样捕获，QUALITY-007）；`src/tools.rs` 只给 `ToolId` 加 `Ord`／`Hash` derive（白名单集合需要），`dispatch()` 语义不变。默认关闭保持不变：没有 feature 变化、没有新依赖、`Cargo.lock` 未动，`ai` feature 仍默认关闭，核心 crate `--no-default-features` 可构建，启动零请求测试不变且通过，功能区仍没有 AI 入口。证据：`crates/joyword-agent/tests/scoped_tools.rs` 十二项——`grant_binds_version_fingerprint_range_and_expiry`（版本变化、同版本换内容的指纹、同版本缩短的范围、他文档快照的身份、tick 过期、非法 ttl 全部显式拒绝）、`tool_allowlist_refuses_unlisted_and_propose_tools`（未授予工具拒绝，全白名单下 propose 仍 DeferredUntilT402，`document_mutations` 为 0）、`read_selection_returns_only_the_authorized_range`、`read_structure_projects_in_range_outline_with_citations`、`search_confines_hits_and_citations_resolve_exactly`（范围外同词不命中、隐藏内容不可检索、空与超长 query 拒绝、命中上限 64 且报截断）、`minimal_context_sends_only_authorized_bytes`（`RecordingProvider` 记录的实际发出字节等于 `build_context` 输出，不含范围外正文、隐藏删除修订与批注，日志只含字节数）、`injected_document_text_cannot_change_permissions_or_leak`（中英文权限覆写指令、伪造工具调用 JSON、伪造系统消息、伪造上下文结束标记都只当数据，白名单／范围／版本不变，单请求，发出内容不越界）、`other_documents_paths_and_out_of_scope_reads_are_refused`、`kill_switch_and_reenable_invalidate_the_session`（关闭后读取与请求均拒绝且零新请求，重开后旧 grant 是 stale generation，新 generation 重新授权可用）、`responses_after_edit_or_expiry_are_discarded`（调用中被编辑→scope-version-changed，调用中过期→scope-expired，遥测只记静态标签，`upload_attempts` 为 0）、`diagnostics_and_debug_redact_body_text`、`budgets_bound_untrusted_document_input`（超大段落、隐藏条目、上下文 >24 KiB、question >4 KiB、结构节点 >512 全部确定性拒绝且零 Provider 调用）。切片内自动验证通过：`cargo test -p joyword-agent --locked --offline` 30 项（18 旧＋12 新）、`cargo test -p joyword-shell --features ai --locked --offline --test ai_toggle`、`cargo check -p joyword-engine -p joyword-model -p joyword-docx -p joyword-layout -p joyword-shell --locked --offline --no-default-features`、`cargo clippy -p joyword-agent --all-targets --locked --offline -- -D warnings`、`cargo clippy -p joyword-shell --all-targets --features ai --locked --offline -- -D warnings`、`cargo test --locked`（CI 口径）、`cargo fmt --all -- --check`、`python scripts/check_docs.py`、`check_gates.py`（含反例）、`check_acceptance.py`，以及 CI「AI off by default」专段；`python -m unittest discover -s scripts/tests` 有 1 项环境性失败（`test_committed_bytes_and_manifest_hashes_match`：本机 Python 3.14.7 链接 zlib-ng 1.3.1，级别 6 的 deflate 字节与 CI 的 stock zlib 不同，5 个 DOCX-FEAT 夹具的压缩字节哈希因此不一致，解压后的成员名、内容与 CRC 完全相同，在干净 main 上同样失败；不是 `core.autocrlf` 造成的，`.gitattributes` 已把 `*.docx` 标为 binary）。未覆盖与未做：人工验收（对合同选区提问、核对引用与实际发送内容不超范围）没有执行，没有启动图形界面；"明确邻域"检索未实现，本切片收紧为仅授权范围；`propose_*` 工具仍只有名字加拒绝（T402），`ChangeProposal.apply` 语义未动；取消沿用 T400 骨架，T403 完整化；`DocumentPort` 还没有真实 shell 实现，`MemoryDocument` 只是测试夹具；内容指纹是 FNV-1a 64、非加密，若威胁模型需要防构造碰撞，后续可换 workspace 已钉扎的 sha2（本切片被禁止新增依赖，未引入）；流式响应（当前是一次性 complete）未涉及；信创真机未验证；AIGC 标识的法律评估仍是 TODO，本切片不写显式或隐式标识，也不声称合规。因此不标 `done`，T401 保持 `in_progress`。详见 [IMPLEMENTATION](IMPLEMENTATION.md)。
- 2026-10-23 合并前加固（`7a453a2`，仍属 #83）：修复公开读取入口绕过工具白名单、端口快照期间停用／重启的迟到结果、前导空段坐标丢失、假文档追加未升版本、签发授权未核对返回文档 ID、tick 回绕与期限上限。新增 17 个回归；协调者独立复跑 agent 47 项测试、fmt、clippy 和加固门禁通过。真实 shell 适配与人工验收仍未做，不改任务状态；详见 [合并前授权加固](IMPLEMENTATION.md#t401-合并前授权加固2026-10-23)。

### T402：修改预览与 AI 修订

- 输入：可验证 ChangeProposal、目标指纹、授权范围及审阅事务。
- [ ] 状态：`in_progress`
- 需求：`AI-003`、`AI-004`
- 前置：`T401`、`T300`
- 产物：结构化 ChangeProposal、差异预览、AI 作者与单事务提交。
- 错误路径：目标指纹／版本不匹配、无效格式命令或重复提案不部分提交。
- 自动验证：重复提交防护、预览不修改原稿、提交与撤销、审阅往返。
- 人工验收：选区改写／格式调整，逐项接受并整体撤销。
- 完成证据：2026-10-23 第一切片 [PR #89](https://github.com/willmove/joyoffice/pull/89)（合并提交 `6c43799`）：单个非空授权选区的纯文本替换提案、私有授权与 runtime 绑定、只读前后文预览；每次预览重新校验授权、版本、指纹、期限与 generation，独立有界 ID 表防重，内容与说明按 UTF-8 字节预算，诊断不含正文。独立审查无阻塞发现；`joyword-agent` 的 47 项既有测试、15 项新增测试与 1 项 compile-fail、全目标 clippy 与 fmt 均通过；该 PR 的 push 与 PR 两套 CI 各 9/9 成功。尚无真实宿主与 Provider 接线、engine 提交与撤销、AI 作者修订与 DOCX 往返，旧 `ChangeProposal.apply` 仍明确阻断；拥有的预览只是授权时点快照，不承诺追溯撤回。GUI 与 Word／WPS 人工验收未做，整个 T402 不标完成。
- 2026-10-23 接受切片（[PR #94](https://github.com/willmove/joyoffice/pull/94)，任务仍 `in_progress`，进度计数不变）：在 #89 的提案与只读预览之上增加接受路径。`ToolSession::accept_replace` 要求 `ProposeReplace`，先核对提案与当前授权、runtime 的绑定，再占用提案 ID；随后重新校验门控相位、generation、文档标识、版本、范围、指纹与期限。过期、关闭、停用、错误 generation、版本或指纹变化、越界与跨段都拒绝，ID 退回可重试，文档零写入。成功时宿主 `DocumentCommit::commit_replace` 在写入前调用 `authorize`，engine 的 `apply_ai_text_replace` 把整个授权选区做成一次 `Origin::Agent`、`UndoGroup::Standalone` 事务，插入与删除都盖 `JoyWord AI` 作者修订（即使文档未开修订跟踪），一次撤销恢复原文。同一 `proposal_id` 在途或已接受时返回 `AlreadyApplied`，并发接受只写入一次；失败后的一次重试可以提交。预览仍不改文档，旧 `ChangeProposal::apply` 仍返回 `DeferredUntilT402`，`allowed_commits` 不增加。生产 `joyword-agent` 依赖保持为空，只在 dev-dependencies 里为集成测试依赖 engine 与 model（`DEV_LAYERS`）。本机 `cargo test -p joyword-agent --locked --offline` 74 项（含 11 项新接受测试与 1 项 compile-fail）、`cargo test -p joyword-engine --locked --offline` 94 项（含 6 项新 `ai_replace` 测试）、两个 crate 的全目标 clippy `-D warnings`、`cargo fmt --all -- --check`、`python3 scripts/check_docs.py` 与 `python3 scripts/check_gates.py` 通过。未做：宿主 UI、真实 Provider、DOCX／Word 修订往返、AIGC 文件标识、跨段替换、选区内隐藏修订或行内对象、非段落正文块、人工验收。整个 T402 不标完成。详见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T402 授权替换的单事务接受」。
- 2026-10-23 第三切片（无 PR，按用户指令直接并入 main；任务仍 `in_progress`，进度计数不变）：补齐 #94 记录的两个未做项——跨段替换与 AI 修订的 DOCX 往返。模型 `Paragraph` 新增 `mark_revision`（`w:pPr/w:rPr/w:del` 的空元素形态，仅 Delete）；engine 的 `delete_tracked` 允许同一容器内跨段跟踪删除：覆盖文本逐段标 Delete、首段到末段前的段落标记逐个盖同一修订 id 的 Delete，接受时按 Word 语义物理合并（拒绝则换行回来），`AcceptAll`／`RejectAll` 全量处理；T301 的 `RevisionBoundary` 从"不跨段"收窄为"不跨表格／内容控件"，跨故事报 `CrossContainer`。`apply_ai_text_replace` 支持跨界范围（含纯分隔符删除＝合并两段），隐藏的他作者删除被覆盖时保留不动，行内对象拒绝；`body_final_paragraphs` 按删除标记合并相邻段成组，agent 接受检查放宽为整篇最终视图内，内存宿主按拼接＋拆回实现塌缩。DOCX 导入把恰好 id／author／可选 date 的空 `w:del` 标记类型化（文档不锁定；`w:ins`、move、多修订、带子元素仍保留 gap），导出在复制／合成的 `w:pPr` 字节里注入或剥离 `w:rPr/w:del`，对照器 `rebuild` 携带全部修订关闭了往返校验的既有漏洞；端到端测试（导入→engine AI 跨段替换→导出→重导入）断言语义一致与 `JoyWord AI` 作者齐全，`joyword-docx` 为此新增 dev 依赖 `joyword-engine`（登记 `DEV_LAYERS`）。已知边界如实记录：布局仍逐段排版，最终视图中删除标记的相邻段渲染为独立行（模型、投影与 DOCX 精确，合并渲染留待后续）；替换文本仍限单行；`w:ins` 标记不类型化。本机 `cargo test --locked --offline` 998 项全过、workspace clippy `-D warnings` 与 fmt 干净、`--no-default-features` 六 crate 构建通过、gates／docs／acceptance 通过、scripts 单测 107 项中 106 过（唯一失败为已登记的 zlib-ng 环境差异，干净 main 同样失败）。GUI 与 Word／WPS 人工验收、宿主 UI、真实 Provider、AIGC 标识未做，整个 T402 不标完成。详见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T402 跨段替换与段落标记修订」。

### T403：取消、竞争与离线门槛

- 输入：在途流式请求、关闭 generation、过期响应与故障 Provider。
- [ ] 状态：`planned`
- 需求：`AI-001`、`AI-002`、`AI-003`、`AI-004`、`AI-005`、`QUALITY-007`
- 前置：`T402`、`T304`
- 产物：任务取消、连接释放、自有进程结束、迟到结果丢弃和关闭验证报告。
- 错误路径：不响应取消的服务、关闭与提交竞争、重新开启旧任务均有确定行为。
- 自动验证：记录假服务请求；关闭后请求／提交为零；无 AI 与有 AI 核心回归。
- 人工验收：在流式生成和提交前关闭，继续普通输入、保存及打印。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

## M5：专业文档

退出条件：长报告、论文、公文、模板和复杂布局工作流通过。

### T500：复杂分页与长文档

- 输入：分页约束图、复杂分节／对象／表格语料及稳定布局缓存。
- [ ] 状态：`in_progress`
- 需求：`LAYOUT-012`、`LAYOUT-013`、`LAYOUT-005`、`LAYOUT-006`、`LAYOUT-010`
- 前置：`T204`
- 产物：多栏、连续及奇偶分节、跨页／嵌套表格、浮动绕排与增量分页。
- 错误路径：过大对象、约束冲突与迭代不收敛返回可定位诊断。
- 自动验证：最小约束样例、稳定页尾复用、几何对照和长文档取消。
- 人工验收：百页报告改首段／末段、改变方向与图文绕排，观察定位与响应。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。
- 2026-10-27 第一切片（任务由 `planned` 改为 `in_progress`，没有人工验收，不标 `done`）：T500 的产物项多数已由 M2 各任务落地（等宽／不等宽分栏与栏分隔线、连续／奇偶／下一栏分节、跨页表格与表头重复、一层嵌套表、页面相对浮动绕排、增量分页与段落缓存），本切片补上还没有的三件自动化证据并修复一个已知性能缺口。其一，稳定页尾复用恢复到带栏分隔线的文档：T202 栏分隔线切片曾记录「带分隔线的文档失去后缀复制复用，退回完整放置」——根因是 `suffix_copies` 的完整性校验按源页最终 rects 数量比较，而分隔矩形由 `finish_stories` 在组装之后追加、复制时不重放。现在 `pages.rs` 把分隔矩形计算提取为纯函数 `separator_rects(section, geometry)`（绘制与计数共用同一几何，无第二套实现），增量路径在构造 `PlaceStop` 时按当前节规格对上一快照逐页计算分隔矩形数并存入 `separator_counts`，`suffix_copies` 的校验改为「复制页 rects + 该页分隔数 == 源页 rects」，重放之外恰好允许这批故事级矩形；`finish_stories` 之后仍统一重画分隔线，复制页不携带旧墨。节规格在两次排版之间变化时计数与源页不符，校验失败退回完整放置，不复制陈旧墨迹。证据：`long_document_edits_reuse_fits_and_the_stable_page_tail`——1500+ 段、65+ 页、含横向节（页面锚定浮动图）与带分隔线双栏节的长文档，未变重排 `shape_calls` 为 0 且后缀从锚点整体复制（`stable_boundary` 有值，栏分隔页在复制范围内）；改首段只重排该段（其余段全部缓存命中，增量与全量逐页一致）；改末段无后缀复制（`stable_boundary` 为 None）、仅该段重排（缓存命中恰为其余全部段）且与全量一致。其二，长文档取消与规模：`a_hundred_page_mixed_section_document_lays_out` 构造 100+ 页混合文档（纵向正文＋41 行跨页表头重复表格→横向节浮动绕排图→奇偶分节双栏＋栏分隔线），断言页数、横纵页框、表头多页重复、分隔线两侧有文字与浮动图落页；`a_cancelled_hundred_page_layout_returns_without_publishing_pages` 在该规模上复验取消令牌的单调性与 `LayoutError::Cancelled`（协作检查点在既有代码中按块粒度存在，应用层新旧作业替代已有 `stale_layouts_lose_to_newer_requests_including_a_to_b_to_a`）。其三，几何对照：`a_mixed_section_document_shares_one_geometry_with_the_pdf`（joyword-pdf）对混合分节样例断言屏幕 PageScene 与 PDF 消费同一次布局——每个 run 原点在 PDF 文本矩阵 0.01 pt 内。未做：人工验收（百页报告改首段／末段、改变方向与图文绕排的人工观察）；含表格文档在增量路径仍整体走完整放置（表行跨页与表头重复使段落级后缀复制不适用，缓存命中与页尾复用因此不覆盖含表文档，是如实记录的已知边界）；`suffix_copies` 对含页面锚定浮动图的页保守不复制（图不是行级图像，复制页校验只重放行级内容，本切片不扩大复制范围）；金样场景（golden-scenes harness）未新增 t500 场景。详见 [IMPLEMENTATION](IMPLEMENTATION.md) 的「T500 第一切片」。

### T501：注释、域与引用体系

- 输入：域代码、字段缓存、引用目标、脚注 story 和页码布局结果。
- [ ] 状态：`planned`
- 需求：`LAYOUT-007`、`STYLE-002`
- 前置：`T500`
- 产物：脚注尾注、目录、题注、交叉引用、文献、索引与域代码／缓存。
- 错误路径：循环引用、缺失目标和迭代页码不能无限布局或冻结错误结果。
- 自动验证：字段求值、引用更新、脚注分页和循环诊断测试。
- 人工验收：长报告插入章节与图表后更新目录、页码和交叉引用。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T502：公式、形状和表单内容

- 输入：公开公式／DrawingML／控件规范、对象语义和保留资源。
- [ ] 状态：`planned`
- 需求：`LAYOUT-006`、`LAYOUT-007`、`FORMAT-011`、`FORMAT-010`
- 前置：`T500`
- 产物：公式结构、常见形状／文本框、内容控件和模板编辑。
- 错误路径：未知公式／对象保留；受限控件不能通过粘贴或 AI 绕过。
- 自动验证：OMML 子集语义、对象关系、控件限制与文件往返。
- 人工验收：编辑带公式和表单的报告，与 Word 交换并修改。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T503：语言工具与视图

- 输入：许可明确的词典与语言数据及视图投影。
- [ ] 状态：`planned`
- 需求：`EDIT-008`、`PLATFORM-003`
- 前置：`T200`、`T501`
- 产物：离线拼写与语言工具、草稿／大纲／阅读视图。
- 错误路径：缺字典、语言不支持和隐私数据不自动上传。
- 自动验证：拼写与语言规则、视图不改变文档及无 AI 测试。
- 人工验收：离线检查中英文拼写，切换草稿、大纲和阅读视图并核对文档未改变。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T504：中文高级版式与公文

- 输入：中文排版规范、GB/T 9704、固定字体和中文样例。
- [ ] 状态：`planned`
- 需求：`LAYOUT-002`、`LAYOUT-011`、`PLATFORM-004`
- 前置：`T500`、`T501`、`T207`
- 产物：拼音指南、双行合一、合并字符、带圈字符、稿纸与 GB/T 9704 公文版式样例；双向与复杂文字保真保留并提示排版受限。
- 错误路径：缺字形、规定字体缺失、语言特性不支持或平台映射异常明确诊断。
- 自动验证：固定字体的 Unicode、cluster、几何和光标测试，公文样例行字网格对照。
- 人工验收：公文、拼音和中文版式样例对照与编辑，在 Word 与 WPS 中检查。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

## M6：格式与保护

退出条件：转换路径有明确矩阵和损失报告；受保护及异常文件安全处理。

### T600：RTF 与 Strict OOXML 交换

- 输入：RTF／Strict OOXML 公开规范、文档模型、审阅子集和 ExportPlan。
- [ ] 状态：`in_progress`
- 需求：`FORMAT-006`、`FORMAT-003`、`FORMAT-011`
- 前置：`T301`、`T302`、`T501`
- 产物：RTF 文件读写、Strict profile 独立读写和元素级损失分析。
- 错误路径：未知属性、修订映射缺口和复杂对象不能静默扁平化。
- 自动验证：规范样例、双向语义、资源关系、损失报告和异常输入。
- 人工验收：与 Word／WPS 交换带样式、脚注、修订的 RTF 与 Strict 文件。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。
- 2026-10-28 第一切片（任务由 `planned` 改为 `in_progress`，进度计数改为 13 done／26 in_progress／27 planned／0 blocked）：实现 `crates/joyword-rtf`，支持带不可信输入资源预算（`RtfBudget`）与损失报告（`LossReport`）的标准 RTF 1.9 读写器。支持组嵌套作用域栈、控制词与控制符分发、Unicode `\uN?` 解析与 `\uc` 替代字符跳过、可忽略目标 `\*` 过滤，以及段落、运行、表格行与单元格转换。执行 `cargo test -p joyword-rtf` 4 项测试全部通过。Word/WPS 人工交换未做，标未验证，见 [T600 证据](verification/t600-rtf.md)。

### T601：HTML、Markdown 与文本

- 输入：HTML／Markdown／TXT 明确方言、编码、导入资源与目标损失策略。
- [ ] 状态：`in_progress`
- 需求：`FORMAT-006`、`FORMAT-003`、`CLIP-001`
- 前置：`T203`、`T501`
- 产物：受控 HTML／Markdown／TXT 文件双向转换和编码诊断。
- 错误路径：脚本／外链、错误编码、目标表达不足产生明确诊断。
- 自动验证：清理、读取顺序、编码、语义子集和转换损失测试。
- 人工验收：导入网页片段／Markdown，导出文本后核对丢失说明。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。
- 2026-10-28 第一切片（任务由 `planned` 改为 `in_progress`，进度计数改为 13 done／25 in_progress／28 planned／0 blocked）：实现 `crates/joyword-textfmt`，支持带有结构化损失报告（LossReport）的 TXT、Markdown（CommonMark + GFM 表格子集）与受控 HTML5 双向转换。支持编码探测（UTF-8、BOM、UTF-16LE/BE）与 CRLF/LF 换行规范化；过滤剥离 `<script>`、`<iframe>` 与 `javascript:` 恶意外链。执行 `cargo test -p joyword-textfmt` 3 项测试全部通过。人工网页片段与 Markdown 导入比对未做，标未验证，见 [T601 证据](verification/t601-textfmt.md)。

### T602：旧版 DOC 导入

- 输入：MS-DOC／CFB 规范、合法原创二进制样例及资源预算。
- [ ] 状态：`planned`
- 需求：`FORMAT-007`、`FORMAT-002`、`QUALITY-005`、`QUALITY-009`
- 前置：`T103`、`T501`
- 产物：依据 MS-DOC／CFB 自研导入（CFB 读取供 T604 加密 OOXML 复用）、原始结构保留及支持变体矩阵。
- 错误路径：损坏链、加密变体、未知对象和过大记录拒绝或安全只读。
- 自动验证：二进制结构最小样例、模糊输入、内容关系和对照导入。
- 人工验收：固定 Word 生成旧 DOC，导入后与原稿语义和版面比较。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T603：PDF 导航、标签与页面图片

- 输入：稳定页面场景、文档导航结构和图片目标配置。
- [ ] 状态：`in_progress`
- 需求：`FORMAT-008`、`LAYOUT-009`
- 前置：`T204`、`T501`
- 产物：PDF 书签、链接与结构标签，页面图片输出。
- 错误路径：字体不可嵌入、标签缺失或链接失效时明确诊断，不声称符合任何 PDF 标准。
- 自动验证：文本抽取、阅读顺序、链接、标签结构和图像尺寸测试。
- 人工验收：两个 PDF 阅读器检查导航、可检索性与结构。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。
- 2026-10-28 第一切片（任务由 `planned` 改为 `in_progress`，进度计数改为 13 done／27 in_progress／26 planned／0 blocked）：实现 `crates/joyword-pdf` 中的 Tagged PDF（ISO 32000-1 §14.7/§14.8）逻辑结构标签生成，支持 `/StructTreeRoot`、`/MarkInfo`（`Marked: true`）及 Artifact 饰品分离。扩展 PDF 结构检测器 `check.rs` 回读并断言标签与结构树；结合 `joyword-cli render` 验证确定性 CPU 页面图片输出。`cargo test -p joyword-pdf` 42 项测试与 `joyword-cli visual` 测试全部通过。屏幕阅读器与外部阅读器人工标签交互未做，标未验证，见 [T603 证据](verification/t603-pdfnav.md)。

### T604：加密、保护与活动内容

- 输入：加密／签名／保护规范、测试密钥、宏容器与不可信文件语料。
- [ ] 状态：`in_progress`
- 需求：`FORMAT-009`、`QUALITY-009`、`FORMAT-003`
- 前置：`T103`、`T300`、`T602`
- 产物：加解密、编辑限制、签名识别与修改即失效提示、宏容器保留、受限解析和来源标记。
- 错误路径：密码错误、签名失效、外链和活动对象不执行；跨格式移除明确告知。
- 自动验证：密码／保护独立测试、签名改动、ZIP／XML／图像预算及无网络验证。
- 人工验收：打开受保护和宏文档，编辑安全区域、另存并检查状态。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。
- 2026-10-28 第一切片（任务由 `planned` 改为 `in_progress`，进度计数改为 13 done／24 in_progress／29 planned／0 blocked）：实现 `crates/joyword-protect`，包含纯自研 FIPS 180-4 SHA-1／256／512、FIPS 197 AES-128／256-CBC、RFC 4231 HMAC-SHA256／512、常数时间比对；支持 `w:documentProtection` 密码哈希校验（ECMA-376 Part 1 §17.15.1.29）；活动内容与外部外链（UNC、本地协议）防御与审计，宏容器保真保留；XML-DSig 数字签名被签部件散列篡改检测；基于 `joyoffice-cfb` 解析 MS-OFFCRYPTO Agile 加密包并完成解密与往返测试。执行 `cargo test -p joyword-protect` 5 项测试全部通过。人工在 Word/WPS 中打开受保护文档并编辑交换未做，证书链信任验证不在范围内，标未验证，见 [T604 证据](verification/t604-protection.md)。

### T605：WPS 文字原生格式导入

- 输入：MS-DOC／CFB 规范、T602 的 DOC 导入、固定版本 WPS 生成的原创 .wps／.wpt 样例及资源预算。
- [ ] 状态：`planned`
- 需求：`FORMAT-014`、`FORMAT-002`、`QUALITY-009`
- 前置：`T602`
- 产物：.wps／.wpt 类型识别、DOC 兼容内容导入、WPS 私有流原样保留、模板新建、另存为 DOCX 的损失计划及支持变体矩阵。
- 错误路径：早期非 CFB 私有变体、加密、损坏链和未知私有流明确诊断或安全只读，原件不被覆盖。
- 自动验证：样例导入语义、私有流散列保留、模糊输入及资源预算测试。
- 人工验收：用固定版本 WPS 生成含样式、表格、批注和修订的 .wps，导入后与 WPS 中的显示和语义比较。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

## M7：稳定版 1.0

退出条件：三端真实运行、性能、恢复、可访问性、安装和发布门槛全部通过。

### T700：三端及信创产品验收

- 输入：全部 1.0 承诺范围、兼容矩阵、三端及信创原生测试环境和发布候选。
- [ ] 状态：`planned`
- 需求：`PLATFORM-001`、`PLATFORM-002`、`PLATFORM-009`、`CLIP-004`、`QUALITY-003`、`QUALITY-008`
- 前置：`T005`、`T403`、`T502`、`T503`、`T504`、`T600`、`T601`、`T602`、`T603`、`T604`、`T605`
- 产物：Mac／Windows／Linux 的全流程对照报告，Wayland／X11 分列，统信 UOS 与银河麒麟按 x86_64／ARM64／LoongArch64 分列。
- 错误路径：任一平台未测不能用其他平台或编译成功代替。
- 自动验证：三端核心、格式、审阅及平台适配测试集合。
- 人工验收：逐平台运行新建、编辑、交换、打印、恢复和关闭 AI。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T701：输入法与可访问性

- 输入：命令注册表、输入及可访问适配、系统 IME 与屏幕阅读器。
- [ ] 状态：`planned`
- 需求：`PLATFORM-004`、`PLATFORM-005`
- 前置：`T700`
- 产物：结构化可访问树、焦点导航、高对比度、IME 与多显示器专项。
- 错误路径：缺失角色、不可达命令、候选框偏移和阅读顺序错误阻断验收。
- 自动验证：键盘焦点、语义树、范围与 DPI 自动检查。
- 人工验收：VoiceOver、Narrator、Linux 辅助技术和中文 IME（含信创系统的 Fcitx 输入法）真实操作。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T702：性能、故障和恢复

- 输入：固定 10／100／1000 页语料、性能基准环境和故障注入方案。
- [ ] 状态：`planned`
- 需求：`LAYOUT-010`、`QUALITY-004`、`QUALITY-005`、`QUALITY-006`
- 前置：`T700`
- 产物：固定三级语料、分位数／RSS／峰值／体积报告、故障回归。
- 错误路径：卡死、无界内存、布局不收敛或恢复丢失不以跳过测试掩盖。
- 自动验证：性能采样、首尾段重排、取消、fuzz 和故障注入。
- 人工验收：在固定硬件反复运行长文档场景，核对原始样本与统计。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T703：安装、签名和升级

- 输入：发布候选、签名凭据引用、安装元数据、迁移及回退版本。
- [ ] 状态：`planned`
- 需求：`PLATFORM-006`、`PLATFORM-009`、`QUALITY-002`
- 前置：`T700`
- 产物：三端安装器与信创 deb 软件包、文件关联、代码签名与统信／麒麟软件包签名、卸载、更新回退与日志迁移。
- 错误路径：无签名凭据、迁移失败、更新中断或磁盘不足保留可恢复版本。
- 自动验证：安装／卸载、升级中断、旧日志迁移、依赖通知和资源许可检查。
- 人工验收：干净机器安装、升级与回退，确认用户文档未改动。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T704：1.0 发布门槛与公开矩阵

- 输入：通过的阶段证据、已知限制、来源记录和真实兼容矩阵。
- [ ] 状态：`planned`
- 需求：`QUALITY-001`、`QUALITY-002`、`QUALITY-003`、`QUALITY-007`、`QUALITY-008`
- 前置：`T701`、`T702`、`T703`
- 产物：兼容矩阵、限制、证据、发布说明、SBOM 和隐私默认值。
- 错误路径：未通过的承诺项阻断稳定版；不能只改版本名。
- 自动验证：完整追踪、发布清单、无静默丢失语料和双构建验证。
- 人工验收：按公开说明从安装到合同审阅执行端到端验收。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

## M8：1.0 之后的扩展

退出条件：按功能组验证新增能力，保留和预览不能标作完整编辑。

### T800：DOC 写出

- 输入：MS-DOC 写出规范、导入语义、真实参考环境与损失分析。
- [ ] 状态：`planned`
- 需求：`FORMAT-013`、`FORMAT-003`
- 前置：`T602`、`T704`
- 产物：MS-DOC 指定变体写出和逐元素损失计划。
- 错误路径：不可表达结构、加密／对象变体明确拒绝或要求另存。
- 自动验证：二进制有效性、Word 重开与内容／修订对照。
- 人工验收：保存旧 DOC 并在指定 Word 版本继续编辑。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T801：图表与 SmartArt 原生编辑

- 输入：公开图表／SmartArt 格式规范、数据模型、对象布局及保留样例。
- [ ] 状态：`planned`
- 需求：`FORMAT-010`、`LAYOUT-006`
- 前置：`T502`、`T704`
- 产物：按类型逐项实现图表／SmartArt 模型、编辑与资源交换。
- 错误路径：未知类型仍保留；关系或布局不足不能伪装可编辑。
- 自动验证：对象语义、资源、撤销、排版与往返测试。
- 人工验收：编辑真实图表数据和布局，Word 重开检查。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T802：稀有域与嵌入对象

- 输入：用户需求优先级、稀有域／对象规范、保留数据与最小样例。
- [ ] 状态：`planned`
- 需求：`FORMAT-010`、`FORMAT-002`
- 前置：`T501`、`T604`、`T704`
- 产物：扩展域和复杂对象的安全预览、保留和已声明的编辑子集。
- 错误路径：活动对象不执行；外链不自动访问；未知结构保护。
- 自动验证：对象保留、字段求值边界、关系及资源预算。
- 人工验收：对特殊文档操作并确认每个对象的实际支持等级。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T803：高级签名与复杂模板

- 输入：高级签名／模板规范、用户证书和受保护模板测试环境。
- [ ] 状态：`planned`
- 需求：`FORMAT-010`、`FORMAT-009`
- 前置：`T503`、`T604`、`T704`
- 产物：指定标准的签名创建／验证、证书选择和复杂模板能力。
- 错误路径：凭据不可用、证书无效、修改签名内容与模板限制有明确行为。
- 自动验证：签名验证、篡改失效、模板实例化与原件保护。
- 人工验收：用独立验证器检查签名，在 Word 检查模板实例。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T804：PDF 重建与 OCR

- 输入：合法 PDF／扫描样例、OCR 模型来源、可见重建误差标准。
- [ ] 状态：`planned`
- 需求：`FORMAT-010`、`FORMAT-003`
- 前置：`T603`、`T704`
- 产物：近似文档重建、OCR 置信度、阅读顺序及可定位误差说明。
- 错误路径：扫描质量差、布局歧义或权限限制不能输出无提示的确定结果。
- 自动验证：原创 PDF／扫描语料、文字与结构误差、无损原件保存。
- 人工验收：将重建结果与原 PDF 并排审校，确认用户能发现不确定区域。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T805：ODT 文件交换

- 输入：ODF 1.3／1.4 公开规范、文档模型、审阅子集和 ExportPlan。
- [ ] 状态：`planned`
- 需求：`FORMAT-005`、`FORMAT-003`
- 前置：`T600`、`T704`
- 产物：ODF 1.3 双向、1.4 扩展和元素级损失分析。
- 错误路径：未知属性、修订映射缺口和复杂对象不能静默扁平化。
- 自动验证：规范样例、双向语义、资源关系、损失报告和异常输入。
- 人工验收：与 LibreOffice／WPS 交换带样式、脚注、修订的文件。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T806：EPUB 与归档 PDF

- 输入：稳定页面场景、文档导航结构、EPUB 与 PDF/A 目标配置。
- [ ] 状态：`planned`
- 需求：`FORMAT-012`
- 前置：`T603`、`T704`
- 产物：EPUB 导出和声明范围的 PDF/A 归档输出。
- 错误路径：字体不可嵌入、标签缺失或合规检查失败不声称合规。
- 自动验证：PDF 合规验证器、EPUB 校验、阅读顺序与文本抽取。
- 人工验收：EPUB 阅读器和 PDF/A 验证器检查导航、可检索性与结构。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T807：邮件合并

- 输入：用户选择的本地数据源、模板、字段映射及输出目标。
- [ ] 状态：`planned`
- 需求：`EDIT-012`
- 前置：`T501`、`T502`、`T704`
- 产物：数据源读取、字段映射、合并预览与批量输出。
- 错误路径：错误数据源、编码错误、字段缺失明确诊断；数据不自动上传。
- 自动验证：合并规则、编码、输出隔离及模板不被修改。
- 人工验收：CSV 合并生成多份信件，检查字段和取消后原模板未被覆盖。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T808：纵排、双向与复杂文字

- 输入：UAX #9／#50、固定字体、纵排、双向与复杂脚本样例。
- [ ] 状态：`planned`
- 需求：`LAYOUT-014`、`PLATFORM-004`
- 前置：`T504`、`T704`
- 产物：纵排、双向文本与复杂脚本的排版、光标移动和编辑。
- 错误路径：缺字形、方向冲突或平台映射异常明确诊断。
- 自动验证：固定字体的方向、cluster、几何和光标测试。
- 人工验收：中英阿文混排与纵排样例对照与编辑。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

## M9：后续独立规格

退出条件：本轮不实现云服务或其他 Office 应用；只登记后续规格入口。

### T900：实时协作规格入口

- 输入：稳定事务／标识／历史契约、离线边界和新的协作需求。
- [ ] 状态：`planned`
- 需求：`COLLAB-001`
- 前置：`T704`
- 产物：另立协作规格：账户、同步、权限、冲突、离线恢复与服务运维。
- 错误路径：稳定 ID 不等于 CRDT；未定义冲突语义前不接入生产同步。
- 自动验证：仅在后续规格获确认后定义服务与协议测试。
- 人工验收：明确多用户审阅与离线冲突场景，不在本轮声称已实现。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。

### T901：套件共享边界规格入口

- 输入：JoyWord 实际模块边界、JoySheet／JoyDeck 新需求与共享基础设施经验。
- [ ] 状态：`planned`
- 需求：`SUITE-001`
- 前置：`T704`
- 产物：JoySheet／JoyDeck 独立模型与可复用资源、主题、平台基础设施方案。
- 错误路径：不把 Word AST 强加给表格和幻灯片；不提前迁移稳定模块。
- 自动验证：后续规格定义共享 API 和各应用独立验证。
- 人工验收：检查跨应用资源／主题交换工作流，保持各自内容语义。
- 完成证据：未执行；完成时记录源码修订、命令、样例和人工结果。
