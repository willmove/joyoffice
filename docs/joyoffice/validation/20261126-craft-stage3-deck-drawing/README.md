# JoyDeck 阶段 3 绘图交换验证

在 Windows 11 x64 使用 PowerPoint 16、冻结 CLI `14cbb296` 和本批候选版验证。绘图 XML 按 ECMA-376 和 Microsoft 公布的扩展结构实现。[ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [SmartArt 绘图部件](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/ec96c9e0-757a-4f24-9725-c52d3c34e310) · [3D 模型架构](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-odrawxml/6c414e62-4825-4100-9ca1-88fefc5fb699)

| 路径 | 切换与验证 |
| --- | --- |
| DrawingML 形状 | 坐标框、组的子坐标框、预设调整和自定义路径点读取接入新的几何路径。真实样本复核旋转圆角矩形、自由形状各坐标及路径、立体效果的相机、光照和挤出参数。 |
| DrawingML 文本 | 段落、文字、软换行、字段和数学片段接入统一读写。原生样本核对文字、软换行、分段、加粗及项目符号。R3 和 R4／R5 扩展覆盖链接、表格、备注和保留编辑。 |
| SmartArt | 按关系类型装入数据、布局、样式和缓存绘图。读取模型树时按连接顺序恢复大纲，同一遍读取时关联节点图片及标题。PowerPoint 回存后保持可编辑的 SmartArt 类型、布局标识和 5 个节点文本。 |
| 3D | 可识别 PowerPoint 原生文稿的 `am3d:model3d` 图形框，重建时重新绑定内嵌模型和备用图。旧内容部件入口仍可读。PowerPoint 回存后识别为 3D 模型，相机、视场和资源散列一致。冻结版使用同一对象的备用图。 |

PowerPoint 16 按形状及文本、SmartArt 和嵌入 3D 模型生成 3 份文稿。原件、保留另存和重新生成共 9 个用例均完成打开、回存和重开。三个源文稿保留另存的包部件逐字节一致。同一份 PowerPoint 回存的 SmartArt 文件，两代 CLI 的个别内边距读数有浮点舍入差异。

冻结矩阵复跑原生格式、PPTX 保留另存、重新生成、原件散列及只读和拒绝边界。详细散列和 PowerPoint 的对象读取得到的结果见[验证数据](verification.json)。本轮 Windows 验收使用 PowerPoint 自动化和 JoyDeck CLI，其他平台及 JoyDeck 3D 渲染留待阶段 4 的呈现测试。

阶段 3 在本批后剩余 3–6 人周，覆盖对象动画、时间节点、原生模型数值序列化、修复和格式边界。[替换计划](../../CRAFT-REPLACEMENT-PLAN.md)
