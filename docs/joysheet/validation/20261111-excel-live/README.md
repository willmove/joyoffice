# 2026-11-11 本机 Excel 对象兼容性续验

延续[对象、分页与细线 Ribbon 验证](../20261111-objects-and-paging/README.md)，分支 `feat/joysheet-excel-ribbon`，版本 0.1.3。参照为本机中文 Excel **16.0.20527.20150**。最终 R15 组合样例已通过 Excel **无修复打开、另存、关闭、独立重开**；SmartArt、WordArt、日程表、公式和嵌入文件图标/文件名均保留。

只编辑本次自建测试工作簿，未修改用户原有工作簿。原始截图、Excel 创建/另存的参考文件、失败轮次及完整日志位于忽略目录 `artifacts/joysheet/excel-live-20261111/`。参考文件含 Office 自动生成的作者信息，不收入仓库。提交截图只裁剪标题/账户区域并转为 JPEG，不重绘界面元素。

## Excel 实测

| 项目 | 实际操作和结果 | 证据 |
| --- | --- | --- |
| 五类对象组合 | R15 导出直接打开，无修复提示；另存为 `excel-saved-objects-r15.xlsx`，关闭后重开，无保存错误或预览裁切 | [打开](excel-opened-objects.jpg)、[重开](excel-reopened-objects.jpg)、[样例](objects-native.xlsx) |
| SmartArt | “计划/执行/验证”完整可读；在原生文本窗格把首节点改为“计划验证”，保存重开仍保留 | R12 本地另存文件；最终 R15 采用同一图形实现 |
| WordArt | 在原生文本编辑中改为 `JoyOffice Verified`，效果保留，保存重开后文字仍在 | [原生公式工具截图中的修改结果](excel-equation-tools.jpg)，R12 本地另存文件 |
| 公式对象 | 显示分式、根式和上下标；双击进入 Excel 原生“公式工具”，结构图库可用 | [公式工具](excel-equation-tools.jpg) |
| 日程表 | R7 无修复打开；滚到一月，点击筛选，透视汇总由 60 变为 10；成功保存 | [原生筛选](excel-timeline-filtered.jpg)；文件状态为 2026-01-01 至 2026-01-31，两个端点均包含 |
| 嵌入文件 | 图标与 `Example.txt` 经 Excel 保存、重开仍正常；独立读取两份 Package，原始 27 字节完全一致 | [重开](excel-reopened-objects.jpg)、[负载校验](native-artifact-verification.json) |
| 页面编辑 | 在页面布局中编辑 B2 为 `Page edit`，输入并提交中央页眉 `Header`，保存为 `&CHeader` | [页眉编辑入口](excel-header-edit.jpg)、[文件状态](native-artifact-verification.json) |
| 分页预览 | 将自动分页虚线向上拖成手动实线；文件保存 `rowBreak id="48" man="1"` | [分页拖动](excel-page-break-dragged.jpg)、[文件状态](native-artifact-verification.json) |
| 公式追踪 | A1=2，B1=A1+1，C1=B1*2；连续两次追踪引用显示 A→B→C。蓝色起点圆点位于单元格约四分之一宽度处 | [原生两级箭头](excel-trace-two-levels.jpg)；JoySheet 已对齐颜色、起点和圆点 |

本机默认实体打印机不可用时，在 Excel 本次会话选择 Microsoft Print to PDF 后完成页面布局验证，没有提交打印。测试曾两次遇到桌面锁定，观察到锁屏即停止输入；用户解锁后继续，未修改锁屏、安全或睡眠设置。已收起早期诊断窗口，保留最终对象样例和分页参考供查看。

嵌入对象激活已到达系统的 TXT 应用选择入口；选择现有编辑器“仅一次”后没有观察到外部编辑器窗口，因此不计作外部文件打开验收。JoySheet 的提取功能另以字节一致性检查验收，不包含宿主 OLE 服务器内嵌编辑。

## 缺陷定位与修正

- 日程表缓存名必须在工作簿名称空间注册为 `#N/A`。缺失注册导致 Excel 修复缓存。新增内部名称注册、大小写不敏感的用户名称冲突避让，以及导入时只消费匹配缓存的内部名称。
- 本机文件证明日程表结束日期是**包含端点**。此前排他端点的判断已纠正；读写统一保留一月的 31 天。
- OLE Package 修正命令字符串长度、Package2 的三段 Unicode 字符串，以及 CompObj 主机元数据；保留旧 JoySheet 缺长度格式的负载读取兼容。
- 为 OLE 建立工作表图片关系、`objectPr` 锚点、VML 图片关系和隐藏 DrawingML 代理。预览图为自行绘制的图标，不使用 Office 图标资源。
- 完整 VML 图片类型定义避免 Excel 把预览转为裁切的自定义形状。代理、工作表和 VML 使用同一个图形编号；新增对象与原有 OLE、控件及批注共存时，预先保留编号块，保持全部关系可解析。
- Excel 会将 OLE 代理和工作表对象包进 `AlternateContent`，并用 `o:spid` 保存实际编号。读取支持相应 a14/x14 分支，避免重复读取回退对象或把已建模对象重新保留成第二个副本。
- 没有 JoySheet 编辑元数据的外部 OLE 继续保留原始 Package/VML 图及关系，隐藏代理不转为普通可编辑矩形；此类对象不提供完整激活或编辑预览。
- 修复 GPU 两点轴向路径丢线，补充分隔线、分页网格与箭头主干像素检查。追踪箭头按实测调整为纯蓝、四分之一单元格宽度起点及圆点。

