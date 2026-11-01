# DOCX UnsupportedBody 打开失败修复

2026-11-01，Windows x86_64、Rust 1.98.1。任务分支 `codex/joyword-open-rich-documents`，基于 main `5c0dd367`。本记录不修改里程碑或完整格式验收状态。

## 原因与处理

用户提供的五份本机 DOCX 均可完成容器及 WordprocessingML 导入，但每份有 17–18 张表超出当前可编辑表格子集，整表成为保留原始 XML 的 `Block::Opaque`；其中一份还包含批注 story 和三个保留行内对象。桌面的 `TextIndex::new_for_story` 拒绝这些结构，`Office::install_bound` 将 `UnsupportedBody` 包装为打开失败。原生窗口复现了“输入未完成：无法打开：text input: UnsupportedBody”。

新增有界的正文文字投影。当 DOCX 输入索引明确返回 `UnsupportedBody` 时，使用独立的显示文档进入**只读兼容预览**，保留原导入模型、包、路径、文件锁及保存绑定。普通受支持 DOCX 仍走原编辑路径。预算错误、无效模型及已经修改的来源不进入该回退。

- 预览按正文顺序展示段落，表格按行和单元格展开，标记 `[Table]`、`[Cell]`；手动换行成为预览段落，制表符成为空格。
- 图片、未知对象和未选择分支的兼容内容显示为 `[Object]`，不猜测其内容。页眉页脚、批注、脚注和其他 story 不在正文预览中。
- 忽略删除／moveFrom 修订和隐藏 run；字符及段落样式中的 `vanish` 通过已有样式解析器判断。字段仅显示存储结果，不计算指令，不访问链接或其他外部目标。
- 复用 DOCX 的有界 XML 读取器，并保持段落数、段落字节、总文本及 run 数预算；失败不返回截断成功。
- 显示持续的只读提示。键入、IME、编辑命令和撤销重做不得写入来源。查找、选择和复制使用显示索引，另存副本使用原始后端。
- 禁用保存原件、PDF／PDF/A／EPUB、打印及邮件合并等可能误用简化显示的入口；菜单状态同步可用性。在命令注册表索取缺失参数之前检查预览限制，避免被禁命令先弹出路径框。

投影依据 ECMA-376 Part 1 §17.2、§17.3、§17.4、§17.5、§17.13.5，以及 Microsoft 的 [WordprocessingML 表格结构说明](https://learn.microsoft.com/en-us/office/open-xml/word/how-to-change-text-in-a-table-in-a-word-processing-document) 独立实现。没有读取 Office 编辑器实现源码，没有改变依赖或锁文件。

## 自动验证

新增六项回归：`crates/joyword-shell/tests/docx_preview.rs` 的五项检查覆盖不透明表格打开、表格文字查找和 UTF-16 选区、只读／IME／命令拒绝、原模型不变、原始副本逐字节相同、隐藏文字／删除修订／字段／内容控件、输入及 XML 预算、普通 DOCX 编辑与撤销；应用测试 `preserved_docx_body_installs_a_read_only_preview_that_reaches_the_page_scene` 覆盖 `Office` 打开到实际页面场景的路径。

| 命令 | 结果 |
| --- | --- |
| `cargo xtask check joyword` | 产品 fmt、严格 Clippy、1,261 项测试通过，0 失败、0 忽略 |
| `cargo xtask build joyword` | 默认 feature 的应用及 CLI 构建通过 |
| `python scripts/check_gates.py` | 分层、文件系统及 no-panic 等门禁通过 |
| `python scripts/check_docs.py` | 文档检查通过 |

Windows 的 Python 3 命令为 `python`。日志、二进制散列和本机样例的打开探针保留在工作区忽略目录 `target/joyword-open-diagnostics/`，不把重复执行次数累加为新增覆盖。

## 原生 GUI 验证

在本机运行真实 GPUI 窗口，使用 `computer-use` 技能操作 Windows 文件对话框、查找栏和正文画布；未使用替代界面。

1. 基线构建打开用户指定的 B 文档，复现 `UnsupportedBody`。
2. 修复后分别打开 A、B、C、修订版、章节整合版五份 DOCX，首屏正文与只读提示均显示。
3. B 文档查找 `[Table]`，返回 17 处匹配，页面跳转及滚动后可见展开的单元格文字。
4. 在预览正文尝试输入验证标记，正文前后相同，未进入未保存状态。
5. GUI 另存 B 文档副本，副本与原文件 SHA-256 一致；原始五份 DOCX 的前后 SHA-256 均一致。
6. 最终构建检查文件菜单，预览中的原件保存和简化输出入口显示为禁用；另存原始 DOCX 副本仍可用。Windows 的 PDF 导出快捷键 `Ctrl+Shift+E` 被本机 PixPin 全局快捷键截获，本次不认定该原生快捷键路径通过；没有更改系统或 PixPin 设置。

私有文档、文件名清单、原文及截图均仅留在上述本地忽略目录，不进入仓库。此处是原生 GUI 自动操作证据，不等于完整人工验收或 Word 版式对照。

## 边界

这次解决的是复杂 DOCX 被桌面直接拒绝、无法阅读的问题。只读文字预览不等于这些表格、图片、批注及其他复杂结构已可编辑或按 Word 原版式显示；不承诺预览页码与 Word 页码一致。当前只对 `UnsupportedBody` 启用回退，其他独立排版错误仍按既有诊断处理。没有扩展 DOC／Strict OOXML 的格式识别或导入范围。

没有执行 macOS／Linux、信创真机、实际输入法、物理键盘／读屏、完整格式往返或正式发布验收；不以本次测试替代这些项目。
