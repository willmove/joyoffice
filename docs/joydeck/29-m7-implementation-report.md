# M7 文件交付增量与前序回归（2026-10-30）

本轮交付 **M7 文件交付开发预览**。新增PPTX原包保留/受限写回、独立静态PPTX输出、PNG/JPEG/PDF与全屏静态放映。完整M7/MVP及M0–M6全部退出条件仍未满足；不因40份原创合成样本通过就声明任意PPTX无损往返。

## 实现范围

### T-M7-01a / 02a：源包保留与静态PPTX写出

`interop.rs`保存原始部件字节、原模型基线及可唯一定位的文字映射。验证包内路径、成员/字节预算、关系namespace/ID/Type、内部目标和完整内部关系闭包；外部关系保留但不联网。原样写回保留每个原部件的字节，ZIP容器压缩/元数据不承诺相同。可修改范围严格限于唯一匹配、单run单段落的单行文字；位置/样式/结构/图片/重复文本、多run等修改拒绝。映射复核原XML文本节点/范围，写回时转义文字并按倒序更新区间，不覆盖未知子树；原型未知owner→关系→二进制资源闭包已有回归。

`pptx_write.rs`独立输出普通OPC/PresentationML：基本形状/文字runs及字体字号颜色粗斜体、图片/裁剪/透明度/旋转、纯色背景、隐藏页和备注。公开PPTX把现有继承/动态字段解析为静态对象；新建有损导出不携带原PPTX未知part/动画/图表/SmartArt/OLE，必须先接受损失声明，只新建目标。

附加的专有XML保存可独立验证的原生模型快照，使用全部公共部件的哈希约束。JoyDeck只在哈希完全一致时恢复该快照；外部应用改写后走受限公共导入并保留原包，不让陈旧快照覆盖外部内容。该快照不等同公共PPTX模型的全功能交换，不能作为C2外部兼容的唯一证据。

### 当前能力分列

| 特性 | 公共PPTX导入/绘制 | 导入后编辑 | 新建公开PPTX输出 | 原包保留写回 |
|---|---|---|---|---|
| 基础矩形/椭圆/线及简单文字 | 沿用已有受限子集 | 原生模型可改；兼容诊断可见 | 支持当前静态几何及文字runs | 原样保留；仅唯一单行文字可改 |
| 原生roundRect/arrow、rich runs/旋转 | 旧公共导入仍有限，可能占位/简化 | 原生文件与有效专有快照支持 | 支持静态输出，外部排版有差异 | 对应修改超范围拒绝 |
| 图片/crop/opacity | 旧公共导入仍可能占位 | 原生/有效专有快照支持 | PNG关系与crop/alpha/rotation | 原样保留；修改拒绝 |
| 母版/版式/主题/字段/节 | 公共导入未完整支持 | 原生与有效快照保有当前M6模型 | 公开内容固化，不保留原生继承语义 | 原样保留；结构修改拒绝 |
| 备注/隐藏页 | 隐藏标志读取；一般备注导入未完整实现 | 原生/有效快照可编辑 | 标准备注页与隐藏标志 | 原样保留；备注修改拒绝 |
| 图表/SmartArt/OLE/未知扩展 | 占位或兼容提示 | 未开放编辑 | 有损导出不携带，先声明 | 原部件/关系字节闭包保留 |

### T-M7-03a / 04a：数据安全与原生持久化

交付面板导入和导出在后台消费不可变快照；导入完成时检查文稿身份/revision，迟到结果丢弃。导出不改变编辑器文档、dirty或保存基线。所有UI输出默认新建，已存在目标拒绝；单文件同目录暂存、sync后以无覆盖方式发布，提交前检查取消。图片批次先暂存，再独占新建目录发布；失败只清理本任务已发布成员，不能视为跨文件系统事务。源包保留和有损另存入口分开，有损操作提供明确确认与取消。

原生升级schema1.5/minReader6，requiredFeatures增加pptx-source-v1，读旧1.0–1.4；旧reader拒绝新文件。原始PPTX部件存`source/package/*`，基线/映射存`source/metadata.json`，不内嵌document.json。源包随保存/恢复快照持久化；资源SHA-256契约不变。未知requiredFeatures仍拒绝；未知新模型字段仍拒绝可编辑打开，**尚无完整unknown-field bag**。包含源包的历史JSON估算可能先触发8 MiB模型预算，不据此宣称任意64 MiB输入均可编辑。顶层保留文件指纹/原子保存机制继续使用M2/M6实现。

### T-M7-05a：PNG/JPEG与基础PDF

- 页范围空白=所有可见页，或`1-3,5`；领域API可显式包含隐藏页，UI当前排除隐藏页。重复/越界/筛选后空页拒绝。
- PNG可透明、JPEG不透明，DPI36–600，写入PNG pHYs/JPEG density。每图边≤8192、≤16M像素、每批≤100页/总编码≤64 MiB，超限不静默缩小。文件名使用源页号slide-NNN。
- PDF可导出幻灯片或含备注页面，使用同源SVG栅格画面；附不可见ActualText行映射与页书签。中文提取及渲染已用Poppler/pypdf独立验证。不是矢量文字、精准逐glyph选择或Tagged PDF；目前没有PDF链接、讲义/打印。备注按字素折行并限40行，超限报错。

### T-M7-06a：全屏静态放映

ShowSnapshot持有不可变文稿，独立全屏窗口fit显示、跳过隐藏页；方向键/空格/Enter/PageUp/PageDown、Home/End及鼠标左右键翻页，Esc关闭回编辑器。源文稿继续编辑不改变已开始的放映。窗口不显示备注或编辑UI。原生adapter验证了全屏状态、首帧、隐藏跳页、前后/末页与退出；未冒充物理键盘/鼠标验收。M10动画/双屏演讲者能力不在本轮声明中。

