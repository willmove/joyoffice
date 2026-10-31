# JoySheet

Rust + GPUI 电子表格应用，现接入 JoyOffice Monorepo。应用和 CLI 版本为 **0.1.0**，功能基线为 **M8-preview6**；全量迁移与人工验收仍按 [状态](STATUS.md)和[任务矩阵](MIGRATION.md)跟踪。本次工程整合不关闭原有未完成里程碑。

计算、工作簿、公式、XLSX/ODS/XLS/XLSB、图表、PDF、引擎与 MCP 来源于 GridCraft 0.12.1，已机械更名为产品包名。原始版权、双许可证、来源哈希和必要适配记录保留，参见 [ATTRIBUTION](ATTRIBUTION.md)、[原始来源](upstream-source.json)和[核心适配](UPSTREAM-PATCHES.md)。

## 开发入口

在含本次迁移提交的 JoyOffice 检出目录开发。合入后原独立目录仅作恢复材料；源码、构建与分发不依赖旧目录或本机归档。

固定 Rust 1.98.1、GPUI Kit 0.8.1 和 GPUI pre 0.3.10；使用根 Cargo.lock。Windows 使用 PowerShell 7、Visual Studio C++/SDK 与 CMake：

```powershell
pwsh -File scripts/joysheet/dev.ps1 run --locked -p joysheet -- --sample sales
pwsh -File scripts/joysheet/dev.ps1 xtask check joysheet --validation
pwsh -File scripts/joysheet/dev.ps1 xtask smoke joysheet --stage M8
pwsh -File scripts/joysheet/dev.ps1 xtask package joysheet --release --validation
```

配置好的开发终端及 macOS/Linux 使用根工程入口：

```text
cargo xtask check joysheet
cargo xtask check joysheet --headless
cargo xtask test joysheet --validation
cargo xtask build joysheet --release
cargo xtask smoke joysheet --stage M8 --engine-only
cargo xtask wasm joysheet
cargo xtask inventory joysheet
cargo xtask bench joysheet
cargo xtask package joysheet --release
```

`check` 覆盖产品全部所属包和实际依赖的公共库，执行 fmt、严格 Clippy 和测试。`--headless` 排除应用、GPUI UI 与原生平台适配器，并检查普通/构建依赖闭包不含 GPUI。CLI 不依赖 GUI、Word 或 Deck。

`--validation` 显式启用原 automation feature 的 GPU 截图能力；普通桌面和普通发布包不启用该 feature。控制服务仍需显式传入 `--control PORT`，既有 token、请求预算与脚本信任语义保持。

## 分发和证据

包输出到 `dist/joysheet/`，包含所属二进制、资源、平台依赖许可、SBOM、源码哈希和构建信息。源码 ZIP 生成 Sheet 专属 workspace，并验证裁剪后的锁文件保留原依赖版本及校验和；完整 Monorepo 门禁仍在 Git 仓库运行。

Windows 原生打印对话框使用 PowerShell 7；系统字体不分发。macOS/Linux 的内置 PDF 预览沿用 Poppler（`pdftoppm`），打印使用系统查看器；Linux deb 声明相应运行依赖。便携包可移动运行，安装器提供隔离安装/卸载测试。签名、公证、三平台原生、真实 IME、多显示器、读屏、GPU 呈现及物理打印等开放项详见 [Windows 质量记录](WINDOWS-QUALITY.md)和[历史验证](VALIDATION.md)。

当前源码检查点由 `pwsh -File scripts/joysheet/checkpoint.ps1` 生成于 `artifacts/joysheet/source-checkpoint.json`；旧检查点及原证据保持历史含义。本次基线、映射和新验证见 [整合记录](../joyoffice/JOYSHEET-INTEGRATION.md)，旧开发包和日志见 [历史产物](LEGACY-ARTIFACTS.md)。
