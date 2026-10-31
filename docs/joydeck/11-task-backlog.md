# 可执行开发任务清单

更新：2026-10-30。**M8、M9本机受支持范围done，M10的软件实现与单屏双窗口验证done，完整M10仍待硬件验收**。本轮含PowerPoint实机对照后的功能区重构、图表/图示/动画/放映、前序回归与系统拼音；证据见 [31](31-m9-implementation-report.md)、[32](32-m10-implementation-report.md) 和 [最终包验证](evidence/m10/final-verification.json)。M0–M7完整父阶段及其真实实现缺口、三平台/MVP条件保持开放；勾选范围必须结合实施记录阅读。

| 阶段 | 当前状态 | 仍需关闭的主要条件 |
|---|---|---|
| M0 | Windows 历史验证、macOS 构建与 adapter 回归通过 | 完整焦点/双窗口人工流程、跨屏 DPI、Linux/远端 CI |
| M1 | native/PPTX 查看器已实现，macOS 最终包回归通过 | 字体/资源诊断、DocumentStore/细粒度进度、裁剪契约、LARGE 基线及完整 AT-02/15 |
| M2 | 本机编辑功能已补齐，macOS 最终包保存/关闭/恢复回归通过 | 完整人工/故障/断电边界矩阵、跨屏 DPI、其余平台与 T-M2-10 |
| M3 | 画布文字预览、系统拼音及样式往返通过 | 统一布局、自动换行、段落列表/混合字号、完整导航/字体及多平台验收 |
| M4 | 当前模型 Scene/GPUI 绘制切片已实现，本机固定基准与回归见26 | 完整输入/driver/present trace、设备丢失、其余平台/跨屏/长会话，以及 M3 前置缺口 |
| M5 | 图片资源/裁剪/剪贴板/对齐分布增量已实现，验证见27 | 组/连接符/锁定、SVG/metafile/高级路径、完整图片面板、跨平台与AT-08 |
| M6 | 同文档双窗口/共享历史、设计继承/文字占位符、备注/节/查找替换、跨文档页复制增量，见28 | 完整母版/格式面板、文本扩展/工作区、本地化/读屏、全平台和完整AT-09/05/14 |
| M7 | 文件交付预览；M8增加真实Office表格编辑往返与全套回归，见30 | 完整公开PPTX往返与外部语料、unknown bag、输出完整能力、三平台/完整MVP验收 |
| M8 | 表格与工作区改造，本机受支持范围done，见30 | Windows/Linux、其他DPI/硬件/物理输入；继承前序完整MVP门禁 |
| M9 | 七类基础图表、网格/XLSX同步、基础图示，本机受支持范围done，见31 | 全量Excel/SmartArt、复杂组合/chartex/OLE跨文稿无损复制不在声明范围；平台门禁继承 |
| M10 | 时间轴、动画/转场/Morph子集、演示者和演示工具的软件子项done，见32 | 07/09/10：真实双屏/DPI/镜像/热插拔/睡眠、完整BENCH-SHOW与AT-12仍开放 |

依赖列为关键前置；每行也默认依赖本阶段入口条件和 10 的统一完成定义。粒度目标为一次可评审提交或少量相关提交，过大的任务在实施时拆子项并保留父编号。任务完成不能只依据“有代码/有按钮”，需满足末列中的可观察结果。

## M0 工程与技术风险验证

Windows 历史 build/check/stage 与 macOS ARM64 构建/原生 adapter smoke 已通过；macOS 系统拼音验证见 [24](24-m3-macos-verification.md)。04、06 的完整真实人机流程、07 的 Linux/完整基准机及 08 全部验收仍待补齐。CI 已定义但尚未远端运行。

对应：FR-APP-01/05，NFR-01/04/08/09/10；[架构](04-architecture-spec.md)、[UI](05-gpui-ui-spec.md)。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [x] T-M0-01 建立 workspace | 无 | 最小 joydeck/xtask、错误/日志入口、版本信息及 check/stage 命令；实际链接运行；模块按用户可见切片独立实现 |
| [x] T-M0-02 锁定依赖 | 01 | 锁 Kit 与匹配 gpui-pre 家族/toolchain/features，记录许可证和来源；cargo tree 无混用 GPUI 类型；干净依赖解析可重现 |
| [x] T-M0-03 诊断工作区 | 02 | GPUI 窗口、按钮/菜单/弹窗、明暗主题、简中/英文；启动失败有错误出口；实际画面可检查 |
| [ ] T-M0-04 输入和焦点 spike | 03 | 普通输入的中文预编辑/commit/cancel、UTF-16范围、Tab/focus/快捷键；记录锁定 API 和各 OS 结果，为 M3 建桥 |
| [x] T-M0-05 绘制和图像 spike | 03 | 自绘 path、透明 RGBA、缩放、裁剪和 DPI；记录纹理格式和 paint API 能力；无复制外部图形上下文的假设 |
| [ ] T-M0-06 双窗口和任务 spike | 03 | 双窗口独立焦点、可取消后台计算、句柄生命周期和关闭后结果丢弃；不在 foreground 做长计算 |
| [ ] T-M0-07 三平台构建与基准机 | 02、03 | 建立原生 CI/build、记录真实 toolchain/OS/驱动/字体；能取得平台则实际弹窗，缺硬件标未验证 |
| [ ] T-M0-08 M0 交付 | 04–07 | 可运行测试包、AT-01、技术风险结论与阶段记录；固定 API/输入/绘制问题的后续 owner 和截止阶段 |

## M1 只读查看器

按用户指示，在保留 M0 未测项的同时推进 Windows 只读切片，不将此视为 M0 全平台验收通过。详细契约见 [18](18-m1-viewer-spec.md)。

对应：FR-APP-02、DOC-02、SLD-05、IO-02；[净室实现规范](01-clean-room.md)、[文件](07-format-compatibility-spec.md)。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [x] T-M1-01 独立只读模型 | M0 Windows build/run | 根据公开规格实现几何、颜色、页/对象模型及 CPU 预览；通用依赖有来源记录；每切片可构建并启动 |
| [ ] T-M1-02 字体输入与兜底 | 01 | 明确系统字体来源；明确合法 UI/文档字体与系统 fallback；缺 CJK/嵌入字体均有可读状态 |
| [ ] T-M1-03 受限导入服务 | 01、02 | native/PPTX 嗅探、预算、progress、cancel、DocumentId 发布；只读 `.joydeck` v1/PPTX 子集；失败不替换旧文档 |
| [ ] T-M1-04 CPU 图像桥 | 03、M0-05 | 后台 render → GPUI 图像，完整 key/revision 防串页；支持 pasteboard/输出裁剪差别；UI 不同步 raster |
| [x] T-M1-05 只读工作区 | 04a | 打开/缩略图/页导航/fit/zoom/状态栏；虚拟列表及渐进占位；无编辑假入口 |
| [ ] T-M1-06 兼容报告 | 03 | 结构化缺字体/未知对象/丢失 part 警告、页/对象定位及只读标签；日志之外有 UI 报告 |
| [ ] T-M1-07 基线与故障样例 | 03–06 | SMALL/LARGE 首屏、RSS、raster/upload 计数；损坏 ZIP/超限/XML 和取消回归；样本来源记录 |
| [ ] T-M1-08 M1 交付 | 07 | AT-02/15、native/PPTX 各一次真实浏览、可运行包与当前兼容范围；旧阶段回归 |

