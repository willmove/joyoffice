# JD-305 文字框布局

2026-11-05，Apple M4／macOS 27.2，1× 实屏。仓库外解压 ARM64 Release 验收包 `joydeck-0.8.0-r3-text-layout.zip`，核对 1,805 项载荷。归档 SHA-256 为 `b330f2acc0c58df2adbcbb82fb9de6e497f3b47716dbe2da28bc8b983df45821`。[原创样例](text-layout.pptx) · [验证数据](verification.json)

选中文字框后，「形状格式」提供七种文字方向、预设与自定义分栏、栏间距、列顺序、垂直对齐、四侧边距、换行及溢出缩小。边距和分栏对话框将混合值留空，仅应用明确更改的项。只改变布局时，PPTX 保留保存修补受影响的 `a:bodyPr` 属性和自动调整元素。保留原始段落、未知布局扩展和链接关系。含未知扩展的自动调整元素在需要替换时拒绝保存 PPTX，可另存原生文稿。[DrawingML BodyProperties](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.bodyproperties?view=openxml-3.0.1)

| 路径 | macOS Release 结果 |
| --- | --- |
| 文字方向 | 中文竖排后汉字保持直立，原生编辑菜单单步撤销、重做。之后改回水平文字。[中文竖排](01-east-asian-vertical.png) |
| 分栏 | 从两栏预设改为三栏和 18.5 磅间距。尝试 17 栏时保持文稿修订，显示有效范围。切换从右向左后，第 01 行在最右侧，后续文字进入中栏。[输入校验](02-invalid-column-count.png) · [分栏](03-right-to-left-columns.png) |
| 对齐与换行 | 通过 Ribbon 选择底端对齐、溢出缩小和关闭自动换行。两个输出均读回对应的布局属性。 |
| 混合边距 | 多选两框，将左边距同设为 24 磅，其余三侧保留各自的值。应用、撤销、重做各一次修订。[混合值](04-mixed-margins.png) |
| 保存与重开 | 从原生面板分别另存 JoyDeck v2 和 PPTX。v2 模型的布局属性与包内 CLI 读回一致，内嵌源 PPTX 与输入逐字节一致。独立 XML 和 python-pptx 验证公开 PPTX 的原段落、超链接关系及未知布局扩展保留。两个格式的 CLI 模型读回一致，PPTX 在新的 JoyDeck 进程重开再次一致。[保存](05-saved.png) · [重开](06-reopened.png) |

JoyDeck 产品检查 916 项通过，0 失败、0 忽略。包内 CLI 的 R1、R2、R3、R4/R5 共通过 28、20、20、23 项复验，并核对 329 条命令注册表。JD-305 仍开放，继续覆盖动态字段、链接编辑、其余字符／段落命令及随文字调整形状。Windows/Linux 和其他 Office 桌面实屏未在本轮执行。
