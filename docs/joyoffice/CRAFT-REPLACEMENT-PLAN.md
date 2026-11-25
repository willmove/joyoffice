# JoyOffice 生产路径中的 Craft 派生代码替换计划

基线为 2026-11-24 的 JoyOffice 0.2.0，提交 `14cbb296`。目标是让三个产品当前支持的行为逐项切换到可追溯的独立实现。优先从 Word 校对、Deck 旧文件兼容和 Sheet 公式长尾开工，这三处可以通过现有产品入口比较替换前后的结果。启动估算 **268–450 人周**，其中第一阶段三个产品与共享读取器合计 **38–62 人周**，另有前置基线工作 **4–7 人周**。[既有重构路线](REFACTOR-ROADMAP.md) · [最近一次三产品原生回归](validation/20261123-uos-refactor/README.md)

## 2026-11-25 阶段 1 进展与剩余估算

已冻结 `14cbb296` 的六个 Windows 桌面及 CLI 二进制、锁文件、验证字体标识与首批 27 份输入。英美拼写变体、加密 PPTX 识别以及 `CONVERT` 已切换到新实现。命令行差分覆盖 2,324 个拼写探针、9 个演示文件、428 条公式和 4 种双版本工作簿读写组合。[首批验收](validation/20261124-craft-pilots/README.md)

第二批完成 JoyWord 下载和库存恢复、JoyDeck v1 预设迁移、JoySheet 概率分布替换，并用 Microsoft PowerPoint 16 生成的加密 PPTX 补验两代 CLI 和桌面入口。Word 对照 75 个发布包。Deck 对照 183 种预设的 370 个对象。Sheet 用 Excel 16 重算 394 条公式，修正 22 条与 Excel 的原有差异，还有 22 条极值或边界行为待核对。[阶段 1 验收](validation/20261124-craft-stage1/README.md)

第三批用 Excel 16 逐条复核上述 22 条差异及 95 条相邻用例，保留 22 条有公开契约或数学依据的冻结版结果。JoySheet 共用数值内核、JoyWord 内置独立发布索引及 Word／Deck 共用图元读取器已切换。JoyWord 的 75 个档案继续通过原发布地址获取，发行包独立托管仍有剩余工作。[阶段 1 续验](validation/20261124-craft-stage1-followup/README.md)

第四批逐包核对 75 个 JoyWord 语言包及 339 条来源记录。41 个归档已发布到 JoyOffice 独立托管，候选版优先使用新地址。34 个继续从原发行方下载，待补齐首选源材料、许可选择和通知证据。JoySheet 已替换 ETS 预测拟合、年金与证券现金流估值核心，继续使用冻结版和 Excel 16 核对 XLSX 重算结果。[第四批验收](validation/20261124-craft-stage1-release/README.md)

第五批逐包复核了 34 个待发行归档，核验 132 个固定来源。18 个包随对应源码与许可通知切换至 JoyOffice，独立托管累计 59 个。JoySheet 复核了两种日期制中位置相同的 76 条差异，修正 4 条长首期票息和 4 条 ETS 样本内预测差异。[第五批验收](validation/20261124-craft-stage1-close/README.md)

第六批切换了 JoyWord 断词模式匹配器，重建并独立发布爱尔兰语包，独立托管累计 60 个。JoySheet 收口了首期长度等于规则付息期的错误判定、ETS 置信区间的样本末点判定及 `CONVERT` 常数复核。JoyWord 页面改为按设备像素绘制，JoyDeck 和 JoySheet 的文件页新增版本查询。[第六批验收](validation/20261124-craft-stage1-remaining/README.md)

第七批独立发布保加利亚语、加利西亚语包，托管累计 62 个。JoyWord 替换词根归并和回退词干排序路径。JoySheet 在两种日期制各修正 5 条主表 ETS 差异，收口已有长首期月底诊断，并替换 GCD／LCM、组合数和近似查找分支。JoyDeck 文件页统一最近路径的显示和检索，Windows 原生窗口复核了旧列表及中文路径打开。[第七批验收](validation/20261124-craft-stage1-next/README.md)

