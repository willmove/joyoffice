# W12：带格式段落结构导出（W05 后续）

- 负责人：TBD（建议 Claude Desktop／Code 或 Codex Desktop）；协调入口 C09。
- 代码基线：PR #10 合并后 main tip；启动 HEAD 为协作包 SHA。
- 分支／目录：`codex/w12-formatted-structure`／`joyoffice-w12`。
- 对应：T103、FORMAT-001／002／003；落实 [W05 提案 3(a)](../reports/W05.md)。先读第三轮 README／INTERFACES、W05 任务卡与报告、SPEC、ARCHITECTURE §5。

## 依赖

- **无硬依赖** W10／W11；可并行开发。
- **合入**：若与 W10 同时改 `export.rs`／`export/**`，由 C09 **先合 W10 再合 W12**（或 rebase）。开发期避免无协调地互相覆盖同一 hunk。

## 首份交付

在 W05 默认格式纯文本结构导出之上，扩展**受影响段落**允许非默认但**完全可证明安全**的字符／段落属性：

1. 条件（须全部满足，否则拒绝）：段落／run 属性在模型中完全映射；无保留片段；无未解释属性／未映射内容；无对象／节边界结构编辑；保护／签名／锁定等既有拒绝仍有效。
2. 策略：对完全映射、可安全复制的 pPr／rPr（及提案所述范围），**原样复制源 XML 属性字节**到新段落／run；不得靠“猜样式名”生成新属性树而丢失未知语义。
3. W05 已支持的默认格式路径与拒绝矩阵不退化；C02 文字补丁与结构编辑可同计划导出的性质保持。
4. 往返：engine／C07 输入 → export → 同预算重导；检查顺序、内容、属性、节位置、保留片段多重集合。
5. 明确**不做**：节边界结构编辑（3(b)）、w:br 迁移（3(c)）、Word／WPS 验收（3(e)）、表格／SDT／复杂结构。

## 独占可写路径

- `crates/joyword-docx/src/export.rs`、`export/**`
- `crates/joyword-docx/tests/w12_*.rs`、`tests/w12_data/**`
- `crates/joyword-shell/tests/w12_*.rs`（真实链路测试 only）
- `docs/coordination/reports/W12.md`

只读：importer、Cargo、model／engine／OPC、公共文档、shell 实现。新增样例自建最小原创数据。

## 必测与验收

- 带 jc／粗体／已知 rPr 等的拆分／合并／空段插入／删除；属性字节在新段中保持；未知属性仍拒绝。
- 中文、emoji ZWJ、XML 转义；与默认格式混合文档中仅安全段可结构导出。
- Unicode／预算／不可变计划／失败不改 backing。

```sh
cargo test -p joyword-docx -p joyword-shell --locked
cargo test -p joyword-docx -p joyword-shell --release --locked
cargo fmt --all -- --check
cargo clippy -p joyword-docx -p joyword-shell --all-targets --locked -- -D warnings
python3 scripts/check_docs.py
git diff --check
```

`acceptance_complete=false`。支持／拒绝矩阵写入报告。

## 不要做

- 不要放宽“不能证明就拒绝”。
- 不要改 shell 导出断言文件除非 C09 授权（参照 W05 先例）。
- 不要宣称格式结构导出已完成 T103。