### M1 本轮子项与证据（2026-10-29）

父任务仍按全部完成定义勾选；以下拆分记录本轮可运行切片，不缩减完整阶段门槛。证据统一见 [19](19-m1-implementation-report.md)。

- [x] T-M1-02a：系统字体在后台一次加载、CJK 合成样例可见；全局字体/排版限制提示。02 剩余：逐字体/缺字/嵌入字体识别。
- [x] T-M1-03a：原生/PPTX 内容嗅探、受限 ZIP/XML、加载状态、分块取消、请求序号防迟到结果；失败保留文稿。03 剩余：细粒度进度与正式 DocumentId/DocumentStore 生命周期。
- [x] T-M1-04a：后台生成 SVG/CPU 图像、BGRA 桥、文档代次/页/像素宽缓存键；快速切页回归。04 剩余：pasteboard/导出裁剪契约专项验证。
- [x] T-M1-06a：结构化 code/page/part 告警、只读标签、兼容弹窗与占位。06 剩余：逐字体诊断、稳定对象定位、完整丢失 part 报告。
- [x] T-M1-07a：自建两份 20 页语料、ZIP/XML/版本/几何故障测试、取消和失败保留；本机 raster/RSS 初值。07 剩余：LARGE 与正式首屏/内存/上传计数基线。
- [x] T-M1-08a：Windows 可运行目录/ZIP、native/PPTX 实际浏览/缩放、原生 smoke、M0 回归及截图。08 剩余：完整 AT-02/15、外部授权语料与未测平台。

## M2 基础编辑与安全

2026-10-29：01–09勾选表示Windows本地实现及自动/原生窗口验证完成，证据见[22](22-m2-completion-report.md)。用户已明确授权以M1已实现查看器为入口推进M2；这不表示M1-08或更早的平台验收已通过。T-M2-10须完整人工/故障/平台证据，保持未完成。


对应：FR-APP-03、DOC-01/03/04/05/06、SLD-01/02、GEO-01/02/03/07，NFR-03。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [x] T-M2-01 接入命令总线 | M1-08 | Session/registry adapter、目标 DocumentId、schema 校验、enabled/error、UI Action；短命令保序，日志不泄露正文 |
| [x] T-M2-02 建立编辑事务 | 01 | 单命令/复合命令回滚、undo/redo、history budget、选择修复；失败无半成品，拖动合一条历史 |
| [x] T-M2-03 页面管理 | 02 | 新建尺寸、页面增删复制隐藏、缩略图多选重排；零页/删末页/ID 溢出拒绝可验证 |
| [x] T-M2-04 基础形状工具 | 02、03 | 矩形/椭圆/线/箭头/文本框创建，命中、框选、拖动、缩放、旋转及 Esc cancel；有实际画布操作 |
| [x] T-M2-05 基础格式与面板 | 04 | 位置/尺寸/旋转、实色/边框/透明度；混合值和 draft commit，工具/面板/快捷键结果一致 |
| [x] T-M2-06 原生格式 v1 | 02 | `.joydeck` 身份/manifest/schema/resource 索引、本项目 schema 升级；保存重开语义相同，未知 requiredFeatures 不静默覆盖 |
| [x] T-M2-07 保存事务与关闭 | 06 | 快照异步保存、原子替换、保存 revision、dirty close/quit 和 cancel；慢旧保存不清除新修改；权限失败保留旧文件 |
| [x] T-M2-08 自动恢复 | 06、07 | debounce/配额、稳定快照、恢复列表及损坏隔离；异常结束后恢复，不覆盖原文件，不保存半个 IME/拖动 |
| [x] T-M2-09 安全和历史回归 | 02–08 | 指针取消、无效参数、保存磁盘错误、dirty close、恢复、redo 与连续拖动；有必要回归和 seeded 命令测试 |
| [ ] T-M2-10 M2 交付 | 09 | AT-03/06/14，通过“制作三页→保存→重开→修改→取消关闭→恢复”演示；测试包持续可用 |

### M2 初始切片历史（2026-10-29）

以下为21记录时的初始子项状态；其中“待补”已由22的后续实现覆盖，以本节父任务及22为当前依据。

按用户授权推进M2；M1-08完整验收仍待补。父项全部条件未满足前不勾选。证据见[21](21-m2-implementation-report.md)，实施范围见[20](20-m2-editor-spec.md)。

- [x] T-M2-01a 类型化命令、Session目标身份、校验和明确错误；完整registry/schema目录待补。
- [x] T-M2-02a 验证后提交、snapshot undo/redo、历史上限与分支回归；复合/拖动事务待补。
- [x] T-M2-03a 页面新增/复制/删除、唯一页ID与末页保护；尺寸/隐藏/多选重排待补。
- [x] T-M2-04a 矩形/椭圆/直线/文本框、画布或列表单选、对象删除；直接拖动/旋转/框选/箭头待补。
- [x] T-M2-05a 属性面板位置/尺寸/实色/文字整事务应用与undo；描边/透明度/混合值待补。
- [x] T-M2-06a 基础v1写入重开、未知扩展拒绝编辑；完整资源索引/升级待补。
- [x] T-M2-07a 后台另存、同目录临时文件、首次提交不覆盖已有目标、旧快照保存不清新dirty、关闭/全局退出取消；覆盖保存冲突和完整save-close待补。
- [x] T-M2-08a 已提交快照后台恢复写入、旧revision防反写、最近快照读为dirty；列表/配额/debounce/损坏隔离待补。
- [x] T-M2-09a 14项测试及原生UI属性/关闭取消/另存/重开/恢复回归；物理指针和故障矩阵待补。
- [x] T-M2-10a 分步build/run、分发包及截图；完整AT-03/06/14未验收。

### M2 剩余实现补齐（2026-10-29）

- [x] D1 稳定对象身份、复合事务、注册元数据/类型化参数校验、undo/redo双向预算。
- [x] D2 拖动、八点缩放、旋转、Shift约束、Alt中心缩放、Esc/失活取消；一次手势一条历史。
- [x] D3 框选/多选、缩略图虚拟列表/拖动重排/批量复制删除隐藏、对象图层重排、混合属性与页面尺寸。
- [x] D4 schema minor升级、样式/ID往返、空资源索引与未知数据编辑拒绝。
- [x] D5 指纹冲突检测、确认覆盖、旧保存不清新dirty、保存/放弃等待完成后关闭、全局退出保护。
- [x] D6 恢复列表、1秒debounce/60秒上限、每会话3份/1GiB预算、跨实例文件锁、坏数据隔离和旧版只读恢复。
- [x] D7 25项测试、真实异常退出子进程、最终分发EXE实际窗口回归、M0/M1回归、截图及可回退包。
- [ ] D8 完整AT-03/06/14人工演示，空间耗尽/占用、多DPI/多显示器与其余平台实测；完成后再关闭T-M2-10。

## M3 富文本与输入法

本机已验证的子项见文末 2026-10-30 增量，最新系统拼音与最终包证据见 [24](24-m3-macos-verification.md)。T-M3-01～08 的完整条件尚未全部满足，父任务保持未勾选；当前代码缺口与外部验收条件分列如下。

