# JoySheet 对象、分页与公式审核补齐

2026-11-11，延续 `feat/joysheet-excel-ribbon`。本记录补充第一轮 [Ribbon 比对](EXCEL-RIBBON-PARITY.md)。

第二轮阶段测试、截图与 XLSX 样例见[验证记录](validation/20261111-objects-and-paging/README.md)。恢复桌面后修复日程表名称注册、Package 数据和预览图关系；最终组合样例已通过 Excel 无修复打开、另存、关闭和独立重开。实测与保留边界见[本机续验记录](validation/20261111-excel-live/README.md)。

## 实现范围

| 项目 | 可执行操作 | 保存形式与边界 |
| --- | --- | --- |
| SmartArt | 流程、列表、层次、循环；逐行编辑节点文本；预览、移动、缩放、删除、撤消/重做 | DrawingML diagramData / diagramLayout / diagramQuickStyle / diagramColors；外部复杂布局保留原始对象 XML 和关联部件，提供基础只读预览 |
| WordArt | 文本、颜色、阴影；普通、上弧形、波形转换；双击编辑 | DrawingML `prstTxWarp`、文字填充/轮廓和阴影；未覆盖 Excel 的完整效果图库 |
| 日程表 | 选择透视表日期字段；点击/拖选日期段、清除、年/季/月/日、左右滚动；刷新报表及公式 | MS-XLSX Timeline view/cache、透视缓存关联；当前每个日程表绑定一张本地透视表，未实现 OLAP 和多报表连接 |
| 嵌入对象 | 从本地文件创建、移动、缩放、删除；双击提取原始文件 | OLE Package / MS-CFB、`Ole10Native`、VML 与 DrawingML 对象关联；每个文件最多 32 MiB；不包含宿主 OLE 服务器内嵌编辑或新建 ActiveX 控件 |
| 公式对象 | 分式、根式、上下标、分组及常用希腊字母；文本编辑与内建预览 | OMML `m:oMath`；支持 `\frac{a}{b}`、`\sqrt{x}`、`x^{2}`、`x_{i}`；4096 字符、32 层嵌套；其他原生结构保留 XML，不提供完整编辑和排版预览 |
| 分页编辑 | 页面布局显示纸张、间距、页边距；编辑单元格和页眉页脚；分页预览拖动断点 | 共用引擎 `PageGrid` 和打印分页；手工断点可撤消且保存到 XLSX；标尺、页边距拖动和完整页眉页脚上下文 Ribbon 未覆盖 |
| 追踪箭头 | 引用/从属箭头、区域边框、逐级展开、移去箭头；编辑/缩放后刷新 | 视图覆盖层，不写入工作簿；跨表引用显示虚线和目标文字，未实现 Excel 的跨表双击导航；按已有稀疏公式索引展开，单次最多 64 个查询、最多 4096 条显示箭头 |

对象模型属于 Sheet，编辑由引擎命令提交，GPUI 和 PDF 使用相同的结构对象场景。未引入 Word/Deck 产品依赖。嵌入文件在历史快照间共享原始字节；新增复杂原生图形保留路径在不能写回时拒绝保存，包含此类对象的 JSON 导出也拒绝，避免把保留的 XML 丢掉。

Ribbon 所有选项卡的图标描边从资源原有 2 减至 **1.25**，输入框、样式样本和分组线使用 **0.5 逻辑像素**，活动选项卡下划线减至 1。此调整只作用于功能区资源，不改变文档内线条。

## 规范来源

- [DrawingML 图表与 SmartArt 部件](https://download.microsoft.com/download/e/1/4/e14fb96f-83b8-4a2a-84db-7fa8acbe061a/Office%20Open%20XML%20Part%201%20-%20Fundamentals.pdf)、[自定义 SmartArt 布局](https://learn.microsoft.com/ja-jp/archive/msdn-magazine/2007/february/create-custom-smartart-graphics-for-use-in-the-2007-office-system)。
- [DrawingML 文本变换](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.presettextwarp?view=openxml-3.0.1)。
- [MS-XLSX Timeline](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/92ac3f95-d5bb-4584-882e-95e4e9c443c8)、[视图](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/0d578770-103b-4d74-8920-6636997b6424)、[缓存状态](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/aa9bf132-1afa-4ce9-b3d2-e6dc00e48fde)。
- [MS-OLEDS OLENativeStream](https://winprotocoldoc.z19.web.core.windows.net/MS-OLEDS/%5BMS-OLEDS%5D-180912.pdf)、[SpreadsheetML OLE Object](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.oleobject?view=openxml-3.0.1)。
- [MS-ODRAWXML CT_TextMath](https://learn.microsoft.com/fil-ph/openspecs/office_standards/ms-odrawxml/cdafda97-9124-48ed-8d64-7f1dd9197c8a)。

从公开规范实现；未提取 Excel 图标、模板或私有实现。

## 验证方式

```powershell
pwsh -File scripts/joysheet/dev.ps1 xtask check joysheet --validation
pwsh -File scripts/joysheet/dev.ps1 xtask build joysheet --validation
python scripts/joysheet/ribbon-smoke.py --output artifacts/joysheet/object-parity-new-run
python scripts/check_docs.py
```

引擎回归覆盖对象编辑/撤消/重做、独立 XLSX 重开、嵌入文件逐字节保留、公式解析预算、日程表筛选重算/滚动、分页断点移动。文件格式回归额外验证原生 SmartArt 的未知 XML 和关联部件保留；几何测试验证分页后的单元格命中和页边距。

窗口回归脚本使用 Python 与 Pillow，额外检查分数像素页面原点下的网格线，以及分页预览中非打印区域没有多余蓝色断点线。分页边界和页码按真实打印范围裁剪，避免在灰色区域显示空白页的断点。公式追踪直接更新工作区箭头，不打开通用结果侧栏。

`scripts/joysheet/validate-openxml.ps1` 使用官方 NuGet 的 Open XML SDK 3.0.1 独立检查导出样例。SDK 把 `a14:m` 建模为叶节点，与 MS-ODRAWXML 的 `m:oMath` 子节点要求存在差异：脚本保留该 SDK 诊断，并单独验证 OMML 子树，不把这项诊断隐藏成“无条件零错误”。SDK/模式检查不等同于 Excel 实际打开、视觉或交互验收。

原始日志、样例与报告位于忽略目录 `artifacts/joysheet/object-parity-20261111/` 和 `artifacts/joysheet/excel-live-20261111/`。本机 Excel 已验证日程表一月筛选为 10、SmartArt/WordArt 文本编辑、公式工具入口、页眉提交和分页线拖动；五类对象组合的保存重开与嵌入文件原始负载也已检查。日期选择结束端点为包含端点。Excel 的 `AlternateContent` 和 `o:spid` 包装另有读取/混合保存回归。完整经过和证据见[续验记录](validation/20261111-excel-live/README.md)，与格式检查、GPUI 自动回归分别记录。
