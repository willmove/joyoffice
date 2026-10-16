# GPUI Kit / GPUI Fast 评估证据

[评估报告](../GPUI-KIT-FAST-EVALUATION.md) 的源码、清单与原始数据。实验在仓库外的临时目录构建运行，只复制了可复现所需文件；构建产物（约 17 GB）未提交。

- 主机：Apple M4（10 核）/ 24 GB / macOS 27.0.1（26A434）/ Rust 1.98.1 / 仅 CommandLineTools，所有变体启用 `runtime_shaders`。
- 仓库基线：`main` @ `0dcc0f0e4f0efa5d7845b697ed7d60bcc2136ff2`。
- 字体：基准界面使用 macOS 系统字体 PingFang SC（Apple 专有许可，不随仓库分发）。

## bench/

| 文件 | 内容 |
| --- | --- |
| `src/main.rs` | 基准程序（单文件，cfg 区分各 GPUI 来源）。默认直接驱动 `Window::draw` 计主线程 CPU；`--vsync` 改为按显示刷新逐帧计量（需解锁屏幕） |
| `v022/` `vpre/` `vfast/` `vkit/` `vkitfast/` | 5 个独立 workspace 的 `Cargo.toml` 与实际使用的 `Cargo.lock`：gpui 0.2.2、gpui-pre 0.3.9、gpui-fast 0.1.8、gpui-kit 0.7.3、gpui-kit 0.7.3 + `gpui-fast` 特性 |
| `build_all.sh` / `build_times.txt` | 顺序清洁 release 构建脚本及计时、编译单元数、二进制大小 |
| `run_matrix.py` / `results.jsonl` | 每帧 CPU 矩阵（4 场景 × 变体，5 轮 × 600 帧） |
| `run_extra.py` / `results_scaling.jsonl` / `results_startup.jsonl` | 8 页与 64 页扩展性（3 轮）；冷启动与最大 RSS（8 轮） |
| `summarize.py` / `summary.txt` | 汇总（中位数、p95、范围、每帧绘制字形数） |
| `license_check.py` / `license_check.txt` | 按 deny.toml 允许列表近似检查许可（只解析 SPDX 字段，非 cargo-deny） |

程序参数：`--scenario blink|type|scroll|refresh`、`--frames N`、`--warmup N`、`--cached`（上游手动视图缓存）、`--ime`（在滚动内容内注册输入处理器）、`--vsync`。环境变量：`BENCH_PAGES`（页数，默认 8）、`GPUI_VIEW_RETENTION=0`、`GPUI_SCROLL_LAYERS=0`（仅 gpui-fast）；`BENCH_DEBUG`、`BENCH_KIND`、`BENCH_FOCUS` 仅用于排查窗口模式。

已知限制：测量期间屏幕锁定，所有结果来自直接驱动模式，不含 Metal 编码、呈现和 GPU 时间。

## migrate/

| 文件 | 内容 |
| --- | --- |
| `gpui-pre-port.diff` | 把 `apps/joyword/src` 迁到 gpui-pre 0.3.9 的全部改动（末段为启动入口改动；同一份代码在 gpui-fast 0.1.8 上无需再改） |
| `joyword-pre.Cargo.toml` / `joyword-fast.Cargo.toml` | 隔离迁移工程清单。路径依赖写的是本机仓库的绝对路径，复现时需替换为自己的仓库位置 |
