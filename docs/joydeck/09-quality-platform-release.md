# 质量性能平台与发布规格

所有数字为待实测的目标预算，非本次测量。M0 记录参考机，M1 建立基线，M4 验证加速，M13 作发布决策。

## 1 平台矩阵

| 平台 | 首期支持目标 | 开发/CI | 真机运行要求 |
|---|---|---|---|
| Windows | Windows 11 x86_64 为主要验收；Windows 10 兼容仅在专项测试后声明 | MSVC Rust、VS 2022 C++ 工具/SDK、CMake、PowerShell 7 | Intel/AMD 集显至少一种，另补独显；125%/150% DPI、中文 IME、剪贴板、双屏 |
| macOS | macOS 15+，Apple Silicon；Intel 作为次级目标单独验证 | Xcode/Command Line Tools、对应原生 target | 字体/IME、原生菜单、文件打开事件、Retina、双屏、签名/公证包 |
| Linux | Ubuntu 24.04 x86_64，Wayland 和 X11 | 原生 Linux CI、fontconfig/xkbcommon/Wayland/X11 等实际依赖 | 两种会话分别启动，中文 IME、剪贴板、Vulkan 驱动、字体和打印 |
| 后续 | Windows ARM64、Linux ARM64、其他发行版、FreeBSD/Web | 按需求新增 | 不由 x64 编译结果推断通过 |

