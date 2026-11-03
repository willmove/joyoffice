# R4/R5 0.5.0 场景与内容工作区切片

本轮按可运行纵向切片推进，完整 R4/R5 完成条件仍保留在 [任务清单](../../TASKS.md)。依赖基线继续对齐 JoyOffice，原 JoyOffice/DeckCraft 目录保持只读。

## 交付与复现

2026-11-03，Windows x86_64 MSVC release 开发包：`joydeck-0.5.0-r4-r5-windows-x86_64-release-6233617cc5.zip`，位于仓库 `dist/joydeck`，22,656,665 字节。解压后运行 `joydeck.exe`，可打开包内 `samples/r4-r5/independent-complex.pptx`；标题栏可切换 GPUI / CPU，并打开渲染诊断。完整操作见 [包说明](../../../../packaging/joydeck/README-r4-r5.md)。

- ZIP SHA-256：`5405e87d36bcc5b5d69715566416cc70ce81a392bb289051c93b43471948deaf`。
- 构建源码指纹：`6233617cc50a8eda7ce097b42eb99cb89ac045d3bedafc33d153eecf9c2a3a32`。
- [包清单](package.json)、[构建日志](logs/accepted-package.txt)、[源码与 Git 暂存对象逐字节核对](source-index-verification.json)。290 个源码记录全部匹配，包内 762 项 SHA256SUMS 校验通过；Windows 主线程栈为 8 MiB。
- [最终桌面运行记录](gui/accepted/run.json)：从 ZIP 解压到仓库外临时目录运行，记录二进制 SHA-256、输入文件指纹、进程和运行路径。输入原件未修改，验收窗口均已关闭。
- [最终验收汇总](accepted-verification.json)：本次交付包的编译、测试、CLI、桌面、像素、公开格式和窄窗口结果。

未签名开发包；SBOM 为实际 normal/build 闭包 446 包。21 项依赖缺少上游包内许可全文的既有发行审计项仍列于 package.json，不能据此声明完成正式发行资格。未分发本机系统字体。

## 已实施的可运行切片

| 范围 | 实际行为与边界 |
| --- | --- |
| R4 场景 | 后端中立显示列表；普通形状实色填充、基本描边及路径、准确的字体 face/字形轮廓交给 GPUI。采用 CPU 相同排版结果，不由 GPUI 二次塑形。旋转、翻转和裁剪随对象转换。 |
| 回退与缓存 | 图片、图表、表格、组合、渐变和效果使用对象位图，诊断显示原因；真正依赖页面背景的填充切回整页 CPU。中立场景 48 MiB、GPUI 资源 64 MiB LRU 预算；几何、背景和资源分开复用，DPI/尺度、字体修订、主题、媒体等参与失效。资源计数不是实际显存测量。 |
| 增量交互 | 拖动时平移场景节点，松开后提交一个命令；保留未变页缩略图。后台构建单任务、支持取消及文稿代次检查，拒绝安装过时结果。 |
| 母版和主题 | 页面/母版实色背景、已有版式、母版图形显隐、主题强调色 1–6、拉丁标题/正文字体、文稿尺寸与标题/作者；尺寸修改不缩放对象。完整母版对象编辑尚未实现。 |
| 图片 | 本地位图插入、替换、裁剪、透明度、亮度、对比度；规范化 PNG。16 MiB 输入、8192 边长、16 MP、64 MiB 解码与文稿媒体预算。 |
| 图表 | 基础分类图表单元格数值、标题和常见类型；公开 PPTX 同步写入图表缓存与数据型 XLSX。带不支持扩展、独立样式或外部依赖的已修改图表拒绝公开保存，原生副本保留。 |
| 设计工作区 | 开始/格式/文字表格/设计/图片/图表标签、真实对象属性、基本形状和实色/线性渐变；窄窗口收起属性栏，长表单和诊断支持滚动。沿用 R2/R3 输入、撤销重做、恢复及保存重开。 |
| 公开格式 | 对原始 PPTX 定向修改 XML、关系、主题及媒体/图表部件，保留无关部件；保存前独立重开比对。未识别的结构不能以静默删除代替保留。 |

