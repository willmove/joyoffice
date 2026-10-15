# W06：有界分页约束与命中几何

- 负责人：待用户指定唯一软件／主机／会话；确认前不启动。协调入口 C08。
- 代码基线：C07 `a88217136a570a8f224d92b808bc0b8840ab26ca`；另记录 C08 协作包起始完整 SHA。
- 分支／目录：`codex/w06-layout`／`joyoffice-w06`。
- 对应：T105／T207 的限定切片，LAYOUT-003／012；不等于完整中文排版验收。先读 AGENTS.md、第二轮 README／INTERFACES、SPEC、ARCHITECTURE §6、QUALITY、C04/C05/C07 报告。

## 首份交付

沿用已有自动 Unicode 断行、单节单字体 layout_document 与 LayoutSnapshot，增量支持直接段落属性 `keep_lines`（段中不分页）和 `keep_next`（与下段同页）。已有 page_break_before、段前／后距与段落对齐继续有效。重点实现约束联合作用后的真实页分配、源范围、光标／选择／命中几何；不是重新实现字体整形或仅增加接受属性的空逻辑。

先制定可复现的约束优先表：显式分页、连续 keep-next 链、空段、链末尾、单段／整链超过一页、段前后距、链中显式分页。不能满足约束时返回明确诊断／拒绝或经已登记规则有界退化，不可无限回退、死循环、漏行、重排行、空白页爆增或假装遵守。规则从公开规范和原创样例独立建立并记录来源；无固定 Word/WPS 对照时标“规则实现已测、互操作未验证”。

本次不承接 mixed-run、多字体回退、四槽字体、网格、孤行规则全量、复杂分节／脚注／浮动对象、增量性能优化或 GUI；这些保持已有拒绝／限制。明确切片以便与 DOCX／存储并行。

## 独占可写路径

- `crates/joyword-layout/src/document.rs`
- `crates/joyword-layout/src/document/**`（内部 helper，由 document.rs 声明）
- `crates/joyword-layout/tests/w06_*.rs`、`crates/joyword-layout/tests/w06_data/**`
- `docs/coordination/reports/W06.md`

其他文件只读：特别是 layout/lib.rs、line.rs、font.rs、shape.rs、scene.rs、model、engine、render、PDF、app、所有 Cargo 文件和公共语料清单。保留所有现有公共签名；新增顶层导出／公共诊断先向 C08 提案。测试可直接用 engine 建立带既有段落属性的文档，不依赖 W05 新导出。

## 必测与验收

- 固定实际字体字节／face 下，比对独立推算的行／页断点、约束组合和页首／页尾位置；包含刚好满页、差一个最小单位及超高段落／长链。
- 每个源字符与空段均有稳定逻辑位置，分页后 caret／hit_test／selection_rects 正反选择、跨页、软换行 Bias 正确；EMUs 不随显示 DPI 改变。
- 同场景用于 CPU／PDF 的接口保持兼容；旧快照、内容变化而版本相同的预编辑身份、不同字体／profile 仍被现有 current 检查拒绝。
- 溢出、页数／行数／整形／诊断预算和取消不放宽；失败不修改 model；不依赖系统字体缺失时静默跳过测试。

先设置可取得的真实字体（示例路径只适用于有该字体的 Mac，不能照抄为通过）：

```sh
export JOYWORD_REQUIRE_TEST_FONT=1
export JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:6'
cargo test -p joyword-layout --locked
cargo test -p joyword-layout --release --locked
cargo fmt --all -- --check
cargo clippy -p joyword-layout --all-targets --locked -- -D warnings
python3 scripts/check_docs.py
git diff --check
```

没有字体时用仓库已有 OFL fixture 完成可覆盖部分，记录其实际覆盖；中文缺字／参考字体测试写阻塞，不提交系统字体。报告包含字体 SHA-256／face、规则样例和实际几何、未覆盖平台／Office，按模板分别填写三个就绪字段。只提交独占文件，推送任务分支，由用户转交 C08。
