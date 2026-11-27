# JoyDeck 阶段 5 模糊扩展、滤镜与纹理字形验收

2026-11-27，Windows x64。冻结基线为 `14cbb296`，冻结 CLI 的 SHA-256 为 `f8fd32a31570e241f17ced00dbce66f7e2ae53739b18e3c70bea03c2e78794f7`。

## 实现

- 模糊扩展按半径向上取整分配边缘，使用原绘制源矩形决定是否把扩展区域画到图上。GDI+ 的小半径只在源图 X 轴扩散，半径从 20 起处理两个轴。扩展时颜色与 Alpha 分开卷积，保留透明边缘中的颜色衰减。[BlurEffect](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/5b45e8f4-f1ff-4701-a058-c85d15d8f0c2)
- 缩小采样使用 EMF+ 状态保存的插值模式。在整像素目标上，最近邻按原图首个像素定位，普通插值在强缩小时按实测退到该取样方式。高质量缩小先对预乘像素做有界面积预滤波，之后按图元坐标绘制。[插值模式](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/51312252-755a-4c8b-84bd-6ae73da1b5ea)
- 亮度与对比度联合调整，色阶按黑白点重映射后调整中间调。颜色平衡按通道乘增益。色调曲线按模式限制参数范围，平滑拐点和饱和区。曲线的对比度、高光、阴影使用从公开 Win32 接口实测的半区控制点，转换时在两组控制点之间插值。HSL 与色调按测试得到的色相、亮度和通道响应更新。[图像效果参数](https://learn.microsoft.com/en-us/windows/win32/api/gdipluseffects/) · [颜色曲线参数](https://learn.microsoft.com/en-us/windows/win32/api/gdipluseffects/ns-gdipluseffects-colorcurveparams)
- 红眼在指定区域内按红色超量和距离渐进减弱，重叠区域用最强修正。锐化保持输入 Alpha，使用实测强度响应和半径对应的卷积轴，避免小半径锐化污染垂直边缘。[红眼参数](https://learn.microsoft.com/en-us/windows/win32/api/gdipluseffects/ns-gdipluseffects-redeyecorrectionparams) · [锐化参数](https://learn.microsoft.com/en-us/windows/win32/api/gdipluseffects/ns-gdipluseffects-sharpenparams)
- 物理字形保留原纹理和变换，合并字形轮廓后再用同一坐标系取样。支持平铺、镜像和越界透明，`SourceCopy` 只覆盖字形轮廓内的底色。[图像画笔和文本记录](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-emfplus/)

## 验证

| 范围 | 本轮结果 |
| --- | --- |
| 模糊扩展 | 原生 EMF+ 的 56 个测点最大通道差 2，覆盖半径 3、20 的内边缘、扩展外缘，以及绘制源矩形继续裁掉外缘的场景。 |
| 缩小重采样 | 82 个测点通过。最近邻及强缩小的普通双线性／双三次测点完全一致。高质量预滤后的平坦区域最大差 2，双线性台阶最大差 4，双三次台阶最大差 14。 |
| 色调与曲线 | 1,020 个测点通过。包含色阶、正负亮度与对比度组合、颜色平衡、八种曲线、单通道曲线、HSL 组合和中性灰调色。HSL 组合最大通道差 8，其余这些测点最大差 3。旧有含色彩的色调样本另用原生参考核对。 |
| 红眼与锐化 | 居中双瞳、偏心选区共 16 个测点，最大通道差分别为 3、15。锐化的 117 个 PPTX 测点最大差 1，另外 12 个原尺寸探针验证了冲激像素、部分锐化强度和半径 20 的轴向边界。 |
| 纹理和物理字形 | 原生 Arial 字形索引样本覆盖半透明红、全透明、全不透明蓝以及未绘制的绿色底。平铺的 `SourceOver`、镜像平铺的 `SourceCopy`、越界透明的 `SourceCopy` 均通过。字体 hinting 和边缘抗锯齿按区域统计。 |
| PowerPoint 与冻结回归 | 7 份定向 PPTX 保留另存逐解压部件一致，JoyDeck 重开像素一致。PowerPoint `16.0.20625.20108` 回存重开共 21 页，图元字节和候选重开对照页像素一致。冻结基线复跑 17 份文稿、100 次写出、234 次读回和 14 页 PDF。 |
| 工程检查 | JoyDeck 桌面与 CLI 构建通过。1,089 项 Rust 测试、126 项 R1–R7 CLI 检查、fmt、严格 Clippy，以及来源、依赖、文档和工程门禁通过。上一轮的调色板、透明字形、图元合成和 164 个矢量探针均复跑通过。 |

## 剩余事项

- 高质量双三次的台阶振铃、非整数或旋转的缩小采样相位，以及小半径模糊最外一圈的原生差异仍有空间继续对齐。
- 亮暗端极值、彩色调色样本、复杂眼部纹理和任意红眼选区需要扩展样本。当前红眼采用有界局部校正，偏心样本已测出仍有像素差异。
- 图元旧式调色板硬件、更多字体和多平台纹理渲染仍待实机补验。剩余生产来源、许可原件及 macOS Metal／UOS Vulkan 继续跟踪。[阶段 5 来源账本](../20261127-craft-stage5-deck-complex-emfplus/README.md#当前生产来源) · [许可追溯](../20261127-craft-stage5-deck-followup/README.md)

## 复跑

从仓库根目录用 PowerShell 7 运行。提交的 [verification.json](verification.json) 含每项最大误差、媒体和回存指纹。复跑目录还保留逐像素探针。

```powershell
$report = 'docs/joyoffice/validation/20261127-craft-stage5-deck-effects-texture'
$frozen = 'backups/craft-replacement-20261124/14cbb296/joydeck-cli.exe'
$candidate = 'target/debug/joydeck-cli.exe'
$shells = 'backups/craft-stage1-followup-20261124/metafile-frozen'
$out = 'target/deck-effects-texture'
& "$report/windows-oracle.ps1" -Output "$out/native"
py -3 "$report/verify.py" $frozen $candidate $shells "$out/native" "$out/cases"
$samples = @(Get-ChildItem -LiteralPath "$out/cases" -Filter '*-preserved.pptx' | Select-Object -ExpandProperty FullName)
& scripts/joydeck/check-stage3-powerpoint.ps1 -Samples $samples -Evidence "$out/office" -FrozenCli $frozen -CandidateCli $candidate
py -3 "$report/verify.py" $frozen $candidate $shells "$out/native" "$out/cases" --office "$out/office"
```
