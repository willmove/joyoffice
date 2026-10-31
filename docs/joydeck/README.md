# JoyDeck

本产品现于 JoyOffice Monorepo 维护。首先阅读[新的开发入口](DEVELOPMENT.md)、[迁移计划](../joyoffice/MONOREPO-MIGRATION.md)和[模块边界](../joyoffice/ARCHITECTURE.md)。以下 M0–M14 为原独立项目的实施记录，迁移后的命令与验证以新开发入口和迁移结果为准。

离线优先的 Rust + GPUI 桌面演示应用。按[净室实现规范](01-clean-room.md)，依据用户需求、公开标准与自建样例独立开发。

当前：M13本机资格化/偏好/发布维护与M14静态页MP4、离线比较/对象接受、标题VTT已交付；122项Rust测试及本机GUI/前序回归见[36](36-m13-implementation-report.md)、[37](37-m14-implementation-report.md)。完整三平台Beta与其余功能池仍开放。[最新开发包（旧归档：joydeck-m14-final-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。

M9/M10基线：M9七类基础图表/图示与M10动画/演示者软件子项已实现，按本机PowerPoint重构功能区和工作区；102项测试、系统拼音、Office实际编辑重读及最终包验证见 [31](31-m9-implementation-report.md)、[32](32-m10-implementation-report.md)。[M10历史开发包（旧归档：joydeck-m10-validated-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md) 支持双击JoyDeck.app；完整双屏/三平台/MVP及前序开放条件见 [STATUS](STATUS.md)。

## 开发与运行

JoyOffice 合并前的依赖／crate 登记／CI 对齐见[融合准备](38-joyoffice-integration.md)。独立目录检查用 Python 3.11+ 执行 `python3 scripts/joydeck/check-repository.py`。

macOS/Linux 使用 `cargo run --locked -p joydeck`；跨平台窗口回归用 `python3 scripts/joydeck/smoke-native.py M0 M1 M2 M3 M4`；M10 自动检查用 `cargo xtask build joydeck`。窗口验证要求可交互桌面。

- `pwsh.exe -File scripts/joydeck/dev.ps1 run`：启动基础编辑器。
- `cargo run --locked -p joydeck -- --diagnostics`：M0 诊断窗口。
- `cargo run --locked -p joydeck -- --viewer`：只读查看器。
- `pwsh.exe -File scripts/joydeck/smoke-m2.ps1`：M2真实桌面回归。
- `cargo xtask check joydeck`：格式、Clippy 和测试。
- `pwsh.exe -File scripts/joydeck/smoke-m1.ps1`：真实桌面窗口回归。
- `pwsh.exe -File scripts/joydeck/dev.ps1 package`：生成当前平台 M12 release 开发包（保留旧包）。
- `cargo run --release --locked -p joydeck -- --cpu-canvas`：CPU 对照画布；默认使用 GPUI Scene。
- `cargo run --release --locked -p joydeck -- --m4-bench`：固定原生 CPU/GPUI 场景基准。
- `python3 scripts/joydeck/benchmark-m4.py`：macOS 窗口像素/RSS/延迟验证（需 Pillow/numpy）。
- `python3 scripts/joydeck/package-native.py --profile release`：创建新名字的 M12 原生开发 ZIP。
- `cargo xtask build joydeck`：构建与阶段自动检查；实际桌面验收另见记录。
- `python scripts/joydeck/make-fixtures.py`：生成独立 `.joydeck`/PPTX 样例。

Rust 1.98.1，GPUI Kit 0.8.1 / GPUI pre 0.3.10；PowerShell 使用 `pwsh.exe`。PPTX已提供源包保留写回与明确有损新建导出（范围见29）；M2支持指纹检查后的覆盖保存、交互变换和恢复列表。

## 文档

[01 净室实现](01-clean-room.md) · [02 产品范围](02-product-spec.md) · [03 功能要求](03-functional-spec.md) · [04 架构](04-architecture-spec.md) · [05 UI](05-gpui-ui-spec.md) · [06 渲染与文字](06-render-text-spec.md) · [07 文件](07-format-compatibility-spec.md) · [08 放映媒体自动化](08-show-media-automation-spec.md) · [09 质量发布](09-quality-platform-release.md) · [10 路线图](10-roadmap.md) · [11 任务](11-task-backlog.md) · [12 验收](12-acceptance-playbook.md) · [13 决策风险](13-decisions-risks.md) · [14 追踪](14-traceability.md) · [15 依据证据](15-evidence-references.md) · [16 动作](16-command-inventory.md) · [17 M0记录](17-m0-implementation-report.md) · [18 M1规格](18-m1-viewer-spec.md) · [19 M1实施记录](19-m1-implementation-report.md) · [20 M2切片规格](20-m2-editor-spec.md) · [21 M2初始记录](21-m2-implementation-report.md) · [22 M2剩余实现与验证](22-m2-completion-report.md)。

需求 `FR-*`、任务 `T-M*-*`、验收 `AT-*` 保持稳定。M0–M7 为基础 MVP，M8–M13 累积到 Beta。任务未勾选或平台未实测均不算完成；性能目标不是已测结果。

[M3 文字编辑预览与此前收尾记录](23-m3-implementation-report.md) · [macOS ARM64 开发包（旧归档：joydeck-m3-verified-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。

[解锁后原生与系统拼音验证](24-m3-macos-verification.md)。


[M4 验证契约](25-m4-scene-validation.md) · [M4 实施/数据/开放条件](26-m4-implementation-report.md) · [M4 macOS ARM64 release 开发包（旧归档：joydeck-m4-final-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。

[M5图片创作实施/限制](27-m5-implementation-report.md) · [M5 macOS ARM64 release开发包（旧归档：joydeck-m5-preview-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。macOS带Pillow/numpy的Python运行 `scripts/joydeck/verify-m5.py --binary <binary>` 验证原生图片/对象剪贴板及CPU/GPUI窗口像素。

[M6文稿设计实施/限制](28-m6-implementation-report.md) · [M6 macOS ARM64最终开发预览包（旧归档：joydeck-m6-preview-final-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。macOS带Pillow/numpy的Python运行 `scripts/joydeck/verify-m6.py --binary <binary>` 验证共享双窗口/跨文档页复制和CPU/GPUI窗口像素。M6历史writer为schema1.4/minReader5，旧版本拒绝新文件。

[M7文件交付/限制](29-m7-implementation-report.md) · [M7 macOS ARM64开发包（旧归档：joydeck-m7-preview-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。`scripts/joydeck/verify-m7.py --binary <binary> --poppler <bin>` 检查原生全屏与输出，需要Pillow/numpy/python-pptx/pypdf/Poppler。M7历史writer为schema1.5/minReader6，旧版本拒绝新文件。


[M8表格与UI改造记录](30-m8-implementation-report.md) · `cargo run --release --locked -p joydeck -- --table-demo` 查看原创表格样例。


[M9 图表/图示与Office数据验证](31-m9-implementation-report.md) · [M10 动画/演示者与UI对照](32-m10-implementation-report.md) · [M10历史开发包（旧归档：joydeck-m10-validated-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。M10历史writer为schema1.8/minReader9；旧程序拒绝新文稿，请保留旧文件。`./joydeck --authoring-demo`展示七类基础图表、基础图示、动画和备注；默认启动为空文稿。`scripts/joydeck/verify-m9-m10.py`验证实际窗口/图表/XLSX/放映时钟。M9受支持范围与M10软件子项done，真实双屏及完整阶段门禁见STATUS和任务清单。

M11/M12检查：`cargo xtask build joydeck`、`cargo xtask build joydeck`；headless：`cargo build --locked -p joydeck-cli`。MCP锁2025-11-25，完整命令/根目录/协议与运行方式见[开发包说明](../../packaging/joydeck/README-m12.md)。当前writer为schema1.10/minReader11，旧文件/旧包请保留。