复用原 JoyDeck 的 GPUI 路径适配、几何/纹理缓存和图像释放方法，接入本项目的模型及渲染结果；未引入旧 JoyDeck 的第二套长期可编辑模型。来源与适配记录已更新到 licenses。Rust 1.98.1 / GPUI Kit 0.8.1 / GPUI pre 0.3.10、feature 和隔离检查继续通过，无 egui/gpui-fast。JoyOffice 基线 `a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d` 保持 clean；DeckCraft 755 文件指纹无变化，见 [上游只读检查](upstream-read-only.json)。

## 自动验证

[完整检查日志](logs/accepted-product-check.txt)：854 项 Rust 测试、5 项 Python 测试通过；fmt、全产品严格 Clippy、依赖/CLI 隔离、214 项引入源码记录和文档检查通过。针对渐变误判、字体缓存修订、缓存复用、像素一致性、命令原子性/历史和图片预算有回归。

最终 ZIP 包内 CLI 在仓库外执行，合计 **63 次调用**，见 [运行日志](logs/accepted-cli.txt)：

- [R4/R5 23 次](accepted-r4-r5.json)：公开/原生/再导出、母版/主题/已有版式/尺寸/元数据、形状/渐变、图片插入替换、图表标题/类型/数值、独立 python-pptx 读回及 XLSX B3、原 ZIP 未修改部件逐字节保留、无效命令不发布文件、强杀后恢复、未知图表扩展拒绝公开保存且原生保留。
- [R3 20 次](accepted-r3.json)：富文本、表格、备注、未知单元格 XML、结构拒绝及强杀恢复回归。
- [R2 20 次](accepted-r2.json)：基础编辑、保存/重开、undo/redo、原件保留、无效输入、损坏恢复记录隔离和强杀恢复。原脚本只在执行内存副本中将诊断阶段断言从 R2 改为 R4-R5，其他断言不变，未改旧脚本。

独立原创语料为 3 页公开 OOXML，含真实母版图形、主题、多语文字、透明图片、两系列四分类图表/工作簿、旋转渐变、组合和合并表格；原始文件 SHA-256 为 `3908ab2ead42451118627853ee5a245fe9591cd57bda14e1ebe2c808de544ffe`。语料生成器和验证器分别为 `scripts/joydeck/generate-r4-r5-pptx.py` / `verify-r4-r5.py`。

## 实际桌面与公开读回

通过 Computer Use 操作实际 release 程序，最终包证据集中在 `gui/accepted`：

| 验收 | 结果与证据 |
| --- | --- |
| GPUI 默认/CPU 对照 | [工作区](gui/accepted/01-workspace.png)、[渐变 GPUI](gui/accepted/04-gradient30-gpui.png)、[CPU 对照](gui/accepted/05-gradient30-cpu.png)。复杂页的渐变、组合、合并表格及中文可见；属性正确显示当前 CPU 模式或局部回退原因。 |
| 渐变局部失效 | 将 #919 椭圆角度从 45° 改为 30°，整页仍为 GPUI；缓存 hits +7、misses +1、CPU tiles +1，背景 raster 不增加，见 [前后计数](gui/accepted/gradient-cache.json)。 |
| 图表和保存 | [数据表单](gui/accepted/06-chart-data-form.png) 将第一系列第二分类 45 改为 55，[重绘](gui/accepted/07-chart55-applied.png)，另存 [公开 PPTX](gui/accepted/desktop-final.pptx) 和 [原生文稿](gui/accepted/desktop-final.joydeck)。 |
| 独立读回 | [python-pptx + XLSX 检查](gui/accepted/independent-readback.json)：图表和 B3 均为 55、标题为 R5 桌面图表；图片左裁剪 0.12、主题字体 Arial、#919 为 ellipse 且渐变 30°。原生重读 slides/masters/slideSize/props/media 与公开读取模型一致，见 [原生读回](gui/accepted/native-readback.json)。 |
| 实际重开 | [公开重开](gui/accepted/08-public-reopened.png)、[原生重开](gui/accepted/09-native-reopened.png)；记录各次读取/保存及场景重建于 [桌面日志](gui/accepted/desktop.txt)。 |
| 主题/窄窗口 | [深色](gui/accepted/10-dark-workspace.png)；[800×600 标题栏](gui/accepted/11-narrow-header.png) 所有右侧按钮可见；[图片长表单](gui/accepted/12-narrow-picture-form.png) 可滚到 [底部](gui/accepted/13-narrow-picture-bottom.png)，应用按钮可达；[诊断](gui/accepted/14-narrow-diagnostics.png) 基准按钮可见。 |

