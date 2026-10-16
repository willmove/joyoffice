# GPUI Kit / GPUI Fast 选型调研与实测报告（JoyWord）

- 日期：2026-10-16
- 性质：调研 + 隔离实测。调研与实测期间未修改仓库；实验工程在仓库外的临时目录构建运行，源码、清单、锁文件、脚本与原始数据随后复制到 [GPUI-KIT-FAST-EVALUATION/](GPUI-KIT-FAST-EVALUATION/)（构建产物未提交）。本报告不认领任务卡，不改 ADR、TASKS 或公共台账。
- 仓库基线：`main` @ `0dcc0f0e4f0efa5d7845b697ed7d60bcc2136ff2`（实测期间仓库被其他会话从 `codex/integrate-cleanup-20261016` 切到 `main` 并提交；本报告只读引用）。
- 主机：Apple M4（10 核）/ 24 GB / macOS 27.0.1（26A434）/ 仅 CommandLineTools（无 Metal 编译器，所有变体统一启用 `runtime_shaders`）/ Rust 1.98.1。
- 调研对象（均为 crates.io 当前最新版，精确锁定）：
  - **GPUI Kit** = `longbridge/gpui-kit`，`gpui-kit 0.7.3`（含 `gpui-base 0.7.3`、`gpui-component 0.7.3`、`gpui-kit-assets 0.7.3`）。
  - **GPUI Fast** = `longbridge/gpui-fast`，`gpui-fast 0.1.8` + `gpui-fast-platform 0.1.8`。
  - 对照：现用 `gpui 0.2.2`；GPUI Kit 默认底座 `gpui-pre 0.3.9` + `gpui-pre-platform 0.3.9`（Zed 主干快照）。

---

## 1. 结论摘要

1. **没有必要“基于 GPUI Kit + GPUI Fast”开发 JoyWord。** 两者都不是 JoyWord 的前置条件；JoyWord 的核心（文档模型、排版、PageScene、画布、IME、选择）无论选哪个都必须自研。
2. **真正值得做的是 ADR-010 的来源切换：从 crates.io `gpui 0.2.2` 迁到 gpui-pre 线（Zed 快照）。** 它一次性补齐 0.2.2 的三个缺口（外部 `GlyphId`、AccessKit、Linux wgpu 含 GL 后端），每帧 CPU 开销实测降到约 1/2.7，且 **JoyWord 现有应用迁移只需 11 行改动**（6 个编译错误），应用自带的 11 个测试全部通过。
3. **GPUI Fast：性能收益真实且巨大，但现在不宜作为正式来源，建议作为“API 兼容的可选加速后端”保留。** 同一 JoyWord 移植代码在 gpui-fast 上零额外修改即编译通过；同一界面每帧 CPU 比 gpui-pre 再降 3.3–24 倍。但它仓库创建仅 3 周、9 天发了 9 个版本、README 自称 experimental，近两周仍在修复 Retained Mode 的正确性问题（陈旧渲染、布局 panic、重挂子树丢失）。另外，JoyWord 的画布在滚动内容里注册 IME 输入处理器，会让其滚动图层失效（实测 p95 变为 4.6 倍，但仍远优于 gpui-pre）。
4. **GPUI Kit：不建议作为应用框架整体采用。** 它适合快速搭建通用桌面外壳，但对 JoyWord 的代价偏高：依赖包 +37%（macOS 303→415）、二进制 7.1→16.1 MB、构建 +62%、同样界面每帧 CPU +136%（1.60 vs 0.68 ms），引入 0BSD 许可（需改 deny 策略）、183 个组件快捷键与独立 i18n（与 JoyWord 命令注册表/本地化双轨），并且 **gpui-kit 门面强制开启 `runtime_shaders`**，与项目“正式构建必须验证不启用 runtime shaders”的规则冲突；开启其 `gpui-fast` 特性时还会**同时编译 gpui-pre 与 gpui-fast 两套核心**。如外壳组件需求增长，可在 M2 以后按需评估**直接依赖 `gpui-component`**（绕开门面的强制特性），或自建小组件集。
5. **供应链需特别注意：** crates.io 上 `gpui-pre*`、`gpui-fast*`、`gpui-kit`、`gpui-component` **全部只由同一个人（huacnlee）发布**；`gpui-pre`/`gpui-fast` 发布包缺少 `.cargo_vcs_info.json`。只有已停更的 `gpui 0.2.2` 归 Zed 团队所有。我抽查了 gpui-pre 的 12 个核心文件（约 3.1 万行），与 Zed 提交 `93d745668805029476e6d7fbb19652116fed51de` **逐字节一致**；若采用，应把“快照与 Zed 提交逐文件比对”做成 CI 门槛，或改用 git 固定 Zed 提交。