第八批切换 JoySheet 的 ETS 自动季节识别、拟合初始状态和误差统计，双日期制主表各消除 60 条差异。JoyWord 切换复合词和多级词缀遍历，验证 75 包建议差分及原生更正、撤销和重开。已找到三个 Frami 原始归档与许可，首选构建源仍需核实。[第八批验收](validation/20261125-craft-stage1-ets/README.md)

JoyDeck 阶段 2 已切换形状坐标、嵌套组变换和编辑时的预设连接线端点。冻结版和候选版通过 32 组交叉读回，Windows 桌面通过嵌套组旋转、缩放、撤销重做及两种格式保存重开。[形状坐标验收](validation/20261125-craft-stage2-deck-coordinates/README.md)

| 剩余范围 | 人周 | 估算依据 |
| --- | ---: | --- |
| 阶段 0 其余路径的调用登记与专项基线 | 2–4 | 六个二进制及首批语料已集中冻结。复杂分页、放映与长尾文件需各补调用点、固定输入及预期结果。 |
| 阶段 1 · JoyWord | 5–9 | 剩余候选生成和校对入口替换及验证 4–6 人周。另有 13 个包的源材料、许可、托管及回退 1–3 人周，外部材料等待时间单计。 |
| 阶段 1 · JoyDeck | 0 | 加密包、v1 预设迁移和最近文档路径已完成。后续文件对象兼容、动画和渲染继续按阶段 2–4 执行。 |
| 阶段 1 · JoySheet | 4–7 | 扩展 ETS 参数与季节识别差异复核 1–2 人周，月底日历及金融计日 1–2 人周，按注册表收口剩余数学和查找分支 2–3 人周。 |
| 阶段 1 · 公共图元读取 | 0 | 有界读取器已切换。两产品的 WMF／EMF 解释与绘制仍计入阶段 4。 |
| 阶段 2–5 | 226–381 | 沿用下表各阶段的区间。三个试点的样本规模和复杂度尚不足以调整核心模型、格式和排版估算。 |
| **合计** | **237–401** | 阶段 1 待办由 11–20 降至 9–16 人周。 |

第三批按完成范围从前次估算中扣减 6–11 人周。JoyWord 发布索引和资源清单扣减 1–2 人周，JoySheet 数值内核和边界复核扣减 3–5 人周，公共图元读取扣减 2–4 人周。

第四批下调 JoySheet 实现余量，为 JoyWord 的待发行归档增加源材料核对上限。已有公开归档和 Excel 回归按验收证据计入进度。

第五批将已验收的长首期计息和样本内预测从 JoySheet 余量扣除 1 人周。JoyWord 保留原审核区间，剩余 16 个包的主要成本在核实原始生成材料和版权通知。

第六批按产品入口拆分余量。Word 先用已安装词典建立不同词缀与复合词的逐语种建议顺序，再迁移剩余的词形和排名逻辑。15 个待托管包逐个取得首选源材料及完整通知后再切换。Sheet 先用可解析序列区分 ETS 自动季节、初始状态、参数估计和置信区间，再按函数注册表替换数学和查找分支。长首期的 30/360 月底样本保留两种日期制各自的期望值。

第七批扣除词根识别和排序已替换的部分。JoySheet 的旧月底边界已经通过，新增月底对齐付息日历样本扩大了核对面，暂保留原总区间。

## 工作量与先后顺序

| 阶段 | 共享工作 | JoyWord | JoyDeck | JoySheet | 合计（人周） |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0　冻结功能、语料与对照版本 | 4–7 | — | — | — | 4–7 |
| 1　有明确输入输出的小模块 | 2–4 | 14–22 | 6–10 | 16–26 | 38–62 |
| 2　模型、编辑或计算内核 | — | 32–52 | 16–28 | 34–56 | 82–136 |
| 3　文件读写与旧数据交换 | — | 28–48 | 18–30 | 26–44 | 72–122 |
| 4　排版、呈现、导出及其余入口 | — | 34–56 | 22–38 | 12–22 | 68–116 |
| 5　核对生产来源并演练回退 | 4–7 | — | — | — | 4–7 |
| **合计** | **10–18** | **108–178** | **62–106** | **88–148** | **268–450** |

