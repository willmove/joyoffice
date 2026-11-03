# R3 文字输入、表格和剪贴板验证

2026-11-02，JoyDeck 0.4.0，Windows x86_64/MSVC，Rust 1.98.1、GPUI Kit 0.8.1 / pre 0.3.10。[使用与范围说明](../../../../packaging/joydeck/README-r3.md)。最终 release 包的基础桌面验收已完成，代码、依赖和 ZIP 与原构建一致。期间因 Windows 锁屏暂停，用户确认解锁后完成以下实际交互检查；未将锁屏中的 accessibility 树当作通过证据。

## Windows 开发包与来源

- release 开发包：`dist/joydeck/joydeck-0.4.0-r3-windows-x86_64-release-6fb3ff2b70.zip`，位置见 [包元数据](package.json)，解压运行 `joydeck.exe`。开发包位于忽略的 dist 目录，不作为全新源码检出的必需文件。原 R0/R1/R2 包保留。
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

[实际输出与哈希](outputs/SHA256.json)保留了最终 CLI 和桌面写出的公开 PPTX、v2、恢复后 PPTX 和保留未知属性的 PPTX，便于独立复核。[包内 r3-edit.json 示例](package-example.json)另已实际执行 edit/inspect/render 并由 python-pptx 读回。

## 桌面验证状态

调试切片中已实际验证：真实 PPTX 标题原位 Unicode 输入、系统拼音候选 n→ni→“你”、一次提交一次撤销；真实表格单元格与演讲备注编辑后保存，独立 python-pptx 读取正确。日志为 [文字与系统 IME](logs/desktop-runtime.txt)、[表格/备注](logs/desktop-table-runtime.txt)，均为真实进程，不是模拟调用 EntityInputHandler 冒充系统输入法。

最终 ZIP 解压到 `%LOCALAPPDATA%/Temp/joydeck-r3-final-c7x8g34r/中文 空格路径/`，校验 759 文件。首进程 28420 的自动化返回 `failed to activate captured window`；重启后截图确认 Windows 锁屏，随即停止界面操作。用户手动解锁后，在同一 release 二进制的进程 **20192** 完成实测，未改代码、依赖或开发包。27 张主窗口截图及 4 张候选附图、[结构化结果](gui-validation.json)已保存。部分画布截图未返回 accessibility，使用 sky 支持的截图坐标操作；备注对话框能返回输入控件树。未将其它应用或锁屏截图存为产品证据。

| 场景 | 实际结果与证据 |
| --- | --- |
| 真实 PPTX 原位文字 | [打开公开文稿](01-release-real-pptx.png)，[输入中文、emoji 和 e+U+0301](02-release-inline-unicode.png)，实际模型 revision 1 |
| 系统拼音提交与历史 | 物理 n/i 键出现 [真实候选](03-release-pinyin-preedit.png)，Space [提交“你”](04-release-pinyin-commit.png)后 revision 2；[一次撤销](05-release-ime-undo.png)移除整个提交，[重做](06-release-ime-redo.png)恢复 |
| 系统组合取消 | [重新组合](07-release-preedit-before-cancel.png)，[Esc 取消](08-release-composition-cancelled.png)恢复标题；下一次 [选区粗体](09-release-bold-after-cancel.png)为 revision 5，取消未新增历史 |
| 内部富文字系统剪贴板 | Ctrl+C 复制粗体标题，Ctrl+V [粘贴到另一文字对象](10-release-rich-text-paste.png)，revision 6；独立 PPTX 读取确认两处文字相同且都为粗体 |
| 单元格文字与导航 | [原位输入](11-release-table-inline.png)后，[Tab](12-release-table-tab-next.png)到 1:2，输入 `Tab OK` 确实落在第二格；[Shift+Tab](13-release-table-shift-tab.png)返回 1:1 |
| 系统 TSV 区域粘贴 | 整表复制至系统剪贴板，在新 3×3 表 [粘贴 3×2 区域](14-release-tsv-paste.png)，revision 10；[单次撤销](15-release-tsv-one-step-undo.png)清空整块、[重做](16-release-tsv-redo.png)恢复，分别 revision 11/12 |
| 表格原生 IME | 新表首格内 [真实拼音候选](17-release-table-system-ime.png)，[提交“你”](18-release-table-ime-commit.png)，revision 13；其它已粘贴单元格保持不变 |
| 备注原生 IME | JoyOffice Textarea 的 [真实候选](19-release-notes-system-ime.png)及 [提交后的多行草稿](20-release-notes-draft-committed.png)；此时文稿仍 revision 13，点击“应用备注”后才到 revision 14 |
| 保存与界面重开 | Ctrl+S [保存 PPTX](21-release-saved-pptx.png)，[另存 v2](22-release-saved-native.png)；[重开 PPTX 文字](23-release-reopened-pptx.png)、[两张表格](24-release-reopened-pptx-tables.png)和 [多行备注](25-release-reopened-notes.png)均保持，重开后新建历史 |
| 项目重开 | [重开 .joydeck 文字](26-release-reopened-native.png)和 [表格](27-release-reopened-native-tables.png)，内容保持，干净状态正常关闭窗口 |

[独立 python-pptx 读回桌面产物](gui-independent-readback.json)逐项核对原始中文/emoji/组合字符、系统 IME 的“你”、粗体粘贴目标、两张表格内容和多行备注，原输入 SHA 不变。[公开 PPTX inspect](gui-saved.inspect.json)、[v2 inspect](gui-native.inspect.json)及 [v2 转公开 PPTX 结果](outputs/gui-native-to-public.pptx)进一步复核同一实际桌面结果；[共享 CLI 渲染首面](gui-render/slide-0001.png)与 [表格页](gui-render/slide-0002.png)可直接查看。上述桌面步骤未使用模拟 IME 回调替代操作系统候选界面。

## 可运行切片

1. 纯 engine 范围/组合草稿/表格命令和 PPTX 保留写回，独立 CLI 编译运行。新集成测试覆盖 UTF-16、组合取消、格式保留、表格 TSV/合并/结构/底色、备注及保存重开。
2. 原位 EntityInputHandler 与共享文字几何、系统 IME 候选和单步 undo；Windows debug 程序启动与实际输入已验证，修复组合视图树的主线程栈预留。
3. 表格/备注/剪贴板及导航切片，独立 python-pptx 读取实际桌面保存的表格和备注。实际桌面测试发现的 Tab 通用焦点抢占，已改为 InlineText 上下文内的专用 action。

## 范围与后续

本轮支持普通水平未旋转文字和单元格，格式 UI 是选区粗体/斜体。旋转/竖排、完整滚动、字体环境变化/variation 缓存、完整格式/列表/超链接、图片/跨文稿对象及 HTML/RTF 剪贴板、矩形单元格选区和复杂焦点/IME 竞争仍未完成，不把全部 JD-300–309 标为完成。未提交组合和备注草稿不写入恢复；恢复和重开新建历史。Windows 以外原生桌面未执行。

表格单元格修改保留周围 XML；行列结构只准简单 XML 子集，未知/扩展属性拒绝公共写回，项目保留源包。编辑后超链接关系、没有可复用备注部件等情形明确拒绝 PPTX 发布，可保存 .joydeck。保存前重新解析核对，不承诺 PowerPoint 全保真。
