# 幻灯片渲染与文字系统规格

本章将“使用 GPUI”转化为可验证的画布性能、画面质量和输入精度要求。预算见 09，阶段为 M1 基线、M3 文字、M4 加速、后续领域持续扩展。

## 1 三条职责链

1. **应用 UI**：GPUI/Kit 完成工具栏、面板、窗口、焦点和界面文字。
2. **文档布局与场景**：统一解析母版/主题、几何、文字、图表、动画及裁剪，产出后端无关 Scene。
3. **场景绘制与输出**：CPU、GPUI 和 PDF 等后端消费一致的布局/场景语义，按能力处理 fallback。

GPUI 提供 Element、canvas、path 等绘制入口，但需要应用自己处理布局和输入几何；其 path 能力不是任意 DrawingML/SVG 效果实现。[GPUI 绘制指南](https://gpui-kit.com/docs/paint/)

M1 独立生成预览场景，使用通用 CPU 栅格化库形成可运行基线；后续以同一布局语义扩展 Scene 和加速路径。

## 2 渲染演进及通过条件

| 阶段 | 方案 | 用户可见成果 | 必须证明 |
|---|---|---|---|
| R0 / M1 | CPU 渲染不可变文档快照，后台返回图像；GPUI 显示与 overlay | 真实 PPTX/原生页可浏览，缩略图渐进出现 | 主线程不做完整页 raster；输入/窗口仍响应；晚到结果不覆盖新页 |
| R1 / M2–M3 | 页面/字体缓存，独立选择与 caret overlay | 拖框、光标移动无需重画整页 | raster/upload 计数在 overlay-only 操作时不增长 |
| R2 / M4 | Scene 提取，常见路径/图片/文字走 GPUI，复杂局部走 CPU 缓存 | 普通页面拖拽缩放平滑且放大清晰 | 基准中普通图形和文字有实际后端覆盖与端到端收益 |
| R3 / M7–M12 | 扩大样式、表格图表、动画及矢量输出覆盖 | 常用内容编辑、播放、输出一致 | 不同后端共享 layout，能力与降级统计透明 |
| R4 / M14 | 可选外部 GPU scene renderer / 硬件视频互操作 | 极复杂效果、大文稿和视频进一步优化 | 共享 surface/同步/设备生命周期在三平台实际证明 |

M4 的最低加速门槛：BENCH-EDIT 普通对象集（无复杂滤镜）的可见实色/描边路径、图片和正文能够经 GPUI 原语或经验证的 GPU scene 路径绘制；移动对象不整页 CPU 栅格化，文字移动使用缓存 glyph/轮廓；达到 09 的帧时目标，或相对 R0 将相同拖动的 p95 端到端延迟降低至少 40%，且保持画面正确。40% 是相对方案的最低门槛，不替代发布时绝对交互预算。

只把整页 RGBA 图显示进 GPUI、只有 FPS 数字变高，不能关闭 M4。若公开 GPUI API 不足，应完成一个可运行的限定范围 adapter 或调整后端方案并记录，也不声称加速已完成。

## 3 场景数据契约

Scene 针对一页/一个 viewport，带 DocumentId、SlideId、layout generation、字体和资源 generation、时间采样信息。它必须可在无 GPUI 环境生成。

| 数据 | 最少字段与语义 |
|---|---|
| SceneHeader | 文档页面尺寸、背景、色彩空间、模式、revision、资源依赖 |
| Node | 稳定对象 ID、z 顺序、局部到页面 transform、可见性、边界、clip、opacity |
| PathItem | 填充规则、路径段、fill/gradient、line/joins/caps/dash、命中数据 |
| TextItem | 字体引用、glyph IDs/位置、cluster 映射、baseline、装饰、段落与 caret 信息 |
| ImageItem | ResourceId、解码尺寸、src crop、dst transform、alpha、采样策略 |
| GroupItem | 局部坐标、子节点、整体 alpha/clip/filter 语义 |
| EffectItem | 效果参数、离屏范围/halo、后端支持与 fallback 标记 |
| Link/Accessibility | 页/对象/文本范围、动作、alt text 和阅读顺序引用 |

Scene 中不存 GPUI Pixels、GPU texture handle、窗口或 command callback。geometry/layout、paint 与 hit metadata 可以分缓存存储；不为了后端无关而每帧复制全部大位图。

转场 Scene 可引用前后页的静态场景或纹理，并携带时间采样。模型中的动画参数与临时绘制状态分离；暂停/预览不修改 Presentation。

## 4 GPUI 后端与 fallback

首选路径：GPUI quad/path 用于可准确表达的形状、线条和选择；图片使用受控 image/texture cache；文字保持统一排版，可以用缓存 glyph mask/outline 实现，或在逐项核对后使用可保持相同 glyph 位置的桥。不能把一整段文字交给另一个排版器后仅凭视觉近似通过。

每个 paint item 有后端能力判断。若阴影、混合、组透明度、渐变、滤镜或字体颜色层暂不支持，选择最小**语义封闭**子树 raster。组效果需要把整组及其裁剪/透明语义共同处理；不能单独 raster 一个子对象破坏叠加结果。

Fallback 图像在原 z-order 合成，只画一次；含 effect halo 的离屏范围要有上限。不可创建与用户无限放大成比例的超大纹理。若后端无法保持效果语义，使用整页 CPU fallback 并展示诊断，不重复覆盖造成双重半透明或模糊。

GPU 资源不跨设备/窗口无条件共享。GPUI 与外部 wgpu/Vello 使用不同 device 或 API 时，零拷贝不能假定成立。M0/M4 的 spike 需要验证像素格式、alpha、surface、fence、纹理生命周期、device lost 及打包；有 CPU readback 的路径必须统计其传输耗时。

## 5 坐标、精度及裁剪

文档计算采用 f64 pt；边界转换到 GPUI logical px，最后由 DPI 生成 physical px。EMU 仅在格式适配边界转换，1 pt = 12,700 EMU。显示比例是应用定义的 document-to-logical scale，不假定所有 OS 都按 96 DPI 显示实际英寸。

坐标链为对象局部 → group child transform → 页面 → viewport → window → OS screen。所有链都有可测试逆变换；奇异矩阵拒绝或安全降级。保留选择/光标精度直到最终绘制，不能每次拖动先四舍五入再写回文档。

区分 shape geometry bounds、stroke/effect visual bounds、hit bounds、clip bounds。编辑 pasteboard 包括页面外对象，输出只含页面范围；组和图片的裁剪优先级由 Scene 明确，不依赖 UI 容器偶然裁剪。

## 6 颜色、图片及效果

首版明确以 sRGB 为输出色彩空间；保留原文件颜色引用和可保留 profile 元数据，但不声称完整 ICC/HDR/P3 色彩管理。UI 与文稿颜色转换独立。

内部图像统一标明通道顺序、alpha 是否 premultiplied、色彩空间和 stride；CPU↔GPUI 上传做一次明确定义的转换。半透明边缘、阴影、带 alpha PNG 和叠加图案必须有测试，避免白边/黑边。

图片解码先检查尺寸/像素数/字节预算，支持按目标尺寸下采样和 EXIF 方向；原始 bytes 仅在资源存储保留一份。GIF 的帧时间和循环数属于可采样资源，解码帧缓存有界，暂停时不持续唤醒画布。

阴影、模糊、反射等扩展 extent 不超过规定设备/内存限制；高倍率缩放使用 tiles 或降低预览分辨率并给出状态，释放交互后补高质量结果。输出分辨率与编辑预览分辨率分别控制。

## 7 文字布局契约

保持原 HarfRust/skrifa 等布局能力，并逐项提升质量。GPUI TextSystem 可用于应用 UI；它的 shape_line 面向单行，不能取代幻灯片的段落、列和母版继承。[GPUI TextSystem](https://gpui-kit.com/docs/text-system/)

文字流程：有效样式解析 → 字体族/face/语言选择 → script+bidi 分段 → shaping → cluster-aware 断行 → 段落/列布局 → autofit → glyph/装饰输出 → caret/selection map。

| 关注点 | 要求 |
|---|---|
| 字体身份 | 缓存使用 FontSource + face index + variation axes + generation，不能只用 family 名 |
| 样式匹配 | weight/italic/variable axes 显式选择；合成粗斜体必须可识别并纳入保真报告 |
| fallback | 优先完整 script/cluster 的正确字体，不能逐字拆散连接脚本；缺字有可见替代 |
| bidi | 保留逻辑顺序与视觉顺序映射；选择和导航遵循清晰语义 |
| 断行 | Unicode 规则与 CJK 禁则逐步验收；不能将字符串字节数当字符宽度 |
| 段落 | 行距、段前后、缩进、列表 marker、tabs、对齐、RTL 与空段均布局 |
| autofit | 用同一字体与行布局测量；迭代次数有界，禁止数值振荡 |
| 字段 | 页码、日期、链接样式按导出/播放上下文稳定求值 |
| 嵌入字体 | 检查可用性和嵌入权限，无法解包的子集保留并用替代字体 |
| 真竖排 | 未实现时保留语义并说明，旋转结果独立标记 |

布局缓存键含文本内容/样式、可用宽高、继承样式、字体版本、语言、字段上下文和编辑提示状态。仅光标变化不重新 shaping；字体变化必须失效相关页面，不能继续用旧度量画新 glyph。

## 8 光标与选区

保留 paragraph/run/cluster 的双向映射；字符导航使用 grapheme 规则，单词导航按平台与语言策略。glyph ligature 可对应多个字符，点击时选择最近合法 caret stop，不制造 glyph 中间的非法位置。

光标保留 upstream/downstream affinity，特别是换行末尾和 bidi 边界；表格单元格、备注、旋转文本和组变换使用相同结构。选区背景、预编辑下划线和 caret 全部依据布局生成，overlay 单独刷新。

布局无结果或字体正在加载时提供稳定占位并禁止错误坐标写入；不得通过 unwrap/panic 处理缺失 glyph。

## 9 增量、缓存和调度

缓存按 shape/layout/scene/raster/texture 分级。可见当前页优先，其次下一页放映预载、可见缩略图、邻页、屏外后台。低优先任务不能挤掉交互中的绘制和文本布局。

快速拖动期间合并高频事件，每帧只提交最新视图；旧任务完成时比较完整 key。缩放先复用已有图片，同时更新 vector viewport，停稳后补精细 fallback；不得为每个 wheel tick 排入永不取消的完整页 raster。

不同 DPI/scale 采用有限 bucket 与复用窗口；缩略图和主画布独立预算，不因缩略图滚动清掉当前页。缓存淘汰只回收可重建派生数据，不影响历史/原文件资源。

## 10 质量验收

测试同时检查模型/Scene 语义、文字坐标、像素容差和真实交互。固定字体/时间/scale 后建立 CPU golden；GPU 与 CPU 容许抗锯齿差异，但不容许缺对象、顺序错、文本换行/裁剪错误或透明度重复。

性能记录 input timestamp、command、layout、raster、upload、paint/present，区分 CPU 时间与端到端帧延迟。缓存命中页不能被当作首次复杂页面渲染结果。复杂效果专项与普通交互专项分别报告。

device lost 时回收 GPU cache、保留文档、尝试受支持的窗口/设备重建；若 GPUI 宿主无法恢复，先保存恢复数据并提供受控退出。CPU 幻灯片渲染能力不等于整个 GPUI 外壳有软件图形 fallback。
