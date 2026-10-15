# W05：保守正文段落拆分／合并导出

- 负责人：待用户指定唯一软件／主机／会话；确认前不启动。协调入口 C08。
- 代码基线：C07 `a88217136a570a8f224d92b808bc0b8840ab26ca`；实际启动 HEAD 为 C08 交付的协作包完整 SHA，二者分别记录。
- 分支／目录：`codex/w05-docx-export`／`joyoffice-w05`。
- 对应：T103、FORMAT-001／002／003。先读 AGENTS.md、[第二轮包](../wave02/README.md)、[接口](../wave02/INTERFACES.md)、SPEC、ARCHITECTURE §5、QUALITY 与 C02/C07 报告。

## 首份交付

C07 输入已可 Enter 拆段和跨段删除，但 C02 导出仍拒绝结构变化。扩展现有 ExportPlan，使无未知锚点、无对象、无节边界、无段落标记特殊属性的**连续纯文本正文段落**支持拆分、合并、插入空段和删除后保留合法正文。第一切片限定默认字符／段落属性；已有可导出文字编辑、未知内容原样保留及拒绝行为不退化。不要重写 importer 或另建一套会话。

从真实 engine 事务取得编辑后的 snapshot，不按文本相似度猜源 XML 所属。保留未受影响部件及段落的原始字节和关系；对受影响原段落分析 namespace、属性、注释、PI、pPr/rPr、sectPr 与源映射。不能证明安全就拒绝，尤其不得把未知内容移到错误段落、复制文档末尾 sectPr 或悄悄丢弃标注。格式导出、复杂结构／表格、宏／模板编辑、Strict、新空白文档建包不在本次范围。

## 独占可写路径

- `crates/joyword-docx/src/export.rs`
- `crates/joyword-docx/src/export/**`（按需新增内部 helper，由 export.rs 声明）
- `crates/joyword-docx/tests/w05_*.rs`、`crates/joyword-docx/tests/w05_data/**`
- `crates/joyword-shell/tests/w05_*.rs`（仅测试真实 importer→engine／C07 输入→export 链路，不改 shell 实现）
- `docs/coordination/reports/W05.md`

除此之外只读，包括 docx/lib.rs、importer、所有 Cargo 文件、共享 fixture 清单／生成器、model／engine／OPC、公共文档。新增测试样例自建最小原创数据，来源与规范条款写进 W05 报告，不重生成公共语料。需增量顶层接口或 importer 修正时由 C08 裁决。

## 必测与验收

1. 导入→TextInput/engine 拆分／合并→export→同预算重新导入；首／中／末段、空段、连续 Enter、首尾／跨段删除、撤销／重做、重复导出及多个原段落的源映射。
2. 中文、emoji ZWJ、组合字符、XML 转义、首尾空白和 tab；新增段落不得打断 Unicode 标量或生成非法 XML。
3. 比较重新导入的段落顺序、内容、属性和 section；未知属性／XML／关系／二进制部件精确保留。对被影响的未知锚点、保护、强制修订、数字签名、对象／节／格式变化有明确拒绝用例。
4. 原 OPC 各预算与源绑定验证不放宽；失败不改 backing／会话，旧计划保持不可变，晚到计划不代表新版本。不能把内部读回当作 Word/WPS 验收。

```sh
cargo test -p joyword-docx -p joyword-shell --locked
cargo test -p joyword-docx -p joyword-shell --release --locked
cargo fmt --all -- --check
cargo clippy -p joyword-docx -p joyword-shell --all-targets --locked -- -D warnings
python3 scripts/check_docs.py
git diff --check
```

报告按第二轮模板分别填写 interface_ready／implementation_verified／acceptance_complete，列出真实支持／拒绝矩阵、规范来源和未覆盖 Office 环境。完成后只提交上述文件、推送自己的分支并经用户交接 C08；未批准跨界则先交付边界内成果与具体提案。
