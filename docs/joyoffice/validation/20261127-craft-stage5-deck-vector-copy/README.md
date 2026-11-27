# JoyDeck 阶段 5 矢量合成与原生模糊验收

2026-11-27，Windows x64，使用阶段 0 冻结版 `14cbb296`。冻结 CLI SHA-256 为 `f8fd32a31570e241f17ced00dbce66f7e2ae53739b18e3c70bea03c2e78794f7`。

## 实现

- EMF+ 模糊在源位图的 X 轴卷积，再由绘制矩阵定位和旋转。最近邻记录随图形状态保存和恢复，显式指定时用于位图采样。卷积边缘按扩展标记处理，计算继续受输入预算限制。[BlurEffect](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/5b45e8f4-f1ff-4701-a058-c85d15d8f0c2) · [SetInterpolationMode](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/2fe03573-1e0c-4cc5-8ab3-74421ef17c18)
- 传统 WMF／EMF 矢量 ROP2 的 16 个模式按笔或刷颜色与已绘制的目标 RGB 执行。一个图形中描边覆盖填充的像素只执行一次。DC 保存恢复会还原混合模式。目标尚未绘制时，按白色设备底面计算。[BinaryRasterOperation](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/781a06bd-af9b-48b7-8e7d-d922de0f9c26)
- WMF 设备相关 Bitmap16 用字对齐或记录给出的有效行跨度解码，覆盖 1 位和常见 16／24／32 位真彩变体。支持 BitBlt 源窗口、StretchBlt 和旧式图案刷。单色源使用 DC 前景与背景色。无位图的记录只接受无需源的 ROP。[Bitmap16](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/dc487315-3bb9-40c8-9f49-55ffc6152d8c) · [BitBlt](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/0d703ede-9633-47a0-a92d-c98b2bca6a2b) · [StretchBlt](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/4839ba2c-dd96-4be1-aa3e-a313a988282b)
- SourceCopy 用图形、画笔或字形的覆盖范围写入颜色和 Alpha。Clear 使用当前设备裁剪。嵌套图元在父目标上按子图自己的状态回放，未绘制区域仍显示原底图。需要直接操作底像素的图元使用非隔离裁剪，普通 SourceOver 图元保留已有的裁剪边缘覆盖方式。[CompositingMode](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/cbfc8c78-778d-466e-a52a-a82f8ce6cfee)

## 验证

| 范围 | 本轮结果 |
| --- | --- |
| 原生 GDI+ 模糊 | 半径 0.75、3、6.5，以及旋转 90° 的半径 3 样本。原生直接绘制与候选读取原生 EMF 后的 68 个 RGB 测点逐通道一致。水平扩散随目标旋转，局部 Y 轴无额外扩散。 |
| 矢量 ROP2 | 64 个原生 GDI 测点逐通道一致，覆盖全部 16 种模式、笔刷重叠、只描边区域和邻接底色。保存恢复按图形顺序执行。 |
| WMF Bitmap16 | 19 个独立编码的测点一致，覆盖字对齐、源裁切、缩放、单色、三种真彩深度、图案刷和无源操作。本机 Windows `PlayMetaFile` 对该测试流只复现了无源反色、白填充和原底色共 3 个测点。旧式内嵌位图和刷图案由规范构造验证，仍需旧设备样本作原生对照。 |
| SourceCopy | 原生 GDI+ 的 13 个测点一致，覆盖半透明描边、实色和透明填充、剪裁内 Clear、状态恢复，以及子图元的透明清除和未绘制区域。 |
| PPTX 和 PowerPoint 16 | 4 份定向样本保留另存的解压部件及 JoyDeck 重开像素均一致。PowerPoint `16.0.20625.20108` 回存并重开全部 12 页，冻结和候选 JoyDeck 都能读回。媒体关系指向同字节的 EMF／WMF，对照页像素一致。 |
| 冻结交换与输出 | 17 个文稿完成 100 份写出和 234 次读回。7 组 PDF 共 14 页通过，两处登记的讲义页脚差异保持。6 个旧图元场景与冻结版逐像素一致。端帽、网纹、文字裁剪、仿射位图和上一轮合成样本复验通过。 |
| 工程测试 | JoyDeck 桌面与 CLI 构建、fmt、严格 Clippy、1,086 项 Rust 测试、126 项 R1–R7 CLI 检查及文档和工程门禁通过。 |

## 剩余事项

- 继续补充大半径、预乘 Alpha 取整和缩小重采样的原生模糊样本。色调滤镜、红眼和锐化仍需原生数值对照。
- 旧调色板设备上的 4 位和 8 位 Bitmap16、非通用设备像素排列、更多矢量字形与透明画笔组合仍待实测。普通文稿的图元输出兼容性仍使用冻结语料回归。
- 生产入口仍有桌面 113 个、CLI 112 个来源账本中的编译文件，另有 1 个共用构建脚本。继续按审计替换生产来源、追溯未取得的许可原件，并在设备可用时补验 macOS Metal 和 UOS Vulkan。[生产来源](../20261127-craft-stage5-deck-complex-emfplus/README.md#当前生产来源) · [许可](../20261127-craft-stage5-deck-followup/README.md)

## 复跑

下面的命令在仓库根目录使用 PowerShell 7。像素测点、媒体和回存文件指纹见 [verification.json](verification.json)。

```powershell
$report = 'docs/joyoffice/validation/20261127-craft-stage5-deck-vector-copy'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$candidate = 'target/debug/joydeck-cli.exe'
$shells = 'backups/craft-stage1-followup-20261124/metafile-frozen'
$out = 'target/deck-vector-copy'
& "$report/windows-oracle.ps1" -Output "$out/native"
py -3 "$report/verify-vectors.py" $frozen $candidate $shells "$out/native" "$out/cases" --prepare-wmf
& "$report/windows-oracle.ps1" -Output "$out/native" -Wmf "$out/native/bitmap16.wmf"
py -3 "$report/verify-vectors.py" $frozen $candidate $shells "$out/native" "$out/cases"
$samples = @('blur', 'copy', 'rop2', 'bitmap16') | ForEach-Object { "$out/cases/$_-preserved.pptx" }
& scripts/joydeck/check-stage3-powerpoint.ps1 -Samples $samples -Evidence "$out/office" `
    -FrozenCli $frozen -CandidateCli $candidate
py -3 "$report/verify-vectors.py" $frozen $candidate $shells "$out/native" "$out/cases" --office "$out/office"
```
