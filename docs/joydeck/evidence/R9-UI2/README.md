# R9-UI2：格式、日常工作区与旧语义兼容

2026-11-03，分支 `codex/joydeck-r9-ui-compat`，已同步 main `8002f27199cc7e0df928438d7f900b0add085861`。非草稿 [PR #122](https://github.com/willmove/joyoffice/pull/122) 已提交，CI 进行中。本轮推进用户指定的最高／高优先级范围，以下区分实现、自动验证和真实窗口；完整 UI 一致、JD-906 与所有平台资格仍开放。原有三份未提交 R9 运行日志保留且不纳入本轮提交。

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
| Word UOS 输入法，T101/701 | 保留 Fcitx5／搜狗拼音直通缺陷，未凭 Windows 或依赖源码推测修复 | 尚未复现 | 用户确认已有 UOS 环境、稍后提供访问方式；等待连接后复现、修复、GUI 补验 |

`legacyCommandSchemaSupported=false` 继续表示**完整**旧 schema 未兼容；能力响应另列 `legacyCommandSubset`。支持 page.add/copy/delete/visibility/reorder/size、object.delete、table.edit 的结构／尺寸／TSV 子集、document.design 的备注、review.edit 的 add/reply/resolve/accessibility、transaction.batch。未覆盖 action 或未知字段明确拒绝，不返回空成功。

PPTX preserve 对无法保真写回的批注／回复／解决、动画编辑、alt／decorative、媒体及表格属性增加保存前回读检查。失败不发布输出，保留原文件与脏状态；可保存 native，或由用户显式选择 regenerate PPTX。显式再生会按原有契约报告损失，不自动替用户切换保存模式。比较当前按页序和对象 ID 配对，页面／文稿设置及资源只报告；对象顺序、媒体字节、备注页面和扩展元数据变化均纳入报告，不把不能接受的变化误报为相同。页面／文稿设置只报告；基础对象逐个接受会校验两个摘要，不能宣称完整审阅合并。

## 自动检查与实际包

[检查汇总](checks.json)：三产品联合 3,595 通过、0 失败、1 项原计时测试忽略；覆盖最终资源 ID 保存校验、比较报告修复和 11 个广告子集工作流。最终产品 `cargo xtask check joydeck` **893 通过**，`cargo xtask check joyword` **1,318 通过**。最终 workspace 严格 Clippy、fmt、270 条来源指纹、1,050 Rust 文件门禁、文档检查通过。共享排版改动已覆盖 Word／Sheet／Deck 联合 feature 组合；Cargo.lock 和 registry pins 未改变。

[Deck 包元数据](package.json)：Release Windows x86_64，源码快照 `3bcc90324c4440921a800863d7e7e720426ed84fcc15f3353d845ab69d6ad77b`，归档 SHA-256 `e729970351e0aa428c78086cc30c48c770380a69b2dc724928d53ef5ebdfacc4`，实际 GUI exe SHA-256 `fab10fd017c06c94c138e092c8642f4b793482e5cbe100273bd797291eca9316`。已实测 Word 产品包 exe SHA-256 `f84a2c721de39ecc2af5de8951cf2d18420499450a3a22d09427d78a004d5bbe`。均在仓库外中文／空格路径解包运行，独立用户状态，不打开用户私有文稿。

开发包未签名；Deck 元数据继续列出 22 个依赖缺失许可全文，不作为正式发行资格。Word 新备用字体原始字节、来源、版权和 OFL 全文已登记并打包；没有打包系统字体。开发中两次出包期间源码／回归变更触发了正确拒绝，以上仅登记源码冻结后的成功包。

最终 delivery-final 包的 R1–R7 脚本全部通过：123 次 CLI 调用、37 次 MCP 响应。逐项摘要：[R1](cli-r1.json)、[R2](cli-r2.json)、[R3](cli-r3.json)、[R4/R5](cli-r4-r5.json)、[R6/R7](cli-r6-r7.json)；摘要保留退出码、输出字节数／SHA，完整本机报告路径和 SHA 一并登记。R6/R7 首次暴露媒体资源 ID 重分配误判，修复为比较真实字节／海报／参数，回归确认普通编辑可保存且真实参数丢失仍拒绝；最终脚本重新全量通过。来源文件按 Git LF 约定冻结后更新精确指纹。

早期 qualified／delivery 字节对照仅为历史记录，见 [摘要核对](binary-equivalence.json)；最终包已包含比较报告修复和完整子集工作流回归。最终 GUI exe 实际桌面 stdio 已验证 attachedDesktop=true、同一 Editor 编辑／重放／过期修订拒绝／撤销恢复干净状态，见 [请求与响应](desktop-control.json)。这项机器接口验证未替代锁屏下无法进行的最终手动／像素场景。

## 真实产品与 Windows 窗口

参照为本机 DeckCraft 0.14.1，使用新建空白文稿观察 Ribbon 与段落对话框。[原始截图](reference/deckcraft-paragraph.png)、[可访问树](reference/deckcraft-paragraph.txt)。参照包含对齐、左右缩进、首行／悬挂、段前／段后、行距和制表位；本轮未补制表位编辑，不标完全一致。

已记录字节摘要的 Word 解包 GUI 已在 100% 显示原创测试文段“JoyWord emoji 验证 😀”，从原生文件页导出 PDF，并用独立 pypdf 抽取 Unicode、Poppler 120 DPI 渲染和人工检查：😀 未变为空白或缺字符方框。证据：[实际窗口](gui/word-emoji.png)、[可访问树](gui/word-emoji.txt)、[独立 PDF 渲染](gui/word-pdf-readback.png)、[回读／摘要](word-pdf-readback.json)。PDF 含系统字体子集仅保留在本机，不提交二进制。此项是普通 PDF 导出，不代替 PDF/A、阿文或 UOS 验收。之后仅规范化两份打包脚本行尾并再打包 Word，Rust 源码输入未变但 exe 摘要不同；[新包记录](word-package.json) 继续标待解锁后 GUI 重验，没有把前一个 exe 的窗口结果移作新字节验收。

Deck 早期候选已通过真实窗口混合字号与保存独立回读；**最终包格式／段落／表格／排序／审阅等 GUI 场景尚待完成，不用候选结果代替最终资格**。2026-11-03 18:30 起 Windows 桌面锁屏，已请用户解锁；自动回归和文档工作继续。UOS 和其余平台未作本轮真机验收。
