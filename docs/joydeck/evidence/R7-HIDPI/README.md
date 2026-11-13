# JoyDeck 放映实际像素与缓存修复

2026-11-13。任务分支 `fix/joydeck-slideshow-dpi` 从 `origin/main` 的 `55b15071` 开始，独立工作树不修改原 JoySheet 分支。此记录只覆盖本次放映清晰度、缓存和常见转场合成优化，不关闭 JD-700/701/703/804 的完整验收。

## 依据与选择

- [PowerPoint 官方性能说明](https://support.microsoft.com/en-us/powerpoint/tips-for-improving-the-performance-of-your-presentation)讨论放映分辨率与硬件加速。公开说明不能证明闭源引擎每帧的具体绘制算法。
- [LibreOffice 官方源代码](https://github.com/LibreOffice/core/blob/master/slideshow/source/engine/slide/slideimpl.cxx)的 `getCurrentSlideBitmap` 按视图变换计算目标尺寸，缓存不存在或尺寸不符时重新生成；`createCurrentSlideBitmap` 用目标视图变换绘制形状。[形状绘制](https://github.com/LibreOffice/core/blob/master/slideshow/source/engine/shapes/viewshape.cxx)同时使用渲染器与透明合成位图缓存。[引擎说明](https://docs.libreoffice.org/slideshow.html)另有 OpenGL 转场。
- 由以上证据推断，成熟实现可结合矢量内容、设备像素栅格化、缓存和图形合成，不能把“每帧重画所有矢量”作为普遍最优方式。本次独立实现，不引入或复制外部代码。

JoyDeck 的现有编辑场景没有承接放映的对象/段落动画状态、Morph、全部转场和媒体合成。直接替换会扩大兼容风险。本次保留 `show::Player` 的呈现语义，将实际设备像素分辨率、任务背压和缓存接入原放映路径，并优化通用合成器的常见矩形情形。最终仍由 GPUI 显示纹理；这不是完整的对象级 GPU 放映引擎。

## 实现

- `show_render::RenderTarget` 用可用逻辑区域乘当前窗口 scale factor 计算目标分辨率。先扣除现有 42 逻辑像素工具栏，再请求画面。窗口缩放、全屏和 DPI 变化重新计算目标。
- Windows 全屏切换系统缩放时，GPUI 可能更新 DPI 却保留旧逻辑区域。观众窗口通过一次设备像素的尺寸变化触发系统重算，再恢复原物理范围；待实际 resize 完成后才清除待恢复状态。此补偿局限于全屏 DPI 变化，没有改公共 GPUI 或其它产品。
- 放映倍率上限与普通页面渲染统一为 8；保留 8192 单边、1600 万像素预算，并按向上取整后的分配尺寸检查。超预算时适应可行倍率，不创建无限分辨率接口。
- `slideshow_frames::Requests` 只允许一个后台任务，期间只更新最新需求。发布时同时校验页面/步骤/操作代次、目标倍率与尺寸、字体环境。允许动画时钟前进，避免因每帧时间戳改变而永远拒绝渲染结果。
- 媒体使用的静态页面底图最多保留一张（现有上限下 64 MB）。合成前取得独立像素副本；动画、转场和含 GIF 资源的文稿不复用静态底图。媒体的解码、音量、暂停、定位和播放控制仍使用原代码。
- 静态页不再每 33 ms 无条件要求 UI 重画；时钟保留上次观察到的状态，仍可发现演示者窗口发出的暂停、定位和继续命令，避免只比较同一次 tick 前后导致外部控制不刷新。
- 像素对齐、均匀透明度的矩形转场层直接复制/混合像素，避免通用三角形覆盖测试和双线性采样。透视、亚像素移动、渐变透明度等继续走通用 mesh 路径。与原采样对照允许至多一个 8 位通道级别的舍入差异。
- 不透明纹理的部分透明度使用 16 位整数权重，允许编译器向量化；含透明像素的纹理沿用预乘 alpha 合成。没有更改转场时序、对象动画状态或媒体解码协议。
- 诊断日志记录画面尺寸、倍率、实际栅格化次数与缓存命中，以及后台准备平均/峰值耗时。该耗时不包含 GPU 呈现，不能解释为实际显示帧率。

## 验证记录

### 自动验证

- `cargo xtask check joydeck --release`：982 项通过，0 失败、0 忽略；包括产品 fmt、严格 Clippy、来源/依赖/文档门禁。没有修改依赖、根锁文件或共享库。
- 其中 UI 14 项、渲染器 199 项、定向放映 5 项通过。新增覆盖 100%/150%/200%/400% 目标像素、极端长宽比预算、非法输入、超过原 4 倍限制的小尺寸页面、缓存独立性、取消、黑屏，以及动画/转场像素一致性。
- 请求队列覆盖连续缩放合并、过时结果拒绝、字体代次、动画不饿死；另有全屏 DPI 延迟 resize、静态/暂停时演示者控制的回归。转场快速路径与原 mesh 路径逐通道对照，覆盖透明/不透明纹理、透明度、裁剪/位移及拒绝亚像素/渐变情形。
- `cargo xtask build joydeck --release` 和 Windows helper 构建通过。最终产物指纹见 [verification.json](verification.json)。
- `python scripts/joydeck/verify-r6-r7.py --cli target/release/joydeck-cli.exe --evidence target/joydeck-dpi-check/r6-r7-final.json`：35 次 CLI 调用、52 条 MCP 响应通过；原始输入文件散列不变。覆盖原有控制、媒体探测、导出和异步任务等流程，详见 [原始记录](r6-r7-final.json)。
- `scripts/panic_baseline.txt` 仅登记新增测试夹具的 28 处断言构造，不豁免生产路径。

### Windows 原生窗口实测

本机 Windows 桌面 2560 × 1440，原缩放 125%，Intel Core Ultra 9 285H；设备列有 Intel Arc 140T、NVIDIA RTX 5060 Laptop 与 GameViewer Virtual Display Adapter。这里是在当前交互桌面进行原生窗口操作，不能据此认定某一物理 GPU/显示器已完成资格化。

- 同一自制 PPTX 的 10/12 pt 文字与细线，对比旧固定 1 倍放映和新放映。125% 全屏从 960 × 540 的底图放大改为 2467 × 1388 的实际请求，边缘清晰度改善。截图包含当前远程桌面的捕获缩放，不作为逐物理像素测量工具。
- 窗口/全屏往返和系统 100%/125%/150%/200% 配置实际操作。窗口缩放到 150%/200% 后请求尺寸变化；最终 Windows DPI 补偿版本复验 200% 启动及全屏 200% → 125%，日志记录两步 refit 后恢复正确全屏范围与 2467 × 1388 目标。没有用单元测试替代这次系统设置操作。
- 原生测试文稿中的淡入转场与标题对象动画仍可触发并显示终态。相同时刻的像素采样、暂停时序由自动测试验证；实屏没有录得 2 秒动画中途暂停的逐帧证据，不把终态截图当作此证明。
- 最终 Release 的演示者窗口控制嵌入 H.264：静音播放、暂停在 2.6 秒、暂停中定位至 0.0 秒、继续播放。观众窗口连续两次截取得到不同的视频画面。翻页跳过隐藏页到第 3 页，解码 helper 随离页退出；Esc 关闭放映和演示者窗口，编辑器正常退出。
- 本次媒体底图日志在第 360 帧为 `rasters=6 cache_hits=355`；页面与控制代次变化会重新栅格化。测量时存在并行编译，不将媒体准备耗时当作稳定性能基准。
- 系统缩放已恢复 125%，测试窗口已关闭。

[修复前全屏](before-fullscreen-125.jpg) · [修复后全屏](after-fullscreen-125.jpg) · [200% 全屏](after-fullscreen-200.jpg) · [暂停](media-paused.jpg) · [暂停中定位](media-seek.jpg) · [播放画面 A](media-frame-a.jpg) · [播放画面 B](media-frame-b.jpg)

### 同机空闲性能对照

使用同一份自制 960 × 540 双页 PPTX、同一淡入转场时刻。旧/新 CLI 在没有 GUI 或编译负载时依次运行，各取 3 次中位数。每次请求包含 MCP/RPC、栅格化、PNG 编码与写盘，独立用 Pillow 校验输出尺寸。没有把端到端耗时换算成屏幕 FPS。

| 输出尺寸 | 静态旧/新（ms） | 淡入转场旧/新（ms） |
| --- | --- | --- |
| 960 × 540 | 14.25 / 11.48 | 40.89 / 16.49 |
| 1920 × 1080 | 20.84 / 19.76 | 137.66 / 35.12 |
| 3840 × 2160 | 53.92 / 50.50 | 527.21 / 112.97 |

本例 4K 淡入端到端耗时减少约 79%。静态页面保持一次渲染，媒体底图缓存避免重复排版，单任务合并限制后台积压。缓存的一张底图预算不代表全程序内存上限，显示纹理、过渡画面、媒体解码仍有各自开销。

[旧版原始数据](baseline-idle-timings.json) · [新版原始数据](final-idle-timings.json) · [测量脚本](bench_comparison.py) · [输入夹具](clarity.pptx)

复跑时从仓库根目录执行：将这里的 `clarity.pptx` 复制到 `target/joydeck-dpi-check/clarity.pptx`，为测试脚本在 `target/corpus-tools` 安装 `Pillow==12.0.0`，再运行 `python docs/joydeck/evidence/R7-HIDPI/bench_comparison.py --cli target/release/joydeck-cli.exe --prefix local-new`。输出路径拒绝覆盖，因此再次测量使用不同前缀。旧版使用基线 `55b15071` 的独立 CLI；本次实际二进制散列已随原始数据保留。夹具生成脚本另需 `python-pptx==1.0.2`，直接使用已保存的 PPTX 不需要此依赖。

### 保留的边界

4K PNG 渲染已自动验证，真实物理 4K 显示器、跨不同分辨率的多屏移动、其它操作系统与信创真机尚未验证。观众时钟维持原有 33 ms 调度；复杂透视转场、Morph 和对象动画继续使用现有 CPU 采样/合成路径，没有声称所有效果达到 60 FPS。媒体沿用原先约 10 Hz 的画面更新策略，本次未改造成完整 GPU 视频管线；静音播放已验证，实际声音输出与音画同步没有听测。JD-700/701/703/704/708/804 与三平台完整性能资格化保持开放。
