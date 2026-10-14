# gpui-font-probe（W02）

独立实验：验证"同一字体字节 + 既有字形位置"的 GPUI 屏幕绘制路径
（ADR-010 线索），不属于正式应用，不修改根 workspace 依赖。

## 结构

- `Cargo.toml`：独立 workspace；`[patch.crates-io]` 指向 vendored gpui。
- `vendor/gpui-0.2.2/`：crates.io gpui 0.2.2 副本 + 一处可见性补丁，
  来源、校验和与 diff 见 `vendor/PATCH.md`。
- `probe/`：探针本体（lib + 窗口程序）。
  - `src/name.rs`：name 表改写器与 TTC face 抽取器（按 OpenType 规范
    原创，来源见文件头）。
  - `src/lib.rs`：加载→抽取→改名→验证流程与场景构建。
  - `src/main.rs`：窗口模式（paint_glyph 绘制 + 标定/标尺 + 自捕获）。
  - `src/conflict.rs`：`--conflict-check` 同名字体冲突实验（无窗口）。
  - `src/crosscheck.rs`：`--glyph-crosscheck` 字形交叉核对（无窗口）。
  - `src/capture.rs`：按窗口 ID 捕获本进程窗口（证据采集；唯一局部
    allow(unsafe_code)，理由见注释与报告）。
- `tools/measure.py`：纯标准库几何测量（窗口/PDF/缩放三个子命令）。
- `artifacts/`：证据输出（JSON/PDF/PNG；系统字体本体永不落盘于此）。

## 构建与运行

本机只有 CommandLineTools 时需要运行时着色器特性（与根仓库
apps/joyword 的 dev-runtime-shaders 同一上游开关）：

```sh
cd experiments/gpui-font-probe
cargo build --features runtime-shaders
cargo test  --features runtime-shaders
```

窗口探针（默认 macOS Songti face 6；`--font path --face N` 可覆盖）：

```sh
cargo run --features runtime-shaders -- --zoom 1.0 --evidence-out artifacts --quit-after-secs 8
```

无窗口实验：

```sh
cargo run --features runtime-shaders -- --conflict-check     --evidence-out artifacts
cargo run --features runtime-shaders -- --glyph-crosscheck   --evidence-out artifacts
```

PDF 栅格化（qlmanage，macOS 自带）与机器测量：

```sh
qlmanage -t -s 1600 -o artifacts artifacts/probe.pdf
python3 tools/measure.py window artifacts/window-z100.png artifacts/evidence-z100.json
python3 tools/measure.py window artifacts/window-z200.png artifacts/evidence-z200.json
python3 tools/measure.py pdf    artifacts/probe.pdf.png     artifacts/evidence-z100.json
python3 tools/measure.py scale  artifacts/evidence-z100.json artifacts/evidence-z200.json
```

## 字体

默认输入 `/System/Library/Fonts/Supplemental/Songti.ttc` face 6
（Songti SC Regular），macOS 系统内置、Apple 专有许可；仅本机实验
输入，不提交、不重分发。冲突实验另用 `/System/Library/Fonts/
Hiragino Sans GB.ttc` face 0，许可同上。正式集成建议用 OFL 字体
（如思源黑体/Noto Sans CJK）复测。

## 已知边界

- macOS `add_fonts` 走 CGFont，TTC 只注册 face 0：探针先抽取目标
  face 为单字体文件再注册（表字节不变，测试覆盖）。
- 窗口捕获按"本进程窗口 ID"进行，不需要屏幕录制权限；整机截屏
  仍需用户授权（未取得，见报告阻塞项）。
