# JoyDeck 阶段 5 EMF+ 裁剪、描边及物理字形替换

2026-11-27 在 Windows 11 x64 验证。从 `bb7128d8` 接续，冻结参照为 `14cbb29616d7c0ababbf40a8f6c0218b996e7ae8`。本机原始产物和日志保存在 `backups/craft-stage5-deck-complex-emfplus-20261127/`。[冻结指纹](../20261124-craft-pilots/verification.json) · [上轮验证](../20261127-craft-stage5-deck-table-media-emfplus/README.md) · [本轮测量和构建文件清单](verification.json)

## 生产切换

- **区域。** 处理非凸、多个轮廓、内孔及相交、并集、异或、排除和补集。先按路径边交点划分扫描带，计算布尔覆盖，再把有界轮廓裁剪蒙版交给绘制。保留设备坐标、偏移和保存恢复行为。非凸、复合轮廓和反向区域应用输出蒙版，简单凸多边形继续走原有快速路径。曲线用有界多边形近似。[区域实现](../../../../crates/joydeck-metafile/src/plus_region.rs) · [MS-EMFPLUS RegionNodeDataType](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/b2c1433b-1c80-4992-a861-bcc0e491c8da)
- **画笔与描边。** 复杂笔刷支持双轴混合因子、预设渐变、色彩插值、沿路径边界的外圈颜色、焦点缩放及纹理镜像。描边解析笔宽、变换、端帽、连接、虚线型与相位、复合条带和对齐，用笔刷填充计算出的描边轮廓。有限渐变采样即使栅格单元小于一个像素也保留空间变化，重复的 WMF／EMF 图案沿用既有消除混叠策略。[画笔实现](../../../../crates/joydeck-metafile/src/plus_paint.rs) · [描边实现](../../../../crates/joydeck-metafile/src/plus_pen.rs) · [MS-EMFPLUS 笔属性](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/5ef071f3-f503-4f16-b027-7c4bcf2d1d81) · [MS-EMFPLUS 渐变选项](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/af50f5e3-e5c2-43ce-b819-d9476e93378e)
- **物理字形。** `DrawDriverString` 携带原始索引，按记录中的坐标或物理字体的字距绘制。以实际字体家族和样式核对字体，缺失或字面冲突时跳过该串。保留 Unicode 字符路径。Windows 的定向样本对照了 Calibri 字体表，测试文件只存字形索引和字体名。[物理字形绘制](../../../../crates/joydeck-render/src/metafile_glyphs.rs) · [MS-EMFPLUS DrawDriverString](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/b794b780-e9a8-4682-af65-9b614aecfbe6) · [OpenType hmtx](https://learn.microsoft.com/en-us/typography/opentype/spec/hmtx)

## 验收

| 项目 | 本轮证据 |
| --- | --- |
| 裁剪边界 | 同一份文稿测量 20 个点，覆盖凹口、内孔、五种布尔组合、无界区排除和偏移。通过原始和保留另存后重开的两次渲染。 |
| 画笔和描边 | 独立测得复合描边两条红带及中间留白，渐变虚线 80 个测点中 49 个为线段实部，颜色从红到蓝。非圆形路径渐变有不同的边界颜色，横向镜像纹理的五个采样点顺序正确。 |
| 字形位置 | 从本机 Calibri 的 `cmap`、`hmtx` 读出 `Ω` 和 `B` 的物理索引 591、17，以同字符显式坐标及按字体表字距构造对照。候选渲染逐像素相同，平移、不同基线和字体字距通过。缺失字体区域无字形。 |
| PPTX 保留另存 | 3 份定向样本的每个包成员逐字节相同，重开再渲染逐像素相同。PowerPoint `16.0.20625.20108` 回存并重开 3 份共 9 页，两版 JoyDeck 重开均通过。三份回存文件内都保留逐字节一致的 EMF。 |
| 冻结图元与 PDF | 6 个既有 WMF／EMF 场景逐像素一致，7 组 PDF 共 14 页通过，两处登记的讲义页脚修正保持。既有同基线渐变场景有 3,064 个像素变化，全部位于原渐变裁剪区，蓝色字形位置保持。三个颜色测点验证渐变方向。多基线字形样本继续复验。 |
| 产品回归 | 桌面和 CLI 构建、fmt、严格 Clippy、1,083 项 Rust 测试和 126 项 R1–R7 CLI 检查通过。MCP 验证收到 58 份响应。冻结交换涵盖 17 个文稿、100 份输出和 234 次读回，12 次既有饼图百分比字段差异按冻结协议核对。 |

用新的输出目录复跑定向用例和冻结渐变差异。

```powershell
$report = 'docs/joyoffice/validation/20261127-craft-stage5-deck-complex-emfplus'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$candidate = 'target/debug/joydeck-cli.exe'
$shells = 'backups/craft-stage1-followup-20261124/metafile-frozen'
py -3 "$report/verify-playback.py" $frozen $candidate "$shells/emf.pptx" target/deck-plus-playback-recheck
py -3 docs/joyoffice/validation/20261127-craft-stage5-deck-table-media-emfplus/verify-brush-glyphs.py `
    $frozen $candidate $shells target/deck-plus-shade-recheck --sample-subpixel-shades
```

## 当前生产来源

以桌面及 CLI 的 Rust dep-info 交叉核对来源账本，分别有 114、113 个编译文件带 DeckCraft 来源，另有一个共用构建脚本。逐文件列表附在验证数据中。八张正常及构建依赖图中，Craft 系列同名包数量均为零。来源计数涵盖同一文件中已切换及仍调用的分支。[来源账本](../../../../licenses/deckcraft/file-map.json) · [上一轮发行包审计](../20261127-craft-stage5-deck/README.md)

| 生产入口 | 仍需继续核查或替换的分支 |
| --- | --- |
| 打开、保存及交换 | PPTX 导入总入口、部分 DrawingML 互转、时间线、图表及 SmartArt 分支。原生容器入口继续核对来源记录，RTF 继续走兼容解析。v1 迁移桥保留为私有只读路径。[文稿入口](../../../../crates/joydeck-shell/src/document.rs) |
| 编辑与演示 | 模型中的属性解析和数值格式、预设和文字变形、播放时钟、效果与 Morph 仍有来源记录。需按运行时调用逐项核对旧实现覆盖范围。[模型](../../../../crates/joydeck-model/src/resolve.rs) · [放映](../../../../crates/joydeck-anim/src/show.rs) |
| 字体、渲染、导出 | 字体目录、Unicode 字符路径、脚本与段落适配、图表／3D／绘制，以及 PDF 合成仍有活跃来源文件。图元含共享 WMF／EMF 播放与画布、规范化、位图仿射和到场景的适配。[字体](../../../../crates/joydeck-fonts/src/fontdb.rs) · [图元场景](../../../../crates/joydeck-render/src/metafile.rs) · [PDF](../../../../crates/joydeck-pdf/src/lib.rs) |
| 桌面 | 桌面仍编译有来源记录的窗口框架代码。[窗口框架](../../../../crates/joydeck-ui-gpui/src/chrome.rs) |

本轮仍保留简单凸裁剪的文字边界框和位图裁切行为。任意自定义路径端帽、其余 GDI+ 网纹和极高复杂度区域需要补做。物理字形的实测限于本机 Calibri 横排。许可独立原件、Pathfinder 发布提交，以及 macOS Metal 和 UOS Vulkan 实机继续跟踪。[许可原件追溯](../20261127-craft-stage5-deck-followup/README.md)
