# 替换前工具与证据

M0–M14 的报告、原生验收工具、`verify-m*.py` / `smoke-m*.ps1`、`verify-unified-ui.py`、旧更新/偏好/比较相关工具保留历史用途。它们针对旧 DTO、UI 或协议，不能用来声称新实现通过，也不能把它们的错误当作已经自动适配。

当前工程入口是宿主 `cargo xtask ... joydeck` 和 `dev.ps1`；当前 CLI 行为回归是 `verify-r1.py` 至 `verify-r6-r7.py`。`sbom.py` 和 `paths.py` 保持产品中立图/路径支持，Word/Sheet 打包仍使用它们；交付工具单测继续覆盖。`diagnose-playback.py` 与对应手动 CI 已改用当前 shell 测试目标。

旧软件的完整代码/历史与恢复材料见 [R9 交接](../joyoffice/JOYDECK-R9-HANDOFF.md)。迁移缺项单列，不因历史工具仍在仓库而视为仍受支持。