**建议提交协调者在 ADR-010 中裁决**：长期来源选 gpui-pre 线（或同代码的 git 固定 Zed 提交），精确锁定 + 快照溯源校验；gpui-fast 以 CI 编译矩阵形式保留切换通道，满足第 8 节触发条件后再切；GPUI Kit 暂不引入。

---

## 2. JoyWord 现状与硬约束

- UI 代码：`apps/joyword/src` 共 2,351 行（`main.rs` 694、`input_ui.rs` 1,182、`document_canvas.rs` 475）。页面由 joyword-render 在后台 CPU 光栅，再以 GPUI `img()` 呈现；`input_ui.rs` 在画布绘制阶段注册 `ElementInputHandler`（IME）。菜单/快捷键/按钮统一走 joyword-shell 命令注册表（T106）。
- 与本选型相关的硬约束（来自 AGENTS.md、ARCHITECTURE.md、ADR-009/010、deny.toml、README）：

| # | 约束 | 来源 |
|---|---|---|
| C1 | 外部整形结果可直接绘制：需要可构造的 `GlyphId` 与公开 `paint_glyph` | ADR-005/010、W02 |
| C2 | Linux 必须有 GL 或 CPU 渲染退路（信创国产 GPU 多为 OpenGL） | ADR-010、ARCHITECTURE §7 |
| C3 | AccessKit / 可访问树 | ADR-010 |
| C4 | 工具链 1.98.1、依赖精确锁定、不用浮动 git main | ADR-009 |
| C5 | 第三方发布需供应链审查；cargo-deny 许可/公告/来源策略 | ADR-010、deny.toml |
| C6 | 正式构建必须验证**不启用** runtime shaders（CI 有预编译 Metal 构建） | README |
| C7 | 自有代码 `unsafe_code = forbid` | AGENTS.md |
| C8 | IME、命令注册表单一入口、zh-CN/en 本地化 | SPEC/T106 |

---

## 3. 调研对象概况

### 3.1 GPUI Fast（longbridge/gpui-fast）

- 定位：Zed GPUI 的性能分叉，“incremental rendering + native window composition”，README 明言 **experimental**。
- 核心机制（源码 `src/fast/`）：
  - **Retained Mode**：每个 `Entity<V: Render>` 都是可复用子树；依赖（读过的实体、全局、ScrollHandle、ListState、几何/样式）不变则直接复制上一帧的 hitbox/监听器/图元，跳过 render/layout/prepaint/paint。`GPUI_VIEW_RETENTION=0` 可关闭。`window.refresh()`、拖拽、首帧开 a11y 时不复用。
  - **Layout 节点与文本测量复用**：Taffy 树跨帧保留（Taffy 0.14），相同文本/样式复用测量。
  - **Scroll Layers**：滚动容器内容光栅为 GPU 瓦片，仅偏移变化时合成。`GPUI_SCROLL_LAYERS=0` 关闭。**内容中有文本输入处理器、锚定/延迟元素或近期请求动画帧时绕过**（`fast/layers/policy.rs::eligible`）。
  - Window composition（可选）：嵌入 WKWebView/WebView2/WebKitGTK 并让 GPUI 菜单浮于其上。
- 代码规模：核心 127,473 行（gpui-pre 核心 99,690 行），`fast/` 模块 38,821 行；对上游文件的“钩子”改动 26 个文件 / 1,611 行。核心加平台 crate 合计比 gpui-pre 净增约 4.5 万行。
- 成熟度：仓库 2026-09-24 创建；crates.io 0.1.0（10-06）→ 0.1.8（10-14）；约 201 次提交（含上游同步）；239 星；主要作者 sunli829、huacnlee。近两周合并的修复包括 #46“retained view splices 的布局 panic 与陈旧渲染”、#52“元素路径变化后陈旧的布局缓存”、#68“恢复被重挂的 retained 子节点”。CPU 自适应渲染（#45）已关闭未合并，**0.1.8 不含 CPU 渲染器**。
- 许可：Apache-2.0（Zed 代码的修改版）。

