# JoyDeck 阶段 3 原生格式与 PPTX 验证

2026-11-26 在 Windows 11 x64 上验证。冻结基线为 `14cbb296`，本批开发从 `a5a4edda` 开始。两代 CLI 的散列、逐样本结果、PowerPoint 回存记录和桌面用例见[验证数据](verification.json)。

## 本批切换

| 路径 | 实现和兼容要求 |
| --- | --- |
| 原生项目 | `.joydeck` v2 一次校验清单、源格式、页和对象绑定。内层模型使用新的归档读取、写入和数值兼容处理。沿用 `.deckcraft` v1 的磁盘结构和早期扩展名，显式迁移 v1 时附带原件。v2 的未知可选清单字段和包部件随保存保留。 |
| PPTX 包 | 以经校验的 ZIP 部件读入，按关系建立页面、母版、版式和主题索引。保留保存只覆盖变化的部件，使用原始部件名，保留被编辑 XML 内的注释和处理指令。只改演讲备注时只写该备注部件。 |
| 内容部件 | 切换母版和版式、主题矩阵、表格网格与样式读取、媒体关系、演讲备注和两种评论、经典图表数据缓存和附带工作簿关系、ChartEx 维度读取、页面转场与时间线分组。重新生成继续使用现有的兼容范围和告警。 |

按公开格式和现有磁盘契约实现。[ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [PresentationML 部件结构](https://learn.microsoft.com/en-us/office/open-xml/presentation/structure-of-a-presentationml-document) · [幻灯片评论](https://learn.microsoft.com/en-us/office/open-xml/presentation/how-to-add-a-comment-to-a-slide-in-a-presentation) · [表格样式](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/90de4085-5fbf-47a7-bc37-d59509481d0a) · [ChartEx 数据维度](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/b895f5cf-d6a5-4fc1-b566-7b594b2a4e60)

## 测试结果

| 验证 | 结果 |
| --- | --- |
| 冻结版与候选版互读 | 17 个输入、100 份写出文件、234 次模型读回。覆盖 `.deckcraft` v1、旧 JoyDeck 原生版本、`.joydeck` v2、PPTX 保留另存和重新生成。独立 ZIP/XML 检查比较页、母版、版式的文件 ID、关系及目标、媒体和嵌入部件散列、转场和动画。扩展图表合成样本验证稀疏值、层级标签、数据维度优先级和数字格式。 |
| 原件与拒绝边界 | 11 份无编辑的 PPTX 保留保存，510 个部件逐字节一致。单独核对大写部件名和未知注释保留、原生附带二进制与旧原件保留、备注最小写入。验证损坏 ZIP、错误 CRC、未来版本、未知必需能力、源格式不符、UTF-16 DTD 和符号链接元数据，另验证签名和含 VBA 部件的包拒绝保留另存。两代 CLI 继续拒绝冻结的 PowerPoint 16 加密样本，提示及原件散列一致。 |
| PowerPoint 16 | PowerPoint `16.0.20625.20108` 打开、另存及两代 JoyDeck 读回共通过 9 份文稿。Office 保存前后的页 ID、对象类型、备注对象数和动画主序列数一致。用例涵盖 JoyDeck 桌面编辑后的 PPTX、表格、图表、媒体、备注、Office 作者动画、POTX 和 PPSX。 |
| Windows 桌面 | 打开原生项目和 PPTX，检查图片、图表和备注，修改后保存并重开。两代 CLI 读回一致，原生项目的 `source.package` 散列与源原件一致，PPTX 正文、媒体和其他部件在单独编辑备注时逐字节一致。 |
| 编辑与放映回归 | 复跑跨文稿粘贴、嵌套组坐标、连接线端点、富文本、主题、撤销重做和双格式读回。48 组转场参数通过 192 个读写组合。R1–R7 共 126 次 CLI 检查通过，覆盖恢复、编辑、PDF、媒体和导出。 |
| 构建与共享依赖 | `cargo xtask build joydeck` 和 `cargo xtask check joydeck` 通过。Deck 的格式检查、严格 Clippy、来源账本、依赖校验和 1,050 项 Rust 测试通过。Word 与 Sheet 无头回归分别通过 4,150 项和 1,486 项，既有忽略用例分别为 2 项和 1 项。 |

旧审阅与媒体迁移样本的保留另存保留图片占位，并仍缺少原音频。这项行为已存在于阶段 2，阶段 2 检查点与本批母版切换后输出的该文件 SHA-256 相同。冻结版读回时该对象为普通形状。互读脚本只允许此对象和所新增图片资产及 ID 的差异，其余模型逐项比较。UTF-16 DTD 在两代均被拒绝，候选版直接报告 DTD。

复跑使用[格式交换脚本](../../../../scripts/joydeck/check-stage3-package-exchange.py)、[PowerPoint 脚本](../../../../scripts/joydeck/check-stage3-powerpoint.ps1)和已有[转场脚本](../20261123-uos-refactor/scripts/transition-exchange.py)。

## 尚待替换与验收

阶段 3 剩余 10–18 人周。仍在用的派生路径包括经典图表系列和轴以及 ChartEx 输出、嵌入工作簿、DrawingML 形状和文本 XML、SmartArt／3D、对象动画行为，以及原生格式的有限数值序列化和模型修复。优先按对象分支冻结独立语料，再用相同的包级比较和 PowerPoint 回存验证替换。[替换计划](../../CRAFT-REPLACEMENT-PLAN.md)

扩展图表通过合成 PPTX 和已有区域地图库级用例验证。PowerPoint 拒绝打开合成树图的重新生成文件，冻结版与候选版对该样本的输出逐字节相同。PowerPoint 新建扩展图表的自动化调用也返回错误，后续仍需用 Office 作者样本验收这一分支。其他桌面平台和信创真机留待有环境时复测。
