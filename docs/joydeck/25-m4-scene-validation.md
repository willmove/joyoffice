# M4 Scene 与原生绘制验证契约

日期：2026-10-30。用户已要求继续 M4；此前 M3 的统一文字编辑布局、段落及其他平台验收仍保持开放。本轮先实现现有模型可表达的绘制切片，不把这些前置缺口改写为通过。

## 验证方法

- 固定 seed `0x4a6f794465636b04`，BENCH-EDIT 为 120 基础几何、60 文字块（约 8,000 个实际 shaped glyph）、20 个独立 512×512 原创程序图像资源/节点。BENCH-EFFECT 为 30 个渐变/透明组/裁剪/模糊组合。数据由 scene_bench.rs 独立生成。
- CPU 与 GPUI 消费同一 Scene、相同文字轮廓和资源，固定 48 帧轨迹，各 backend 预热 3 帧。CPU 对照也复用排版，逐帧做整页 raster/upload；GPU 复用几何三角形、逐 glyph mask 与纹理，仅改变节点位置。比较不包含首次加载与文本排版。
- Native 计时从程序提交节点变换到 GPUI post-render frame callback，记录 p50/p95/p99、frame interval、raster/绘制/资源准备耗时和适配器资源计数。它是原生绘制反馈，不能称为物理输入到硬件 scan-out 时间。驱动层实际上传次数和 GPU VRAM 未直接读取。
- release 绘制门槛：EDIT p95 ≤20ms 或比 CPU 对照降低 ≥40%；普通对象有实际 GPUI path/image 覆盖，测量期间无整页 raster。此门槛只是 M4 完整验收中的一项。
- 每组结束后截图实际窗口，按原生 frame/content 差值裁出画布，与同一末帧 CPU PNG 对照。CPU 截图也做相同比较，以检查裁图和色彩链路。CPU raster/参考按窗口实际 device scale 生成，截图与参考使用相同设备分辨率；报告保留 deviceScale/rasterWidth。

## 像素门槛

独立高反差图像复测还发现 GPUI paint_image 的设备像素吸附会伸缩小数坐标的图片，已通过缓存透明保护边和设备整像素裁剪 quad 保持原图采样坐标。旧失败记录 run-1793326724157898000 / run-1793326842069797000 保留；未放宽下列阈值。

首轮调试发现透明图像在 GPUI 边界被重复预乘而变暗。已改为在边界将 tiny-skia premultiplied RGBA 转成 straight BGRA，由 GPUI shader 执行一次预乘；新增半透明像素回归。修复后 EFFECT 平均 RGB 误差约 0.10。

普通对象的调试原始平均误差约 3.88，但 3×3 颜色范围 ≤8 的稳定内部区域平均误差为 0，差异集中在文字/曲线/缩小图像的抗锯齿边缘。依据渲染规格允许 AA 差异，release 前固定以下分区门槛，并保留所有原始指标和 difference PNG，不将调试结果冒充 release 结果：

| 指标 | 上限 |
|---|---|
| 全画布平均 RGB 误差 | 5.0 / 255 |
| 稳定内部区域平均 RGB 误差 | 1.0 / 255 |
| 内部区域 RGB 最大通道误差 >16 的像素比例 | 0.1% |
| 边缘区域最大通道误差 >128 的像素占全画布比例 | 0.2% |

文字轮廓由同一 SVG shaper 提供，Scene/CPU 回归另查几何、z-order、透明度、glyph 数量、缓存失效；像素门槛不能替代这些结构检查，也不证明 M3 活动文本编辑已统一布局。

## 当前实现范围

Scene 在无 GPUI 的模块生成；含文档/页/revision 身份、顺序节点、局部位置、path 及填充规则、文字轮廓、局部 clip、image 和隔离 fallback。对象缓存忽略位置/ID，样式和几何改变才重新生成。GPU 缓存为窗口独立，保留 CPU 切换入口。

原生文档当前尚无用户图片导入/组/滤镜模型；这些图像/效果由 Scene 基准节点直接提供以验证 backend，不能声称 M5 的导入与持久化已实现。M3 活动文字编辑仍使用既有输入桥；静态正文的 CPU/GPUI 共享同一轮廓。

实际 release 数据、截图、故障/旧阶段回归、产物与开放条件见 [26 实施报告](26-m4-implementation-report.md)。任务清单、STATUS 与追踪矩阵已同步。

## 内存复核与最终字形路径

首个逐帧绘制完整正文轮廓的方案虽然 p95 达标，但原生进程 RSS 采样约 2.7 GiB；vmmap 显示大量已空闲的 Large allocation 高水位。不能仅用适配器约 2 MiB 的缓存统计宣称总内存通过。该方案的包仅保留作历史对照，不作为最终交付推荐。

最终正文保留 SVG shaper 的 glyph ID/位置与真实字体轮廓，按实际 scale/DPI 生成逐 glyph alpha mask；16 个四分之一像素相位打包为同一字形纹理，用相同纹理的裁剪区域实现亚像素位置。平移不重新 shaping、不重新生成/上传相位纹理，避免每帧复制整段复杂轮廓，也避免逐段文本位图替代独立 glyph。

最终基准额外要求原生进程 RSS 每 0.5 秒采样的最大观察值 ≤450 MiB。该值参考资源规格的 SMALL 预算，但本次运行的是 EDIT/EFFECT 进程，不能据此关闭完整 SMALL/LARGE 或长期资源验收；它也不是精确的硬件峰值。原始 RSS、二进制 SHA-256 与像素/延迟结果均保留在 verification.json。