一个人周按五个工作日计算。估算涵盖规范推导、实现、审阅、差分测试和产品实测，假设沿用现有 GPUI、OPC、通用字体与 PDF 库，并能取得现有语料、授权字体和 Office 参考环境。下限假设现有样本可直接复用，上限预留了补语料和修正兼容差异的工作。以校对语种、文件对象族、分页场景和公式族拆分工作，历史文件数用于排查遗漏。字体创作、从零编纂词典及新增产品功能各有独立范围。

**高优先**用于会改变已保存数据、计算结果、正文分页或恢复行为的工作，以及能先消除一条独立上游调用的试点。**中优先**用于有稳定输入输出、可与核心开发并行的替换。**低优先**用于依赖已稳定的呈现细节。阶段表示生产切换顺序。阶段 0 即可录制复杂分页、放映和工作簿重算基线，各产品按自己的依赖推进。

**首批启动估算为 8–14 人周。** 其中 4–7 人周用于阶段 0，其余用于 Word 英美拼写变体（1–2）、Deck 加密 PPTX 识别（1–2）和 Sheet 的 `CONVERT`（2–3）。三个试点已计入阶段 1。用这三批的实耗、兼容例外和新增样本校准后续估算，尤其是 Word 跨节分页、Deck 文本与动画、Sheet 动态求值的上限。

## 共用基线和读取器

- **阶段 0，高，4–7 人周。** 固定六个桌面与 CLI 入口的旧二进制及 SHA-256、锁文件、测试字体的文件散列与 face、语料及期望结果。在每条实际调用旁登记上游文件或函数、产品入口、触发条件（默认、兼容回退、可选构建）、替换负责人和复跑用例。WordCraft 有 575 条来源记录，DeckCraft 有 194 条映射、192 个目标路径，GridCraft 有 317 条核心记录，Sheet CLI 另有归属。逐条标出测试、资源及已重写部分。根依赖图中独立 Craft 包已为零，仍要追踪内联源码、构建脚本和运行时数据源。[Word 来源](../../licenses/wordcraft/SOURCE.json) · [Deck 映射](../../licenses/deckcraft/file-map.json) · [Sheet 核心来源](../joysheet/upstream-source.json) · [Sheet CLI 归属](../joysheet/ATTRIBUTION.md)
- **阶段 1，中，2–4 人周，已验收。** `joyoffice-metafile` 通过独立实现的有界字段和点列视图服务 Word 与 Deck 的 WMF／EMF 入口。四份文档的八张页面场景或 PDF 光栅与本机冻结版一致。两端各自的图元解释、绘制记入产品阶段 4。OPC、CFB、DrawingML 等宿主库按实际调用复用，工作量只在一处登记。[共享读取器](../../crates/joyoffice-metafile/src/lib.rs) · [第三批验收](validation/20261124-craft-stage1-followup/README.md) · [历史双产品实测](validation/20261118-desktop-sharing/README.md)

## JoyWord · 108–178 人周

WordCraft 0.15.0 的文档模型、编辑会话和完整分页仍在桌面与 CLI 的正常调用链上。已有的有界局部重排、第一方校对规则和语言包发布事务作为对照基线保留。`joyword-proof` 承接了原 `craft-proof` 工作区里的代码。该副本也包含后续在 JoyOffice 编写的算法，归属要追溯到具体函数与提交。`craft-font-catalog`、`craft-font-store`、`craft-atomic` 和 `craft-proof` 的独立包已退出产品依赖。[Word 迁移架构](../joyword/r8/migration/docs/joyword/ARCHITECTURE.md) · [校对与存储迁移记录](validation/20261118-desktop-sharing/proof-and-packs-retirement.json)