- 待开发：统一 CPU/编辑布局、自动换行、段落/列表/缩进/段距、完整字符格式和混合字号、旋转输入、视觉行/逐词导航和 bidi affinity、完整字体及 rich clipboard。
- 待验证：其余输入法和 Windows/Linux、物理键盘、跨屏 DPI、读屏、完整工具栏/弹窗焦点与阶段演示。
- 已解除：macOS 无活动窗口/首帧阻断；最终包原生回归和系统拼音已通过。

对应：FR-APP-06、TXT-01/02/03/04/05，NFR-07/08；[文字规格](06-render-text-spec.md)。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [ ] T-M3-01 文本布局接口 | M2-10 | 继承格式→font→glyph/line/caret/cluster 契约；坐标单测；不让 UI 和 renderer 各排一套 |
| [ ] T-M3-02 文本选择导航 | 01 | 点击定位、范围拖选、词/段、Home/End/方向键、bidi affinity、grapheme 删除；Unicode 合法 |
| [ ] T-M3-03 CanvasTextInput | 01、02、M0-04 | 实接 GPUI surrounding/marked/selected/replacement 范围，预编辑/commit/cancel 及候选屏幕位置 |
| [ ] T-M3-04 焦点和事务整合 | 03 | 工具栏格式化后恢复选区、属性输入不穿透、切页/保存/弹窗 settle、一次 IME 一次 undo，避免重复 commit |
| [ ] T-M3-05 字符和段落格式 | 01、02 | 字体 face/字号/粗斜/颜色/列表/缩进/段距；混合格式、空段列表、输入继承及 undo |
| [ ] T-M3-06 字体替代及文字粘贴 | 03、05 | 缺字/缺字体说明，CJK/emoji/组合音标与 plain/rich text fallback；拒绝非法替换范围 |
| [ ] T-M3-07 多平台输入验收 | 03–06 | Windows/macOS/Linux 实际 IME smoke，测试 DPI、跨屏、失焦、撤销和多语言；记录未验证项 |
| [ ] T-M3-08 M3 交付 | 07 | AT-04/05、可制作真实中文图文文稿；保存重开文字样式/Unicode 不破坏；发布测试包 |

## M4 场景与加速

用户已授权在保留 M3 未完成项的情况下推进。父任务按原完整定义保持开放；以下子项记录本机已完成的范围，不将后续领域模型、真实 device-lost 或未测平台改写为通过。

对应：FR-SLD-05，NFR-02/05/07；[渲染规格](06-render-text-spec.md)、[性能预算](09-quality-platform-release.md)。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [ ] T-M4-01 固定性能方法 | M3-08 | 固定 BENCH-EDIT/EFFECT、轨迹/seed、基准机，记录 R0 的 input/command/layout/raster/upload/present 与内存 |
| [ ] T-M4-02 提取 Scene | 01 | 背景、Path/Text/Image/Group、clip/effect/命中/链接，CPU adapter 保持正确；分批迁，不删除可运行基线 |
| [ ] T-M4-03 GPUI 图形图片 | 02、M0-05 | 常用 fill/stroke/image/transform/clip 与缓存；半透明/z-order/裁剪正确；拖动不整页 CPU raster |
| [ ] T-M4-04 文字绘制桥 | 02、M3-01 | 保持原 glyph layout，缓存 glyph mask/outline 或验证可用 bridge；放大/移动不重复全文 shaping |
| [ ] T-M4-05 局部 fallback | 03、04 | 按语义封闭 subtree raster，halo/opacity/clip 不重复；复杂 effect 对照 CPU，无错序和双重 alpha |
| [ ] T-M4-06 失效与调度 | 02–05 | changed IDs/母版/字体/资源 generation、tile/scale cache、有界 worker、合并拖动与缩略图优先级 |
| [ ] T-M4-07 诊断与设备恢复 | 03–06 | 后端覆盖、实际耗时、raster/upload/cache bytes；device lost 清理及支持范围内恢复/受控退出 |
| [ ] T-M4-08 画质及性能验证 | 01–07 | 同机 CPU/GPU 对照、glyph 和像素容差、09预算/06相对门槛、overlay-only 计数不增长 |
| [ ] T-M4-09 M4 交付 | 08 | AT-07/15、加速对照演示、可运行包、ADR 后端结论；不以静态 FPS 替代结果 |

### M4 本轮实现与验证（2026-10-30）

证据、最终包和完整遗留矩阵统一见 [26](26-m4-implementation-report.md)。

- [x] T-M4-01a：固定 seed、48 帧轨迹、EDIT 的 120 几何/60 文本/20 个独立 512² 资源与 EFFECT 的 30 组合；同机 CPU/GPUI release、画质、RSS 和后端计数。01 剩余：完整 OS input/command/layout/driver upload/present trace。
- [x] T-M4-02a：无 GPUI 类型的不可变 Scene、稳定 ID/文档/页/revision、路径/glyph/image/clip 和 CPU 对照。02 剩余：公开 Group/Effect/Link/Hit 元数据、后续原生模型集成。
- [x] T-M4-03a：常见 fill/stroke/transform、image/sprite 与缓存；透明度/z-order/裁剪对照，平移不整页 raster。03 剩余：完整前置条件与跨平台/跨屏验收。
- [x] T-M4-04a：沿用 CPU SVG glyph ID/轮廓/位置，逐 glyph 的 16 相位 mask 共享纹理，平移不重新全文 shaping/上传。04 剩余：M3-01 统一活动文字编辑布局与完整 DPI/字体矩阵。
- [x] T-M4-05a：语义封闭对象隔离 fallback，渐变/透明组/clip/blur CPU 对照；修复重复预乘，保留原顺序。05 剩余：后续复杂领域 subtree 和未测平台。
- [x] T-M4-06a：内容 LRU/窗口 GPU 预算、scale/DPI 失效、单后台任务合并/取消、跨文档迟到结果丢弃、atlas 移除；文字在 shaping 前限额且保留原文。06 剩余：母版/动态字体 generation、tile、完整优先级、多窗口/长会话预算。
- [x] T-M4-07a：显示后端和真实适配器计数、CPU 切换、缓存重建且文档/undo 不变。07 剩余：真实 device-lost 注入/恢复；公开 GPUI 接口限制见26，不把缓存清理当作设备故障验收。
- [x] T-M4-08a：45 项自动测试、release 实际窗口反馈、CPU/GPU 像素差分与 RSS 采样、生产处理器平移/undo/redo/过期 job 断言。08 剩余：完整输入链路、09 的全部数据集/资源门槛及平台矩阵。
- [x] T-M4-09a：可运行 release ZIP、校验清单、QA 脚本、源码回退及后端决策；M0–M4 与系统拼音回归见26。09 剩余：完整 AT-07/15 人工/硬件矩阵、设备恢复与前置阶段退出条件。

## M5 图片与对象操作