本轮更早候选包还完成了真实母版背景 E8EFF8、强调色 2678D0、主题撤销重做、Arial 字体、图片裁剪/透明度/亮度、折线图标题/数值、原生路径拖动及撤销重做，记录在 `gui/01–16` 与 `gui/final`。最终包重开这些编辑结果并继续编辑/独立读回。`gui/final` 和名称带 final 的早期 CLI 日志属于 **6ee2171d73 等候选版本**，不是本次交付包的验收依据；本次仅以 accepted 和 package.json 的源码指纹为准。

## 像素与实际性能

阈值预先固定，未因失败放宽。3 页 CPU / 中立场景参考图以 960×540、scale 1 比较，通道误差 >12 的比例上限 1%；最终最大误差分别为 1、2、3，超限通道均为 0。每页运行 100 次缓存平移对照，字体字节 SHA-256 和 face 随报告保存，见 [headless 汇总](accepted-headless-summary.json)。这是 vello_cpu 场景参考，不能当作 GPUI GPU 绘制证据。

实际桌面同窗口画布区域 `(257,300,985,709)` 的 RGB 对照，固定“通道差 >32 比例 ≤2%、MAE ≤4”；最终含新渐变的复杂页比例 **0.0891%**、MAE **0.5243**，见 [像素报告](gui/accepted/pixel-comparison-gradient.json)。候选包另有普通页、图片/图表页对照，均通过既定阈值。这些是本轮语料结果，不能代表全部 PPTX。

[实际桌面对照原始 120 帧与汇总](gui/accepted/benchmark.json)：同一真实 GPUI 窗口、同一对象交替 ±1 pt，GPUI/CPU 各 60 帧；不修改编辑历史或文稿。Intel Core Ultra 9 285H / Windows 11 26300 / 32 GB，实际 GPUI 选择 **Intel Arc 140T，驱动 32.0.101.9150，非软件模拟**；DPI 2、物理画布 1456×819、zoom 0.7583，见 [硬件](hardware.json) 和运行记录。

| 采样指标（毫秒） | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| GPUI 画布 CPU 提交 | 0.082 | 0.157 | 0.233 |
| CPU 整页重绘与准备 | 11.519 | 13.638 | 14.120 |
| CPU 图像画布提交 | 0.707 | 0.978 | 1.644 |
| GPUI 样本就绪至画布回调 | 6.008 | 17.487 | 18.530 |
| CPU 样本就绪至画布回调 | 10.647 | 14.337 | 18.257 |

GPUI 热平移无需重新准备几何或纹理，此项 prepare_us 记为 0，60 帧新增 tessellation / texture preparation 均为 0；该计数不等于驱动上传字节或 GPU 时间。20 ms 定时器将采样频率限制为最高 50 Hz。以上环节不同，**不能用 CPU 整页准备除以 GPUI 提交宣称整体加速倍数**；尚未测量真实呈现、GPU timestamp、输入到光子延迟或 FPS，也未达成全平台性能资格化。[进程内存快照](gui/accepted/memory.json) 工作集约 306.4 MiB，仅为内容编辑/重开后的单点，不是峰值或泄漏压力测试。

## 修复与待推进范围

本轮实际对照发现并修复了多语字体懒加载造成的场景缓存失效遗漏、普通渐变被误判成背景依赖而整页 CPU 回退、窄窗口右侧标题按钮裁切、长表单/诊断按钮不可达和基准结束状态未主动刷新。新增针对性回归并重新构建、重验上述最终 ZIP。失败或被替代的构建日志保留，不计为验收通过。

完整 R4/R5 仍为进行中：原生组合/图片管线、全部路径/描边/字体效果、完整字体热更新、母版对象/占位符完整编辑、高级图表和公式/样式工作簿、SmartArt 编辑、排序/节/跨文稿与审阅、全部 329 命令界面和 PDF 三后端对照仍待后续切片。动态媒体/动画仍属后续阶段。本轮没有 Linux/macOS 真机验证，没有大型文稿、长期内存或真实多屏/DPI 切换资格化。任务表 98 个 ID 和命令表 329 个 ID 均保留，局部实现不冒充整项完成。