| 阶段与优先级 | 仍在用的派生代码及替换内容 | 人周 | 兼容性验收标准 |
| --- | --- | ---: | --- |
| 1 · 高，校对 | 替换 `joyword-proof` 中仍承自上游的拼写、建议排序与断词规则，第一批做英美拼写变体。保留已独立编写的 `joyword-proof-rules` 和已验证的词典算法。 | 9–14 | 比对词形接受结果、UTF-8 标记范围、建议顺序及断词位置。原生审阅窗口完成更正、撤销、保存重开。英文默认词典和已装语言包均覆盖。 |
| 1 · 高，语言包与数据 | 下载、库存恢复和内置发布索引已切换。62 个已核包在 JoyOffice 托管并保留原地址回退。13 个继续原发行路径，逐包补匹配的源材料和授权证据。默认数据清单覆盖 SCOWL、Moby、hyph-utf8、DesignCraft 英语断词模式与补充词表。 | 5–8 | 75 个既有正式包均可读。旧安装目录、库存与用户联网授权可继承。核对断网、拒绝联网、篡改归档、取消下载、中断发布、替换与卸载后的库存、词典及分页。默认资源与按需资源均可追溯到文件指纹和实际许可。 |
| 2 · 高，模型与编辑 | `joyword-model`、`joyword-engine` 中的段落、表格、故事范围、选区、编辑命令与历史按对象族替换。 | 29–47 | 同一操作轨迹逐步核对正文、选区、格式、批注、修订和撤销边界。桌面 IME 合成、提交和取消分别验证。用 JWREC001–003 及双槽样本恢复到新文档，核对原件与恢复副本。 |
| 2 · 中，控制接口 | `joyword-mcp`、`joyword-control-key` 按同一会话的文档身份、实例发现、认证和版本契约迁移。 | 3–5 | 旧调用脚本分别连接桌面与无界面会话，核对能力声明、JSON、退出码、目标文档和过期版本拒绝。拒绝的调用保持原修订号。 |
| 3 · 高，文件与格式交换 | `joyword-docx`、`joyword-doc`、`joyword-formats`。先做 DOCX 读取、保留和写回，再做二进制 DOC 导入及 RTF、ODT／FODT、HTML、Markdown、TXT、LaTeX 等当前支持的方向。覆盖邮件合并输入和带损失报告的导出入口。 | 28–48 | 两代交叉读写，固定样本的正文、字段、节、图表、嵌入对象和关系均可核对。按现有契约原样保留的未知部件、宏字节和只读原件逐件验散列。保护、只读、超预算及取消保存沿用既有结果。代表性文件经 Word 或 LibreOffice 独立打开、另存和重开。 |
| 4 · 高，完整分页与字体 | 替换 `joyword-geom`、`joyword-fonts`、`joyword-layout` 中的派生实现。先验证新的完整分页，再接已有局部重排及其回退入口。独立解析可选 `CRAFT_FONTS_DIR` 字体清单。 | 22–36 | 固定字体字节、face、DPI，复跑 14 步分节／重锚、956 页场景和 2,868 次命中比较。断行、页码、文字位置逐项核对，复杂文档及超预算编辑覆盖回退到完整分页。旧字体清单与默认构建各验一次。 |
| 4 · 中，绘制与 PDF | 替换 `joyword-metafile`、`joyword-render`、`joyword-pdf` 的剩余解释与输出，以及生产 PDF 后端的 `craft-pdf` 适配。屏幕、打印与 PDF 消费同一页面场景。 | 12–20 | 复跑两版 69 页 PDF 栅格对照，独立读回页数、文本、链接和嵌入字体，PDF/A-2b 用归档校验器复核。字体禁嵌和输出失败的样本验证错误及目标文件状态。 |

语言包的两个历史上游文件已与 `craft-libs@762a06a4` 的原始 SHA-256 核对。候选版用有界进度通知和可取消的响应体读取处理 HTTP 下载，按现有安装目录重建库存。内置索引固定全部 75 个数据档案的 SHA-256 和原发行地址。62 个包使用 JoyOffice 主地址，新旧索引缓存分别保存。[导入指纹](../../licenses/craft-libs/SOURCE.json) · [下载实现](../../crates/joyword-engine/src/packs/http.rs) · [内置索引](../../crates/joyword-engine/data/language-pack-index.json) · [独立发行清单构建](../../scripts/joyword/stage-reviewed-language-packs.py)

