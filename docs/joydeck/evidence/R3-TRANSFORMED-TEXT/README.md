# R3 旋转与竖排文字原位编辑

2026-11-05，Apple M4／macOS 27.2。使用仓库外解压的 ARM64 Release 验收包 `joydeck-0.8.0-r3-transform-final.zip`，校验 1,805 项包内文件。归档 SHA-256 为 `ebb2d5a32d51aa1e917ab4196091721ee29f57e81f51d64903e365d6811adfe6`。桌面截图、样例与可执行文件指纹见 [验证数据](verification.json)。

原位插入点、选区和输入法下划线使用排版变换后的四边形。指针通过同一变换的逆矩阵命中视觉字素，并保留软换行和双向文字的插入点亲和性。方向键按屏幕方向进入行内或跨行移动，支持物理行序与逻辑行序相反的排版。缩放、文字重新排版或移动插入点时，画布视口滚动到插入点。文本输入服务的范围坐标由同一投影计算。旋转与竖排属性沿用 DrawingML 定义。[ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [文字旋转](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.bodyproperties.rotation?view=openxml-3.0.1) · [文字方向](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.textverticalvalues?view=openxml-3.0.1)

| 路径 | macOS 实测结果 |
| --- | --- |
| 独立形状与文字旋转 | 形状 33°、文字 −10° 的选区实测主轴 23.21°。点击第一字素的前缘，按向右键和 X，原 `ABCD EFGH` 变为 `AXBCD EFGH`，只提交一个文稿修订。[选区](01-rotated-selection.png) · [编辑](02-rotated-hit.png) |
| 视口滚动 | 保持原位输入，从 99% 放大到 174%，预览栅格宽度达到 1,668 px。视口横向和纵向跟随，旋转插入点保持在可见画布内。[放大后](03-caret-after-scroll.png) |
| 东亚竖排 | 选区沿竖排字形。双击首字前缘进入原位编辑，按向下键和 X，读回 `日X本語中文`。[选区](04-east-asian-vertical.png) · [编辑](05-east-asian-vertical-edit.png) |
| 反向竖排表格 | `vert270` 单元格用向上键沿文字移动，输入得到 `OXNE TWO`。相邻单元格内容一致，实际菜单撤销与重做各执行一次。[选区](06-reverse-vertical-cell.png) · [编辑](07-reverse-vertical-cell-edit.png) |
| PPTX 保存 | 原生另存显式输入 `.pptx` 文件名。包内 CLI 独立读回三个编辑结果，直接读取 ZIP 内 DrawingML 核实形状旋转、文字旋转、`eaVert` 与 `vert270`。原 [测试 PPTX](transformed.pptx) 散列一致。 |

JoyDeck 产品检查 911 项通过，0 失败，0 忽略。包内 CLI 完成 R1/R2/R3 的 28／20／20 项复验和 329 条注册表核对。坐标回归覆盖 1×／2× backing、三个缩放和五种角度。当前物理显示器仅提供 1920×1080 的 1× 模式。2× 实屏与其他平台另行复验。非仿射艺术字、输入法候选窗口实测和输入生命周期由后续内容与输入任务跟踪。
