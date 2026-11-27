# JoyDeck 阶段 5 XML、关系与图元记录替换

2026-11-27 在 Windows 11 x64 复跑。冻结参照为 `14cbb29616d7c0ababbf40a8f6c0218b996e7ae8`。生产修改始于 `4f387ce2`。原始产物及日志位于 `backups/craft-stage5-deck-xml-vector-20261127/final/`。[冻结指纹](../20261124-craft-pilots/verification.json) · [上轮回归](../20261127-craft-stage5-deck-followup/README.md) · [本轮验证数据](verification.json)

## 生产切换

- **PPTX XML。** 生成和保留另存使用同一个 XML 1.0 写出器。迭代遍历 XML 树，统一转义文本和属性。编辑幻灯片后保留未知命名空间、属性、扩展、注释、处理指令和形状树中的注释位置。改写部件中的 CDATA 会按等值文本写出，未改部件保持原始字节。未建立模型映射的源绘图保留原有拒绝契约。[XML 1.0](https://www.w3.org/TR/REC-xml/) · [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- **PPTX 生成关系。** 生成关系由独立模块分配和写出，支持重复边的复用以及必须独立分配的边。ChartEx 重定位保留所带资源的原关系 ID，新边避开已占用 ID。包内路径、外部目标和 URI 转义按关系读回逐项校验。[关系模块](../../../../crates/joydeck-pptx/src/relationships.rs) · [ECMA-376 第 2 部分](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- **WMF／EMF。** 支持集合内的多边形、圆弧、矩形、椭圆、Bezier 及 EMF 路径记录，先解出有界指令再应用到设备上下文。设备映射使用 GDI 世界、窗口、视口和设备单位。矩形区域支持相交、排除和组合，达到矩形上限时继续应用后续排除。两种格式的文字记录共用读取及定位路径，保留 ANSI／UTF-16、参考点、裁剪框、对齐和横向字距。[MS-WMF](https://learn.microsoft.com/en-us/openspecs/windows_protocols/MS-WMF/4813e7fd-52d0-4f42-965f-228c8b7488d2) · [MS-EMF](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emf/) · [GDI 映射模式](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-setmapmode)

## 验收

| 项目 | 本轮核对 |
| --- | --- |
| 构建与工程检查 | 桌面、CLI 构建通过。fmt、严格 Clippy、1,078 项 Rust 测试和 126 项 R1–R7 CLI 检查通过。 |
| 原生与 PPTX 交换 | 17 个文稿、100 份输出、234 次模型读回。检查 OPC 引用闭合、未知包部件和 XML 扩展、两类受限写回与 7 类不支持输入。12 次既定饼图百分比字段差异按冻结协议核对。 |
| 编辑后未知 XML | 编辑含未知扩展的第一页页脚，核对未知属性及命名空间、CDATA 文本、内外层注释与处理指令。第二页字节相同，写出文件重开可读。 |
| PowerPoint 16 实机 | 在 16.0.20625.20108 保存并重开形状／富文本和树图的保留另存及重新生成文件。两个 3D 保留另存副本由 PowerPoint 回存并重开，检查 3D 对象类型和 GLB。 |
| 3D 冻结回退 | 候选与冻结保留另存后 44 个部件逐字节相同。冻结重新生成退为海报图。复杂 3D 幻灯片复制仍明确拒绝，且无写出文件。 |
| 图元渲染 | 冻结对照的 7 组 PDF 共 14 页通过，2 份原有图元场景和对应 PDF 栅格一致，两处已知页脚修正保持。另对组合图元场景各跑一组 WMF 和 EMF，逐像素一致，独立统计形状和文字像素。 |

PowerPoint 回存的形状／富文本文稿由两版 JoyDeck 互读。树图和 3D 的对象类型分别由 PowerPoint 报告为 117 和 30，回存副本交给候选版重开。3D 另跑两版 CLI 的冻结回退，PowerPoint 写出的两份 GLB 散列与输入一致。

组合场景复跑命令使用新建输出目录。

```powershell
$report = 'docs/joyoffice/validation/20261127-craft-stage5-deck-xml-vector'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$shells = 'backups/craft-stage1-followup-20261124/metafile-frozen'
py -3 "$report/verify-metafiles.py" $frozen target/debug/joydeck-cli.exe $shells target/deck-vector-recheck
```

## 阶段 5 剩余事项

两处旧生产文件 `joydeck-pptx/src/xml.rs` 和 `joydeck-metafile/src/clip.rs` 已退役。Rust dep-info 中带 DeckCraft 来源记录的文件降至桌面 116 个、CLI 115 个，另有一个共用构建脚本。其中部分文件已切换多个调用分支，来源账本保留逐文件历史。[来源账本](../../../../licenses/deckcraft/file-map.json)

- PPTX 方面继续处理图表／占位符保留编辑、表格、媒体和包写出的其余来源分支。
- 图元方面继续处理 EMF+ 记录与图像解包、凸多边形裁剪、路径几何、画笔和码页解码。当前矩形区域仍使用设备轴对齐边界。字形索引与 DBCS 兼容范围保持先前契约。
- 模型、字体布局、放映、绘制、PDF 适配和桌面界面的其他生产来源沿用原审计。继续追溯两项许可独立正文及 Pathfinder 精确发布提交。macOS Metal 和 UOS Vulkan 等有实机时补验。[逐项审计](../20261127-craft-stage5-deck/README.md) · [许可原件记录](../20261127-craft-stage5-deck-followup/README.md)