校对先运行建议顺序与语言包安装对照，分页则从桌面、PDF 和打印入口取证。Word 的 R8 交接列出了既有 CLI、恢复和另存契约。[拼写顺序脚本](../../scripts/joyword/check-spelling-order.py) · [语言包索引](../../crates/joyword-engine/src/packs/store.rs) · [分节重排脚本](../../scripts/joyword/check-section-splice.py) · [PDF 适配来源](../../crates/joyword-pdf/src/backend.rs) · [R8 兼容表](JOYWORD-R8-HANDOFF.md)

## JoyDeck · 62–106 人周

DeckCraft 0.14.1 的演示模型、文本、动画、格式和渲染实现仍参与构建。现行编辑路径的 183 项预设形状、5 种文件连接线及 49 类 PresentationML 转场生成器已独立实现，继续参加回归。静态形状和嵌套组变换已切换，动画时间线、对象效果及旧文件转换按各自路径推进。[逐文件来源](../../licenses/deckcraft/file-map.json) · [产品移植说明](../../THIRD_PARTY-JOYDECK.md) · [已替换的生成器](validation/20261123-uos-refactor/README.md)

| 阶段与优先级 | 仍在用的派生代码及替换内容 | 人周 | 兼容性验收标准 |
| --- | --- | ---: | --- |
| 1 · 高，旧文件兼容 · 已验收 | `joydeck-pptx/src/cfb.rs` 已接入宿主 CFB 读取器。v1 迁移桥用旧格式的名称及调整元数据发出 DrawingML，由现行几何路径生成形状。原 `preset/vendor/` 的 6 个文件已退出构建。 | 6–10 | 正常 PPTX、已知加密容器及损坏输入的分类、提示和预算均有对照。旧 v1 样本经只读桥转入 v2，形状轮廓、调整值、连接点及对象 ID 可核对。迁移报告齐备，v2 附件中的旧原件逐字节匹配，重开后可编辑、撤销。 |
| 2 · 高，演示模型与编辑 | 撤销历史、媒体引用、跨文稿资源映射、静态形状坐标与组变换已切换，`history_cost.rs`、`media_refs.rs` 和 `transfer.rs` 已退役。继续推进 `joydeck-model` 和 `joydeck-color` 中的形状数据、富文本和主题色路径。[历史验收](validation/20261125-craft-stage2-deck-history/README.md) · [跨文稿验收](validation/20261125-craft-stage2-deck-transfer/README.md) · [形状坐标验收](validation/20261125-craft-stage2-deck-coordinates/README.md) | 16–28 | 固定命令序列逐步比对页序、对象 ID、层级、主题绑定及撤销／重做。跨文稿粘贴含图片、母版、链接的对象，验证 ID 冲突、同一资源复用和清空历史后的媒体引用。嵌套组的旋转、翻转、缩放和连接端点须通过坐标与渲染对照及双格式读回。多窗口编辑共用一个模型，过时修订按基线处理。 |
| 3 · 高，原生格式与 PPTX | 替换 `joydeck-format` 和 `joydeck-pptx` 的剩余读写。以母版、主题、表格、图表、媒体、备注、审阅、动画分别交付，覆盖 `.joydeck` v2、`.deckcraft` v1 导入与 PPTX。 | 18–30 | 在各自支持的读写方向上，两代 CLI 交叉读回，并用独立 XML 核对关系、ID、媒体散列、时间信息与保留部件。PowerPoint 打开代表样本、另存后由 JoyDeck 重开。超过能力的样本维持登记的保留、只读或拒绝行为。 |
| 4 · 中，文本、放映与 PDF | `joydeck-fonts`、`joydeck-text`、`joydeck-metafile`、`joydeck-render`、`joydeck-anim`、`joydeck-pdf` 中的派生路径，包括文本适配、CPU 合成、缓动、对象动画、Morph、放映时间线及定位字形导出。 | 20–34 | 核对换行、字形命中、动画触发顺序、实际放映终态及 PDF 页、文字、链接和书签。复跑既有 1,850 帧 GPU／CPU 对照和 192 组文件互读。固定样本的首尾帧一致，中间帧每通道最大差沿用同配置 Metal ≤1、UOS Vulkan ≤2 的基线。 |
| 4 · 低，功能区呈现 | 用宿主 GPUI 组件替换 `joydeck-ui-gpui/src/chrome.rs` 中来自上游 Ribbon、控件与主题的布局逻辑。 | 2–4 | 在原生宽窗、窄窗和深浅主题下核对可见命令、折叠、禁用状态、键盘焦点与动作结果。 |

