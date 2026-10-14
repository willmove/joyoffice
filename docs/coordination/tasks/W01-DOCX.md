# W01：DOCX 最小只读导入

分配：Claude Desktop 的 Code 会话；分支 codex/w01-docx-import。T103 的第一切片，不是全量 T103。

## 目标

复用 joyoffice-opc、joyword-model 和现有 5 个原创 DOCX 样例，建立真实可调用的只读导入器。按 ECMA-376/微软公开规范核对语义并在报告记录章节/链接，不读取其他编辑器实现。

## 本轮工作

1. 新建 crates/joyword-docx，识别 officeDocument 正文关系和 WordprocessingML 命名空间；错误类型、缺正文、未实现 Strict 等显式拒绝/诊断。
2. 映射普通正文段落、文本/空白保留、基础 run/para 属性、样式引用与必要定义。内置样式按 w:name 规范名称识别，不能猜 styleId。
3. 未指定兼容模式按 12，保留已有值；不把新文档默认 15 强加给导入文件。
4. 实现最小真实 ImportResult/PackageBacking：原始包/部件、支持边界和诊断。未知嵌套不能静默丢失，应保留并限制编辑。仅保存原包不能宣称编辑后保真。
5. 外链不访问；限制编辑/强制修订在未实现时只读或拒绝。沿用 OPC 预算，新增 XML 解析也检查深度/事件预算，禁 DTD/外部实体。
6. 测试现有样例的正文、规范样式名、模式、未知保留、外链、保护；增加损坏/截断/预算错误测试。

不实现保存/导出、原子替换、应用按钮、表格保真或空壳成功接口。

## 文件边界

- crates/joyword-docx/**。
- 根 Cargo.toml（仅接入真实 crate 和必要本地依赖）、Cargo.lock（对应更新）；不升级现有依赖，quick-xml 可复用。
- docs/coordination/reports/W01.md。

其他文件只读。模型/OPC API 缺口提出增量建议，先交付可验证子集。

## 验证与交接

运行 cargo test -p joyword-docx --locked、cargo test --locked、cargo fmt --all -- --check、cargo clippy -p joyword-docx --all-targets --locked -- -D warnings、python3 scripts/check_docs.py。

未知/保护输入不能返回无诊断可编辑结果。报告公开签名、保留边界、支持子集、实际测试和下一轮写回缺口。提交/推送自己的分支，等待协调者审查，不自行推进整个 T103。
