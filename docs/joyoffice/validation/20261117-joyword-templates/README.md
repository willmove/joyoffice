# JoyWord 文档模板与编辑页面缓存

2026-11-17，本轮基于 `f8b2f100e8128080a17a0154ff19147225532792`，分支 `feat/joyword-localized-templates-loading`。

## 中英文模板与编辑提示更新

- 文件首页和完整新建页按界面语言选择两套九项模板。项目计划书、会议纪要、工作周报、通知公告和学习笔记新增英文正文与缩略图；信函、简历新增中文正文与缩略图；欢迎文档新增英文正文与缩略图。切换语言只改变后续新建入口，不改写已经打开的文档。
- 英文偏好下的首次启动欢迎文档也改用英文资源，不再绕过模板语言规则。
- 原实现每次编辑先移除变化页的缓存位图，再异步栅格化，因此重绘间隙显示「页面载入中…」。现改为按文档代次标记旧图，旧图持续显示到新图完成后再替换；单页变化不会重绘其他未变化页。缓存替换会扣除旧图像素并释放旧图，性能计时只统计当前代次的新帧。
- 新增八份 DOCX 和八张对应首屏缩略图，资产散列与来源登记在 [SOURCE.json](../../../../assets/samples/SOURCE.json)，生成脚本仍为一式可复跑的 [make_templates.py](../../../../scripts/joyword/make_templates.py)。

## 本次自动验证

| 检查 | 结果 |
| --- | --- |
| `cargo test --locked -p joyword-ui-gpui --lib` | 31 项通过 |
| `gallery_document_bodies_match_the_interface_language` | 八组成对模板的正文分别核对中文和英文标志文本 |
| `gallery_documents_open_edit_and_roundtrip_with_their_tables` | 两种界面语言共 16 份 DOCX 均解码缩略图、加载独立会话、编辑、导出 DOCX 再读回，正文和表格数量保留，无原文件路径关联 |
| `stale_page_remains_available_until_the_current_epoch_is_rendered` | 代次前进后旧页仍可显示，同时标记为等待当前代次新帧 |
| `cargo clippy --locked -p joyword-ui-gpui --lib --bins -- -D warnings` | 通过 |
| `cargo fmt -p joyword-ui-gpui -- --check` | 通过 |
| `cargo xtask package joyword --out …` | macOS ARM64 桌面和 CLI 调试包通过，来源快照未发生变化 |
| 模板脚本重复生成 | 13 份生成式中英文 DOCX 字节一致；八份新增文档均由 JoyWord CLI 确认为单页并生成缩略图 |
| `python3 scripts/check_docs.py` | 通过，含资产 SHA-256 核对 |

一次较早的打包因构建期间继续修改缓存实现而被来源快照门禁拒绝；随后在源码固定的独立输出目录完整重打并通过，拒绝产物未用于验收。

## 本次 macOS 桌面验证

在 macOS ARM64 使用最终打包的独立 `JoyWord.app`、临时配置目录和认证回环控制接口验证，不使用已有编辑会话或用户文档。

- 英文偏好首次启动显示 `Welcome to JoyWord.docx`，活动会话正文包含 `Cooperation Agreement` 且不包含原中文标题：[英文欢迎文档](welcome-en.png)。
- 对同一活动文档连续执行 80 次 `text.insert`，每次核对当前文档和版本；最终版本从 1 增至 81，80 段插入文本全部读回，仍为单页。80 次均有 `edit_to_paint` 新帧日志，最终画面没有载入提示：[连续编辑后](editing-no-loading.png)。
- 当前终端没有 macOS 无障碍读取和输入注入权限，因此未把原生键鼠点击画廊、实际键盘连续键入或逐帧无障碍扫描标记为已验证。编辑验证通过控制接口进入与界面相同的 `try_edit`／`apply_input` 路径；模板语言资源映射由自动测试覆盖。Windows、Linux、信创真机和其他字体组合未验。

## 初始中文模板实现

此前基线为 `eb551e36622c32feaebcdb029d1b06abb26aea26`，分支 `feat/joyword-document-templates`。

文件首页的「工作室手册（示例）」替换为「项目计划书（示例）」，正文改为原创的《社区阅读活动项目计划书》，包含目标、交付范围、进度分工表、预算、风险和验收安排。新增会议纪要、工作周报、通知公告和学习笔记四类中文模板，保留原有信函、简历和欢迎文档。

首页展示空白文档、项目计划书、会议纪要、工作周报、信函和简历；「新建」或「更多模板」展示完整九项。所有新模板采用 A4 页面、可编辑的标题样式和正文、明确的填写提示及带表头的表格。首页缩略图由对应 DOCX 的 JoyWord 首屏渲染生成。旧 handbook 资产和引擎 `sample_document()` 保留为历史测试基线，不再作为首页模板入口。

### 初始自动验证

| 检查 | 结果 |
| --- | --- |
| `cargo xtask build joyword` | 桌面和 CLI 构建通过 |
| `cargo test --locked -p joyword-ui-gpui --lib` | 当时的 23 项通过 |
| `gallery_documents_open_edit_and_roundtrip_with_their_tables` | 当时目录中的全部八份 DOCX 均完成打开、编辑和 DOCX 读回 |
| `cargo fmt -p joyword-ui-gpui -- --check` | 通过 |
| `cargo clippy --locked -p joyword-ui-gpui --lib --bins -- -D warnings` | 通过 |
| `python3 scripts/check_docs.py` | 通过，含资产 SHA-256 核对 |
| 模板脚本重复生成 | 当时的五份 DOCX 字节一致 |
| 独立渲染 | bundled LibreOffice 的 `render_docx.py` 与 JoyWord CLI 均生成五份单页文档，逐页检查中文、标题、表格和页末，无截断或多余页 |

使用 Rust 1.98.1。独立 LibreOffice 需要在临时 fontconfig 中将 Microsoft YaHei 映射到本机 PingFang SC，并加入系统 Font8 字体目录；仅影响验证进程，没有修改应用或系统字体配置。初次未配置字体的渲染缺字，不计入通过证据。

重新生成需要 Python `python-docx==1.2.0`，运行：

```sh
cargo xtask build joyword
python3 scripts/joyword/make_templates.py --cli target/debug/joyword-cli
python3 scripts/check_docs.py
```

### 初始 macOS 桌面验证

在 macOS ARM64 以独立 `JoyWord Templates.app` 和临时配置目录验证，不使用已有编辑会话。已点击创建全部五份中文模板，确认窗口标题、正文标题和表格显示正确。

- [首页六项模板](home.png)
- [完整九项模板及自动换行](gallery.png)
- [项目计划书正文](project-plan.png)
- [会议纪要正文](meeting-minutes.png)
- [工作周报正文](weekly-report.png)
- [通知公告正文](notice.png)
- [学习笔记正文](study-notes.png)

初始实现与验证限于中文模板内容、模板目录及静态缩略图。未做 Windows、Linux、信创真机、其他字体组合、屏幕阅读器、实际打印或 Word/WPS 人工往返验收。