来源账本保留旧几何导入路径和散列。v1 只读迁移仍沿用现有 R1–R7 产品验证入口。[历史几何映射](../../licenses/joyoffice/source-map.json) · [v1 预设参数表](../../crates/joydeck-legacy/src/model/preset/defaults.rs) · [旧文件转换入口](../../crates/joydeck-shell/src/document.rs) · [加密识别](../../crates/joydeck-pptx/src/cfb.rs) · [放映逐帧证据](validation/20261118-desktop-sharing/remaining-transitions.json) · [R9 兼容表](JOYDECK-R9-HANDOFF.md)

## JoySheet · 88–148 人周

GridCraft 0.12.1 的 11 个产品库及 CLI 有导入来源。现行密集标量路径、循环分组与迭代调度、已替换的文本日期、条件聚合、矩阵、复数、贝塞尔和折旧函数，以及已统一的窗格、视图、行列、打印与保护映射，均用作回归样本。按函数注册名和格式对象逐项确认哪些调用仍走上游实现。[来源与 CLI 归属](../joysheet/ATTRIBUTION.md) · [函数与格式替换记录](../joysheet/UPSTREAM-PATCHES.md)

| 阶段与优先级 | 仍在用的派生代码及替换内容 | 人周 | 兼容性验收标准 |
| --- | --- | ---: | --- |
| 1 · 高，公式长尾 | 已切换 `CONVERT`、概率分布、共用特殊函数内核、ETS 平滑拟合及年金与证券估值核心。已验收长首期票息固定日计基础、规则首期判定、样本内预测入口、置信区间样本末点和 `CONVERT` 常数。继续校验 ETS 自动季节识别、拟合统计、区间估计与月底计日，按注册表收口仍派生的数学、查找等分支。 | 16–26 | 每族建立规范或数学参考用例，并保存可用 Excel 版本的强制重算结果。类型、错误码、空白语义和溢出范围精确匹配。数值按函数预先定义绝对／相对误差及近零、病态输入门槛。复跑既有 8 份工作簿、604 个公式缓存，以及 Windows 修正过的 `IMPRODUCT`、`VDB` 用例。 |
| 2 · 高，模型与通用求值 | `joysheet-core`、`joysheet-numfmt`、`joysheet-formula`、`joysheet-model`、`joysheet-calc`、`joysheet-engine`。先固定值、地址和日期系统，再替换公式解析、通用求值、动态依赖与数组写回，接入原有单写者会话、命令事务和不可变快照。 | 34–56 | 桌面与 CLI 按逐格值、公式、错误类型、依赖覆盖和提交版本比对。覆盖跨表引用、动态引用、阻塞后再展开、循环计算、剪贴板和撤销重做。10k／100k 扇出及小表在同机 Release 对照引擎与首帧 P95，小表 P95 和峰值 RSS 沿用 5% 回退预算。 |
| 3 · 高，工作簿格式 | 替换 `joysheet-xlsx` 中剩余的 XLSX／XLSM／模板读写及 BIFF8 XLS、BIFF12 XLSB、ODS 的导入，补齐共享字符串、样式、公式缓存、绘图、透视、外链和未知部件处理。覆盖引擎当前的 CSV／TSV、JSON 和 HTML 交换入口。 | 26–44 | 复跑 39 份三格式输入、156 组通过 XLSX 的两代组合，增加每种待替换对象的独立样本。Excel 打开新生成的兼容样本无修复提示，另存后由两代重开。VBA、透传部件、关系和告警按格式契约验证。XLS／XLSB／ODS 经受支持的导入与另存路径验收。 |
| 4 · 中，图表与 PDF | 替换 `joysheet-chart`、`joysheet-pdf` 的布局和输出逻辑，接回现有 GPUI 画布。 | 7–13 | 比对坐标轴、系列、图例和命中。独立读取 PDF 页数、文本、打印区域及分页，核对预览与导出覆盖同一单元格范围。 |
| 4 · 中，命令行与控制 | 替换 `joysheet-mcp` 和 `apps/joysheet-cli` 的导入逻辑，接回同一引擎和宿主控制协议。 | 5–9 | 用相同脚本对照命令目录、返回结构、窗口／工作簿定向、版本拒绝与退出码。真实窗口完成公式编辑、撤销、保存及新进程重开。 |

