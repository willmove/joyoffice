# JoyDeck 阶段 3 图表交换验证

2026-11-26 在 Windows 11 x64 上使用冻结版 `14cbb296`、本批起点 `761233fb` 和 Microsoft PowerPoint `16.0.20625.20108` 验证。样本、二进制散列及逐文件结果见[验证数据](verification.json)。

## 树图打开失败

通过 PowerPoint 16 的 `Shapes.AddChart2`（类型 117）生成树图。原件含 16 个叶节点和两级父分类。旧版 JoyDeck 保留另存以及无编辑重新生成后，PowerPoint 均可重开。将第一项数据从 22 改为 125 后，`761233fb` 重新生成的 PPTX 被 PowerPoint 拒绝，错误为 `0x80070570`。上批合成树图的重新生成文件也触发该错误。数据改动通过本地 `.joydeck` 公开模型验证，覆盖显式重新生成入口。

在同一份失败文件上保持图表 XML、幻灯片和数据内容逐字节一致。分别补回 ChartStyle 或 ChartColorStyle 仍会失败，同时补回两者及其内部关系和内容类型后，PowerPoint 可打开、回存和重开。新路径保留来源样式部件，在新建图表或缺少样式时生成完整的样式对。分箱图另通过受控替换确认，PowerPoint 16 可读采用 `val` 属性的分箱数量、宽度和帕累托副轴 ID。[ChartEx 部件](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/5d0d453e-adac-43be-a797-59b9916593dd) · [ChartStyle 架构](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/4019845e-6b38-47b0-88b9-c6d739e7292f) · [分箱参数](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/2e5dfc5a-d3b7-4390-bc00-188a2fefcb3a)

## 切换和验收

| 路径 | 实现与验证 |
| --- | --- |
| 经典图表 | 独立读写系列、数据缓存和轴引用。通过轴 ID 绑定绘图区和主副轴，并按 `c:order` 读取系列。组合图包含交错的主副轴系列，PowerPoint 回存后仍按原顺序读回，副轴的最小值、最大值、主刻度及数字格式一致。冻结版把两个主轴柱形序列连续读入。验证脚本仅对该样本调整冻结版的系列顺序，其余字段仍逐项比较。 |
| ChartEx | 独立写出系列、维度、层级、分箱和相关样式。PowerPoint 识别生成的树图、旭日图、漏斗图、瀑布图、直方图、帕累托图和箱线图。复验上批失败样本，以及原生树图保留另存、无编辑重新生成和修改数据后重新生成。保留另存的原生树图 42 个包部件逐字节一致，两种重新生成均保留原树图的样式和配色字节。 |
| 嵌入工作簿 | 图表缓存和 XLSX 单元格共用列映射。层级标签、散点和气泡图的各系列坐标、气泡大小及稀疏数值分别核对。独立 ZIP/XML 校验贯穿 19 份 JoyDeck 输出和 22 份 Office 回存文件，覆盖 21 个工作簿、79 条公式引用和 346 个缓存位置，工作簿散列在回存前后一致。PowerPoint 的「编辑数据」调用 Excel 16 读回修改后的树图，`D2` 为 125，邻点 `D3` 为 12，末点 `D17` 为 11。 |
| PowerPoint 16 回存 | 22 份文稿可打开、回存、由 PowerPoint 重开，再由冻结版和候选版读回。含 3 份 Office 原始样本、19 份 JoyDeck 输出，涉及经典分类图、组合图、散点、气泡和扩展图表。回存前后候选版读出的图表标题、层级、系列、数据、主副轴设置和各类专项参数一致。PowerPoint 检查页 ID、对象及图表类型、备注对象数和动画主序列数。 |
| 冻结基线 | 17 个输入、100 份写出文件、234 次模型读回。11 份未编辑 PPTX 的 510 个部件逐字节一致。两代保留签名、宏、加密和异常输入的既有拒绝行为。ChartEx 合成样本重新生成时新增且仅新增两条内部样式关系及其部件，其他关系和模型按原比较方式验证。 |
| 产品回归 | `cargo xtask check joydeck` 和 `cargo xtask build joydeck` 通过，1,054 项 Rust 测试通过。R1–R7 共 126 次 CLI 检查通过，包含图表编辑、撤销重做、双格式读回、独立工作簿验证和超出保留编辑范围时的拒绝。 |

经典图表和 XLSX 格式实现依据 ECMA-376。ChartEx 实现依据 Microsoft 的公开扩展文档和上述 PowerPoint 16 实测。[ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [ChartEx 工作簿引用](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/9a9d715b-dcae-4b03-89e9-a6362994e1a2)

复跑使用[格式交换脚本](../../../../scripts/joydeck/check-stage3-package-exchange.py)、[PowerPoint 脚本](../../../../scripts/joydeck/check-stage3-powerpoint.ps1)和[工作簿校验脚本](../../../../scripts/joydeck/check-stage3-chart-workbooks.py)。原作者样本、生成样本及逐项日志位于本机 `backups/craft-stage3-deck-charts-20261126/`。

## 验收范围和余量

直接改动带原有扩展样式的 ChartEx 后，保留另存继续按既有契约拒绝并提示保存 `.joydeck`，验证确认拒绝时没有生成目标文件。已验证该原生项目的显式重新生成。区域地图通过现有离线测试，本轮未运行 PowerPoint 在线制图，也未开展其他桌面平台或人工窗口验收。

JoyDeck 阶段 3 剩余 7–13 人周，覆盖 DrawingML 形状与文本、SmartArt／3D、对象动画与时间节点，以及原生模型数值序列化、修复和格式边界。[替换计划](../../CRAFT-REPLACEMENT-PLAN.md)
