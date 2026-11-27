# JoyDeck 阶段 5 图表、占位符与 EMF+ 替换

2026-11-27 在 Windows 11 x64 验证。冻结参照是 `14cbb29616d7c0ababbf40a8f6c0218b996e7ae8`，本轮从 `5792b869` 接续。原始日志和本机测试文稿存于 `backups/craft-stage5-deck-preserve-emfplus-20261127/verified/`。[冻结指纹](../20261124-craft-pilots/verification.json) · [上轮验证](../20261127-craft-stage5-deck-xml-vector/README.md) · [本轮数据](verification.json)

## 生产切换

- **图表。** 在原始图表树中按系列顺序或 ChartEx 数据集 ID 更新支持的数值缓存，按源公式定位 XLSX 工作表和单元格。复制编辑对象的图表与需要修改的工作簿，保留同稿共用的原部件。已知 Office 样式、标识扩展、未改公式、外观和附属关系继续保留。新增经典图表标题或修改已有的单个文字节点时保留其他图表 XML。图表类型转换继续通过生成路径写出。无法确定依赖关系的未知图表扩展，以及公式格、外部工作簿等无法安全更新的情况按保留另存契约报错，可保存为 `.joydeck`。[图表保留编辑](../../../../crates/joydeck-pptx/src/preserve_chart.rs) · [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- **占位符。** 保留编辑与导入共用母版及版式的候选关系顺序，识别关系表中可读但未列在母版列表中的版式。换版只重绑幻灯片的布局关系，保留占位符和继承链。显式取消占位符的位置覆盖时，移除已知变换节点并按目标版式、母版回退。带未知内容的变换仍保留拒绝契约。[占位符保留编辑](../../../../crates/joydeck-pptx/src/preserve_placeholders.rs)
- **EMF+ 与路径。** 分段对象组装、位图与嵌套图元解包、设备状态命令和裁剪算子接入独立模块。WMF、EMF 和 EMF+ 使用同一组路径构建器，覆盖矩形、圆角矩形、椭圆、圆弧、扇形与基数样条。切过边缘的凸多边形填充和线段使用同一组有向半平面；文字继续按剪裁区域的设备边界框处理。[MS-EMFPLUS 对象记录](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/978404e2-c994-4e60-bade-cb6ab78e7ffc) · [MS-EMFPLUS 位图](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/112a5e2c-6bb3-4daf-8ee3-0f3d3984410f)
- **码页。** 依据 Microsoft 发布的映射表生成 CP1250、CP1251、CP1252，统一按 Unicode 字符投影字距。符号字体的私用区映射、UTF-16 代理对总字距、未定义字节和双字节码页的既有回退行为均保留。三份映射的 SHA-256 见验证数据。[Unicode 发布的 Windows 码页表](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/)

## 验收

| 项目 | 验证内容 |
| --- | --- |
| 构建和工程检查 | 桌面与 CLI 构建、fmt、严格 Clippy、1,080 项 Rust 测试、126 项 R1–R7 CLI 检查及工程门禁通过。 |
| 图表与工作簿 | Office 原始柱形图通过编辑、撤销、重做和保留另存。缓存与原公式指向的工作簿单元格 B3 都读出 65.25，旧图表和旧工作簿字节相同。新工作簿除编辑的工作表外，其余成员逐字节相同。两版 CLI 重开读到相同新标题和值。ChartEx 单测复核树图的缓存、样式关系、层级和工作簿，含经典图表回退。 |
| 母版与版式 | 两版对同一 PowerPoint 文稿换版，移动到同一目标版式，原母版、版式成员及幻灯片占位符保持。回归用例覆盖未声明的有效版式关系和移除显式变换后恢复继承。 |
| 未知 XML | 保留另存后，在新图表中回读自定义命名空间属性、处理指令和已知 Office 的扩展。R4–R5 的不明图表扩展保持安全拒绝。 |
| PowerPoint 16 | 实际回存重开柱形图编辑件、换版文稿和树图保留另存件。独立读回柱形图缓存和 XLSX 单元格，以及换版文稿的布局关系。 |
| 冻结回归 | 17 个文稿、100 份输出、234 次模型读回。12 次已知饼图百分比字段差异按冻结协议校验。7 组 PDF 共 14 页通过，两处已记录的讲义页脚修正保持。原有和新增共 6 个 WMF／EMF 图元场景逐像素匹配，包含 EMF+ 分段图像、凸形裁剪、UTF-16 和三个 Windows 码页。 |

ChartEx 的数值编辑覆盖模型层保留写出和工作簿读回。PowerPoint 16 实机树图使用保留另存件。CLI 和桌面图表编辑命令继续按现有类别图表范围提供。

复跑使用两个新的输出目录。

```powershell
$folder = 'docs/joyoffice/validation/20261127-craft-stage5-deck-preserve-emfplus'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$candidate = 'target/debug/joydeck-cli.exe'
$shells = 'backups/craft-stage1-followup-20261124/metafile-frozen'
py -3 "$folder/verify-preserving-edits.py" $frozen $candidate target/deck-preserve-recheck
py -3 "$folder/verify-plus-render.py" $frozen $candidate $shells target/deck-plus-recheck
```

## 阶段 5 剩余事项

原路径构建文件 `joydeck-metafile/src/geom.rs` 已退役。Rust dep-info 中带 DeckCraft 来源记录的文件为桌面 115 个、CLI 114 个，另有一个共用构建脚本。PPTX 的表格、媒体和包写出仍有带来源记录的分支。图元方面，EMF+ 画笔、区域布尔近似、部分绘图与字形放置、图元到场景的适配仍待继续替换。当前 EMF+ 凸裁剪对文字使用包围框，位图仿射近似、传统字形索引和 DBCS 回退沿用现有范围。模型、字体适配、放映、PDF 与桌面来源余项仍按审计推进。许可原件和 macOS Metal／UOS Vulkan 实机尚待补验。[逐文件来源](../../../../licenses/deckcraft/file-map.json) · [阶段 5 审计](../20261127-craft-stage5-deck/README.md) · [许可原件追溯](../20261127-craft-stage5-deck-followup/README.md)