### 3.2 GPUI Kit（longbridge/gpui-kit，原 gpui-component）

- 定位：基于 GPUI 的桌面应用框架，75+ 组件（按钮、表单、对话框、菜单/原生菜单、Dock、Sidebar、Tab、Toolbar、TitleBar、Select/Combobox、ColorPicker、Table、List、Notification、Setting、Markdown/HTML、图表、代码编辑器等），AccessKit、Web(wasm) 支持，为 Longbridge Pro 商业产品打磨。
- 结构：`gpui-kit`（门面，`pub use gpui::*` 或 `gpui_fast::*`）→ `gpui-base`（无样式行为层，116,135 行）+ `gpui-component`（样式组件，105,142 行）+ `gpui-kit-assets`（1,830 个 Lucide SVG，7.4 MB）。
- 底座：默认 `gpui-pre =0.3.9`；`gpui-fast` 特性切到 gpui-fast，但 **`gpui-pre` 与 `gpui-pre-platform` 在 Cargo.toml 中是非可选依赖**，所以两套核心都会编译。
- 门面对平台 crate 固定开启 `font-kit, x11, wayland, runtime_shaders`（Cargo 特性只能叠加，下游无法关闭）。`gpui-component` 本身不依赖平台 crate。
- 文本输入：`gpui-base/src/input` 基于 ropey 的输入框/代码编辑器（tree-sitter、LSP），实现了 `EntityInputHandler`；**不是分页富文本文档模型**，不能替代 JoyWord 画布。
- 成熟度：2024-06 起，约 2,598 次提交、1.76 万星；提交高度集中（huacnlee 1,488 次）。0.6.2→0.7.3 约 3.5 周 8 个版本，0.7.0 含大量破坏性 API 变更（Plot、Root、文本选择）。
- 其他：组件源码中约 183 处 `KeyBinding::new`（按上下文绑定，`init` 时注册）；rust-i18n 内置 en/zh-CN/zh-HK/zh-TW。

### 3.3 来源关系

```
Zed 主干 ──快照──> gpui-pre 0.3.x（huacnlee 发布，每周）──> gpui-kit 默认底座
   │                      └──分叉 + fast/ 模块──> gpui-fast 0.1.x ──> gpui-kit 可选底座
   └──（2025-10 停更）gpui 0.2.2（Zed 团队发布）──> JoyWord 现用
```

---

## 4. 实测

### 4.1 方法

- **隔离工程**：同一份约 800 行基准源码（[bench/src/main.rs](GPUI-KIT-FAST-EVALUATION/bench/src/main.rs)）编译成 5 个变体，各自独立 workspace、`Cargo.lock`、target 目录；release 配置与仓库一致（thin LTO、codegen-units=1、strip debuginfo）。变体间仅以 cfg 区分 4 处 API 差异（启动入口、`inactive_frame_interval`、`window.focus` 签名、`ArenaClearNeeded::clear` 签名）。
- **基准界面（模拟 JoyWord 窗口）**：8 个功能区页签 + 2×24 个工具按钮、80 项大纲侧栏、8 页文档（每页 40 行中英混排，**用 `paint_glyph` 按预整形字形绘制**，即 W02/ADR-010 规划的屏幕链路，按可见行裁剪）、IME 光标视图、状态栏。
- **场景**：`blink`（仅光标视图变化）、`type`（第 1 页第 1 行每帧追加一字并重新整形）、`scroll`（文档每帧滚动 7 px）、`refresh`（`window.refresh()`，无状态变化）。附加：`--cached`（上游手动 `AnyView::cached` 缓存工具栏/侧栏/文档视图）、`--ime`（像 JoyWord 一样在滚动内容内注册 `ElementInputHandler`）。
- **计量**：屏幕处于锁屏状态（`CGSSessionScreenIsLocked=Yes`），没有 vsync，窗口模式只出首帧，因此改为**直接驱动**：每帧先用一次独立 update 施加场景变化，再调用公开的 `Window::draw` 并以主线程 CPU 时间计时（`cpu-time` 线程时钟）。该值即“构建→布局→预绘制→绘制→生成 Scene”的 CPU 成本，与 gpui-fast 自报的“CPU time per frame”同口径；**不含 Metal 编码/呈现与 GPU 时间**。每次运行预热 120 帧、计量 600 帧；5 轮交错重复；同时记录每帧实际调用 `paint_glyph` 的次数以核对工作量。
- 构建计时为顺序清洁 release 构建（registry 已缓存，不含下载）。

