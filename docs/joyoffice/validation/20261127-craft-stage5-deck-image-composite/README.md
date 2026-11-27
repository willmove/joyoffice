# JoyDeck 阶段 5 图像效果与 GDI 位图合成验收

2026-11-27，在 Windows x64 上使用阶段 0 冻结版 `14cbb296`。冻结 CLI 的 SHA-256 为 `f8fd32a31570e241f17ced00dbce66f7e2ae53739b18e3c70bea03c2e78794f7`。

## 生产路径

- EMF+ 的矩形和平行四边形图像读取属性对象，支持平铺、X／Y 翻转平铺和边缘填色。根据效果标记应用序列化的颜色矩阵、逐通道查找表及其余九种效果。位图的 SourceCopy 替换目标处的颜色和 Alpha。SourceOver 沿用现有混合，保存与恢复图形状态同时还原该设置。滤镜处理受图像与计算预算限制，超过预算时保留原图绘制。[图像属性](https://learn.microsoft.com/zh-tw/openspecs/windows_protocols/ms-emfplus/9f73a2af-8c9c-4d05-98ab-19a485064364) · [图像效果类型](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/7a07343f-ad48-47ed-8602-81bc85e91fc8)
- GDI 的 MaskBlt 依据可重复的单色掩码选择前景或背景 ROP3。PlgBlt 将源和掩码映射至目标平行四边形。TransparentBlt 用明确的 RGB 色键保留目标像素。WMF／EMF 的 DIB 按位合成使用源、笔刷和已绘制的目标像素。目标读取受预算限制，运算保持绘制顺序与裁剪。[MaskBlt](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emf/e6f715c0-d034-4eb3-952e-f8ee66adb9ed) · [PlgBlt](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emf/b6c8b39d-42c7-4221-ab70-93f1e09b644e) · [TransparentBlt](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emf/aa216051-2dc0-4317-b343-525431cfa103)

## 验证结果

| 项目 | 本轮证据 |
| --- | --- |
| 颜色和透明度 | Windows GDI+ 原生 EMF+ 的 RGB 通道交换、偏移和 Alpha 得到 `(183, 217, 147)`，候选逐通道一致。独立原生平铺、翻转平铺及两种钳制对照共 16 点一致。另 6 点覆盖查找表、两种合成模式、状态恢复及无效果标记的绘制。 |
| 其他 EMF+ 效果 | 对亮度、单通道色阶、色相、色阶中性参数和红眼局部范围作数值或边界检查。色彩平衡、色调、模糊及锐化验证调整方向和边缘覆盖。 |
| 掩码和混合 | 25 点覆盖单色掩码原点与重复、前景／背景按位运算、裁切、仿射 PLGBLT、色键、常量乘源 Alpha 和无源反色。AlphaBlend 测点与独立计算相差 1 个通道值，其余测点一致。Rust 测试穷举 256 种 ROP3 的 8 组二进制输入。 |
| PPTX 与 PowerPoint 16 | 4 份定向样本保留另存的全部解压部件逐字节一致，候选重开像素一致。PowerPoint `16.0.20625.20108` 回存并重开 4 份共 12 页，两版 JoyDeck 均可读取。独立检查确认所引用 EMF 的字节与候选渲染逐像素一致。 |
| 冻结回归 | 17 个文稿、100 份写出和 234 次模型读回。7 组 PDF 共 14 页通过，两处已登记讲义页脚修正保持。6 个既有图元场景与冻结版逐像素一致。端帽、网纹、凸裁剪、仿射、嵌套场景、渐变和字形样本继续通过。 |
| 产品门禁 | JoyDeck 桌面和 CLI 构建、格式检查、严格 Clippy 和 1,086 项 Rust 测试通过。R1–R7 共 126 项 CLI 检查通过。 |

## 阶段 5 剩余事项

- 色调曲线、红眼判断和锐化内核仍需更多原生样本确认。候选的预乘 Alpha 高斯模糊沿两个轴执行。本机 GDI+ 的半径 3 脉冲对照只观察到横向扩散，轴向差异已记录。位图还需验证更极端的重采样、设备边界和 Alpha 舍入。
- 传统 GDI 矢量 ROP2 仍沿用旧路径，WMF 设备相关位图变体仍待处理。EMF+ 的 SourceCopy 本轮覆盖位图，嵌套图元和矢量绘制仍待单独处理。极复杂裁剪及不同系统字体组合还有待扩充语料。
- 生产来源账本中还有桌面 113 个、CLI 112 个编译输入，另有 1 个共用构建脚本，含 PPTX 专用读写、原生与 RTF 兼容、字体适配、放映、图表与 3D、PDF 和桌面框架。两项独立许可正文、Pathfinder 精确发行提交以及 macOS Metal／UOS Vulkan 实机验证仍开放。[生产来源审计](../20261127-craft-stage5-deck-complex-emfplus/README.md#当前生产来源) · [许可追溯](../20261127-craft-stage5-deck-followup/README.md)

## 复跑

在仓库根目录用 PowerShell 7 执行。原始构建、文件和日志位于 `backups/craft-stage5-deck-image-composite-20261127/`。各测点和回存文件指纹见 [verification.json](verification.json)。

```powershell
$report = 'docs/joyoffice/validation/20261127-craft-stage5-deck-image-composite'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$candidate = 'target/debug/joydeck-cli.exe'
$shell = 'backups/craft-stage1-followup-20261124/metafile-frozen/emf.pptx'
$output = 'target/deck-image-composite'
& "$report/windows-oracle.ps1" -Output "$output/native"
py -3 "$report/verify-images.py" $frozen $candidate $shell "$output/native" "$output/cases"
$samples = @('matrix','attributes-lut','gdi','filters') | ForEach-Object { "$output/cases/$_-preserved.pptx" }
& scripts/joydeck/check-stage3-powerpoint.ps1 -Samples $samples -Evidence "$output/office" `
    -FrozenCli $frozen -CandidateCli $candidate
py -3 "$report/verify-images.py" $frozen $candidate $shell "$output/native" "$output/cases" --office "$output/office"
```
