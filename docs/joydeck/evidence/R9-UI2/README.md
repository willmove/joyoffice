# R9-UI2：格式、日常工作区与旧语义兼容

2026-11-03，分支 `codex/joydeck-r9-ui-compat`，已同步 main `8002f27199cc7e0df928438d7f900b0add085861`。非草稿 [PR #122](https://github.com/willmove/joyoffice/pull/122) 已提交，CI 见 PR 实时检查状态。本轮推进用户指定的最高／高优先级范围，以下区分实现、自动验证和真实窗口；完整 UI 一致、JD-906 与所有平台资格仍开放。原有三份未提交 R9 运行日志保留且不纳入本轮提交。

## PR #119 与 QEMU

[PR #119](https://github.com/willmove/joyoffice/pull/119) 已按用户授权先合入 main，合并提交 `8002f27199cc7e0df928438d7f900b0add085861`。保留有价值的 CLI 子进程 QEMU runner 修复，复用 main 已正确同步的实验锁，移除无关 registry 升级。合入前 20 项 CI 全绿，包括 aarch64／LoongArch QEMU；本机交换 3 项、恢复 15 项、fmt／严格 Clippy 通过。QEMU 结果不作为信创桌面或输入法验收。PR #121 已合入 main `6689c6e`，其 R9-UI1 证据保留历史原义。

## 本轮实现与自动验证

| 范围／父任务 | 本轮已实现 | 自动验证 | 完整范围尚缺 |
| --- | --- | --- | --- |
| 混合格式，JD-301–305/308 | 继承后字符／段落汇总，选区／多对象／单元格；混合字号／字体／粗斜下划线；逐 run 相对字号与显式键入格式 | Unicode 半开范围、独立段落、混合继承、原子多目标、composition 取消 | 全部复杂脚本、原生 IME、读屏与所有快捷键 |
| 段落／表格，JD-305/504/508 | 列表样式、行距、缩进／段距对话框；表格区域／样式／底色／边框／边距／锚点／方向／尺寸／行列／合并拆分 | 新表及现有 PPTX 表格独立保存重开，保留未知 XML，混合未改字段不被写零，非法区域原子拒绝 | 所有主题表格／复杂合并、完整图库及跨 Office 逐项对照 |
| 日常工作区，JD-208/501/505/508 | 整体缩放／旋转，页面多选／拖动排序／隐藏／节；最近文件、3 个原创模板、多窗口 | 稳定 ID／节顺序／复制批注动画锚点、撤销、最近列表预算与损坏拒绝；窗口独立关闭 | 完整路径／连接手柄、母版对象、全部图库与全 DPI 命中 |
| 审阅／旧文件，JD-506/600/906 | 一次性补齐旧批注／回复／解决、alt／阅读顺序标签、媒体原始字节与片段参数、链接／自定义放映；新批注／回复／解决／alt GUI | 原创 1.11 语料转 native／重开与原件逐字节一致；显式再生 PPTX 批注／媒体回读 | 完整媒体播放／设备／多片段、辅助技术消费阅读顺序、复杂动画与长尾交换 |
| 比较／自动化，JD-506/706/906 | 离线比较；摘要绑定的单个基础对象接受与撤销；旧字符串命令严格子集；显式桌面 stdio 绑定唯一 Editor | 过期摘要／修订、重放、未知字段、批次原子性；附着会话自动回归 | 全 16 领域旧 schema、复杂组／媒体对象接受、跨页稳定匹配、桌面 MCP 分发入口 |
| Word 字形，T101/204/701/806 | 固定 OFL 单色 Noto Emoji；缺字 cluster 回退在断行前发生，真实 font key 进入屏幕／PDF；预算与缓存保持 | 两项真实字形／raster／PDF 回归；长文档 4 项；Word 产品全量 | 新 Unicode／彩色 emoji、阿文与完整复杂文字不在本轮覆盖 |
| Word UOS 输入法，T101/701 | 保留 Fcitx5／搜狗拼音直通缺陷，未凭 Windows 或依赖源码推测修复 | 尚未复现 | 用户于 2026-11-03 明确本轮暂缓 UOS，后续进入该环境时再复现、修复和 GUI 验证；当前不作通过声明 |

`legacyCommandSchemaSupported=false` 继续表示**完整**旧 schema 未兼容；能力响应另列 `legacyCommandSubset`。支持 page.add/copy/delete/visibility/reorder/size、object.delete、table.edit 的结构／尺寸／TSV 子集、document.design 的备注、review.edit 的 add/reply/resolve/accessibility、transaction.batch。未覆盖 action 或未知字段明确拒绝，不返回空成功。 [16 项兼容矩阵](legacy-command-matrix.csv) 的 GUI 栏已按解锁后的实际场景更新；GUI 调用有类型命令，不冒充旧参数自身的 GUI 验收。

PPTX preserve 对无法保真写回的批注／回复／解决、动画编辑、alt／decorative、媒体及表格属性增加保存前回读检查。失败不发布输出，保留原文件与脏状态；可保存 native，或由用户显式选择 regenerate PPTX。显式再生会按原有契约报告损失，不自动替用户切换保存模式。比较当前按页序和对象 ID 配对，页面／文稿设置及资源只报告；对象顺序、媒体字节、备注页面和扩展元数据变化均纳入报告，不把不能接受的变化误报为相同。页面／文稿设置只报告；基础对象逐个接受会校验两个摘要，不能宣称完整审阅合并。

## 自动检查与实际包

[检查汇总](checks.json)：三产品联合 3,596 通过、0 失败、1 项原计时测试忽略；覆盖最终资源 ID 保存校验、比较报告修复和 11 个广告子集工作流。最终产品 `cargo xtask check joydeck` **894 通过**，`cargo xtask check joyword` **1,318 通过**。最终 workspace 严格 Clippy、fmt、291 条来源指纹、1,050 Rust 文件门禁、文档检查通过。 根脚本工具 152 项、产品路由 14 项、Deck 工具 11 项和本地依赖／归档关联 6 项自动测试通过。共享排版改动已覆盖 Word／Sheet／Deck 联合 feature 组合；registry 版本与所有其余依赖边保持不变；为 GUI 发现的 HWND 生命周期竞态，仅将 AccessKit Windows 0.34.0 改为同版本本地补丁，原始来源、逐文件指纹和 MIT／Apache 全文见 [补丁记录](../../../../licenses/accesskit-windows/README.md)。应用 crate 仍 forbid unsafe，原有平台 FFI 留在第三方依赖内。

[Deck 包元数据](package.json) 与 [Word 包元数据](word-package.json) 记录当前字节摘要。打包后在仓库外中文／空格路径运行，隔离用户状态，不打开用户私有文稿。最终 Deck 包已完成真实格式编辑／保存、审阅文稿重开／修改、最近文件重开、跨窗口主题同步和子窗口关闭补验；独立回读见下表。之前逐场景记录保留各自真实 exe SHA，不将旧包证据改写成新包。 最后只补登记模板测试中的 8 个断言位置后重新封装 final-qualified，GUI／CLI 与已实测 delivery-qualified 逐字节一致；原生 C# helper 由相同源码／编译器／SDK 重新构建、身份不同，媒体／打印设备本轮仍未验收。源码快照及生成元数据随封装更新，见 [逐文件核对](final-package-equivalence.json) 与 [实测封装记录](delivery-package.json)。

开发包未签名；Deck 元数据继续列出 21 个依赖缺失许可全文，不作为正式发行资格。Word 新备用字体原始字节、来源、版权和 OFL 全文已登记并打包；没有打包系统字体。开发中两次出包期间源码／回归变更触发了正确拒绝，以上仅登记源码冻结后的成功包。

字节一致的最终包 R1–R7 脚本全部通过：123 次 CLI 调用、38 次 MCP 响应。逐项摘要：[R1](cli-r1.json)、[R2](cli-r2.json)、[R3](cli-r3.json)、[R4/R5](cli-r4-r5.json)、[R6/R7](cli-r6-r7.json)；摘要保留退出码、输出字节数／SHA，完整本机报告路径和 SHA 一并登记。R6/R7 首次暴露媒体资源 ID 重分配误判，修复为比较真实字节／海报／参数，回归确认普通编辑可保存且真实参数丢失仍拒绝；最终脚本重新全量通过。来源文件按 Git LF 约定冻结后更新精确指纹。

早期 qualified／delivery 字节对照仅为历史记录，见 [摘要核对](binary-equivalence.json)；最终包已包含比较报告修复和完整子集工作流回归。桌面 stdio 在其记录的 GUI exe 上已验证 attachedDesktop=true、同一 Editor 编辑／重放／过期修订拒绝／撤销恢复干净状态，见 [请求与响应](desktop-control.json)。此项机器接口保留当时字节身份，与后续真实窗口／像素场景分开登记。

## 真实产品与 Windows 窗口

参照为本机 DeckCraft 0.14.1，使用新建空白文稿观察 Ribbon 与段落对话框。[原始截图](reference/deckcraft-paragraph.png)、[可访问树](reference/deckcraft-paragraph.txt)。参照包含对齐、左右缩进、首行／悬挂、段前／段后、行距和制表位；本轮未补制表位编辑，不标完全一致。

最终 Word 包 `c10593d0…` 已在 100% 显示原创测试文段“JoyWord emoji 验证 😀”，从原生文件页导出 PDF，并用独立 pypdf 抽取 Unicode、Poppler 120 DPI 渲染和人工检查：😀 未变为空白或缺字符方框。证据：[实际最终窗口](gui/word-final-emoji.png)、[可访问树](gui/word-final-emoji.txt)、[独立 PDF 渲染](gui/word-pdf-readback.png)、[最终回读／摘要](word-final-pdf-readback.json)、[包记录](word-package.json)。最终 PDF 的 SHA-256 与此前已检查的 PDF 完全相同；回读明确绑定新 exe。早期包的 word-pdf-readback／word-repack-pdf-readback 记录保留各自字节身份。PDF 含系统字体子集仅保留在本机，不提交二进制。此项是普通 PDF 导出，不代替 PDF/A、阿文或 UOS 验收。

用户解锁后，`facc1c4e…` 格式修复包已完成以下真实 GUI 场景，并由 Python zipfile／json／python-pptx 独立读回：

| 场景 | 结果与证据 |
| --- | --- |
| 混合文字／段落 | 逐 run 相对字号、保留混合未改字段、字体改 Arial；Ctrl+Enter 保留全部原文；左缩进 18、首行 -9、段后 6 pt、行距 1.5；[回读](gui-format-readback.json)，截图 gui/deck-01–03 |
| 表格 | 首行两格橙底、123456／1.5 pt 四边框、12 pt 左边距；行高 72／60、列宽各 180；样式、居中锚点、合并／拆分内容与 Undo／Redo；[回读](gui-table-readback.json)，截图 gui/deck-04–05 |
| 比较 | 单项接受后原文变为“比较后…”；Undo／Redo 和保存；[回读](gui-comparison-readback.json)、[比较报告窗口](gui/deck-06-comparison.png) |
| 审阅 | 添加／回复／解决、替代文字；preserve PPTX 明确拒绝且原件摘要不变；原生另存完整保存审阅；[回读](gui-review-readback.json) |
| 页面组织 | 实际拖动将稳定页 ID 11 移到 10 前，隐藏／显示、建立节并保存；[回读](gui-organization-readback.json) |
| 整体变换 | 框选两对象后拖动缩放、绕共同中心旋转约 89.85°；独立方程核对尺寸／中心，两次 Undo 恢复原几何；[回读](gui-transform-readback.json) |

后续包按实际二进制分别登记，文件名中的 final 不是对后续包的自动资格继承：

| 实际 GUI exe／包记录 | 真实窗口结果与独立回读 |
| --- | --- |
| `facc1c4e…`，[格式包](formatting-package.json) | 上表六类完整工作流，逐项保留当时 PPTX／native 摘要 |
| `11c2a1c8…`，[窗口修复包](window-package.json) | 再次编辑混合字号为 32／44／28、左缩进 19、第一格蓝底并保存 PPTX，[独立读取](gui-final-readback.json)；从文件页创建三页课程模板、另存 native，[回读](gui-template-readback.json)、[实际窗口](gui/deck-final-template.png)；子窗口关闭后主进程和 UIA 查询存活 |
| `7c38d969…`，[最终交付包](package.json) | 重开带表格／排序／节／审阅的 native，重开已解决批注后保存；真实新建副窗口、切暗色并关闭，主窗口完整同步且迟到 UIA 查询不再 abort；最近文件重开；再次选中混合文本、增大字号为 32／44／28、Ctrl+Enter、保存，原文／100×80 pt 位置／18 pt 缩进等保持。[独立回读](gui-delivery-readback.json)、[格式保存](gui/deck-delivery-format-saved.png)、[主题与关闭](gui/deck-delivery-theme-main.png)、[最近重开](gui/deck-delivery-recent-reopened.png) |

GUI 额外发现并修复双击带入多选、4 px 双击抖动意外移动、Ctrl+Enter 覆盖原文、模板无效主题编号，以及全局主题改变后其他窗口局部颜色未同步。三个模板已通过实际编辑器创建／保存／重开自动回归，课程模板另有上表 GUI 证据。关闭副窗口时迟到 UIA 坐标查询造成 AccessKit abort 的路径已修复并有 invalid HWND 自动回归，上表最终包已现场复验。Windows 锁屏仅是此前暂停原因，已解除。窄窗口调整本轮未成功完成，不记为通过；全 DPI／读屏、UOS 和其余平台未作本轮真机验收。