### 4.2 构建与产物

| 变体 | 清洁 release 构建 | 编译单元 | 二进制 | 依赖包（macOS / Linux / Windows） | 全目标解析包数 |
|---|---|---|---|---|---|
| gpui 0.2.2（现用） | 77.1 s | 455 | 4.77 MB | 455 / 538 / 446 | 710 |
| gpui-pre 0.3.9 | **59.7 s** | 303 | 7.06 MB | **303 / 324 / 273** | 578 |
| gpui-fast 0.1.8 | 67.1 s | 301 | 7.75 MB | 301 / 324 / 271 | 578 |
| gpui-kit 0.7.3 | 96.6 s | 415 | 16.07 MB | 415 / 524 / 401 | 800 |
| gpui-kit + gpui-fast | 120.3 s | 421 | 17.44 MB | 421 / 530 / 406 | 810 |

- gpui-pre 线依赖更少、构建更快，二进制比 0.2.2 大约 2.3 MB；Kit 的增量主要来自组件层与内嵌图标（可关默认特性去掉图标）。
- Kit+fast 比 Kit 多 24 s：两套 GPUI 核心同时编译。

### 4.3 供应链与策略（对照 deny.toml）

| 项目 | 0.2.2 | gpui-pre | gpui-fast | gpui-kit |
|---|---|---|---|---|
| 许可（按 deny 允许列表近似检查） | 全部允许 | 全部允许 | 全部允许 | **2 个 0BSD**（enum-iterator 2.3.0、enum-iterator-derive 1.5.0）需加入允许列表 |
| deny 中 7 条“仅因 0.2.2 引入”的公告忽略项，按包名仍存在的 | 7/7 | 3/7（paste、rustybuzz、ttf-parser） | 3/7 | 5/7（另含 instant、rustls-pemfile） |
| crates.io 所有者 | Zed 团队 | huacnlee（个人） | huacnlee | huacnlee |
| 发布包 VCS 信息 | 有（dirty） | **无** | **无** | 有 |
| 平台与 GPU 代码中的 `unsafe` 关键字（参考） | 783（单 crate） | 约 941（含平台） | 约 1,009 | 另 +96（组件层） |

- 公告比对只按包名，未核版本，需 cargo-deny 正式复核（本机未安装 cargo-deny，未执行）。
- `unsafe` 均在依赖内部，不违反自有代码 `forbid`（C7）。

### 4.4 迁移成本（JoyWord 真实代码）

把 `apps/joyword/src` 原样复制到隔离工程，依赖仓库内 crate 的路径版本：

| 目标 | 编译错误 | 修复改动 | 应用测试 |
|---|---|---|---|
| gpui-pre 0.3.9 | 6 个（另 1 个是复制目录导致的 fixture 相对路径问题，与 API 无关） | 11 行：`Application::new()`→`gpui_platform::application()`；`window.focus(&h)`→`window.focus(&h, cx)` ×3；`Menu` 加 `disabled`；`MenuItem::Action` 加 `checked`/`disabled`；删未用导入 | 11/11 通过 |
| gpui-fast 0.1.8 | 0（复用上面的移植） | 0 | 11/11 通过 |
| 对照 gpui 0.2.2 | — | — | 11/11 通过 |

另见 API 差异：`AsyncApp::update` 在 gpui-pre 返回 `R`（0.2.2 为 `Result<R>`）；`ArenaClearNeeded::clear(cx)` 多一个参数；`WindowOptions` 新增 `inactive_frame_interval`（**默认非激活窗口限 30 fps**）。**未验证**：迁移后应用的 GUI 运行、菜单、IME、多显示器行为。

### 4.5 每帧 CPU（主线程 `Window::draw`，毫秒，5 轮中位数；括号内为 p95）