对应：FR-IMG-01/02/03/04、GEO-03/04/05/06/08。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [ ] T-M5-01 资源与图片导入 | M4-09 | 类型嗅探、decode budget、EXIF、拖放、替换、资源去重；图片在 CPU/GPU 输出一致 |
| [ ] T-M5-02 裁剪与图片面板 | 01 | 非破坏 crop、aspect/重置、填充/变换；预览取消还原，保存重开不破坏原图 |
| [ ] T-M5-03 系统剪贴板 | 01、M3-06 | plain/rich text、图片、版本化内部对象；平台 adapter、fallback 与独立 transferable snapshot |
| [ ] T-M5-04 SVG 与 metafile | 01 | 受限 SVG，无脚本/联网；EMF/WMF 实现范围与 PNG fallback；异常路径/超大图受控 |
| [ ] T-M5-05 预设及高级路径 | M2-04、M4-02 | 批次迁预设/调节点、freeform/curve/scribble、Edit Points 和布尔合并；每批可用且有退化输入验证 |
| [ ] T-M5-06 对齐分布和组 | M2-05 | 顺序/对齐/分布/翻转/锁定、嵌套 group transform、组内选择；数值和手势相同结果 |
| [ ] T-M5-07 连接符与选择面板 | 06 | glue/reroute/delete target、名称/可见/锁定/z-order；逻辑阅读顺序不由 z-order 覆盖 |
| [ ] T-M5-08 资源和平台回归 | 01–07 | 跨文档对象暂存迁移接口，三平台图片粘贴、连续 copy/delete/undo、资源引用/缓存回收 |
| [ ] T-M5-09 M5 交付 | 08 | AT-08/03，“粘贴→裁剪→组→连接→对齐→保存重开”，可运行包与能力清单 |

### 本轮已完成子项（完整父任务不勾选）

- [x] T-M5-01a：PNG/JPEG内容嗅探、有界解码、EXIF、独立资源哈希与去重、后台路径导入/替换和单文件拖放接线；图片CPU/Scene输出验证见27。三平台真实拖放仍待测。
- [x] T-M5-02a：归一化非破坏crop/reset、非法输入整次拒绝、单次undo/redo与资源保存重开；交互preview/cancel/aspect仍开放。
- [x] T-M5-03a：系统图片/文字fallback、v1内部对象快照含rich runs与资源、按钮/画布快捷键；macOS adapter验证通过，外部rich与Windows/Linux仍开放。
- [x] T-M5-06a：按旋转视觉边界的六向对齐/横纵等间距分布，单事务undo，数量不足拒绝；组/翻转/锁定仍开放。
- [x] T-M5-08a：独立transfer在源会话销毁后仍合法，ID重新分配、资源去重/删除回收/undo/redo及恢复快照回归；长期压力与三平台未完成。
- [x] T-M5-09a：stage M5、50项测试、release开发包、能力/格式声明、原生窗口和旧阶段回归证据；完整AT-08因组/连接等缺失未通过。

## M6 文稿组织与设计

对应：FR-APP-04/05/07、DOC-08、SLD-03/04/06、TXT-06/07/08、GEO-07、IMG-05、DSN-01/02/03/04。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [ ] T-M6-01 ViewSession 解耦 | M5-09 | 选择/视口与文档分离，多文档/同文档双窗口共用写入者和历史；跨窗口 undo 不串目标 |
| [ ] T-M6-02 继承和占位符 | M4-02 | slide/layout/master/theme 优先级、kind/idx 匹配、reset 不丢内容、依赖页失效 |
| [ ] T-M6-03 母版版式编辑 | 02 | 插入/复制/重命名/删除/占位符工具；拒绝悬空引用；被用版式删除策略可见 |
| [ ] T-M6-04 主题及背景 | 02、03 | 主题颜色/字体、渐变/图片/图案背景、页码/页脚、模板保存；主题色不固化 RGB |
| [ ] T-M6-05 完整格式面板 | M5-02、04 | 阴影等已支持效果、图片压缩/基础修正、mixed/draft/preview/cancel；未支持功能按能力禁用 |
| [ ] T-M6-06 文字扩展 | M3-08、02 | autofit、列/边距/锚点、字段/链接、查找替换/字体替换；动态字段上下文稳定 |
| [ ] T-M6-07 节视图与备注 | 01 | 节、大纲/排序/备注视图、文档属性；切换保留有效焦点，不修改页面实际尺寸 |
| [ ] T-M6-08 跨文档页复制 | 01–04、M5-03 | 重新映射 IDs/母版/媒体/关系；保留源格式或目标主题；history/关闭源文档后仍合法 |
| [ ] T-M6-09 本地化和访问性 | 01–07 | 所有已交付文案 resource keys、简中/英文、菜单/区域键盘/role/name/state/画布对象语义；真实读屏初验 |
| [ ] T-M6-10 M6 交付 | 08、09 | AT-09/05/14，多母版/双窗口/跨文档演示，可运行包与本地化覆盖报告 |

### M6 本轮完成子项（完整父任务继续开放）

- [x] T-M6-01a：SharedSession单写入者/历史、同文稿双窗口独立选择/当前页、过期写入与远端草稿守卫、RAII保存租约/旧快照dirty保护；本机原生共享修改/跨窗undo与选择验证见28。多文档标签/同路径自动复用和全部关闭/恢复/长期矩阵仍待。
- [x] T-M6-02a：theme/master/layout/文字placeholder kind/idx，母版→版式→direct解析、CPU/Scene/缩略图共源、应用/重置内容和ID保留；完整继承属性与非文字占位符仍待。
- [x] T-M6-03a：原创设计、复制/重命名母版/版式、title字号/字段占位符操作；在用版式删除拒绝/领域替代、悬空母版保护和原生往返。完整母版画布编辑仍待。
- [x] T-M6-04a：主题颜色/正文字体与纯色背景、页码/固定日期上下文；渐变/图片/图案、完整页脚与模板格式未完成。
- [x] T-M6-06a：区分大小写的文字查找定位/全局替换、字体替换、UTF-8范围及10000匹配限额、一次undo；autofit/列/锚点/链接及完整字段未完成。
- [x] T-M6-07a：文稿属性、多行备注的页面/冲突守卫、节创建/重命名/选中页归节/保留页删除与大纲导航；节折叠/移动及完整排序/备注视图仍待。
- [x] T-M6-08a：独立PageTransfer，源销毁后可粘贴；重映射页/对象/设计/节IDs、哈希媒体、自持样式/目标主题入口、尺寸比例转换与undo；完整格式/平台/长期矩阵仍待。
- [x] T-M6-10a：stage M6、60项测试、release包、能力/格式/回退声明、最终包旧阶段/原生/像素/输入法证据见28；T-M6-09和完整AT-09/05/14未通过，完整交付不勾选。
- [x] 前序收尾：M5普通对象transfer固化设计外观并解除binding/field；新建页清空备注/节，避免复制第一页私有备注；共享视图背景结果发布前同步revision。不是M0–M5全部收尾完成。

## M7 MVP 文件交换

