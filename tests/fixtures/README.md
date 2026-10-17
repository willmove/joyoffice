# JoyWord 原创语料登记

登记两类原创用例：UTF-8 文本种子（`text/`）与规范最小 DOCX 样例（`docx/`）。**它们都不是已通过的兼容测试**：`manifest.json` 中所有用例均为 `planned`，等待格式与排版实现以及固定参考环境产出参考结果。

执行 `python3 scripts/check_docs.py` 校验唯一 ID、需求编号、输入散列、路径和证据状态。这只证明登记信息自洽，不执行文档功能。不要通过修改预期结果或重新计算散列来掩盖实现回归。

每个用例包含：`id`、`kind`、`path`（相对此目录）、`sha256`、`origin`、`license`、`requirements`、`expected`、`status`、`evidence`。原始种子按 UTF-8、LF 保存；组合字符不做 Unicode 规范化。

`docx/` 中的最小样例由 `cargo run -p joyword-corpus --bin generate_corpus` 确定性生成（固定条目顺序与时间戳），`cargo test -p joyword-corpus` 逐字节核对仓库中的文件与重新生成的结果一致，并检查它们是合法、无诊断的 OPC 包。修改样例须重新生成、审阅差异并更新清单散列。它们是手写的 WordprocessingML 输入，只经 macOS `textutil` 独立读取冒烟，尚未用 Word／WPS 打开确认；在建立参考结果前，`reference_profile.status` 为 `unverified`，检查器不允许此时标记 `passed`。

格式用例使用 `document` 和 `reference_profile`：输入文件、规范修订／章节、参考软件完整版本、系统、locale、字体文件散列、页面／打印设置、操作步骤、预期语义和几何结果。`evidence` 指向实际存在的报告；报告必须记录 tested revision、命令、时间、实际结果和失败分类。`passed` 不能仅引用一张截图。

用户文件不能直接进入公开仓库；只纳入授权明确且去敏的样本。字体资源必须单独列出许可及嵌入权限。[fonts](fonts/README.md) 包含一个改名的 OFL 拉丁字体最小测试子集，固定 fi 连字/AV 字距测试输入；不分发系统字体，不以此替代 CJK 参考字体环境。

初始种子用途：

| ID | 场景 |
| --- | --- |
| CJK-001 | 原创合同短文、中英混排和中文标点 |
| UNICODE-001 | 组合字符、ZWJ emoji、补充平面字符及双向文字 |
| REVIEW-001 | 原稿、目标稿和批注锚点的审阅投影 |
| AI-001 | 待处理文档里的越权指令不得成为 Agent 权限 |
| DOCX-MIN-001 | 最小正文包；缺少 settings 时兼容模式按 12 解释 |
| DOCX-MIN-002 | 中文 Word 样式 ID（`a`、`1`）按规范名称识别，兼容模式 15 |
| DOCX-MIN-003 | mc:Ignorable 未知元素与 customXml 部件的保留 |
| DOCX-MIN-004 | 外部超链接与 file:// 主机附加模板：只记录不访问 |
| DOCX-MIN-005 | 限制编辑与强制修订：实现前只读或提示 |
| DOCX-SAMPLE-001 | 应用启动示例文档：正文逐行取自 `text/cjk-contract.txt`，可编辑、可另存 |

joyword-model 的集成测试读取这些种子，检查逐码点保留与偏移换算；这属于模型测试，不改变种子的 planned 状态。比较工具：模型层的 `Document::semantic_difference` 以不含 ID 的转储比较语义；`scripts/compare_pdf_lines.py` 用同一抽取器（poppler）比较参考 PDF 与 JoyWord PDF 的页数、每页行数、逐行文字和行位置（默认 0.5 pt），`--dump` 输出可附在证据中的行几何。T004 仍缺固定 Word／WPS 参考环境与排版规则目录，不能以此清单宣称完成。
