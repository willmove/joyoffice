# JoyDeck 阶段 5 EMF+ 端帽、网纹与图元场景

2026-11-27 在 Windows 11 x64 验证。本轮从 `1fa0e1d9` 接续，冻结参照为 `14cbb29616d7c0ababbf40a8f6c0218b996e7ae8`。本机产物和日志位于 `backups/craft-stage5-deck-metafile-scene-20261127/`。[冻结指纹](../20261124-craft-pilots/verification.json) · [上轮证据](../20261127-craft-stage5-deck-complex-emfplus/README.md) · [本轮测量](verification.json)

## 生产切换

- **自定义端帽。** 分别读取起点和终点的实心路径、描边路径及其组合，按笔宽、比例、基底内缩和端点切向绘制。中央线段按插入端帽所需长度裁短，保留线段和曲线类型。沿用可调箭头端帽。[端帽实现](../../../../crates/joydeck-metafile/src/plus_caps.rs) · [MS-EMFPLUS CustomLineCapData](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/6fbd47ae-6c34-424c-b4cc-0e86e78d9bd4)
- **GDI+ 网纹。** 余下 47 种网纹使用有界重复位图，按前景色、背景色和设备原点绘制。保留已有六种基础网纹的历史相位，支持 EMF+ 原点记录及保存恢复。扩展网纹数据来自对 Windows `HatchBrush` 可见输出的测量。[网纹实现](../../../../crates/joydeck-metafile/src/plus_hatch.rs) · [MS-EMFPLUS HatchStyle](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/ac72d44f-1ec9-401b-82d6-f16c8e84134d)
- **凸区文字。** EMF+ 文字使用裁剪区的实际轮廓，与文字布局框及上级裁剪叠加。旋转后的布局框保持多边形形状。传统 GDI 文字的既有行为继续参加冻结回归。
- **位图和图元场景。** 位图保留从源窗口到目标平行四边形的完整变换，支持绝对浮点、绝对短整数及相对坐标，处理源窗口裁切、有符号翻转、旋转和斜切。嵌套图元保留各自的图片坐标、裁剪、图案和文字，统一通过有界场景投影和回放。退役原 `place.rs`，幻灯片、缩略图及 PDF 共用当前栅格入口。[位图映射](../../../../crates/joydeck-metafile/src/image_placement.rs) · [场景投影](../../../../crates/joydeck-metafile/src/projection.rs) · [场景回放](../../../../crates/joydeck-render/src/metafile_scene.rs) · [MS-EMFPLUS DrawImagePoints](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/9bad3867-71b0-46fb-9b90-f7d8fb37ff76)

## 验收

| 项目 | 本轮证据 |
| --- | --- |
| 自定义路径端帽 | 原生 GDI+ 生成的 EMF+ 样本包含实心起点端帽和描边终点端帽。候选图中位于冻结版主线两端之外的红色像素分别为 1,046 和 1,739。 |
| 网纹 | 47 种新增网纹的 3,008 个采样点与独立生成的原生双色图案一致，含原点偏移及恢复后的相位。 |
| 字形裁剪 | 使用菱形实际边界计数，凸区外的深蓝字形像素从 4,208 降至 0。旋转并裁取源窗口的嵌套场景从 822 降至 0。 |
| 位图变换 | 29 个测点核对 EMF+ 与传统 EMF 的旋转、反射、斜切、源矩形裁切、短整数及相对坐标。有符号源窗口的翻转在两条生产路径分别通过。 |
| PPTX 和 PowerPoint 16 | 6 份定向 PPTX 保留另存后包成员逐字节一致，重开再渲染逐像素一致。PowerPoint `16.0.20625.20108` 实际回存并重开全部 6 份共 18 页。两版 JoyDeck 均能读回，独立检查确认幻灯片媒体关系和所指 EMF 文件逐字节保留。候选版重开回存文件后的比对页逐像素一致。 |
| 冻结渲染 | 6 个既有图元场景逐像素一致，7 组 PDF 共 14 页通过，保持两处登记的讲义页脚修正。上轮的渐变和多基线字形修正按既定范围继续通过。既有非凸与布尔裁剪、复杂描边、物理字形样本复验通过。 |
| 产品回归 | 桌面和 CLI 构建、fmt、严格 Clippy、1,083 项 Rust 测试及 126 项 R1–R7 CLI 检查通过。冻结交换包含 17 个文稿、100 份输出和 234 次读回。 |

使用新的目录复跑原生对照及定向用例。

```powershell
$report = 'docs/joyoffice/validation/20261127-craft-stage5-deck-metafile-scene'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$candidate = 'target/debug/joydeck-cli.exe'
$shell = 'backups/craft-stage1-followup-20261124/metafile-frozen/emf.pptx'
& "$report/windows-oracle.ps1" -Output target/deck-metafile-native-oracle
py -3 "$report/verify-metafile-scene.py" $frozen $candidate $shell target/deck-metafile-native-oracle target/deck-metafile-scene
$samples = @('caps','hatch','clip','bitmap','gdi','nested') | ForEach-Object { "target/deck-metafile-scene/$_-preserved.pptx" }
& scripts/joydeck/check-stage3-powerpoint.ps1 -Samples $samples -Evidence target/deck-metafile-office `
    -FrozenCli $frozen -CandidateCli $candidate
```

## 阶段 5 剩余事项

- 导入来源账本的生产构建文件现为桌面 113 个、CLI 112 个，另有一个共用构建脚本。PPTX 专用读写、原生和 RTF 兼容、字体和文字适配、放映、图表与 3D 渲染、PDF 和桌面框架仍按审计继续替换。[来源账本](../../../../licenses/deckcraft/file-map.json) · [生产来源审计](../20261127-craft-stage5-deck-complex-emfplus/README.md#当前生产来源)
- 图元余项包括 EMF+ 图像属性和效果、传统 GDI 的带掩码位图及其他合成方式、极高复杂度裁剪，及更多实际端帽和字体组合的互操作样本。许可原件追溯和 macOS Metal、UOS Vulkan 实机继续跟踪。[许可情况](../20261127-craft-stage5-deck-followup/README.md)