| 变体 | blink | type | scroll | refresh |
|---|---|---|---|---|
| gpui 0.2.2 | 1.821 (1.889) | 1.833 (1.895) | 1.841 (2.088) | 1.820 (1.879) |
| gpui 0.2.2 + 手动 cached | 1.408 | 1.420 | 1.458 | 1.837 |
| **gpui-pre 0.3.9** | **0.678** (0.710) | **0.681** (0.715) | **0.680** (0.713) | **0.676** (0.708) |
| gpui-pre + 手动 cached | 0.141 | 0.142 | 0.141 | 0.488 |
| **gpui-fast 0.1.8** | **0.028** (0.031) | **0.058** (0.064) | **0.034** (0.037) | **0.202** (0.215) |
| gpui-fast，`GPUI_VIEW_RETENTION=0` | 0.196 | 0.196 | 0.197 | 0.196 |
| gpui-fast，`--ime`（JoyWord 式输入处理器） | 0.028 | 0.059 | 0.073 (**0.171**) | — |
| gpui-fast，`GPUI_SCROLL_LAYERS=0` | — | — | 0.060 | — |
| gpui-kit（Kit 按钮，gpui-pre 底座） | 1.603 | 1.620 | 1.655 | 1.602 |
| gpui-kit + gpui-fast | 0.177 | 0.397 | 0.154 | 0.632 |

工作量核对：非复用变体每帧绘制 1,672–1,681 个字形；gpui-fast 在 blink 下为 0（整页复用），scroll 下为 54（仅合成瓦片外的少量重绘）。5 轮间差异均 <2%。

解读：

- **0.2.2 → gpui-pre**：约 2.7 倍。仅换来源即可获得。
- **gpui-pre → gpui-fast**：blink 24 倍、type 11.7 倍、scroll 20 倍、refresh 3.3 倍。即使关闭 Retained Mode，gpui-fast 仍快 3.5 倍（布局节点/文本测量复用）。
- **上游手动缓存很有效**：gpui-pre 对视图加 `.cached()` 即降到 0.14 ms，拿到 gpui-fast 收益的大部分；代价是要手工维护缓存边界。
- **IME 处理器对滚动图层的影响**：在滚动内容内注册输入处理器后，scroll 均值 0.034→0.073、p95 0.037→0.171（图层失效，退回重绘）。若采用 gpui-fast，可把 `handle_input` 移到不滚动的覆盖层元素上规避（需验证 IME 候选窗定位）。
- **Kit 的样式组件不便宜**：48 个 Kit Button 让每帧多约 0.92 ms；叠加 gpui-fast 后仍比纯 gpui-fast 慢 3.1–6.8 倍——gpui-fast 自己的文档也指出 Kit 在 render 期间写窗口级全局状态（`sync_focused_input_registry`、`GlobalState`、`SelectionStateRegistry`），迫使祖先视图每帧重建。
- **量级**：60 Hz 帧预算 16.7 ms。M4 上 0.2.2 占约 11%，gpui-pre 约 4%，gpui-fast 不超过 1.2%。在信创低功耗 CPU 上同样负载可能放大数倍（**未实测**，仅推断），这是 gpui-fast 对 JoyWord 最有现实意义的场景；而 JoyWord 自身的排版和页面光栅成本远大于 GPUI 帧构建。

### 4.6 文档长度扩展性（8 页 → 64 页，中位数 ms）

| 变体 | blink | type | scroll |
|---|---|---|---|
| gpui 0.2.2 | 1.826 → 1.931 | 1.854 → 1.952 | 1.825 → 1.921 |
| gpui-pre | 0.679 → 0.776 | 0.680 → 0.773 | 0.681 → 0.771 |
| gpui-pre + cached | 0.142 → 0.211 | 0.142 → 0.211 | 0.140 → 0.212 |
| gpui-fast | 0.028 → 0.030 | 0.058 → 0.104 | 0.034 → 0.061 |
| gpui-kit | 1.630 → 1.715 | 1.654 → 1.739 | 1.663 → 1.760 |
| gpui-kit + fast | 0.169 → 0.171 | 0.399 → 0.446 | 0.153 → 0.181 |

