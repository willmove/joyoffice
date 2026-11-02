# R3 文字输入、表格和剪贴板验证

2026-11-02，JoyDeck 0.4.0，Windows x86_64/MSVC，Rust 1.98.1、GPUI Kit 0.8.1 / pre 0.3.10。[使用与范围说明](../../../../packaging/joydeck/README-r3.md)。最终包已生成并完成无界面验证；Windows 锁屏阻止了最终包交互验收，待用户解锁后继续，不据锁屏中的 accessibility 树宣称桌面验证通过。

## Windows 开发包与来源

- [release 开发包](../../../../dist/joydeck/joydeck-0.4.0-r3-windows-x86_64-release-6fb3ff2b70.zip)，解压运行 `joydeck.exe`。原 R0/R1/R2 包保留。
- ZIP SHA-256：`f4c9c8cba4e647dab2a1c0fed46b65b71ce4445431289566b6ce9109954ed5c6`。
- 生产源码指纹：`6fb3ff2b709b1d358b5ff933e796d90e96f58c7d071c7f508b97f7ff9246be1e`，与当前源码逐文件一致。
- GUI SHA-256：`3a4dfe7c2b53412e4f41fa0f3866167e8247b68c92e096c1cb05ec01cdbab454`。
- CLI SHA-256：`46ec18711deecb580640c9a3967038861777c347e9f9d2341cc9eb7d24546fd0`。
- [构建日志](logs/package-final.txt)、[包元数据](package.json)、[仓库外启动和 759 个文件校验](standalone-launch.json)、[验证摘要](verification.json)。实际 PE64 栈预留核对为 8 MiB。
- 没有新增 registry 依赖；SBOM 446 项，原有 21 项 registry crate 缺少随 crate 附带的完整许可文本，明细随包。未签名开发构建；正式分发审计属于 R8。211 条来源记录通过；DeckCraft 原快照 755 文件 SHA 未变，JoyOffice 固定检出保持干净。

## 完整门禁与 CLI 实测

[完整产品](logs/product-check.txt)通过 **847 项 Rust + 5 项 Python 测试**，[无界面](logs/headless-check.txt)通过 **846 项 Rust + 5 项 Python 测试**，fmt、严格 Clippy、依赖/来源/文档门禁均通过。两个配置有重复测试，不相加为独立测试数。新增 [8 项作者操作集成测试](logs/slice2-check.txt)覆盖组合取消/失败原子性、非 BMP/组合字符、保留 run/段落、公开 PPTX 表格/备注/粗斜体及共享光标几何。

[开发 CLI](cli-verification.json)及 [最终包 R3 CLI](cli-release-verification.json)每轮 **20 次实际调用**：公开 PPTX 及 v2 重开、粗体/斜体、Unicode/希伯来文/组合字符、表格 TSV/合并/底色/结构、备注、PNG、原件不变、未触及 ZIP 成员字节一致；未知单元格属性保留，未知结构写回拒绝且无输出；强制终止探针进程后恢复富文字、表格和备注，再转回公开 PPTX。使用独立 python-pptx 1.0.2 读回实际结果，未用私有快照冒充公开格式验证。

[最终包 R2 回归](r2-regression-release.json)另 **20 次调用**通过，包括新文稿、原文件/已有输出保护、故障与未来恢复记录隔离和真实强制终止恢复。复用原 `verify-r2.py`，只在内存中将 `diagnostics.stage` 的期望值从 R2 改为 R3，功能断言不变；原脚本未修改。起初未经该阶段调整的运行停在最后的版本断言，见 [首次日志](logs/cli-release-verification.txt)；[调整后完整通过日志](logs/r2-regression-release.txt)。

[实际输出与哈希](outputs/SHA256.json)保留了最终 CLI 写出的公开 PPTX、v2、恢复后 PPTX 和保留未知属性的 PPTX，便于独立复核。

## 桌面验证状态

调试切片中已实际验证：真实 PPTX 标题原位 Unicode 输入、系统拼音候选 n→ni→“你”、一次提交一次撤销；真实表格单元格与演讲备注编辑后保存，独立 python-pptx 读取正确。日志为 [文字与系统 IME](logs/desktop-runtime.txt)、[表格/备注](logs/desktop-table-runtime.txt)，均为真实进程，不是模拟调用 EntityInputHandler 冒充系统输入法。

最终 ZIP 解压到 `%LOCALAPPDATA%/Temp/joydeck-r3-final-c7x8g34r/中文 空格路径/`，校验 759 文件，并从该目录启动。首进程 28420 成功读取 PPTX、完成后台光栅且 Responding=True，但自动化连续返回 `failed to activate captured window`；重启后实际截图确认 Windows 处于锁屏。已按 computer-use 技能停止界面操作并请求用户手动解锁，未保存锁屏截图为产品证据。

**待解锁验收**：最终 release 的原位/候选/取消/undo/redo、修复后的 Tab/Shift+Tab、内部富文字与系统 TSV 剪贴板、表格/备注保存重开及截图。当前不能把这些最终包交互场景写成通过。

## 可运行切片

1. 纯 engine 范围/组合草稿/表格命令和 PPTX 保留写回，独立 CLI 编译运行。新集成测试覆盖 UTF-16、组合取消、格式保留、表格 TSV/合并/结构/底色、备注及保存重开。
2. 原位 EntityInputHandler 与共享文字几何、系统 IME 候选和单步 undo；Windows debug 程序启动与实际输入已验证，修复组合视图树的主线程栈预留。
3. 表格/备注/剪贴板及导航切片，独立 python-pptx 读取实际桌面保存的表格和备注。实际桌面测试发现的 Tab 通用焦点抢占，已改为 InlineText 上下文内的专用 action。

## 范围与后续

本轮支持普通水平未旋转文字和单元格，格式 UI 是选区粗体/斜体。旋转/竖排、完整滚动、字体环境变化/variation 缓存、完整格式/列表/超链接、图片/跨文稿对象及 HTML/RTF 剪贴板、矩形单元格选区和复杂焦点/IME 竞争仍未完成，不把全部 JD-300–309 标为完成。未提交组合和备注草稿不写入恢复；恢复和重开新建历史。Windows 以外原生桌面未执行。

表格单元格修改保留周围 XML；行列结构只准简单 XML 子集，未知/扩展属性拒绝公共写回，项目保留源包。编辑后超链接关系、没有可复用备注部件等情形明确拒绝 PPTX 发布，可保存 .joydeck。保存前重新解析核对，不承诺 PowerPoint 全保真。
