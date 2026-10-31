# M1 基础只读浏览实施记录

日期：2026-10-29（Asia/Shanghai）。Windows x64 的本轮切片已完成；完整 M1 仍进行中。这里只声明本机、当前格式子集和自建样例的结果。

## 文档与实现基线

01 改为净室规范；02–14 的产品、功能、架构、渲染、格式、路线、任务、验收与追踪统一采用独立实现；15 保留公开标准/通用依赖与项目证据；16 由 FR 推导本项目动作。需求和父任务编号保留。新增 18 明确 M1 的原生 schema、PPTX 子集、预算与可观察行为。

本轮仅读取本项目材料、通用依赖官方 API/本机依赖包和公开 OOXML 资料；模型、容器/关系解析、预览转换、工作区及样例均独立编写。材料边界与既有资料知识隔离的限制见 01，不以清理引文追溯性证明人员隔离。

## 已交付实现

| 模块 | 能力 |
|---|---|
| document.rs | 独立模型、`.joydeck` v1 ZIP+JSON、内容嗅探、预算、数值/ID/版本校验 |
| pptx.rs | OPC 根关系定位主文档，按 presentation 顺序定位页，Transitional 子集读取，复杂内容报告/占位 |
| preview.rs | 从验证后的模型生成受控 SVG，后台 resvg CPU 栅格化、系统字体复用、BGRA 输出；最长边限制 2048 px |
| viewer.rs | 选择/路径/CLI 打开、加载取消、虚拟页列表、32 张缩略图缓存、页导航、fit、25–400% 缩放、滚动画布、兼容弹窗 |
| scripts / xtask | 独立样例生成器、实际窗口回归、截图、stage M1、M1 包；保留诊断入口 |

读取先验证后发布。窗口拥有取消 token；读取请求序号和不可变文档代次隔离迟到结果，预览键包含文档代次/页/像素宽。翻页取消旧渲染；回到缓存页面时同步当前请求状态。失败的缩略图不在每帧无限重试。

无编辑、保存或源文件写入入口。原生/PPTX 在桌面回归前后的 SHA256 相同。此 CPU 图像桥不代表 M4 文稿 GPU 加速完成。

## 分步编译与实际启动

| 切片 | 结果 | 证据 |
|---|---|---|
| A 原生读取 | build 成功；M0 原生窗口创建成功。仅记录启动，不把隐藏窗口未执行的 adapter 序列算作通过 | [启动](evidence/m1/step-a-startup.log) |
| B PPTX/CPU 预览 | build 和读取/像素测试通过；可见 M0 窗口的输入、弹窗、双窗口与任务 smoke 通过 | [回归](evidence/m1/step-b-startup.log) |
| C 只读工作区 | build、check、stage M1 通过；真实原生/PPTX 浏览、导航/缩放/取消/失败行为通过 | [检查](evidence/m1/check-m1.txt)、[阶段](evidence/m1/stage-m1.txt)、[桌面 smoke](evidence/m1/desktop-smoke.log) |
| 分发包 | 从 dist 的 EXE 重新运行同一桌面回归通过；另以包内 PPTX 启动并保持响应 | [最终窗口](evidence/m1/packaged-viewer.png)、[M0 回归](evidence/m1/m0-regression.log) |

原生窗口通过 GPUI Action 分发与生产方法验证：两份 20 页文稿、前后/首尾页、快速离开再返回、缩放上下限、resize 后 fit、立即取消和连续打开、报告弹窗、损坏 ZIP 后保留当前文稿。此脚本不声称模拟了物理鼠标、键盘或 OS 文件选择器操作。

已检查截图：[原生](evidence/m1/native-window.png)、[PPTX 第二页与占位](evidence/m1/pptx-window.png)、[最终包首屏](evidence/m1/packaged-viewer.png)。

check 的 fmt、Clippy `-D warnings`、10 项测试通过（含 M0 两项）。覆盖 ZIP 路径/重复成员/扩展预算、截断包、版本/必需特性、重复 ID、几何、关系/页面顺序、缺少 part、外部主文档、DTD/XML 深度和 BGRA 像素。

## 本机初值与限制

Windows x64 / Rust 1.98.1 / Kit 0.8.1 / GPUI pre 0.3.10，debug 构建，与 M0 同机系统字体环境。分发包 smoke 单次约 1000 ms，结束 RSS 254.1 MiB。单独启动包内 20 页 PPTX：解析 4 ms、首张 1561 px 宽 CPU 预览 88 ms，启动后 RSS 181.1 MiB。来自 logs/m1.stderr.log、logs/packaged.stderr.log 和进程采样；不是 release 门槛、端到端首屏时间或 p95/p99。

最大预览 2048×2048，单张 BGRA 最多 16 MiB；缩略图最多 32 张。当前只有 20 页自建语料，LARGE、GPU 上传耗时/计数、长会话与正式 RSS 基线待补。

可显示直接坐标、实色矩形/椭圆/线和换行纯文本。图片、组、母版/版式/主题、复杂文字、旋转/效果、表格/图表未完整显示；未知对象有页级提示。Strict、宏、加密格式拒绝。指定字体和自动换行未保证，报告目前到页/部件，稳定对象级定位未实现。能力为 C0 只读预览。

T-M1-01、05 完成；02a、03a、04a、06a、07a、08a 完成，父项其余条件未勾选。继续按 11 补逐字体/对象诊断、细粒度进度、正式文档生命周期、LARGE/上传基线、完整 AT-02/15 与授权外部语料。macOS/Linux、物理键盘/IME、多显示器/DPI仍未验收；M0 状态独立保留。

## 产物与回退

- `dist/joydeck-m1-windows-x64/joydeck.exe`、同名 ZIP，含 samples、许可证、Cargo.lock、SHA256。
- 运行：`pwsh.exe -File scripts/dev.ps1 run`；带路径：`cargo run --locked -p joydeck -- tests/fixtures/native-20.joydeck`。
- M0：`cargo run --locked -p joydeck -- --diagnostics`；保留 dist/joydeck-m0-windows-x64/。其 EXE SHA256 仍为 `FC25DF5410346121140C8AE3A1D00D1710CE34542696584DB004E28031BEF2AF`。
- M1 EXE SHA256：`0C92C2D6D2B4CDC4FADF8740AC0B0C2EC2D7BAE3DC74638C7644C034D498CCF7`。
- 当前无 Git 元数据，`dist/joydeck-m1-source.zip` 提供源码/规格恢复快照，不称为 Git 提交。

M0 分发说明同步清理文档基线，EXE 保持不变。没有执行远端 CI、正式签名或安装器发布。
