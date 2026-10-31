# M4 Scene 画布实施与本机验收

日期：2026-10-30。按用户要求继续 M4；M3 未完成项保留。**现有模型的核心绘制切片已实现，并完成 macOS release 画质、延迟、RSS 和旧阶段回归；完整 M4 退出仍有开放条件。** 方法与预先固定的误差/内存门槛见 [25](25-m4-scene-validation.md)。

## 实现

- 新增无 GPUI 类型的不可变 Scene：文档/页/revision 身份、稳定对象 ID、顺序节点、局部位置、路径/描边展开/填充规则、静态文字 glyph 轮廓/位置、clip、bitmap 和隔离降级。CPU 对照与 GPU 使用同一布局来源。
- 现有矩形、椭圆、圆角矩形、箭头、线走 GPUI path；静态正文从同一字体 face/glyph ID/transform 生成逐 glyph mask。16 个亚像素相位打包为同一字形纹理，平移复用纹理并选择对应裁剪区域。单独字形可复用，不把整段文字或整页图片假称为文字加速。
- 普通图像 Scene primitive 经 GPUI image/sprite 实际绘制；本阶段用 20 个独立原创 512² 程序图像资源验证；Node::image 对尺寸/字节/坐标先校验。图片文件导入、原生资源持久化、可编辑组/滤镜模型仍按 M5 及后续阶段实施。
- 透明对象、彩色/位图 glyph 和不支持的组/clip/filter 在语义封闭的对象节点局部 raster，按原 z-order 只合成一次。修复 premultiplied RGBA → straight BGRA 边界，避免 GPUI shader 重复预乘。图片纹理另加有界透明保护边，在整设备像素 quad 内按原图坐标裁剪，修复上游 bounds 吸附引起的小数位置拉伸；独立高反差纹理复测确认，失败记录保留且阈值不变。
- 位置/ID 不进入内容几何缓存键；内容/样式/尺寸/旋转改变才重新布局。窗口独立 GPU 缓存按 scale/DPI 失效，平移不重新 shaping/tessellate/upload。每帧仅复制当前页的交互投影，避免拖动时复制整份大文稿。
- 一个后台 Scene job，输入变化合并为最新状态；逐对象检查取消，发布时校验文档/revision/page/geometry generation，窗口释放时取消。过期/跨文档结果不得发布。
- Scene 内容缓存 64 MiB、活动 Scene 估算 128 MiB、窗口 GPU 派生缓存 64 MiB，支持 LRU 与显式重建；旧 image 经 GPUI drop_image 移除 atlas 条目。数值是适配器可观测的估算/保留预算，不能等同于驱动 VRAM 或整个框架 RSS。
- 对文字排版在分配 glyph 前限额：单框 ≤50,000 Unicode scalar、单页 ≤100,000；CPU/GPU 共用检查，超限提供错误并保留原文，可继续保存或缩短后重试，不截断内容。
- 保留 CPU 切换与 `--cpu-canvas`；适配器出错自动切回 CPU 内容路径并保留文档。增加画布计数、缓存重建、stage M4、跨平台打包入口与原生 QA 工具。Windows 旧打包入口已改为新脚本，避免以当前代码覆盖历史 M2 包。

## 验证

- `cargo xtask check`：fmt、all-targets Clippy（-D warnings）和 **45 项测试通过**；[日志](../logs/m4-check.log)。
- `cargo xtask stage M4`：实际链接及 workspace tests 通过；[日志](../logs/m4-stage.log)。release 实际链接见 [日志](../logs/m4-release-build.log)。
- 新增 10 项 Scene 测试：CPU 原画面/几何/透明度/顺序、位置复用与样式局部失效、真实 image 数据与隔离效果、取消/非法模型、CPU 基准入口、alpha 边界、LRU/清理不改文档、共享 glyph 轮廓/位置、超限文字在 shaping 前拒绝且不改文档、image primitive 拒绝非法尺寸/字节/非有限坐标。原 35 项测试保留。
- M0–M4 原生 adapter 回归覆盖窗口、浏览、保存/关闭/恢复、文字输入桥、正文平移缓存、CPU 切换、undo/redo、atlas 重建和跨文档过期 job 丢弃。最终分发包 [M0–M4 汇总](../logs/m4-final-packaged-native.log) 与 [M4 日志](../logs/m4-native-darwin.log) 均通过；它调用生产处理器，不能等同于物理鼠标/键盘或三平台验收。

- 最终包实际 macOS 系统拼音预编辑候选、提交/取消、单次 undo、保存重开通过；[运行日志](../logs/m4-packaged-pinyin.log)、[机器结果](evidence/m3/pinyin-1793327159880251000/result.json)、[候选窗](evidence/m3/pinyin-1793327159880251000/candidate.png)、[提交后](evidence/m3/pinyin-1793327159880251000/committed.png)。自动发送 OS 键盘事件，原输入源已在 finally 中恢复，非物理键盘验收。

## 性能与画质结果

本机 Apple M4 / 24 GiB，macOS 27.0.1，Rust 1.98.1；[硬件记录](evidence/m4/hardware.json)。最终分发二进制每 backend 预热 3 帧、测量 48 帧；这是程序变换提交到 GPUI post-render callback 的反馈，不是物理输入到 scan-out。CPU 对照也缓存排版，逐帧整页 raster；CPU/GPUI 按窗口同一 device scale（本次 1.0、1000 像素画布宽）绘制。

