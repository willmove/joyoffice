# JoyDeck 0.8.0 · R8 Windows 开发包

运行 `joydeck.exe` 新建演示文稿，或将 PPTX / .joydeck / .deckcraft 拖到 EXE，或使用 Ctrl+O 打开。

本版以本机 DeckCraft 为主要界面参照，采用原生 GPUI Ribbon、灰白工作区、橙色选页边框、缩略图和备注区。字体/段落、图片/形状/表格、保存和另存、撤销重做、放映及输出沿用真实模型与事务。初次保存打开系统“另存为”对话框；默认新项目为 .joydeck，也可明确输入 .pptx。保存冲突或不支持的保留写回会报告错误并保留原件。

常用快捷键：Ctrl+O 打开、Ctrl+N 新建、Ctrl+S 保存、Ctrl+Shift+S 另存、Ctrl+Z/Y 撤销/重做、Ctrl+C/X/V 剪贴板、Ctrl+D 对象副本、Ctrl+M 新页、方向键移动选中对象（Shift 10 磅）、Enter 编辑选中文字、双击编辑文字/表格、F5 从头放映、Shift+F5 从当前页放映、Esc 退出。Tab/Shift+Tab 在表格中导航。窄窗口可使用功能区左右箭头，诊断在“视图”内。

对象剪贴板支持形状、文字、嵌入位图、连接线及基础表格；复杂组、未知扩展、外链资源会明确拒绝复制，剪切失败不删除原对象。PNG/JPEG 等系统图片按相同预算转为 PNG。文本编辑中保留文字范围与富文字剪贴板。

此包尚未达到 DeckCraft 或 PowerPoint 的完整功能/像素一致：完整形状库、动画编辑、绘图、图表插入、SmartArt、评论审阅、多页排序和母版编辑等仍需继续迁移；未提供空成功的入口。使用前请查看源码仓库 docs/joydeck/UI-PARITY.md。现有播放/导出/CLI/MCP 范围沿用 R6/R7 声明。

Windows x64 MSVC 开发版本，未签名。不包含系统字体。媒体、MP4、打印 helper 位于应用同目录；打印取消不会提交作业。正式发行签名/关联/升级、macOS/Linux/ARM/信创真机资格尚未完成。本包可与既有版本并存；删除独立包不删除文稿和恢复文件。

CLI：`joydeck-cli.exe --help`。样例在 samples；来源/许可见 THIRD_PARTY.md、licenses、sbom.json，文件指纹见 SHA256SUMS、source.json。源 JoyOffice/DeckCraft 工作目录均未作为运行依赖。
