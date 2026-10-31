# M8 表格、原生工作区改造与前序收尾

日期：2026-10-30；macOS ARM64，Rust1.98.1、gpui-kit0.8.1 / gpui-pre0.3.10。

## 结论与交付范围

M8表格的开发、领域测试、本机原生窗口、系统拼音及原创PowerPoint交换闭环完成；任务T-M8-01–06在本报告列出的受支持范围标为done。完整三平台/MVP门禁及前序尚未实现的功能继续独立开放；本轮不将M0–M7父阶段批量勾选。M9未开始。

同时重做主编辑工作区：白色标题栏、紫色强调、六个功能区页签、全高虚拟幻灯片导航、灰色工作台中居中的白色画布与阴影、280pt上下文属性栏、底部页码/状态/缩放/适应窗口和可折叠备注。文件路径与调试计数退出默认工作区，导出高级选项和母版管理按需展开。保留图片替换/裁剪、形状、对齐/分布、图层、页面排序/隐藏、共享窗口等既有入口。功能区页签及常用按钮在原生AX树可见；不等于已完成读屏验收。

文件打开、另存为、图片插入/替换、PPTX/PDF导出改用系统选择器；支持Cmd/Ctrl+O、S及Shift+S。原有指纹冲突、dirty close、后台快照/取消/迟到结果守卫继续生效。默认1360×900，fit同时受宽高限制、画布可滚动与缩放；不同窗口尺寸实测见GUI记录。其他DPI/设备仍需补齐。

## 表格模型与命令

独立table模型含行高/列宽、物理cell矩阵、主格span、覆盖格、rich runs、全表样式与局部覆盖。覆盖格不允许隐藏文字。验证拒绝重叠/空洞/越界span、NaN尺寸、非法颜色/对齐和超限数据；所有动作在draft验证后原子提交到共享Session，单次结构/尺寸/粘贴操作对应一次undo。

画布命中将覆盖格映射到主格，范围选择自动扩展完整合并区域；非法领域交叉范围仍拒绝。双击输入，Tab/Shift+Tab跳过覆盖格，末格Tab在预算内新增行；非编辑态方向键/Shift范围，编辑态方向键留给文字。文字使用既有EntityInputHandler/TextEdit，组合态为草稿，取消不修改文稿，提交后才发布；保留UTF-16/字素与富文本操作。点击“开始”保留活动文字选择，粗斜体/字体/颜色可作用于单元格文字。

支持插删行列、合并拆分、均分、分隔线拖动、外框缩放、Alt移动、标题行/条纹、主题accent/body、边框/内边距/字号/对齐及局部填充与恢复继承。合并按行优先串接非空内容并保留已解析样式；拆分保留主格内容。TSV要求有界矩形，遇覆盖/合并冲突、超限或非法范围整次回滚。对象剪贴板保留独立表格快照，普通TSV走单元格通道。

预算：每表1–128行/1–64列且≤4096格，全文稿≤16384格；导入行列≥8pt、≤4096pt，分隔线拖动≥24pt；每格≤40000字节、每表≤100000字符，渲染另有既有文字总预算及8MiB模型预算。所有几何仍受Object/文稿上限约束。

## 渲染与交换

CPU SVG与Scene使用同一表格展开、文字/填充/边框/旋转；GPU允许对表格局部栅格fallback，不宣称纯矢量表格加速。缺字体诊断覆盖表格默认/局部字体和runs，保留原名。PDF可见表格沿用栅格页，ActualText包含单元格文字，Poppler提取验证通过；不声称精准glyph选择、矢量或Tagged PDF。

writer升级为schema1.6/minReader7/table-v1，可读旧1.0–1.5，旧程序拒绝新文件。table/cell样式、span、runs和textAlign为显式模型；继承与直接覆盖分开保存。跨文稿普通对象复制冻结源主题外观，页面复制保留/映射设计并缩放表格网格及文字样式。

公开PPTX输出真实p:graphicFrame/a:tbl、gridCol/tr/tc、rowSpan/gridSpan/hMerge/vMerge、文字runs、对齐/边距、直接颜色与统一单元格边框。公开导入能重建这些受支持属性；去除专有快照的输出由python-pptx独立读取。合并语义参照[微软公开规范](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/de4562ba-7901-48df-9803-414ded131f2b)。未复制第三方演示文稿实现、模板或资源。

PowerPoint16.114.1（build16.114.26102031）打开本次GUI导出的原创合并表格无修复提示，识别表格设计/布局上下文；修改R2C2的130→131，另存powerpoint-edited.pptx。JoyDeck真实文件选择器重开显示131和2×2合并；固定样例纳入集成测试，再保存native及新PPTX验证模型与表格不变。专有快照哈希失效后读取公共数据，没有恢复旧130。此次Office编辑可用，解除此样例的旧只读许可验证障碍；不扩展为任意PPTX完整往返通过。