对应：FR-DOC-07、IO-01/02/03/04、SHW-01。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [ ] T-M7-01 PPTX package sidecar | M6-10 | 未知 part/relationships/owner/raw bytes、继承扩展、对象复制删除映射；unknown closure 测试而非只检查 XML 存在 |
| [ ] T-M7-02 PPTX 写回与能力表 | 01 | 受支持模型写回、content types/namespaces、图表/SmartArt/OLE 保留；Import/Render/Edit/RoundTrip 分列 |
| [ ] T-M7-03 损失与冲突策略 | 02、M2-07 | C0–C3、损失清单、默认另存、external fingerprint/只读/权限/空间不足；保存前后 revision 正确 |
| [ ] T-M7-04 native 迁移加固 | M2-06、01 | unknown field bag/requiredFeatures、资源 hash、schema 迁移、保存重开与本项目扩展数据 |
| [ ] T-M7-05 图片与基础 PDF 导出 | M4-02、M6-06 | page range、DPI/透明、备注/讲义、selectable text/links/bookmark；栅格范围如实标注 |
| [ ] T-M7-06 静态放映 | M4-09、M6-07 | 全屏 current/first、隐藏页、键盘/点击/退出、ShowSnapshot；不依赖 M10 时间轴即可运行 |
| [ ] T-M7-07 MVP 语料库 | 01–05 | ≥40 样本及≥5授权外部样本，结构/语义/视觉/PowerPoint 重开验证，记录版本和授权 |
| [ ] T-M7-08 三平台 MVP 验证 | 03–07 | 实际打开/编辑/IME/保存/剪贴板/静态放映 smoke，检查测试包，不仅 cargo check |
| [ ] T-M7-09 MVP 发布 | 07、08 | AT-10/11/12静态/14/15，MVP 范围声明和包、回退点；不把合成语料泛化为任意 PPTX 无损 |

### M7 本轮完成子项（完整父任务继续开放）

- [x] T-M7-01a：原PPTX部件字节/关系/模型基线/文本节点映射，内部关系闭包验证；unknown owner→关系→二进制资源回归，原样及唯一单行文字patch保留。复杂对象修改/复制删除映射未完成。
- [x] T-M7-02a：独立OPC/PresentationML静态输出（常见形状/文字runs/图片crop/变换/隐藏页/备注），受公共部件hash约束的原生快照；公共内容由独立读取器验证，PowerPoint单样本无修复打开。完整C2与外部编辑往返未通过。
- [x] T-M7-03a：原包保留/有损新建入口、兼容诊断/损失确认取消、不可变快照导出、不覆盖/失败清理及取消守卫；完整损失等级/竞争/故障UI仍待。
- [x] T-M7-04a：schema1.5/minReader6/source部件持久化、原生/源包往返、旧版读取及requiredFeatures拒绝；完整unknown-field bag与所有迁移未完成。
- [x] T-M7-05a：页范围/可见页、PNG alpha/JPEG与DPI、图像/批次预算、基础栅格PDF含Unicode文本行映射/书签/备注；Poppler/pypdf渲染与提取通过，精准选择/链接/讲义/打印未完成。
- [x] T-M7-06a：不可变ShowSnapshot、首/当前页全屏、隐藏过滤、键鼠导航/Esc退出，编辑期间快照独立；本机adapter/截图通过，物理输入/跨屏/三平台完整流程仍待。
- [x] T-M7-07a：40份原创合成PPTX结构/原生快照语义与独立公共读取，记录来源；≥5授权外部语料及全部视觉/PowerPoint闭环未完成。
- [x] T-M7-07b：M8本轮原创表格由PowerPoint16.114.1实际打开/编辑/另存，JoyDeck公共导入和集成样例重读131及合并结构；仅此受支持表格样例完成，不代表整套外部语料或完整PPTX，见30。
- [x] T-M7-09a：stage M7、66项测试、release包/哈希/源码回退、M0–M6及M7/拼音最终包回归与能力清单。完整AT-10/11/12静态/14/15和三平台MVP仍开放。
- [x] 前序收尾：导入/导出后台revision守卫、源包兼容诊断随native重开恢复、图片透明/来源资源的输出覆盖、M6模型在有效快照中重开；首轮空备注母版导致PowerPoint修复已定位并改用标准独立备注页，复验通过。不是M0–M6全部收尾完成。

## M8 表格

对应：FR-TBL-01/02/03/04。**本机受支持范围done**；具体预算、格式子集、79项测试、GUI/系统拼音/PowerPoint闭环及平台限制见 [30](30-m8-implementation-report.md)。前置M7未关闭的跨平台/复杂兼容门禁仍保留，不因表格开发完成而消失。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [x] T-M8-01 表格场景与选择 | M7-09 | table model/Scene/cell hit、合并主单元格映射、range selection；CPU/GPU一致 |
| [x] T-M8-02 单元格输入 | 01、M3-03 | rich text/IME、Tab/Shift+Tab/方向键、行末新增策略；焦点与文字选择不串 |
| [x] T-M8-03 结构与尺寸编辑 | 01 | 增删/合并拆分/拖动/均分，最小尺寸和非法交叉区域拒绝；单事务 undo |
| [x] T-M8-04 表格样式 | 01、M6-02 | 主题、边框/标题行/条纹/内边距/对齐；直接覆盖与继承关系正确 |
| [x] T-M8-05 数据与格式往返 | 02–04 | TSV 矩形粘贴、预算、失败回滚、native/PPTX merged cells 与 border 回归 |
| [x] T-M8-06 M8 交付 | 05 | AT-16，真实制作/交换表格，更新 corpus 和能力矩阵，测试包 |

本轮额外收尾（done）：重构功能区/缩略图/工作台/上下文属性栏/缩放/备注；系统文件选择器和快捷键；恢复原有形状/图片/对齐入口；修复Tab路由、合并主格/范围、对象剪贴板、表格缩放草稿及跨文稿缩放；表格PDF文字与字体诊断；M0–M8最终包回归。

## M9 图表与 SmartArt

对应：FR-CHT-01/02/03/04。本机受支持范围done；七类基础图表、受限公共格式和真实Office往返见 [31](31-m9-implementation-report.md)。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [x] T-M9-01 图表模型与场景 | M8-06 | 基础类型、数值/空值/分类/系列、单位/axis、Scene 与 CPU/GPU/PDF入口 |
| [x] T-M9-02 数据网格 | 01 | 编辑/粘贴/范围校验/undo，数据改变立即可见；UI Table 不成为 document model |
| [x] T-M9-03 元素和样式 | 01、02 | 标题/图例/标签/坐标轴/网格/颜色，受支持类型切换；未知特性不伪造默认值 |
| [x] T-M9-04 图表包数据同步 | 02、M7-01 | cached data/嵌入 workbook/chartex sidecar 保留或更新策略；修改后不留互相矛盾的两套数据 |
| [x] T-M9-05 SmartArt 基础编辑 | M6-02 | 列表/流程/层级、文本面板、layout switch、组转换损失提示、undo |
| [x] T-M9-06 复杂保留回归 | 04、05 | 未知 chart/SmartArt/OLE closure、图表 parts 大小预算、跨文档复制 native/PPTX |
| [x] T-M9-07 M9 交付 | 06 | AT-17，基础图表数据演示和 PowerPoint 重开；明确非全量 Excel/SmartArt 支持，测试包 |

本轮M9完成证据：七类图表（面积非堆积、环形内径50%）、空值/预算/原子网格与TSV/undo、缓存与嵌入XLSX一致生成/导入核对；三类基础图示与转换确认；未知复杂部件原包闭包和native重开、受支持图表/图示跨文稿复制。PowerPoint实际Excel保存后133公共重读，旧缓存91/工作簿132拒绝误读。未知包闭包不等于可编辑未知对象或跨文稿无损迁移，完整外部语料门禁仍在M7/M13。

## M10 动画与双屏