按可见行裁剪后，页数 ×8 只增加约 0.1 ms；上游的主要成本来自外壳（工具栏/侧栏）而非文档。真正的长文档（数百页）仍应按 ARCHITECTURE 要求只为视口及邻近页建元素，与选哪个 GPUI 无关。

### 4.7 冷启动与内存（8 次中位数）

| 变体 | 进程启动到退出（含开窗、首帧、runtime shader 编译） | 最大 RSS |
|---|---|---|
| gpui 0.2.2 | 0.132 s | 83.8 MB |
| gpui-pre | 0.120 s | 78.2 MB |
| gpui-fast | 0.116 s | 79.0 MB |
| gpui-kit | 0.185 s | 91.7 MB |
| gpui-kit + fast | 0.181 s | 92.8 MB |

### 4.8 未能实测 / 局限

- **窗口 vsync 实测未完成**：屏幕锁定导致窗口模式无法出帧；基准保留了 `--vsync` 模式，解锁后可复测（见第 10 节）。直接驱动法不含 GPU 编码、呈现与滚动瓦片光栅成本。
- **Linux / 信创 / Windows 未实测**：无对应机器。GL 退路结论来自源码阅读（4.9）。
- **正确性**：未独立验证 gpui-fast 复用帧与从头绘制的像素一致性（上游提供 `gpui_perf --headless --verify` 与 oracle 测试）。
- 仅一台 Apple Silicon，Retina/DPI=2 未测；cargo-deny 未执行（仅近似许可检查）。

### 4.9 源码核对（非运行实测）

- **C1 外部字形**：gpui-pre、gpui-fast 均为 `pub struct GlyphId(pub u32)`，`Window::paint_glyph` 公开——W02 的一行可见性补丁不再需要。基准在三种核心上都通过 `paint_glyph` 绘制了预整形字形。
- **C2 Linux GL**：gpui-pre-wgpu / gpui-fast-wgpu 使用 wgpu 29，Linux 启用 `vulkan` + `gles`；实例以 `Backends::VULKAN | Backends::GL` 创建，枚举全部适配器，按“非软件优先”排序，并逐个以真实 surface 试配；窗口渲染路径**不拒绝软件适配器**（`WgpuContext::new` 传 `reject_software=false`），即 GL 硬件和 llvmpipe 类软件栈在结构上都可选中。**需在信创真机验证**。gpui-fast 0.1.8 没有自研 CPU 渲染器。
- **C3 AccessKit**：gpui-pre/gpui-fast 依赖 `accesskit 0.24`（Linux 另有 `accesskit_unix`）；Kit 组件内置 AccessKit 角色与测试。
- **C6 runtime shaders**：gpui-kit 门面强制开启；gpui-pre/gpui-fast/gpui-component 不强制。

---

## 5. 逐项对照硬约束

| 约束 | gpui 0.2.2 | gpui-pre 0.3.9 | gpui-fast 0.1.8 | gpui-kit 0.7.3 |
|---|---|---|---|---|
| C1 外部 GlyphId | 需补丁 | ✅ | ✅ | ✅（继承底座） |
| C2 Linux GL/CPU 退路 | ❌ 仅 Vulkan | ✅ wgpu GL（源码，待真机） | ✅ 同左 | ✅ 同左 |
| C3 AccessKit | ❌ | ✅ | ✅ | ✅ 组件级 |
| C4 精确锁定 / 工具链 | ✅ | ✅ `=0.3.9`，1.98.1 可编 | ✅ 1.98.1 可编 | ⚠️ 内部依赖用 `^`（如 `gpui-base ^0.7.1`），需靠 Cargo.lock |
| C5 供应链 | ✅ 官方 | ⚠️ 个人发布、无 VCS 信息；抽查与 Zed 一致 | ⚠️ 个人发布、无 VCS 信息、新分叉 | ⚠️ 个人发布、0BSD、依赖 +37% |
| C6 不启用 runtime shaders | ✅ | ✅ | ✅ | ❌ 门面强制（直接用 gpui-component 可规避） |
| C7 自有代码 forbid unsafe | ✅ | ✅ | ✅ | ✅ |
| C8 IME / 命令注册表 / 本地化 | ✅ 已实现 | ✅ API 兼容 | ⚠️ 滚动图层遇输入处理器失效 | ⚠️ 183 个组件快捷键与独立 i18n 需与 T106/Catalog 整合 |