公开子集限制：主题表格style ID、四边不同的复杂边框、复杂段落/排版、嵌套对象不完整支持；外部字体替换会改变度量。长单元格使用显式换行，旋转表格需旋转归零再编辑文字。源包保留写回仍拒绝表格内容/结构修改，修改表格使用新建PPTX路线。M3统一布局/自动换行/bidi等前置缺口未因此关闭。

## 验证与前序收尾

- `cargo xtask check`：fmt、all-targets Clippy -D warnings、79项测试；`stage M8`实际链接与测试。包括500步固定seed结构扰动、恶意span/总格预算、TSV回滚、rich合并、主题/跨文稿缩放、public-only合并、真实Office改写样例、Scene/CPU像素与字体诊断。
- 最终包M0–M4原生回归、M5图片/剪贴板/像素、M6共享双窗口/主题/跨文稿/像素、M7输出/隐藏页/独立全屏快照/退出、M8表格/结构撤销/导航/组合草稿/分隔线事务/对象剪贴板及CPU/GPUI截图对照。
- macOS系统拼音分别在普通文字与表格中测试：真实系统候选、8次marked调用/1次commit、取消、单次undo/redo、native保存重开。是OS事件自动化，不是物理键盘或其他输入法验证。
- 真实GUI制作：中文粘贴、Tab/反向Tab、矩形TSV、Shift范围、2×2合并、系统保存/导出、Office编辑另存与JoyDeck公共导入；截图与保存文件可检查。最终分发App双击与首个Cmd+O通过；分隔线真实鼠标拖动将行高60→88.8，一次撤销后的native文件与原文件逐字节相同；1360×933及1920×969窗口、适应缩放与备注抽屉通过。[GUI验证索引](evidence/m8/gui/final-gui-verification.json)。

前序收尾：恢复原先对齐/分布、圆角矩形、图片替换入口；缩放同时考虑画布高度；修复启动首帧键盘焦点、Tab被框架焦点遍历抢先消费及Esc退出单元格导航；表格编辑期间只抑制活动格的底图文字；对象缩放草稿和跨页面尺寸交换同步网格；合并后选择映射回主格；保留富文本合并和源主题剪贴板；PDF文字层与缺字体诊断覆盖表格；更新M6/M7截图测试以打开新的设计/文件面板。M4旧窗口测试在fit坐标转换后产生100.000015pt，精确f32相等断言改为0.001pt容差；绘制/缓存/像素门槛未放宽。

第一次CUA文字按键不能驱动物理拼音组合，保留失败试验目录pinyin-release；随后用已有系统按键集成脚本完成并保留成功日志，失败记录不作为通过证据。M8初轮PDF提取器pypdf不解析ActualText，改用既有M7所用的Poppler提取，另以pypdf检查PDF结构；实际文字断言保持不变。

## 产物与证据

[最终release包（旧归档：joydeck-m8-release-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)包含双击JoyDeck.app、命令行binary、README、许可证、校验清单、fixtures和QA脚本。[最终验证索引](evidence/m8/verification.json)记录binary/ZIP哈希、全部结果及原始日志。GUI/PowerPoint证据在[evidence/m8/gui](evidence/m8/gui/powerpoint-verification.json)，受支持能力对照及实测边界以本报告为准。

[源码快照（旧归档：joydeck-m8-release-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md)；[开发前源码（旧归档：joydeck-before-m8-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md)；[M7回退包（旧归档：joydeck-m7-preview-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。本地目录无Git元数据，未创建提交。未做正式签名、公证、干净机器安装或远端CI；配置升级为stage M8不等于远端已运行。

截图对照：[改造前M7](evidence/m7/run-1793340619859559000/editor.png) · [改造后标准工作区](evidence/m8/gui/final-normal.png) · [宽窗口](evidence/m8/gui/final-wide.png) · [备注抽屉](evidence/m8/gui/final-notes.png)。

## 继续开放

Windows/Linux当前M8构建/桌面、其他DPI、跨屏/真实device-lost、物理键盘/读屏和外部授权兼容语料仍未验证。M0–M7尚未实现的DocumentStore/全格式字体诊断、完整文本布局/母版/本地化、组/连接符/SVG/高级路径、native unknown bag、完整PPTX复杂包编辑、矢量PDF/链接/打印及长期硬件/性能条件仍按原父任务开放。上述项不使用“M8开发完成”替代验收；详见任务清单及STATUS。
