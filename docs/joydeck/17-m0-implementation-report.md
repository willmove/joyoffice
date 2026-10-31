# M0 Windows 技术预览实施记录

日期：2026-10-29。当前交付为真实 GPUI Windows 原生应用，M0 完整跨平台验收仍待补齐；这是 M0 交付时的历史记录；M1 当前进展见 [19](19-m1-implementation-report.md)。

## 构建和运行

- 工具链：Rust/Cargo 1.98.1，x86_64-pc-windows-msvc，锁定 rust-toolchain.toml 与 Cargo.lock。
- UI：gpui-kit 0.8.1，匹配 gpui-pre 0.3.10，默认 Component/Assets；未启用 Fast，UI 与领域边界清晰。
- 本机：Windows 11 Pro 10.0.26300，Intel Core Ultra 9 285H；已安装 Intel Arc 140T、NVIDIA RTX 5060 Laptop 与虚拟显示适配器。未测定实际渲染设备，不能据此宣称已覆盖这些 GPU。
- 实际通过：`cargo build --locked -p joydeck`、`cargo xtask check`、`cargo xtask stage M0`。check 包括 fmt、Clippy -D warnings、workspace 测试；两个取消/重启 token 测试通过。
- `joydeck.exe --version` 返回 0.1.0 / Kit 0.8.1 / pre 0.3.10。Windows 原生窗口实际启动且响应，截图的 DPI scale 为 1.25。

当前启动诊断入口：`cargo run --locked -p joydeck -- --diagnostics`；默认 run 已切换到 M1 查看器。M0 原入口：`pwsh.exe -File scripts/dev.ps1 run`；构建用 build，检查用 check，打包用 package。脚本在 PATH 缺少 CMake 时从 Visual Studio 安装信息中查找，不把本机绝对路径写入跨平台 Cargo 配置。已具备 CMake/MSVC 环境时直接运行上述 cargo 命令。

## 交付内容

诊断页面有原生 Input、主题/语言切换、下拉菜单、关于弹窗、自绘路径、CPU BGRA/alpha 图像、缩放/裁剪、第二窗口和可取消后台任务。窗口各持有输入、任务及取消 token；关闭视图会取消任务，弱 Entity 防止关闭后投递。

`pwsh.exe -File scripts/smoke.ps1 -LeaveRunning` 在真实原生窗口中调用生产 Entity/input handler，检查中文+emoji 提交、UTF-16 替换、预编辑取消、弹窗、第二窗口关闭和任务完成。它不是模拟物理键盘或 OS 输入法。去掉 LeaveRunning 则检查后关闭测试进程。

输入使用 Kit InputState 的 EntityInputHandler：replace_and_mark_text_in_range、replace_text_in_range，范围为 UTF-16。M0 实测将“中文😀”的 2..4 范围替换为“✅”，结果为“中文✅”；组合文本取消后为空。此接口验证不能替代 Windows 中文输入法候选定位、物理键盘和完整 M3 画布文字验收。

绘图直接使用 GPUI PathBuilder/window.paint_path；RenderImage 通过 image::Frame 承载 BGRA 数据，不把 RGBA 字节顺序直接当作 GPUI 输入。无外部 GPU texture 共享实验，未据此宣称 M4 画布加速已完成。

## 产物和证据

- 可运行目录：`dist/joydeck-m0-windows-x64/`，含 EXE、使用说明、许可证及 SHA256。已从分发目录启动EXE并通过相同smoke，窗口响应。另提供同名ZIP。这是未签名开发构建，不是正式发布安装包。
- 构建/检查日志：`logs/build.txt`、`logs/check.txt`、`logs/stage-m0.txt`；原生 smoke 日志见 `logs/smoke.stderr.log`。
- 保留证据：[build](evidence/m0/build.txt)、[check](evidence/m0/check.txt)、[stage M0](evidence/m0/stage-m0.txt)、[分发EXE的native smoke](evidence/m0/native-smoke.txt)。EXE SHA256：`FC25DF5410346121140C8AE3A1D00D1710CE34542696584DB004E28031BEF2AF`。
- [实际窗口截图](evidence/m0/windows-startup.png)。
- CI 定义：`.github/workflows/ci.yml`，Windows、macOS 15、Ubuntu 24.04 原生构建/检查；当前未推送、未执行远端工作流。

## 未通过完整验收的项目

| 项目 | 状态与后续任务 |
|---|---|
| 真实 IME 预编辑/候选定位、Tab/快捷键/菜单指针实操 | 输入桥 smoke 通过，OS 人机操作待 T-M0-04/08；M3 再接画布 |
| 跨显示器移动及多个 DPI | 仅 1.25 环境有截图；T-M0-05/07/08 补实测 |
| macOS/Linux 构建和弹窗 | CI 已定义但未执行，T-M0-07 保持未完成 |
| 第二窗口独立焦点与关闭时任务 | 关闭/生命周期原生 smoke 通过，物理焦点实操待 T-M0-06/08 |
| GPU 性能/外部纹理互操作 | 本阶段仅建立基线，后续 T-M4-01 起验证 |

阶段结论：Windows 可运行预览已交付，M0 保持“待完整验收”。补齐上述证据前不把整阶段标记完成，不将单机成功推广为三平台通过。