对应：FR-ANM-01/02/03/04、SHW-02/03/04/05/06。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [x] T-M10-01 ShowController 与时钟 | M7-06、M9-07 | 状态机、snapshot、monotonic/virtual clock、pause/seek、repeat/revisit 规则 |
| [x] T-M10-02 动画 evaluator | 01 | 基础效果、start rule/duration/repeat/rewind、按段target、无效数据限额，确定性采样 |
| [x] T-M10-03 动画面板与预览 | 02 | 排序/时间/目标、预览/停止还原、同一 evaluator、原数据 timing 保留 |
| [x] T-M10-04 基础转场 | 01、M4-05 | none/fade/push/wipe、前后页缓存、fallback、时间采样和视觉对照 |
| [x] T-M10-05 Morph 子集 | 04 | 明确匹配key/冲突/无匹配规则，几何/opacity、crop专项；不把路径和逐字效果一起默认达标 |
| [x] T-M10-06 演讲者窗口 | 01、M0-06 | 当前/下一页/备注/计时/导航，两个窗口单一时钟，观众无私密备注 |
| [ ] T-M10-07 屏幕 adapter | 06 | 选择/交换display、DPI、全屏、镜像/热插拔；设备 ID 变化安全退回 |
| [x] T-M10-08 演示工具和顺序 | 02、06 | 黑白屏/激光/墨迹保留放弃、链接跳转、自定义顺序、排练和自动翻页 |
| [ ] T-M10-09 放映专项验证 | 03–08 | BENCH-SHOW、睡眠/唤醒、前后跳转、双屏热插拔、PPTX动画参数往返 |
| [ ] T-M10-10 M10 交付 | 09 | AT-12完整，真实双屏演示、支持效果表、可运行包 |

### M10已完成子项与未完成门禁

- [x] T-M10-07a：原生display枚举/选择/交换、唯一时钟代次、屏幕丢失暂停与超长间隔暂停的软件路径；单有效显示器下双窗口、静态全屏实测。07剩余：两块物理屏幕、不同DPI、镜像/热插拔和设备ID变化的真实验收。
- [x] T-M10-09a：确定性时间轴、基础公共PPTX参数往返、原生时钟/pause/黑白屏/导航/快照/时钟移交/退出焦点、600帧CPU基准、GUI演示工具与前序回归。09剩余：真实睡眠唤醒/双屏热插拔、端到端显示帧率及完整性能矩阵。
- [x] T-M10-10a：可运行macOS ARM64包/哈希/回退、效果与格式能力表、PowerPoint与JoyDeck真实GUI证据及任务/状态文档。10剩余：完整AT-12和真实双屏演示，不能用单屏两窗口替代。
- [x] 前序收尾：补齐图标资源、PowerPoint式功能区分组/尺寸/上下文页签、系统字体/形状/表格菜单、默认收起属性栏、备注折叠；修复演示者图像挤出备注、输入对比度与退出放映后编辑器焦点；M7截图等待实际首帧，保留原像素阈值。

以上done限定已说明的软件子项。M0–M7尚未实现的文字布局、DocumentStore、完整组/母版/本地化/读屏、公开兼容与外部语料等，仍是开发任务；并非全部等待硬件。M11/M12开发切片已实现，完整阶段仍开放；见[33](33-m11-implementation-report.md)、[34](34-m12-implementation-report.md)。

## M11 音视频

2026-10-30：macOS媒体切片已实施，完整阶段未done；[33实施/证据/限制](33-m11-implementation-report.md)。

对应：FR-MED-01/02/03。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [ ] T-M11-01 最小媒体依赖 | M10-10 | codec/container 按 feature 引入通用库；记录独立适配器与依赖许可；probe 返回真实格式/profile支持 |
| [ ] T-M11-02 音频输出与时钟 | 01 | cpal/平台音频适配、bounded PCM、resample/mix、音频master；回调不锁文档/解码/大分配 |
| [ ] T-M11-03 视频解码显示 | 01、02 | H.264 MP4指定样本后台解码、帧队列/海报/过期帧丢弃，GPU图像与预算 |
| [ ] T-M11-04 媒体交互与参数 | 02、03 | play/pause/seek/trim/volume/loop/autoplay/cross-slide，编辑参数可撤销，播放态不写文档 |
| [ ] T-M11-05 错误设备及往返 | 04 | 设备切换、断音频、非法/不支持流、seek generation、media timing native/PPTX 保留 |
| [ ] T-M11-06 性能与稳定性 | 03–05 | BENCH-MEDIA、连续seek/loop、10分钟drift、队列/内存，无冻结 |
| [ ] T-M11-07 M11 交付 | 06 | AT-13，带音视频实际演示和支持格式/profile清单，测试包 |

### M11本机已完成子项（父任务保留开放）

- [x] T-M11-01a：独立AVFoundation adapter、真实codec/profile/fps探测；PCM/WAV、MP3、H.264 Baseline/Main 8-bit MP4/AAC样本，许可/来源与预算有记录。
- [x] T-M11-02a：系统默认音频输出与AVPlayer媒体时钟；600秒实际音轨/host时钟/RSS采样。多轨mix、可观测PCM回调、完整ShowClock联动未完成。
- [x] T-M11-03a：1080p30 H.264 Main/AAC后台真实解码、最新帧队列、960宽BGRA→GPUI图片；20次seek/暂停/循环验证。独立视频矩形/海报及完整A/V标定仍待。
- [x] T-M11-04a：媒体面板、参数undo、play/pause/seek/stop、trim/volume/loop/autoplay、每页首个clip/跨页音频软件路径；实际预览/暂停帧/全屏放映截图已验证，预览区域压缩问题已修复；M13/M14补齐导入后真实时长/音量回显与并发临时目录保护。
- [x] T-M11-05a：非法流/profile拒绝、seek代次丢弃、native独立资源/hash/参数保存重开、进程清理。设备显式切换/拔除及公共PPTX媒体往返未完成。
- [x] T-M11-06a：实际十分钟音频位置/host drift与adapter RSS、真实帧预算及重复seek/loop；不是完整BENCH-MEDIA/A/V/设备矩阵。
- [x] T-M11-07a：本机开发包、支持格式清单、媒体面板真实帧与全屏视频演示已验证；设备矩阵/完整AT-13仍属于父任务。

## M12 审阅自动化与输出

2026-10-30：M12a/b/c已有实际可运行切片，完整阶段开放；[34实施/证据/限制](34-m12-implementation-report.md)。

对应：FR-REV-01/02、AUT-01/02/03/04、IO-05/06/07。按 a/b/c 三次小发布保持可见进展。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [ ] T-M12-01 评论与线程 | M11-07 | 作者/回复/解决、页/对象锚点、legacy/modern comments保留、undo与PPTX回归 |
| [ ] T-M12-02 文稿无障碍 | 01、M6-09 | alt text/装饰/独立阅读顺序、缺标题/替代文字检查、可定位报告，schema迁移 |
| [ ] T-M12-03 M12a 交付 | 01、02 | AT-19，审阅可用测试包；文稿检查不冒充 Tagged PDF或完整WCAG认证 |
| [ ] T-M12-04 无框架 dispatcher | M7-03 | transport请求/响应、target、job、error、request去重，UI宿主适配；协议模块不依赖GPUI |
| [ ] T-M12-05 CLI | 04 | commands/describe/inspect/run/render/convert、退出码/stdout/stderr、headless构建与测试 |
| [ ] T-M12-06 本地服务边界 | 04 | 默认关闭、loopback/IPC、token、roots、请求/队列预算、queued取消与outcome-unknown |
| [ ] T-M12-07 MCP 后端 | 05、06 | 锁协议版本，新版discover/请求级能力或旧版initialize按版本隔离；tool/resource/notification/cancel、operationId去重、headless/remote一致 |
| [ ] T-M12-08 M12b 交付 | 05–07 | AT-18，UI/CLI/MCP同结果、重复请求/未认证/越界/dirty close回归，自动化可用包 |
| [ ] T-M12-09 矢量 PDF | M4-02、M9-01 | Scene路径/文字→PDF，字体子集/提取/阅读顺序/链接，局部raster，画面与结构验证 |
| [ ] T-M12-10 系统打印 | 09 | 三平台print adapter，纸张/页范围/方向/缩放预览，取消不改文档，fallback PDF |
| [x] T-M12-11 大纲 GIF及变体 | M10-02、M7-02 | outline/RTF/GIF、静态/动画能力标签，potx/ppsx真实content type专项；不声称MP4已支持 |
| [ ] T-M12-12 M12c 交付 | 09–11 | AT-11扩展/19，打印真实测试与PDF放大/提取、输出能力报告和测试包 |