---

## 6. 优缺点

### 6.1 GPUI Fast

优点
- 每帧 CPU 实测比 gpui-pre 低 3.3–24 倍，比现用 0.2.2 低 9–65 倍；对长时间滚动、输入、动画与低端 CPU（信创）最有价值。
- 与 gpui-pre API 兼容：JoyWord 移植代码零修改即可切换，切换成本极低，可用 cargo 依赖别名做成可选后端。
- 构建与依赖规模与 gpui-pre 基本相同（301 包，+7 s，+0.7 MB）。
- 自带复用/图层的开关与验证工具（环境变量、oracle 测试、`--verify`），便于 A/B。

缺点 / 风险
- 新：仓库 3 周、9 天 9 版、自称 experimental；Retained Mode 的正确性修复仍在持续合并，文档类应用对“陈旧渲染”零容忍。
- 单一发布者、无 VCS 元数据；约 4.5 万行净增代码（核心 `fast/` 模块 3.9 万行）需要审查，并长期跟随上游同步。
- 编程约束：渲染相关状态必须放在实体/全局/滚动状态中，否则需要手动 `cx.notify()`；render/paint 期间不应每帧写全局。
- JoyWord 式在滚动内容中注册 IME 处理器会让滚动图层失效（实测 p95 ×4.6），需调整结构。
- 未提供 CPU 渲染器；Linux 退路与上游相同（wgpu GL）。

### 6.2 GPUI Kit

优点
- 组件齐全且成熟（17.6k 星、商业产品验证）：对话框、菜单/原生菜单、Dock、侧栏、Select/Combobox、颜色选择、表格、通知、设置页等可显著缩短外壳开发；带 AccessKit、中文本地化、无头 UI 集成测试。
- 主题与设计规范统一，可选 gpui-fast 底座。

缺点 / 风险
- 不解决 JoyWord 的核心问题：画布、分页、选择、IME、富文本仍需自研；其 Input/Editor 面向代码编辑。
- 成本：依赖 +112 包（macOS）、约 23.8 万行代码与 1,830 个图标进入供应链；二进制 +9 MB；构建 +62%；同样界面每帧 CPU +136%；冷启动 +65 ms、内存 +13 MB。
- 策略冲突：门面强制 runtime shaders（C6）；0BSD 许可；内部 `^` 版本约束；开启 gpui-fast 时两套核心同时编译。
- 双轨：183 个组件快捷键、rust-i18n 与 JoyWord 命令注册表（T106）/本地化 Catalog 重叠，需要逐项审计冲突。
- 演进快：0.x 小版本即有破坏性变更，主要由一人维护，长期跟随成本高。

---

## 7. 对“是否有必要”的直接回答

| 问题 | 结论 |
|---|---|
| 必须基于 GPUI Kit 开发吗？ | **否。** 收益集中在通用外壳组件；与项目的供应链/构建/命令体系约束冲突较多。 |
| 必须基于 GPUI Fast 开发吗？ | **现在否，将来可能。** 性能收益大，但成熟度不足；API 与 gpui-pre 一致，可延后零成本切换。 |
| 现在最该做什么？ | **推进 ADR-010：迁到 gpui-pre 线（或 git 固定同一 Zed 提交）。** 它满足 C1–C3，实测更快、依赖更少，迁移只需 11 行改动。 |

---

## 8. 建议路线（供协调者裁决）

1. **ADR-010 定源**：gpui-pre `=0.3.x`（精确锁定）或 git 固定 Zed 提交。若选 gpui-pre，新增 CI 校验：按 description 中的 Zed 提交逐文件比对 crates.io 包源码；同时更新 deny.toml 的忽略项（7 条中预计只剩 3 条待复核）。
2. **迁移切片**：以本报告的 11 行 diff 为起点完成 `apps/joyword` 迁移；去掉 W02 的 GlyphId 补丁路线；在三端 CI 与 Linux 真机（含 Vulkan 不可用、仅 GL 的环境）验证窗口、IME、菜单与 GL 退路。
3. **gpui-fast 作为可选后端**：增加一个 cargo 特性（依赖别名 `gpui = { package = "gpui-fast" }`）和一个 CI 编译 + 测试任务，持续验证兼容性但不发布。满足以下全部条件后再评估切换：
   - 上游取消 experimental 标记，且连续 8 周以上没有 Retained/Scroll Layers 正确性修复；
   - 在 JoyWord 自己的 CI 中运行“复用开/关输出一致”校验（上游 `--verify` 思路）；
   - 在目标机（尤其信创）上测得帧时间瓶颈，且 gpui-pre 下手动 `.cached()` 不足以解决。
   - 切换前把 IME 输入处理器移出滚动内容，或接受滚动图层失效。
