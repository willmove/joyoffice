# GPUI 来源迁移交接报告：gpui 0.2.2 → gpui-pre 0.3.9

- 任务：用户在 Claude Code 会话中直接指派“从 gpui 0.2.2 迁到 gpui-pre 线”（无任务卡，不占用 Wxx/Cxx 编号）。
- 执行者：Claude Code（Claude 桌面应用 Code 会话），模型 Opus 5.5。
- 主机：Apple M4 / arm64 / macOS 27.0.1（26A434）/ Xcode 27.1（未下载 Metal Toolchain 组件）/ Rust 1.98.1（48a229cea 2026-09-01）。
- 代码基线：`main` @ `0dcc0f0e4f0efa5d7845b697ed7d60bcc2136ff2`（启动时实际 HEAD 与 origin/main 一致）。
- 分支：`claude/gpui-pre-migration`（独立 worktree `.claude/worktrees/gpui-pre-migration`）。
- 依据：[GPUI Kit / GPUI Fast 选型调研](https://github.com/willmove/joyoffice/blob/claude/gpui-kit-fast-evaluation/docs/coordination/reports/GPUI-KIT-FAST-EVALUATION.md)（分支 `claude/gpui-kit-fast-evaluation`，尚未合入 main）第 7–8 节的建议与其隔离迁移 diff；本次在仓库内正式落地，并修正了该隔离清单遗漏的平台特性（见下文“平台特性”）。
- 状态：完成本轮切片（编译、无窗口测试、lint、依赖策略）；GUI 运行与非 macOS 平台未验证。
- interface_ready：true。应用代码继续以 `gpui` / `gpui_platform` 名称引用（Cargo 包重命名），对外部 crate 无影响；`GlyphId(pub u32)` 与公开的 `Window::paint_glyph` 已在依赖中可用。
- implementation_verified：部分。macOS 上 dev-runtime-shaders 构建、clippy、应用 11 项测试、默认成员 364 项测试与 cargo-deny 通过；不启用 runtime shaders 的预编译 Metal 构建、Linux、Windows 在本机受环境阻塞，未验证。
- acceptance_complete：false。未启动窗口；菜单、IME、焦点、多显示器、Linux GL 退路均未人工验收。

## 实际变更与支持范围

| 路径 | 变更 |
| --- | --- |
| `Cargo.toml` | `gpui = "=0.2.2"` 改为 `gpui = { package = "gpui-pre", version = "=0.3.9" }`；新增 `gpui_platform = { package = "gpui-pre-platform", version = "=0.3.9", features = ["font-kit", "wayland", "x11"] }` |
| `apps/joyword/Cargo.toml` | 新增 `gpui_platform` 依赖；`dev-runtime-shaders` 改为转发 `gpui_platform/runtime_shaders`（特性名不变，README 中的命令继续可用） |
| `apps/joyword/src/main.rs` | `Application::new()` → `gpui_platform::application()`；`Menu` 增加 `disabled: false`，`MenuItem::Action` 增加 `checked: false`、`disabled: false`；删除未用的 `Application` 导入；`window.focus(&h)` → `window.focus(&h, cx)` |
| `apps/joyword/src/input_ui.rs` | `window.focus(&h)` → `window.focus(&h, cx)` ×2 |
| `Cargo.lock` | GPUI 依赖树整体替换：锁文件条目 733 → 672；新增 19 个 `gpui-pre*` 包（全部 0.3.9）。JoyWord 自有 crate 的 `=` 固定直接依赖版本均未变化 |
| `deny.toml` | 删除 4 条已不命中的忽略项（async-std、instant、proc-macro-error2、rustls-pemfile），剩余 3 条（paste、rustybuzz、ttf-parser）改写为 gpui-pre 下的实际引入路径；许可允许列表删除已无包使用的 `NCSA` |

上游来源：crates.io `gpui-pre 0.3.9`（2026-10-12 发布，当前最新），包元数据 `zed-rev = "93d745668805029476e6d7fbb19652116fed51de"`。所有 `gpui-pre*` 包在 Cargo.lock 中以校验和固定。

### 平台特性

`gpui-pre-platform` 默认只开 `multithreaded`，必须显式选择：

- `font-kit`：`gpui-pre-macos` 未开此特性时使用 `NoopTextSystem`，**界面上的 GPUI 文字（菜单外的按钮、状态栏、提示等）不会渲染**（源码 `gpui-pre-macos-0.3.9/src/platform.rs` 第 233–240 行）。调研报告的隔离清单只开了 `runtime_shaders`，若照搬会在 GUI 中丢字。
- `x11`、`wayland`：`gpui-pre-linux` 由平台 crate 以 `default-features = false` 引入，不开则 Linux 没有窗口后端，也不会编译 wgpu 渲染器。
- 未开 `screen-capture`、`inspector`、`test-support`。

### 已知 API/行为差异（编译通过但需 GUI 关注）

- `WindowOptions::inactive_frame_interval` 默认 `Some(33_333 µs)`：非激活窗口限 30 fps。应用沿用默认值。
- `AsyncApp::update` 改为直接返回 `R`；应用中 `spawn` 闭包使用的是 `WeakEntity::update`（仍返回 `Result`），无需修改。
- 依赖中 `block 0.1.6`（经 cocoa/metal）的 future-incompat 提示仍在；原 `proc-macro-error2` 提示消失。

### 未改动

- `experiments/gpui-font-probe/`：独立 workspace，仍用 vendored gpui 0.2.2 + GlyphId 补丁，作为 W02 历史证据保留。gpui-pre 已公开 `GlyphId(pub u32)`（`text_system.rs:1255`）与 `Window::paint_glyph`（`window.rs:4821`），该补丁路线对正式应用不再需要。
- README、ARCHITECTURE（ADR-010）、IMPLEMENTATION、PROVENANCE、CI：按 AGENTS.md 由协调者更新，建议文本见下节。
- 未引入 gpui-fast 或 gpui-kit。

## 验证

| 完整命令或人工步骤 | 实际结果 | 环境/输入/日志 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | passed | — |
| `cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings` | passed | 仅 `block v0.1.6` future-incompat 提示 |
| `cargo test --locked -p joyword --features dev-runtime-shaders` | passed，11 passed | 无窗口 |
| `cargo test --locked` | passed，364 passed / 0 failed | 默认成员，不依赖 GPUI；用于确认锁文件变化未影响其他 crate |
| `cargo build -p joyword --features dev-runtime-shaders --locked` | passed | 开发构建，不是发行或 GUI 验收 |
| `cargo deny --locked --all-features check`（cargo-deny 0.20.2，与 CI 同版本，安装到会话临时目录） | passed：`advisories ok, bans ok, licenses ok, sources ok`，无告警 | 修改 deny.toml 前同命令也通过，但报告 4 条未命中的公告忽略与 1 条未使用的许可（NCSA） |
| `python3 scripts/check_docs.py` | passed | — |
| `python3 -m unittest discover -s scripts/tests` | passed，19 项 | — |
| `cargo build -p joyword --locked`（不启用 runtime shaders，预编译 Metal） | blocked | `gpui-pre-apple` 构建脚本：`cannot execute tool 'metal' due to missing Metal Toolchain; use: xcodebuild -downloadComponent MetalToolchain`。本机 Xcode 27.1 未下载该组件；CI macos-15 覆盖 |
| `cargo check -p joyword --locked --target x86_64-pc-windows-msvc` | blocked | `gpui-pre` 构建脚本嵌入 Windows manifest 需要 rc 工具，macOS 主机没有；CI windows-2022 覆盖 |
| `cargo check -p joyword --locked --target x86_64-unknown-linux-gnu` | blocked | `wayland-backend` 构建脚本需要目标 sysroot 的 pkg-config；CI ubuntu-24.04 / 24.04-arm 覆盖 |
| GUI 启动、菜单、快捷键、IME、关闭确认、文件对话框 | skipped | 按 AGENTS.md，GUI 验证需先与用户约定时段 |

## 接口或依赖提案

供协调者裁决并更新公共文档：

1. **ADR-010**：记录选定 crates.io `gpui-pre =0.3.9`（Zed `93d745668805029476e6d7fbb19652116fed51de` 快照，Apache-2.0），精确锁定；平台 crate `gpui-pre-platform =0.3.9` 开 `font-kit`、`wayland`、`x11`。后果：外部 GlyphId 与 AccessKit 由上游提供；Linux 渲染为 wgpu（Vulkan + GL），GL 退路**仍待信创真机验证（T005）**；发布者为个人（huacnlee），发布包无 `.cargo_vcs_info.json`，需供应链补偿措施（见第 5 条）。
2. **README 第 25 行**：“GPUI 固定 0.2.2”改为“GPUI 固定 gpui-pre 0.3.9（Zed 93d7456 快照）”。
3. **ARCHITECTURE**：§6.1 第 108 行与 §7 第 128 行关于 0.2.2 的 GlyphId 不可构造、Linux 仅 Vulkan 的描述改为历史说明，并注明 gpui-pre 的现状与待验证项；剪贴板/打印仍需自建的结论不变。
4. **IMPLEMENTATION 第 168 行依赖表、PROVENANCE 第 41 行**：GPUI 来源改为 gpui-pre 0.3.9；cargo-deny 忽略项由 7 条减为 3 条；许可列表去掉 NCSA；为 GPUI 新增一行来源记录（包名、版本、zed-rev、Apache-2.0、发布者）。
5. **供应链校验（建议的后续切片）**：CI 增加一步，对 Cargo.lock 中每个 `gpui-pre*` 包按 `package.metadata` 的 `zed-crate`/`zed-rev` 与 Zed 源码逐文件比对（调研只抽查了 12 个核心文件）。
6. **CI**：本次未改 workflow。若 Linux 构建因系统库失败，需补 apt 包（预计不需要：wgpu 运行时动态加载 Vulkan/EGL，x11rb 为纯 Rust，xkbcommon/wayland/fontconfig/freetype 开发包已在列表中）。

## 阻塞与下一步

- 预编译 Metal 构建：本机执行 `xcodebuild -downloadComponent MetalToolchain` 后复跑 `cargo build -p joyword --locked`，或以 CI macos-15 结果为准。
- 三端 CI：推送分支后看 build 矩阵（macOS 预编译 Metal、Windows、Linux x86_64/ARM64）与 dependencies 作业。
- GUI 冒烟（需约定时段）：`cargo run -p joyword --features dev-runtime-shaders --locked`，核对窗口与界面文字渲染、菜单与快捷键、页面画布、IME 组合与候选窗位置、未保存关闭确认、打开/另存对话框。
- 后续可选：按调研第 8 节给工具栏/侧栏等稳定视图加 `.cached()`；gpui-fast 作为 CI 编译矩阵中的可选后端。
