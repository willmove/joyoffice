# JoyWord 中文文档模板

2026-11-17，基于 `eb551e36622c32feaebcdb029d1b06abb26aea26`，分支 `feat/joyword-document-templates`。

## 实现范围

文件首页的「工作室手册（示例）」替换为「项目计划书（示例）」，正文改为原创的《社区阅读活动项目计划书》，包含目标、交付范围、进度分工表、预算、风险和验收安排。新增会议纪要、工作周报、通知公告和学习笔记四类中文模板，保留原有信函、简历和欢迎文档。

首页展示空白文档、项目计划书、会议纪要、工作周报、信函和简历；「新建」或「更多模板」展示完整九项。所有新模板采用 A4 页面、可编辑的标题样式和正文、明确的填写提示及带表头的表格。首页缩略图由对应 DOCX 的 JoyWord 首屏渲染生成。旧 handbook 资产和引擎 `sample_document()` 保留为历史测试基线，不再作为首页模板入口。

资产及 SHA-256 来源登记见 [SOURCE.json](../../../../assets/samples/SOURCE.json)。中文正文和格式由 JoyOffice 原创，生成脚本为 [make_templates.py](../../../../scripts/joyword/make_templates.py)，不复制系统字体。

## 自动验证

| 检查 | 结果 |
| --- | --- |
| `cargo xtask build joyword` | 桌面和 CLI 构建通过 |
| `cargo test --locked -p joyword-ui-gpui --lib` | 23 项通过 |
| `gallery_documents_open_edit_and_roundtrip_with_their_tables` | 对目录中的全部八份 DOCX 解码缩略图、加载独立会话、编辑、导出 DOCX 再读回，正文和表格数量保留，无原文件路径关联 |
| `cargo fmt -p joyword-ui-gpui -- --check` | 通过 |
| `cargo clippy --locked -p joyword-ui-gpui --lib --bins -- -D warnings` | 通过 |
| `python3 scripts/check_docs.py` | 通过，含资产 SHA-256 核对 |
| 模板脚本重复生成 | 五份 DOCX 字节一致 |
| 独立渲染 | bundled LibreOffice 的 `render_docx.py` 与 JoyWord CLI 均生成五份单页文档，逐页检查中文、标题、表格和页末，无截断或多余页 |

使用 Rust 1.98.1。独立 LibreOffice 需要在临时 fontconfig 中将 Microsoft YaHei 映射到本机 PingFang SC，并加入系统 Font8 字体目录；仅影响验证进程，没有修改应用或系统字体配置。初次未配置字体的渲染缺字，不计入通过证据。

重新生成需要 Python `python-docx==1.2.0`，运行：

```sh
cargo xtask build joyword
python3 scripts/joyword/make_templates.py --cli target/debug/joyword-cli
python3 scripts/check_docs.py
```

## macOS 桌面验证

在 macOS ARM64 以独立 `JoyWord Templates.app` 和临时配置目录验证，不使用已有编辑会话。已点击创建全部五份中文模板，确认窗口标题、正文标题和表格显示正确。

- [首页六项模板](home.png)
- [完整九项模板及自动换行](gallery.png)
- [项目计划书正文](project-plan.png)
- [会议纪要正文](meeting-minutes.png)
- [工作周报正文](weekly-report.png)
- [通知公告正文](notice.png)
- [学习笔记正文](study-notes.png)

本次实现与验证限于模板内容、模板目录及静态缩略图。未做 Windows、Linux、信创真机、其他字体组合、屏幕阅读器、实际打印或 Word/WPS 人工往返验收；没有修改编辑引擎、排版算法或历史样例，不将这些范围标记为完成。
