# JoyDeck 阶段 5 表格、媒体、包写出与 EMF+ 替换

2026-11-27 在 Windows 11 x64 验证。本轮从 `bdf00a53` 接续，冻结参照为 `14cbb29616d7c0ababbf40a8f6c0218b996e7ae8`。原始日志和验证文稿保存在本机 `backups/craft-stage5-deck-table-media-emfplus-20261127/`。[冻结指纹](../20261124-craft-pilots/verification.json) · [上轮验证](../20261127-craft-stage5-deck-preserve-emfplus/README.md) · [本轮数据](verification.json)

## 生产切换

- **表格。** 生成路径切换独立的内置样式定义。保留另存按原始表格树更新行列、合并标记、文字、填充、边框和边距。合并标记与单元格格式可独立修改。引用新样式时在原样式部件补入定义，并保留已有样式和扩展。结构改动涉及无法映射的未知内容、或保留编辑涉及写出所需资源无法绑定时，按保留另存契约拒绝。[保留写出](../../../../crates/joydeck-pptx/src/preserve_table.rs) · [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- **媒体和包。** 独立媒体路径读写音视频、海报图、裁剪、淡入淡出和书签。保留另存中的生成对象按引用角色重绑媒体，内部资源按类型和字节复用。包写出按实际成员清单分配内容类型与关系部件，拒绝归一化后的重复成员名，沿用既有 ZIP 编解码器。[媒体读写](../../../../crates/joydeck-pptx/src/media_part.rs) · [包写出](../../../../crates/joydeck-pptx/src/package_writer.rs) · [MS-PPTX Media](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-pptx/5e1b21c6-979f-45c5-8a46-fbef53a6e80a)
- **EMF+。** 支持范围内的实色、网纹、渐变和纹理填充，以及描边画笔的颜色读取，切换到独立模块。矩形、路径和区域组合读入有界区域模型，凸区相交使用多边形计算。点列、路径、常用几何与特殊记录通过有界解码后绘制。驱动字符串读取每个字形的显式坐标，保留多个纵向基线，并支持由首点推导后续字距的记录。[画笔](../../../../crates/joydeck-metafile/src/plus_paint.rs) · [区域](../../../../crates/joydeck-metafile/src/plus_region.rs) · [绘图](../../../../crates/joydeck-metafile/src/plus_shapes.rs) · [MS-EMFPLUS DrawDriverString](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/b794b780-e9a8-4682-af65-9b614aecfbe6)

## 验收

| 项目 | 本轮核对 |
| --- | --- |
| 构建与工程检查 | 桌面和 CLI 构建、fmt、严格 Clippy、1,083 项 Rust 测试和 126 项 R1–R7 CLI 检查通过。本次 MCP 协议验证收到 54 份响应。 |
| 表格合并和格式 | 保留另存、撤销重做后，两版 CLI 均读回两列合并。独立解析核对合并锚点、从属格标记、加粗、填充和新样式定义。合并后锚点的边框和边距不变。普通编辑保留单元格的未知属性，编辑内部含未知属性的填充明确拒绝。R3 另核对了未知单元格 XML 和不安全结构改动的拒绝行为。 |
| 媒体和包关系 | 重新生成的视频与海报图散列和冻结版一致。两种媒体关系共用一份视频资源。旧稿原始 WAV 在新保留另存件中恢复，音频和 Office 嵌入关系均指向同一部件，读回裁剪结束值 5,000 毫秒。冻结版在这一用例只读到占位对象。这处变更逐字节核对原 WAV。 |
| PowerPoint 16 | `16.0.20625.20108` 实际回存并重开表格、视频和旧稿音频三份文稿，共 8 页。两版 JoyDeck 重开回存件。独立解析确认合并、字符加粗、填充、表格样式、视频、WAV、两份海报图和音频裁剪值。PowerPoint 将旧式音频关系改写为外部 `NULL`，Office 嵌入关系仍可读，两份媒体的字节保持一致。 |
| 冻结兼容与图元渲染 | 17 个文稿、100 份输出、234 次模型读回，两类受限写回和 7 类无效输入按契约处理，12 次已登记的饼图百分比字段差异继续逐项校验。7 组 PDF 共 14 页通过，两处已登记的讲义页脚修正保持。7 个 WMF／EMF 图元场景逐像素相同。另一个 EMF+ 图元按记录中的多条基线渲染，蓝色文字覆盖高度由冻结版的 8 像素增至 51 像素。 |

未知 XML 保留验证针对 JoyDeck 本身的保留另存。PowerPoint 实机使用表格、视频和旧稿音频三份兼容样本。

复跑使用新的输出目录。

```powershell
$report = 'docs/joyoffice/validation/20261127-craft-stage5-deck-table-media-emfplus'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$candidate = 'target/debug/joydeck-cli.exe'
$shells = 'backups/craft-stage1-followup-20261124/metafile-frozen'
py -3 "$report/verify-package.py" $frozen $candidate target/deck-table-media-recheck
py -3 "$report/verify-brush-glyphs.py" $frozen $candidate $shells target/deck-emfplus-brush-recheck
py -3 docs/joyoffice/validation/20261127-craft-stage5-deck/exchange.py target/deck-package-recheck --frozen $frozen --candidate $candidate
$clips = @('target/deck-table-media-recheck/table.pptx', 'target/deck-table-media-recheck/media-candidate.pptx',
    'target/deck-package-recheck/sample-14/preserve-candidate.pptx')
& scripts/joydeck/check-stage3-powerpoint.ps1 -Samples $clips -Evidence target/deck-office-recheck `
    -FrozenCli $frozen -CandidateCli $candidate
```

## 阶段 5 剩余事项

- 旧 `joydeck-pptx/src/tables.rs` 已退役。Rust dep-info 中仍有桌面 114 个、CLI 113 个带 DeckCraft 来源记录的生产构建文件，另有一个共用构建脚本。后续逐项核查剩余的 PPTX 导入、绘图与动画交换、原生模型、字体布局、放映、渲染、PDF 和桌面分支。[逐文件来源](../../../../licenses/deckcraft/file-map.json) · [阶段 5 审计](../20261127-craft-stage5-deck/README.md)
- EMF+ 还需处理任意非凸、并集和相减区域的精确绘制，复杂画笔和描边，位图仿射、按物理字体索引定位字形以及图元到场景的适配。当前区域使用单凸轮廓能力，范围外继续使用边界或原裁剪区回退。
- 继续追溯两项依赖的独立许可原件和 Pathfinder 精确发布提交。macOS Metal 和 UOS Vulkan 等有实机时补验。[许可原件追溯](../20261127-craft-stage5-deck-followup/README.md)
