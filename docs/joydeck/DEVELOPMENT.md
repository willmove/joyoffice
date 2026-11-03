# JoyDeck 开发入口（R9）

只在 JoyOffice 仓库开发，原 joydeck-new 目录退出开发。先读 [交接](../joyoffice/JOYDECK-R9-HANDOFF.md)、[状态](STATUS.md)、[任务](TASKS.md)、[界面对照](UI-PARITY.md)。

```powershell
cargo fetch --locked # fresh checkout before offline dependency gates
cargo xtask check joydeck --headless
cargo xtask check joydeck
cargo xtask run joydeck
cargo xtask package joydeck --release
python scripts/joydeck/check_sources.py
python scripts/joydeck/check_dependencies.py
python scripts/check_gates.py
python scripts/check_docs.py
```

Windows 使用 PowerShell 7；`scripts/joydeck/dev.ps1` 和产品工具会定位 VS 的 CMake。修改根锁、feature 或公共库时同时验证 Word/Sheet 与联合 workspace。平台辅助程序通过 `build-windows-helpers.py` 构建，开发运行由产品入口准备；包中已包含。应用和 CLI 版本均为 0.8.0，内部库继承宿主元数据；其余产品不随 Deck 升版。

旧 M* 脚本/报告保留历史含义，不可作为本次产品检查。可用的真实 CLI 回归为 `verify-r1.py`、`verify-r2.py`、`verify-r3.py`、`verify-r4-r5.py`、`verify-r6-r7.py`；见 [R9 验证](evidence/R9/README.md)。
