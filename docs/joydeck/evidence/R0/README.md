# R0 Windows 原生开发切片

2026-11-02。JoyOffice 来源 `a973f75d7d58ab8dd5f05b0e3db5f0c53137d57d`。本阶段复用记录在 `licenses/joyoffice/source-map.json`；DeckCraft 本机来源的 755 文件清单已固定，尚未导入其核心源文件。

## 构建和检查

| 检查 | 实际结果 |
| --- | --- |
| `cargo xtask check joydeck` | 文档、依赖/CLI 隔离、导入文件指纹、fmt、严格 Clippy、3 项 Rust 测试及 5 项 Python 回归通过 |
| `cargo xtask check joydeck --headless` | CLI、shell、xtask 独立检查通过，CLI 无 GPUI/其它产品依赖 |
| release GUI / CLI 与包装 | 当前源码构建、406 个 normal/build 包 SBOM、内容校验和、有效 ZIP |
| GPUI provenance | 21 个包：16 个逐文件相同，5 个符合宿主固定审阅差异，0 失败 |
| 依赖实验 | 同一字体 face 的 skrifa/harfrust、Krilla 字体接受与 PDF writer、ZIP 往返、Vello CPU 矩形像素通过 |
| JoyOffice 临时宿主 | 隔离 clone 内新 GUI/CLI 使用宿主锁文件和公共 UI 编译通过；旧应用 facade 随替换移除 |

字体实验使用本机宋体 face 0，不复制该系统字体到源码或包中。实验只验证已调用接口，完整文字排版、GPUI 字体身份映射及 PDF 保真仍属后续阶段。

首次 ZIP 包装发现依赖文件的 epoch 时间戳早于 ZIP 支持范围，已加入兼容处理与回归测试；未完成 ZIP 不再以最终文件名发布。失败输出与本次成功产物分开保存，使用 [package.json](package.json) 标识的开发包。

## 原生窗口

初始 debug 包覆盖工作区、翻页、深色主题、备注控件、系统拼音候选、提交、撤销及关于对话框；截图 `01`–`07` 记录对应场景。文本注入工具的 Unicode packet 不能用于证明系统键盘输入，因此改用实际按键触发拼音候选，记录中文提交与撤销；没有据此声明 emoji 或完整 IME 矩阵通过。

最终 release 包从独立的中文空格临时目录启动，使用 `--window-size=980x800` 核对紧凑布局；CLI 在同目录执行 diagnostics，未知参数正确返回失败。原始日志在 `logs/`，实际可执行文件 hash 与位置见 [standalone-launch.json](standalone-launch.json)。

开发包未签名，SBOM 已明确列出少量尚缺独立许可正文的依赖，正式再分发审查在 R8 继续完成。本轮没有公开发布或推送到远端。

## 未覆盖范围

macOS/Linux 原生编译/运行、远端 CI、真实文稿读取/编辑、GPUI 文稿场景、完整 IME/读屏/字体/多屏矩阵、端到端性能与正式发行资格尚未完成。宿主演练只验证 R0 GUI/CLI 编译，不是现有 JoyDeck 全功能替换或 Word/Sheet 联合回归。

## 任务验收复核

2026-11-05 复核 JD-007。临时宿主使用隔离 clone 和锁文件成功编译 GUI/CLI，原仓库保留。[宿主演练结果](host-check.json)；当前 JoyOffice 中的 JoyDeck CI 已覆盖 Windows、macOS、Linux x64 与 ARM64 的锁定构建和验证特性测试，另有 headless 来源、依赖与 CLI 隔离检查。PR #125 在合并提交 `edd627f` 前的 20 项检查全绿。平台实屏资格仍由 R8 对应任务记录。[CI 记录](https://github.com/willmove/joyoffice/actions/runs/41975304897)

JD-008 的原 R0 Windows Release 包在仓库外中文空格路径启动，保留归档散列、二进制散列与紧凑窗口截图。R0 Debug 窗口记录了导航、明暗主题和面板。这些场景覆盖当时的公共 UI 外壳。[包记录](package.json) · [移位启动](standalone-launch.json) · [Release 窗口](09-final-release-980.png) · [导航](02-navigation.png) · [暗色](03-workspace-dark.png) · [关于面板](07-about.png)