### M12已完成子项与未完成门禁

- [x] T-M12-01a：原生页/对象评论线程、作者/回复/解决、预算、undo、锚点删除联动与native重开。Office legacy/modern评论解析/重新输出尚未实现。
- [x] T-M12-02a：原生alt/装饰/独立阅读顺序与迁移、标题/alt/低透明文字定位检查；完整对比度与读屏未验收。
- [x] T-M12-03a：审阅开发包、评论/回复/解决/undo与真实面板截图、检查结果定位完成；Office评论/完整无障碍和AT-19属于父任务。
- [x] T-M12-04a：无GPUI依赖的请求/target/error/有界去重dispatcher、桌面SharedSession宿主；异步job协议未实现。
- [x] T-M12-05a：真正headless CLI构建，commands/describe/inspect/run/render/convert、JSON/退出码；当前开放page.add/object.add/review.edit及历史/文件方法，非全部领域命令。
- [x] T-M12-06a：默认关闭、loopback/token/0600凭据、独立读写roots/符号链接逃逸拒绝、≤8同步连接、容量/超时/outcome-unknown、关闭清理；没有异步队列job/主动取消。
- [x] T-M12-07a：锁2025-11-25 stdio initialize/initialized、tools/resources/ping、operationId、真实Headless/Remote客户端与共享历史；完整进度/取消通知未实现。
- [ ] T-M12-08a：重复/冲突/未认证/越界/dirty-new与共享undo已验证并打包；完整UI/CLI/MCP命令覆盖、dirty close及AT-18仍开放。
- [x] T-M12-09a：实际矢量路径/文字、subset字体/ToUnicode、局部raster、多页/书签/内部链接、Poppler像素与提取验证；独立PDF阅读顺序/Tagged语义未完成。
- [x] T-M12-10a：macOS打印adapter、按页面比例默认方向、纸张/范围/方向/缩放控件、预览截图与取消不改文稿/修订号/dirty完成；真实打印及Windows/Linux adapter仍属于父任务。
- [x] T-M12-11：纯文字/RTF大纲、明确标注静态页GIF、真实POTX/PPSX主content type与独立读取；不含完整表格/图表大纲或MP4编码。
- [ ] T-M12-12a：输出开发包与结构/像素验证已有；完整打印、PDF阅读顺序和AT-11扩展/19未通过。
- [x] 前序收尾：GPUI Kit成为可选desktop依赖、原生媒体/打印helper及CLI随包、M11/M12 stage/CI入口、media/review schema与旧版拒绝、许可/源码回退。前序真实双屏、三平台和未实现功能继续开放。

## M13 跨平台 Beta

2026-10-31：本机资格化、偏好/原生集成和发布维护子项已实施，完整三平台Beta保持开放。见[36](36-m13-implementation-report.md)。

对应：FR-APP-07/08、TXT-09、全部 NFR 和已交付 FR。

| 任务 | 关键前置 | 工作与完成证据 |
|---|---|---|
| [ ] T-M13-01 扩充兼容语料 | M12-12 | ≥100份、授权/字体/PowerPoint版本、按特性覆盖，修复已声明C2中的结构/语义/视觉损失 |
| [ ] T-M13-02 长会话与资源 | 01 | seeded engine/UI soak、反复open/close、历史/缓存/媒体RSS，恢复与断电边界测试 |
| [ ] T-M13-03 性能发布门槛 | 02 | 所有BENCH的release数据、p95/p99/设备、绝对预算；未达标修复或明确缩范围 |
| [ ] T-M13-04 输入文字与读屏 | M6-09、M12-02 | 三平台IME、RTL/复杂脚本、NVDA/VoiceOver/Orca、键盘-only、DPI焦点；不以语义树存在代替读屏 |
| [ ] T-M13-05 GPU显示媒体矩阵 | M10-09、M11-06 | 集显/独显、Wayland/X11、双屏/镜像/热插拔、音频设备；已知驱动问题有重现/策略 |
| [ ] T-M13-06 偏好原生集成 | M6-01 | 最近文件/布局迁移、坏配置重置、mac菜单/打开事件、OS文件关联、单实例/多实例策略 |
| [ ] T-M13-07 打包签名与安装 | 03–06 | ZIP/安装器、mac签名公证、deb/tar；干净环境安装/启动/卸载/路径/依赖验证 |
| [ ] T-M13-08 更新与回退 | 07 | 签名/来源验证、保留可用版本、失败恢复、schema兼容；没有凭据时不宣称正式签名验证完成 |
| [ ] T-M13-09 许可帮助与发布说明 | 01–08 | notices/SBOM/字体来源、支持能力/平台/恢复帮助、诊断脱敏、版本/lock/features/hash |
| [ ] T-M13-10 Beta 发布验收 | 09 | AT-20、全部发布阻断项关闭，使用正式包完成闭环，STATUS/追踪矩阵更新 |

### M13已完成本机子项（父任务保持完整验收范围）

- [x] T-M13-01a：128份原创变体native重开/强制公开PPTX读取/渲染，python-pptx独立检查1152页/896图表/标题数据与hash；0份独立外部语料，父任务≥100份授权外部Office文件仍待。
- [x] T-M13-02a：600秒seeded引擎370081次命令、740次原子保存/重开、undo/redo与100项历史上限；原生24次开关窗口/120次编辑undo；并发媒体临时目录修复。非小时级全功能会话/真实断电认证。
- [x] T-M13-03a：本机release M4四场景像素/性能/450MiB门槛，命令p95/p99与可追踪内存采样；完整SMALL/LARGE/端到端设备矩阵仍待。
- [x] T-M13-04a：最终包macOS普通文字/表格系统拼音、候选窗/提交/取消/undo/native重开回归；三平台IME/RTL/读屏/完整键盘-only仍待。
- [x] T-M13-06a：schema2偏好/最近12文件/布局恢复、v1迁移、坏配置保留、未来schema保护、文件锁与更新代次；mac菜单/本地file事件、重复打开激活现有视图，菜单撤销/重做及保存页数回读。完整三平台关联/实例策略安装验收仍待。
- [x] T-M13-07a：自包含macOS ARM64 App、CLI/三个helper/材料随包，中文空格路径移出仓库实际运行，manifest/系统动态依赖检查。签名/公证/三平台干净安装仍待。
- [x] T-M13-08a：可信hash、私有副本、安全解包/容量/reader门槛、不可变版本目录/原子选择/上一版回退；14项负例与真实两包升级回退验证。正式签名正例缺凭据，父任务开放。
- [x] T-M13-09a：483项SPDX依赖图/官方schema验证、原始许可索引/39项精确提交补充、帮助/脱敏diagnostics、toolchain/lock/features/source hash和每文件hash。16项notice文本来源及完整再分发审查仍待。
- [x] T-M13-10a：本机开发资格化索引/报告/旧阶段回归、Windows/Linux无界面核心跨目标check/Clippy、开发包与源码回退；不是正式Beta/AT-20验收。

