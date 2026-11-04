# JoyWord R8 验证

2026-11-04 在 JoyOffice 的 `codex/joyword-r8-integration` 工作树执行。测试输入均为仓库自带样例或其副本。平台结果按本次集成重新记录。

## Windows x64

三个原生应用和 CLI 从同一工作区联合编译。三个便携开发包分别解压，桌面应用同时运行。最终归档中的 GUI 可执行文件与执行桌面重开检查的文件哈希一致。

| 应用 | 桌面流程 | 保存及重开结果 |
| --- | --- | --- |
| JoyWord 0.1.1 | 打开含中文、表格和图片的 DOCX，编辑标题并保存测试副本 | 重开显示新中文内容。独立检查 `word/document.xml`、表格和媒体部件均保留。使用仅有旧配置的隔离目录重开，读取 `ui.locale=en`。 |
| JoySheet 0.1.0 | 打开 XLSX，在 C2 输入 `=SUM(800,8)`，计算并保存 | 重开显示 808。独立检查 `sheet1.xml` 同时含原公式和计算值。 |
| JoyDeck 0.8.0 | 打开三页 PPTX，切到第二页，复制幻灯片并保存 | 重开显示四页。独立检查演示文稿 XML 中的页数为四。 |

[Windows 归档与校验值](evidence/windows-archives.json) · [桌面编辑的 OOXML 读回](evidence/windows-ooxml.json) · [测试统计](evidence/test-summary.json) · [依赖核查](evidence/dependency-audit.json)

JoyWord 的格式检查、生产 Clippy 与测试通过，3482 个用例通过，2 个忽略。JoySheet 的引擎／XLSX 和 JoyDeck 的外壳／PPTX 共 962 个回归用例通过。Python 工具测试 152 项，其中 2 项显式跳过旧实现专用验证。当前文档、组件边界、固定依赖和负例门禁通过。

Windows 调试包首次检查发现 JoyWord CLI 主线程栈溢出。修正后从最终包运行 DOCX 往返、PNG 和 PDF 导出、拒绝覆盖及损坏输入，64 个变体均正常解析或干净拒绝。三个 Windows 归档都核对了 SHA-256 和完整性。

## Linux x64

Ubuntu WSL2 联合构建三应用及 CLI。使用 WSLg 和 Mesa 软件 Vulkan 三应用同时启动，JoyWord 完成 DOCX 首屏绘制，JoySheet 初始化工作簿窗口，JoyDeck 打开三页 PPTX。此处覆盖启动与基本显示，Windows 表中的交互编辑验证单独记录。

三个解压后的 Linux 开发包通过 ELF、动态链接与 SHA-256 检查，并在 WSLg 同时启动。JoyWord 绘制已保存 DOCX 的首屏，JoySheet 初始化工作簿窗口，JoyDeck 打开四页 PPTX。JoyWord CLI 通过 DOCX 编辑、保存、重开、PNG 和 PDF 输出及 64 个损坏输入变体。JoySheet CLI 读回 C2 的 808，JoyDeck CLI 读回四张幻灯片。最终 JoyDeck 归档中的 GUI 和 CLI 哈希与冒烟测试文件一致。[Linux 归档与验证结果](evidence/linux-archives.json)

JoySheet 的 Debian 打包在 WSL 的 ext4 内完成，保留 Debian 控制目录权限。使用 `cargo xtask package joysheet --output <ext4 目标目录>` 可复现该路径。Debian 控制元数据为 `joysheet 0.1.0 amd64`。两个平台的 JoySheet 源码包都保留了工作区使用的 AccessKit 路径补丁。

## 待验

- macOS 原生构建与桌面运行按约定待验。集成后的 Linux ARM64 原生桌面、信创真机与托管 CI 待验。
- Windows 拼音和读屏沿用独立版证据，R8 包复验、完整 Word/WPS 互通、安装器安装／卸载与文件关联见 [交接任务](../../joyoffice/JOYWORD-R8-HANDOFF.md)。
- 旧恢复文件的兼容、固定金样和覆盖引导 fuzz 的迁移，以及旧 CLI/API 和 Agent 的差距见交接中的接口表。
- JoyDeck 开发包的清单列出了未自动收集到的依赖许可文本，发布验收时仍需补齐对应记录。