Kit 官方安装说明提供 Windows MSVC/CMake、macOS 15+ 及 Ubuntu 24.04 的起始条件。实际依赖由锁定 feature 图确定；没有用 WebView/speech 不应无理由引入其全部运行时。Linux Vulkan loader 存在不代表有可工作的 GPU driver。[官方安装条件](https://gpui-kit.com/docs/installation/)

工程固定 Rust 1.98.1、Kit 0.8.1 与匹配 GPUI pre 0.3.10；升级必须重新记录构建结果和完整依赖图。

M0 三平台编译并实际弹出诊断窗口为目标；若当时缺远端硬件，Windows 可继续推进，但 macOS/Linux 状态须标为未验证。M7 前三平台基础编辑实际冒烟是 MVP 的跨平台退出条件，M13 必须覆盖完整正式支持矩阵。编译成功与实际运行分别记录。

## 2 基准集

未来测试资源全部有来源与授权记录；本次只定义规格。

| ID | 构成 | 关注 |
|---|---|---|
| BENCH-SMALL | 20 页，每页约 30 对象；中英文字、2 张普通图片；文件约 10 MiB | 冷/热启动、打开首屏、保存与日常内存 |
| BENCH-EDIT | 1 页 200 对象：120 基础几何、60 文本块（共约 8,000 字形）、20 张 512² 图片；无复杂滤镜 | 拖动/缩放、命令和 scene/texture 计数、GPU 覆盖 |
| BENCH-LARGE | 500 页，每页约 100 对象，复用母版和部分资源，累计约 50,000 对象 | 首屏与总加载、虚拟列表、缓存/历史内存、取消 |
| BENCH-EFFECT | 1 页约 30 个阴影/渐变/透明组/裁剪组合 | fallback 范围、画质、filter 内存、最坏帧 |
| BENCH-SHOW | 30 页，每页 20 个对象及 5 个基础动画，含 Morph/转场和隐藏页 | 时间采样、帧间隔、双屏/暂停/恢复 |
| BENCH-MEDIA | H.264 MP4 1080p30 + 音频，以及 10 分钟独立音轨 | 解码、A/V drift、seek、设备更换、队列预算 |

M0 指定参考硬件建议为 4 核以上 CPU、16 GiB RAM、主流集显、NVMe/SSD、1920×1080 60Hz 屏幕，记录精确型号、OS、驱动和字体。各平台都保存自己的基准结果，不把一台高配机器的结果推广到所有设备。

## 3 性能预算

| 指标 | MVP/M4 起目标 | Beta 发布目标 |
|---|---|---|
| BENCH-SMALL 冷启动到可操作空窗口 | p95 ≤ 2 s | p95 ≤ 1.5 s |
| SMALL 打开到当前页可用 | p95 ≤ 3 s | p95 ≤ 2 s |
| LARGE 打开到首屏 / 完整索引 | p95 ≤ 5 s / 20 s | p95 ≤ 4 s / 15 s |
| EDIT 连续拖动/平移 frame interval | p95 ≤ 20 ms，或按 M4 相对门槛先达成 | p95 ≤ 16.7 ms，p99 ≤ 33.3 ms |
| 输入到可见反馈 | p95 ≤ 80 ms | p95 ≤ 50 ms |
| 可见缓存页翻页反馈 | p95 ≤ 100 ms | p95 ≤ 80 ms |
| 主线程独立工作块 | 常态 ≤ 8 ms，不出现 >100 ms 长阻塞 | 常态 ≤ 4 ms，>50 ms 必须有诊断和修复/范围裁决 |
| 保存 SMALL | p95 ≤ 2 s，操作不阻塞 UI | p95 ≤ 1.5 s |
| 长任务取消 | 500 ms 内显示取消中 | ≤1 s 到安全检查点释放工作，IO 提交已发生时如实报告 |
| 静态空闲 | 不持续重画整页；无无限 worker 重试 | 空闲 CPU 平均 ≤ 1% 一逻辑核等效；按系统噪声解释 |
| 媒体 | 不阻塞交互，支持样本实时播放 | 1080p30 指定样本过期/丢帧率 <2%，10 分钟 A/V drift 绝对值 ≤80 ms |

M4 的相对通过门槛用于迁移阶段；M13 若仍达不到绝对预算，需修复或正式缩小所支持的场景/硬件范围，不能偷偷降低 benchmark 对象数量。

帧间隔、应用 CPU 工作时间和输入到显示延迟分别测量。软件 timestamp 到提交不等于真实光子延迟；若未有 present/外部采样，只标“输入到提交”，不冒充精确屏幕可见延迟。

### 内存与资源

| 类别 | 初始默认预算 | 验收 |
|---|---|---|
| 历史 | 500 步或额外估算 256 MiB | 按完整事务淘汰，redo/恢复引用合法 |
| CPU 派生缓存 | 256 MiB，全局可动态收缩 | layout/raster/decoded image 分项可观察 |
| GPU texture cache | 256 MiB 或设备更小安全预算 | 视口/缩略图/视频独立优先级，device lost 可回收 |
| 媒体工作队列 | 单流 64 MiB 默认，总计受全局预算约束 | 不累积整段解码视频 |
| SMALL 私有内存/RSS | 目标 ≤450 MiB | 含字体/框架与已用缓存，记录 OS 差异，不与 GPU 显存混记 |
| LARGE 私有内存/RSS | 目标 ≤1.5 GiB | 不包含所有页全分辨率预渲染；超预算预警/降级 |

缓存和历史上限是最大允许值，不代表每个文档都预分配满额。总内存压力优先回收可重建缓存，不能保留全部子预算而突破全局目标。连续打开/关闭 20 次并稳定空闲后，资源应回落接近基线，增量目标不超过 10% 或 50 MiB 中较大者，结合分配器行为复核。

## 4 基准方法

Release 构建、固定 lock、固定字体和数据、关闭日志高频输出，记录 backend/features 与显示刷新率。冷启动至少 10 次、热交互至少 30 轮，报告 median/p95/p99、样本数、最坏值；绘制轨迹固定并含 warmup。

同一硬件比较原 CPU baseline 与新后端；区分冷字体/缓存和命中结果。每次性能修改只扩大与该修改相关测试，不反复无意义跑全套。M4 起持续记录 frame、layout、raster、upload、cache hit、queue 和 bytes，防止 UI FPS 掩盖后台拥塞。

## 5 测试分层

| 层 | 测试内容 | 起始阶段 |
|---|---|---|
| 领域单测/属性测试 | ID、引用、矩阵、选择、继承、事务、Unicode 映射、history | M1–M3 |
| 文件与语料 | native/PPTX 结构/语义/未知 parts 往返，损坏和资源预算 | M1；M7 强制 |
| 场景/渲染 | glyph 坐标、Scene、CPU golden、GPU 容差、PDF 结构和文本提取 | M1；M4/M12 扩展 |
| GPUI 集成 | 真正生产视图的点击、键盘、焦点、状态、可访问树 | M0 起关键路径 |
| OS 手工/自动冒烟 | IME、系统剪贴板、文件事件、DPI、双屏、打印、音频设备 | 每平台功能出现时 |
| 稳定性 | 随机命令+undo/save/reopen、UI soak、内存/时长预算 | M2 起；M13 长跑 |
| 发布包 | 干净环境安装、首次启动、文件关联、升级与卸载 | M0 可运行包，M13 正式门禁 |

不为文档静态措辞或低影响视觉调整编写镜像实现的测试；数据安全、格式、输入、异步竞争和平台行为必须有针对性回归。测试失败修复后保留复现样例/seed。

## 6 CI 与阶段构建

M0 提供 future `cargo xtask check` 统一入口：格式、clippy、适当测试、依赖方向/许可基础检查。通过 `cargo build --locked -p joydeck` 证明实际链接，不能只 cargo check。Release 性能验证使用 `cargo build --release --locked -p joydeck`。

开发期每个受影响平台构建；主干三平台 native build/test，Linux 核心测试可在无显示器环境运行。GPUI headless 测试不代替真实 GPU/OS 集成；GPU smoke 可在受控 runner 或手工矩阵记录。三平台执行结果分别记录。

自定义命令在 M0 创建，后续阶段新增 `xtask stage Mx`、`xtask bench`、`xtask corpus`、`xtask package` 时同步文档。当前没有这些实现，文档命令不是本次通过的检查。

## 7 可靠性与输入安全

领域层不使用生产 panic/unwrap/expect/todo 等处理外部输入，checked arithmetic、get 和有界递归作为基础。panic guard 是最后防线；启动驱动崩溃不能由它兜住。OOM/abort/驱动访问违规也不由 Result 自动解决。

按 07 处理 ZIP/XML/图片/媒体预算，限制控制请求。宏/脚本和外部关系不自动执行；不把本地文档自动发送网络。外部 URL 打开和可下载资源有明确用户动作。

关键回归：dirty close、旧 save 完成覆盖新 revision、重复远程请求、设备断连、恢复损坏、失焦 IME、超限图像和未知 PPTX 关系。

## 8 打包与发布

| 平台 | 阶段测试产物 | 正式产物及验证 |
|---|---|---|
| Windows | 可直接解压运行 ZIP/EXE | 可安装 EXE 或 MSI 选一种；签名、升级、卸载、文件关联、运行库验证 |
| macOS | `.app` 包，允许明确标记未签名开发版 | Developer ID 签名、公证、staple 的 DMG/ZIP；干净机验证 |
| Linux | 带必要运行资源的 tar.gz | 首选 deb + tar.gz；AppImage 按依赖验证再加；X11/Wayland 均实测 |

产物附版本、阶段、commit、target、lock digest、features、已知限制和 SHA256。测试实际分发目录里的文件，不能只跑 target/release 中的二进制。原生构建分架构，跨编译成功不等于运行成功。[Kit 打包指导](https://gpui-kit.com/docs/packaging/)

更新器保留最后可运行版本，验证签名/来源，更新失败可回退；更新平台适配需独立实现并验证。M13 之前可以手动下载安装包，不能用未验证自动更新阻塞基础创作。

## 9 来源与许可

应用实现按净室规范独立编写；通用依赖逐项记录许可证、版本和来源，保留必要 notice；完整传递依赖审核不能用直接依赖清单代替。

资源后续另立清单：来源 URL、版本/hash、作者、license、用途和嵌入权限。先验证 Kit 默认 assets 和字体是否满足本项目选择的政策，再决定启用；资源变更必须随实施记录更新。公开规范与功能观察用于兼容设计，微软产品截图、模板和字体不作为新应用内置资源。

## 10 发布阻断条件

任何受支持常规路径出现静默数据丢失、无法恢复的崩溃、C2 语料关系/文本丢失、关闭无提示、未提交保存误标成功、未知内容无告警删除、主平台启动失败、IME 数据破坏或签名/安装失败，均阻断对应版本发布。

可延期的长尾功能必须从能力声明和可点击入口中清楚移除，保留导入告警；不能只把失败测试标忽略然后宣称完整 Beta。


## M9/M10 本轮实测与开放门禁（2026-10-30）

本机macOS ARM64：102项测试、fmt/all-targets Clippy -D warnings、stage M9/M10、release链接；最终包M0–M10原生adapter/画布与输出验证、普通文字/表格系统拼音，证据统一见 [最终验证索引](evidence/m10/final-verification.json)。图表缓存/XLSX由独立python-pptx读取；PowerPoint/Excel实际改为133、负例缓存91/工作簿132及Office制作500ms淡化进入固定fixture。未知闭包、预算、原子失败、undo、静态/动画快照与焦点/时钟移交都有回归。

BENCH-SHOW固定30页×20对象/5效果、600个1024宽CPU独立帧，记录p50/p95/max；另记录原生图表帧渲染准备耗时。数值见32；离线CPU/提交准备不等于显示帧率。当前单有效显示器，真实双屏/DPI/镜像/热插拔/睡眠、全平台、完整AT-12/性能矩阵及正式签名/干净安装仍开放。M7全屏自动截图对macOS过渡期旧backing store增加5秒有界首帧等待，原像素阈值保持不变，所有尝试误差留存。

## 2026-10-31 本机资格化增量

122项Rust/41项headless检查、128份原创变体独立公开读取、600秒370081次seeded命令与740检查点、M0–M14本机GUI/拼音回归、自包含App移位运行及离线升级回退有记录；Windows/Linux核心仅跨目标check/Clippy。命令p95/p99不是显示延迟，RSS采样缺启动段，原创语料不是100份独立Office文件；完整Beta/AT-20与性能/平台门槛不改。详见[36](36-m13-implementation-report.md)、[37](37-m14-implementation-report.md)和[索引](evidence/m14/final-verification.json)。