## M14 后续功能池

2026-10-31：静态页MP4、离线比较/逐对象接受、标题VTT已有可运行扩展；其余功能池未完成，见[37](37-m14-implementation-report.md)。

以下为独立立项任务，前置均含对应基础阶段，不纳入 Beta 必须完成的总工作量。

| 任务 | 关键前置 | 下一步具体工作及立项门槛 |
|---|---|---|
| [ ] T-M14-01 长尾文件格式 | M7、M13 | 分开评估PPT二进制/ODP/宏保留/加密，明确公开规范、许可、语料和无损边界，再拆独立里程碑 |
| [ ] T-M14-02 动画与设计长尾 | M9、M10 | 全量效果、复杂chartex/SmartArt/3D、母版扩展和离线建议，逐特性能力表与视觉golden |
| [ ] T-M14-03 高级文字 | M3、M6、M13 | 真竖排、公式、warp、拼写/同义词/自动更正，先定义文本语义和授权字典 |
| [ ] T-M14-04 录制与视频输出 | M10、M11、M12 | 确定编码/容器/旁白/摄像头权限、时间轴和同步，交可播放小视频再扩展 |
| [ ] T-M14-05 硬件媒体及GPU互操作 | M4、M11 | 各OS surface/device/fence/format验证，测收益，保留稳定CPU fallback与失败边界 |
| [ ] T-M14-06 比较与协作 | M6、M12 | 比较合并先做离线差异，协作另定服务/冲突/权限协议；不先无理由引入CRDT |
| [ ] T-M14-07 额外平台与扩展 | M13 | ARM64/BSD/Web/插件分别评估工具链/窗口/安全模型，独立可运行产物 |
| [ ] T-M14-08 高级输出与文档检查 | M12 | Tagged PDF、caption/字幕、更多可访问语义和版式检查，独立结构/读屏验证 |

### M14本轮已完成扩展及下一步

- [x] T-M14-04a：macOS静态页H.264 Main MP4、显式无音轨/动画/转场/录制范围、有界帧通道/取消/新目标保护；最终包导出后实际播放，独立解码270帧/18秒/PTS/像素验证。
- [x] T-M14-06a：稳定ID离线比较、≤4096项报告、base/reference hash保护、逐对象新增/替换/删除进入undo；GUI/CLI/MCP同核。资源须已存在，非完整三方合并/实时协作。
- [x] T-M14-08a：按静态视频相同时间轴生成页标题WebVTT，标记转义、无私密备注；不含语音转录/内嵌字幕或Tagged PDF。
- [ ] T-M14-04b：动画/转场/音轨/嵌入媒体时间轴、旁白/录制与同步标定，分别定义语义、权限及可播放样例。
- [ ] T-M14-06b：完整资源/版式/主题依赖迁移与三方冲突选择，再评估实时服务与协作协议。
- [ ] T-M14-08b：PDF独立阅读顺序/Tagged结构和实际读屏，语音字幕及更多可访问语义。
- [ ] T-M14-01/02/03/05/07：长尾格式、全量动画/设计、高级文字、硬件互操作及额外平台/插件继续独立开发；未将现有基线回归计为新功能完成。

## 实施记录模板

开始任务时记录：任务 ID、对应 FR、前置状态、拟改模块、验证方式、兼容/性能/迁移风险。结束记录：实际结果、构建与平台、测试/手工步骤、已知限制、产物和回退版本。没有证据的结果保持未完成。

遇到需要用户偏好的选择，先完成不依赖该偏好的可评审工作；已有授权的日常实现不重复请求确认。外部语料授权、签名凭据、没有的硬件或新付费服务需要明确补齐条件，不把等待时间当作已验收。

## 2026-10-30 M3 预览与此前收尾增量

完整证据与逐阶段遗留见 [23](23-m3-implementation-report.md)。父任务仍按完整退出定义保留未勾选；以下表示已实现且有相应自动验证的子项。

- [x] T-M3-02a：Unicode 字素导航/删除、UTF-16 非法范围拒绝、反向选区模型；视觉行/bidi 与完整人工操作仍待完成。
- [x] T-M3-03a：画布输入桥、周边文字/marked/selected/候选几何，最终分发包 native 回归及 macOS 系统拼音已通过（见24）；物理键盘与其余输入法/平台尚未验证。
- [x] T-M3-04a：预编辑独立草稿、单次提交 undo、取消恢复、属性焦点守卫与格式化保留选区；系统拼音提交/取消/单次撤销已通过，完整工具栏/弹窗焦点人工验收待补。
- [x] T-M3-05a：run 模型、局部粗斜体/字体/颜色、样式 splice、schema 1.2 保存重开；段落与完整布局缺口未关闭。
- [x] T-M3-06a：plain clipboard 接线、按对象缺字体提示；rich clipboard、逐 cluster 字体诊断仍待完成。
- [x] T-M0-07a：macOS ARM64 本机实际链接与 M0 adapter smoke；Linux 和远端 CI 未执行。
- [x] T-M2-07b：新增保存目标占用/路径/Unix 权限故障验证；非全部故障矩阵。
- [x] T-M2-08b：macOS/Linux 恢复目录改为持久用户目录；异常退出子进程恢复回归通过。
- [ ] T-M3-01b：统一编辑/CPU 文字布局，自动换行、混合字号与坐标契约。
- [ ] T-M3-05b：段落列表、缩进、段距与空段语义。
- [ ] T-M3-07a：最终包 native 复跑及 macOS IME、DPI/跨屏/读屏验收；native 复跑及系统拼音已通过，DPI/跨屏/读屏与其余输入法仍待验收。

### 2026-10-30 解锁后验证补充

见 [24](24-m3-macos-verification.md)。最终分发包 M0/M1/M2/M3 adapter 回归全部通过。原先首帧/无活动窗口阻断解除；T-M3-07a 中完整 DPI/跨屏/读屏等条件仍未完成，父任务不勾选。

- [x] T-M3-03b：实际系统拼音集成，OS 键盘事件输入“zhongwen”，8 次预编辑/1 次提交、取消下一次组合、单次撤销/重做和保存重开；候选窗位于画布文字旁，截图已检查。
- [x] T-M3-01a：活动文本缓存按对象/比例/布局位置失效，native 断言证明 caret-only 不增加 shaping 计数；CPU/编辑统一布局仍未完成。
- [x] T-M3-06b：移除 LastResort 假覆盖、按完整 emoji 字素选择系统字体；中文/emoji/组合音标 glyph 回归通过。尚非所有脚本/字体的完整 fallback 验收。
- [x] T-M2-05b：修复画布 overlay 缺少 top/left 锚点造成的输入坐标偏移；真实输入截图和 M2 native 回归通过。