## 验证

`cargo xtask check`：fmt、all-targets Clippy、**66项测试**通过（旧60+6项交付集成测试）。`cargo xtask stage M7`通过，release实际链接。新测试覆盖独立公开XML及关系图、快照哈希失效、未知关系闭包与安全文字patch、native源包往返、非法路径/悬空/重复关系、页范围/透明/DPI、不覆盖/取消清理、不可变ShowSnapshot、PDF内容/书签结构与外部修改冲突。

原生QA生成40份原创合成PPTX，做结构/原生快照语义往返，并由**python-pptx独立读取公共内容**，包括公开shape/文字/图片关系；生成样例包含中文、图片、隐藏页和备注。它们不是40份授权外部语料。PDF使用Poppler渲染并提取Unicode、pypdf检查页数/书签；PNG/JPEG使用Pillow检查尺寸和144 DPI。全屏截图对照导出画面。最终包结果、像素值、截图和哈希见 [交付索引](evidence/m7/verification.json)、[M7输出指标](evidence/m7/run-1793340619859559000/verification.json)、[放映窗口](evidence/m7/run-1793340619859559000/slideshow.png)、[交付面板](evidence/m7/run-1793340619859559000/editor.png)、[PDF渲染](evidence/m7/run-1793340619859559000/slides-render.png)、[备注PDF渲染](evidence/m7/run-1793340619859559000/notes-render.png)、[PowerPoint实机记录](evidence/m7/powerpoint-verification.json)。最终包M0–M6回归和真实系统拼音均通过。PDF/PNG meanRGB=0.35894，全屏/PNG meanRGB=0.96527；144DPI输出为1920×1024，PDF2页/2书签，备注页高652pt。本轮只给出这些固定样例的像素指标，不冒充完整性能/兼容性矩阵。

### PowerPoint实机发现与修复

本机Microsoft PowerPoint **16.114.1 / 16.114.26102031**以只读模式打开。初版虽通过独立XML/关系/读取器检查，仍触发PowerPoint修复。经公共内容、备注页、备注母版分离测试，问题定位到初版输出的空可选备注母版；改为不输出该可选部件、保留独立标准备注页后，完整导出样例（含专有快照）无需修复即可打开，中文/图片/隐藏页/备注已在UI检查。诊断变体保留于`evidence/m7/powerpoint-diagnostic/`。当前Office许可仅允许查看/打印，未开通订阅或登录，因此**不能验证在PowerPoint编辑保存后重开**，不能登记该项通过。该实机检查不推广为整个语料库或所有平台通过。

独立QA最初还暴露两个验证脚本问题：Poppler的pdftotext位于嵌套bin，已增加查找；全屏CGWindow截图含窗口frame区域，已按实测viewport计算内容位置，未放宽原5级meanRGB阈值。失败证据保留。

[macOS ARM64 release开发预览包（旧归档：joydeck-m7-preview-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md) · [当前源码（旧归档：joydeck-m7-preview-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md) · [开发前源码（旧归档：joydeck-before-m7-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md)。包未签名/公证、未做干净机器安装；目录无Git元数据，未创建提交。检查日志见 [check](evidence/legacy-logs/m7-check-full.log)、[stage](evidence/legacy-logs/m7-stage.log)、[打包](evidence/legacy-logs/m7-package.log)。CI已更新为M7，未远端运行。

## 未关闭门槛

- T-M7-01/02：任意对象编辑后的unknown closure、对象复制/删除关系重映射、完整公开PPTX继承/rich/图片导入与PowerPoint编辑往返；当前保留写回严格限单行文字，公开模型输出是静态子集。
- T-M7-03/04：完整损失等级/冲突UI、全部故障/外部应用竞争矩阵、unknown-field bag及完整native迁移。
- T-M7-05：精准文本选择/链接、讲义与完整输出范围/打印；后续矢量PDF与Tagged PDF不因本轮ActualText提取通过而关闭。
- T-M7-06/08：显式跳转隐藏页、物理输入/焦点返回、跨屏/三平台真实静态放映和长期任务/窗口回收矩阵；M10完整放映未实施。
- T-M7-07：≥5授权外部样本、全部40份的结构/语义/视觉/PowerPoint闭环与正式C2能力声明；本轮40份仅原创合成。
- T-M7-09：完整AT-10/11/12静态/14/15及三平台MVP仍未通过，父任务/完整M7不勾选。
- M0–M6原有布局/IME平台、组/连接符/路径、完整母版/格式/本地化/读屏、真实device-lost/完整输入trace、LARGE/故障/跨屏/长期预算继续保留。此前记录见[28](28-m6-implementation-report.md)、[27](27-m5-implementation-report.md)、[26](26-m4-implementation-report.md)。

实现依据：独立查阅Microsoft官方的 [PresentationML结构](https://learn.microsoft.com/en-us/office/open-xml/presentation/structure-of-a-presentationml-document)、[NotesSlide说明](https://learn.microsoft.com/en-us/office/open-xml/presentation/working-with-notes-slides)。通用codec/压缩库用于输出，python-pptx/Poppler/PowerPoint仅用于QA，不是生产文档引擎；未复制第三方主题/模板资产。


M8后续更新（2026-10-30）：[30](30-m8-implementation-report.md)新增标准表格、UI重构及最终包全套回归；本轮PowerPoint已能编辑原创表格并另存，JoyDeck公共导入重读实际修改，解除该新样例的只读验证障碍。本文保留M7当时证据，不将该更新扩展为完整M7/MVP通过。