计算验证分别保存规范期望、文件缓存和 Excel 实际重算结果。文件验收按支持方向分别执行，特别覆盖宏工作簿和加密原件另存副本。[通用求值入口](../../crates/joysheet-calc/src/eval.rs) · [单位换算](../../crates/joysheet-functions/src/engineering/units.rs) · [格式方向](../../crates/joysheet-engine/src/io.rs) · [既有两代格式对照脚本](../../scripts/joysheet/check-sheet-print.py) · [现有命令目录](../joysheet/commands.json)

## 每批切换与最终验收

1. **先确定独立依据。** 每个替换单元记录规范版本与条款，配原创样本或授权的黑盒参考。复用现有公共接口，保持唯一可编辑模型。在独立进程运行旧版，与候选版比较读回的语义、显示结果和拒绝行为。旧版与规范冲突时，单列用例、参考应用版本、修正理由和新预期。[ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [Microsoft Office 文件格式规范](https://learn.microsoft.com/en-us/openspecs/office_file_formats/) · [OASIS OpenDocument 1.3](https://docs.oasis-open.org/office/OpenDocument/v1.3/os/part3-schema/OpenDocument-v1.3-os-part3-schema.html)
2. **再通过产品入口验收。** 两代指基线 `14cbb296` 与候选版。对支持双向写入的格式运行旧写旧读、旧写新读、新写旧读和新写新读四种组合。只读格式验证导入、另存副本和原件散列。截断、超预算及未知内容样本核对诊断、只读或拒绝行为。每批运行相关 crate 测试、fmt、严格 Clippy 及所属产品的 `cargo xtask check`。共享模块覆盖所有实际使用方。性能固定硬件、字体、语料、构建模式和采样次数，在阶段 0 登记门槛。自动差分、原生操作和人工验收分别留证，信创真机按仓库约定有条件时补验。
3. **切换和回退各验证一次。** 生产入口切到新实现后，用真实桌面编辑、撤销、保存、关闭、重开。首次切换差分失败时，按该能力恢复上一验证版本，用旧版读回新写的文件、日志和用户配置。失败保存须保全原件。临时兼容开关仍能执行派生代码的单元，状态记为“已切换，待退役”。
4. **阶段 5，共享 4–7 人周。** 逐产品核对桌面、CLI、导出、兼容回退及可选发行功能的调用，结合六入口依赖图、源码映射、构建输入和实际包内容复查。每项“已退役”附最后一个旧调用、替换提交、规范来源和通过的用例。计划完成条件为范围内生产可达的旧实现和未解释兼容差异均为零，保留的词典、字体及其许可另列资产清单。历史文件保留原始指纹与迁移记录，活跃条目继续校验适配指纹。更新产品 SBOM，用打包二进制演练回退。[工程与平台约定](../../AGENTS.md) · [Deck 来源检查](../../scripts/joydeck/check_sources.py) · [Word 回退验证](../../scripts/joyword/verify-r8-headless-rollback.py) · [Sheet 回退验证](../../scripts/joysheet/check-version-rollback.py)

六个发行入口的依赖图可从仓库根目录复跑。

```text
cargo tree --locked --target all --all-features -e normal,build -p joyword -p joyword-cli -p joydeck -p joydeck-cli -p joysheet -p joysheet-cli
```