4. **GPUI Kit**：M1 期间不引入。外壳组件需求增长后（M2+），按组件逐个评估**直接依赖 `gpui-component`**（避开门面强制的 runtime shaders 与双核心），先做许可、快捷键冲突、i18n 整合与每帧成本评审；或参考其交互模式自建少量组件（遵守 PROVENANCE 独立实现规则）。
5. **无论选哪个，都在 gpui-pre 上给工具栏/侧栏等稳定视图加 `.cached()`**：实测可把外壳的每帧成本降约 80%（0.68→0.14 ms）。

---

## 9. 待验证清单

- [ ] 解锁屏幕后用 `--vsync` 窗口模式复测（含 Metal 呈现），并在 Retina/DPI=2 下复测。
- [ ] Linux x86_64 / ARM64 / LoongArch64 真机：wgpu GL 后端实际选中、渲染正确性、IME（fcitx/ibus，X11/Wayland）、性能。
- [ ] Windows 真机：DirectX 后端、IME。
- [ ] cargo-deny 正式检查（advisories 带版本比对、sources）。
- [ ] gpui-pre 快照与 Zed 提交的全量逐文件比对（本次抽查 12 个文件）。
- [ ] gpui-fast 复用正确性：在 JoyWord 场景跑复用开/关的帧级比对。

---

## 10. 复现

证据目录：[GPUI-KIT-FAST-EVALUATION/bench](GPUI-KIT-FAST-EVALUATION/bench)（源码、5 套 Cargo.toml/Cargo.lock、脚本、原始 JSONL、汇总、构建计时、许可检查）与 [GPUI-KIT-FAST-EVALUATION/migrate](GPUI-KIT-FAST-EVALUATION/migrate)（JoyWord 迁移 diff 与隔离清单）。说明见 [GPUI-KIT-FAST-EVALUATION/README.md](GPUI-KIT-FAST-EVALUATION/README.md)。

脚本会在 bench 目录下生成数 GB 的 `target-rel/`、`target-check/`（不在 .gitignore 的 `target` 规则内），请先把 bench 目录复制到仓库外再运行：

```sh
cp -R docs/coordination/reports/GPUI-KIT-FAST-EVALUATION/bench /tmp/gpui-bench && cd /tmp/gpui-bench
# 顺序清洁构建 5 个变体并记录时间/产物
./build_all.sh
# 每帧 CPU 矩阵：5 轮、每次 600 帧（锁屏也可运行）
python3 run_matrix.py 5 600 results.jsonl
# 页数扩展性与冷启动
python3 run_extra.py
python3 summarize.py
# 窗口 vsync 模式（需解锁屏幕；会弹出不抢焦点的窗口）
target-rel/vpre/release/bench --vsync --scenario blink --frames 600
```

## 11. 来源

- GPUI Fast：<https://github.com/longbridge/gpui-fast>；`docs/retained-mode.md`、`docs/scroll-layers.md`；PR #45、#46、#52、#68（GitHub API）。
- GPUI Kit：<https://github.com/longbridge/gpui-kit>、<https://gpui-kit.com/docs/components/>、`release-notes.md`。
- crates.io API：`gpui`、`gpui-pre*`、`gpui-fast*`、`gpui-kit`、`gpui-component` 的版本、依赖、所有者与发布者。
- Zed 源码：`zed-industries/zed@93d745668805029476e6d7fbb19652116fed51de`（抽样比对）；Linux 渲染器改用 wgpu：<https://git.secluded.site/zed/commit/af8ea0d6c26192c45f44f473c0d4a7d6f72ed018>。
- 仓库内：`docs/joyword/ARCHITECTURE.md`（ADR-002/005/009/010）、`docs/coordination/reports/W02.md`、`W02-REVIEW.md`、`deny.toml`、`README.md`。
