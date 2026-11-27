# JoyDeck 阶段 5 大半径滤镜、索引位图与透明字形验收

2026-11-27，Windows x64。冻结基线为 `14cbb296`，冻结 CLI 的 SHA-256 为 `f8fd32a31570e241f17ced00dbce66f7e2ae53739b18e3c70bea03c2e78794f7`。

## 实现

- 原生 GDI+ 对 601 × 601 阶跃位图的实测显示，半径 19.5 只在源图 X 轴扩散，从 20 起 X、Y 两轴均扩散。JoyDeck 在分界以上使用有界的可分离高斯卷积，按原绘制矩阵放置。输入 RGB 按 Alpha 就近舍入再卷积，计算保留中间精度，并清除 Alpha 为零像素中的隐藏颜色。[BlurEffect](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/5b45e8f4-f1ff-4701-a058-c85d15d8f0c2)
- WMF Bitmap16 支持高半字节在前的 4 位索引和逐字节的 8 位索引，按记录行跨度读取。位图和图案刷在绘制时查逻辑调色板，DC 保存与恢复维护当前调色板对象。缺失条目和显式设备索引使用本机 GDI+ 的索引位图默认颜色作为可移植回退。[Bitmap16](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/dc487315-3bb9-40c8-9f49-55ffc6152d8c) · [播放设备上下文](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/6af060b6-07dd-4b1a-ad1d-972a9ce1603b) · [逻辑调色板](https://learn.microsoft.com/en-us/windows/win32/gdi/logical-palette)
- `DrawString` 和 `DrawDriverString` 保留刷类型及其位置。渐变、网纹和纹理以图元坐标取样。字符轮廓和物理字体字形都支持透明覆盖，`SourceCopy` 可以清除字形覆盖的底色，字形外的底色继续显示。[DrawString](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/ae7927c3-e416-4069-a9b8-3200113d6c41)

## 验证

| 范围 | 本轮结果 |
| --- | --- |
| 大半径模糊 | 半径 19.5、20、32、255 的 64 个原尺寸轴向探针最大通道差为 2。原生 EMF+ 进入 PPTX 后的 120 个像素探针也通过，允许缩小投影时的一源像素采样偏移，超出该投影区间的最大通道差为 1。 |
| 预乘 Alpha | 原生 `LockBits` 在半径 3、20 下均返回预乘 RGBA `(63, 36, 18, 64)`，Rust 定向测试与 PPTX 渲染的 18 个像素探针逐通道一致。 |
| WMF 4／8 位 Bitmap16 | 122 个颜色探针逐通道一致，覆盖奇宽行填充、BitBlt、StretchBlt、默认色、逻辑调色板的切换与恢复、更新、动画和已建图案刷重新取色。预期颜色来自样本中明确保存的条目及本机 GDI+ 默认索引色。 |
| 透明画笔与字形 | 原生 Arial `DrawString` 渐变与网纹样本验证了 SourceOver、SourceCopy、全透明清除及字形外像素。独立 `DrawDriverString` 样本验证同一渐变跨物理字形、重叠字形和透明实色清除。按区域核对颜色分类，字形边缘使用各自的抗锯齿与 hinting。 |
| PowerPoint 回存 | 9 份定向 PPTX 的保留另存逐解压部件一致，JoyDeck 重开像素一致。PowerPoint `16.0.20625.20108` 回存并重开共 27 页，冻结与候选均可读回，媒体关系指向同字节的图元，候选重开对照页的 RGB 像素完全一致。 |
| 冻结兼容回归 | 17 份文稿完成 100 次写出和 234 次读回。7 组 PDF 共 14 页通过，两处登记的讲义页脚差异保持。既有的矢量、编码页、端帽、裁剪、位图、嵌套、图像合成和上一轮 164 个探针均复跑通过。 |
| 工程检查 | JoyDeck 桌面与 CLI 构建通过。产品检查通过 1,088 项 Rust 测试、fmt、严格 Clippy、依赖和来源门禁。R1–R7 的 126 项 CLI 检查以及仓库文档与工程门禁通过。 |

## 剩余事项

- 旧式调色板设备的实际颜色重映射、非通用像素排列，以及显式系统索引需要旧硬件样本。当前回退的默认色已按本机 GDI+ 校验。
- 继续补验原绘制源矩形以外的模糊扩展、不同缩小重采样方式、色调滤镜、红眼与锐化的原生数值。透明字形仍可增加纹理、更多物理字体、竖排和多平台样本。
- 核查余下生产来源、许可原件及 macOS Metal／UOS Vulkan 真机。[生产来源](../20261127-craft-stage5-deck-complex-emfplus/README.md#当前生产来源) · [许可](../20261127-craft-stage5-deck-followup/README.md)

## 复跑

从仓库根目录用 PowerShell 7 运行。PPTX 包与像素证据见 [verification.json](verification.json)。

```powershell
$report = 'docs/joyoffice/validation/20261127-craft-stage5-deck-filter-palette'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$candidate = 'target/debug/joydeck-cli.exe'
$shells = 'backups/craft-stage1-followup-20261124/metafile-frozen'
$out = 'target/deck-filter-palette'
& "$report/windows-oracle.ps1" -Output "$out/native"
py -3 "$report/verify.py" $frozen $candidate $shells "$out/native" "$out/cases"
$samples = @(Get-ChildItem -LiteralPath "$out/cases" -Filter '*-preserved.pptx' | Select-Object -ExpandProperty FullName)
& scripts/joydeck/check-stage3-powerpoint.ps1 -Samples $samples -Evidence "$out/office" `
    -FrozenCli $frozen -CandidateCli $candidate
py -3 "$report/verify.py" $frozen $candidate $shells "$out/native" "$out/cases" --office "$out/office"
```