| 场景 | CPU p95 | GPUI p95 | GPUI frame p95 |
|---|---|---|---|
| BENCH-EDIT | 50.044 ms | 16.724 ms | 16.724 ms |
| BENCH-EFFECT | 33.359 ms | 17.418 ms | 17.419 ms |

EDIT 的 p95 降低 **66.6%**，达到 ≤20ms 与相对 ≥40% 两种门槛。固定 200 节点：120 几何、60 文本块、8,100 shaped glyph（missing=0）和 20 个独立 512² 图像资源/节点。GPUI 阶段没有整页 raster，glyph mask 与 bitmap 复用。

最终基准进程每 0.5 秒采样 RSS 最大观察值 **205.72 MiB**，通过本轮 450 MiB 观察门槛；它不是精确峰值，也不能替代 SMALL/LARGE/长期验收。首轮逐轮廓方案约 2.7 GiB 的问题经 glyph mask 相位纹理方案修复，没有通过放宽内存门槛掩盖。

最终分发包 [完整基准 JSON](evidence/m4/run-1793327137407213000/verification.json)、[原生日志](evidence/m4/run-1793327137407213000/native.log)、[运行输出](../logs/m4-packaged-benchmark.log)。其二进制 SHA-256 为 `b5115e4b5f0bfcd8e3c6deaecfe0c819855fcfe0a77d04aebfbe8a12a90fe264`，与包内 macOS Mach-O 可执行文件逐字节一致。

四组实际窗口像素比较均通过既定门槛：CPU 自身截图误差 0；EDIT GPUI 平均 RGB 误差 3.441，稳定内部误差 0，边缘 >128 差异像素占 0.0876%；EFFECT GPUI 平均误差 0.099。保留原始差分，不将抗锯齿误差称为逐像素相同。

[EDIT 实际画布](evidence/m4/run-1793327137407213000/BENCH-EDIT-GPUI-canvas.png) · [EDIT CPU 参考](evidence/m4/run-1793327137407213000/BENCH-EDIT-GPUI-reference.png) · [EDIT 差分](evidence/m4/run-1793327137407213000/BENCH-EDIT-GPUI-difference.png) · [EFFECT 实际画布](evidence/m4/run-1793327137407213000/BENCH-EFFECT-GPUI-canvas.png)。已逐一查看截图；无缺对象、透明度变暗或相位纹理串格现象。


## 开放条件

| 任务/阶段 | 已验证部分 | 未满足的完整条件 |
|---|---|---|
| T-M4-01 | 固定 EDIT/EFFECT、seed/轨迹、硬件、release 帧反馈与 RSS/资源计数 | 完整 OS 输入→领域 command→layout→driver upload→硬件 present 分段 trace；当前指标为程序变换到 post-render callback |
| T-M4-02 | 当前模型的 Scene 与 CPU 对照、glyph/mask/image、对象级隔离 | 完整公开 Group/Effect/Link/Hit 元数据及后续模型集成；不把基准专用图像/效果当作原生文档功能 |
| T-M4-03/04/05 | 本机图形/image/glyph mask、透明/裁剪/隔离降级与像素对照 | 未测平台保留；M3 活动文字编辑仍使用既有 GPUI 输入布局 |
| T-M4-06 | LRU/预算、取消/最新任务合并、有限 scale/DPI 缓存、atlas 清理 | 母版/字体动态 generation、分块/更完整优先级、跨屏与长会话/多窗口全局预算验证 |
| T-M4-07 | 适配器错误 CPU 切换、显式缓存重建且文档不变 | 真实 device-lost 注入和恢复；锁定 GPUI 无本应用可用的完整设备丢失回调。CPU 内容回退不等于整个 GPUI 宿主可软件渲染 |
| T-M4-08/09 | 本机 release 渲染门槛、画质、缓存断言和可运行包 | 完整 AT-07/15 人工演示、真实输入链路与硬件/平台矩阵；完整 M4 不勾选 |
| M3 及此前 | 已通过的原生/系统拼音与数据安全回归保留 | M3 完整排版/段落/输入等软件缺口、此前 LARGE/完整故障/跨屏/平台验收未因此关闭 |

Windows/Linux 的 M4 本机结果未取得；CI 已更新但未远端执行。包未签名/公证、未做干净机器安装。没有真实设备丢失测试，不能用“清空缓存”冒充此项已通过。

## 产物与回退

- 推荐：[M4 macOS ARM64 release 开发包](../dist/joydeck-m4-final-darwin-arm64.zip)。包内有校验值、fixtures、许可证和 QA 脚本；交付归档的外部校验值见 [checksums](../dist/joydeck-m4-final-checksums.json)。
- 回退：[M4 开发前源码](../dist/joydeck-before-m4-source.tar.gz)、[M3 已验证包](../dist/joydeck-m3-verified-darwin-arm64.zip)。源码目录没有 Git 元数据，未虚构提交。
- `joydeck-m4-verified-darwin-arm64.zip` 为首轮逐轮廓方案，RSS 高，不推荐使用；保留作故障对照。glyph-cache 中间包也不是本次最终交付入口。
- [本轮源码快照](../dist/joydeck-m4-final-source.tar.gz)；[机器可读交付索引](evidence/m4/verification.json)。复现：`cargo xtask check`、`cargo xtask stage M4`、`python3 scripts/smoke-native.py M0 M1 M2 M3 M4 --binary <release binary>`；原生像素基准使用带 Pillow/numpy 的 Python 运行 `scripts/benchmark-m4.py`。