历史失败未覆盖为成功：R0 删除日程表，R1/R2 修复缓存，R3–R6 隔离缓存关联；R7 完成日程表筛选/保存。R8–R11 隔离 Package 与预览关系；R12/R13 可另存但重开预览裁切，R14 定位完整 VML 图片类型，R15 正式导出完整链路通过。保留 [R0 修复日志](round0-excel-repair.xml)、[R1 修复日志](round1-excel-repair.xml)及[早期像素失败](strokes-before.json)。

## Ribbon 对照

本机默认显示九个编辑选项卡；文件后台另在[第一轮](../20261111-excel-ribbon/README.md)记录。JoySheet 继续保留自己的“自动化”扩展选项卡。所有 Ribbon 图标描边减至 1.25，按钮边框/输入框/样式样本/分组线为 0.5 逻辑像素，活动下划线为 1。

| 本机 Excel 参考 | 截图 |
| --- | --- |
| 开始 / 插入 / 绘图 | [开始](excel-tab-home.jpg)、[插入](excel-tab-insert.jpg)、[绘图](excel-tab-draw.jpg) |
| 页面布局 / 公式 / 数据 | [页面布局](excel-tab-page-layout.jpg)、[公式](excel-tab-formulas.jpg)、[数据](excel-tab-data.jpg) |
| 审阅 / 视图 / 帮助 | [审阅](excel-tab-review.jpg)、[视图](excel-tab-view.jpg)、[帮助](excel-tab-help.jpg) |

分组、命令层次和细线外观已对照；图库完整度、布局密度、字体、部分弹出框及上下文 Ribbon 仍有差异。完整 SmartArt/WordArt 效果、OLAP/多报表连接、全部 OMML 结构、分页标尺/页边距拖动、跨表箭头双击导航仍未覆盖。实现边界见[对象与分页记录](../../STRUCTURED-OBJECTS-AND-PAGING.md)，不认定为完整 Excel 或像素级等价。

## 自动验证与重跑

完整产品检查 **1,418 项通过、1 项忽略**，fmt、严格 Clippy 和验证构建通过；生产 GPUI **38 组通过**。二进制 SHA-256：`5ac58daf08dadc4b4527f4fadafe04265b273da6de1ce321d9043fff2b3437a3`。

最终数据见[验证数据](verification.json)、[窗口报告](native-report.json)及[负载/分页状态](native-artifact-verification.json)。[Open XML SDK 报告](openxml-report.json)为 0 个普通问题、1 个记录的 `a14:m` 叶节点建模限制，OMML 子树单独通过校验。SDK 结果与 Excel 实测分别记录。

窗口回归同时覆盖 JoySheet 原生对象编辑/撤消/重开、提取原始文件、日程表筛选/拖选/清除/缩放层级、页面/页眉编辑、分页线拖动、多级追踪，以及 Excel 原生参考、Excel 筛选后的日程表和 Excel 重写后的五类对象反向导入。源参考文件前后哈希不变。合成事件/GPU 读回不等于系统物理输入或其他平台验收。

| 最终 JoySheet GPU 读回 | 截图 |
| --- | --- |
| 细线 Ribbon、结构对象 | [开始](tab-home.jpg)、[对象](objects-on-sheet.jpg) |
| 日程表及 Excel 参考导入 | [筛选](timeline-filtered.jpg)、[原生参考导入](excel-authored-timeline-filtered.jpg)、[Excel 重写对象导入](excel-resaved-objects-imported.jpg) |
| 两级追踪箭头 | [箭头](formula-trace-arrows.jpg) |

```powershell
pwsh -File scripts/joysheet/dev.ps1 xtask check joysheet --validation
pwsh -File scripts/joysheet/dev.ps1 xtask build joysheet --validation
python scripts/joysheet/ribbon-smoke.py --output artifacts/joysheet/new-unique-run --excel-reference artifacts/joysheet/excel-live-20261111/excel-clean-timeline.xlsx --excel-filtered-reference artifacts/joysheet/excel-live-20261111/round7/excel-filtered-timeline.xlsx --excel-objects-reference artifacts/joysheet/excel-live-20261111/round15/excel-saved-objects-r15.xlsx
python scripts/check_docs.py
```

三个 `--excel-…-reference` 参数均可选，输入为本次创建的本地测试参考，按字节副本导入，不覆盖源文件。构建与窗口回归串行执行，避免 Windows CLI 文件占用。此前跨产品回归 Word 引擎 1,157 项、Deck 引擎 6 项通过，本轮仅改 Sheet 模块，未改共享实现。

规范依据：[Timeline 缓存名注册](https://learn.microsoft.com/en-sg/openspecs/office_standards/ms-xlsx/f45ff6ef-fb62-4e19-8e8c-822e3be9ef75)、[Timeline 日期状态](https://learn.microsoft.com/en-us/dotnet/api/microsoft.office.interop.excel.timelinestate?view=excel-pia)、[MS-OLEDS](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oleds/142e0420-2f74-4ed9-829b-0b3d5a684d01)、[ECMA-376 VML 图片示例](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.picture?view=openxml-3.0.1)。从公开规范实现，不搬运 Excel 实现或私有资源。
