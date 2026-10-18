# T207 中文排版纯函数交接报告

- 任务 ID：T207（LAYOUT-008、LAYOUT-002）的计算切片，不是 T207 完成，也不是 T206。
- 执行者：Grok，JoyWord 排版会话。集成会话此前未连上本会话；本报告是原执行者对已提交切片的确认。
- 主机／系统／架构：Darwin arm64，macOS 27.0.1（26A434），rustc 1.98.1（48a229cea 2026-09-01），edition 2024，`unsafe_code = forbid`。
- 代码基线：`b097f55374761acf1a3f701afcf365b3aae2c90b`。
- 分支／目录：`m2/t207-cjk-measures`，`/Users/willm/Coding/joyoffice-m2-t207`。
- 状态：限定切片已提交。主线已含同一布局树，不要再套用一遍 diff。
- interface_ready：true，仅限 `joyword_layout::cjk` 纯函数。`layout_document`／`wrap_mixed` 的签名与分页循环未改，禁则只是把既有字符集挪到 `cjk`，`slots` 继续调用它。
- implementation_verified：true，仅限下列自动测试所覆盖的计算、证据标签和拒绝路径。
- acceptance_complete：false。没有 GUI，也没有 Word／WPS 打开文档后的版式对照。

## 实际变更与支持范围

任务分支 tip 与主线提交 `679a849dbc30d5353a467a903ba4cd6600294fbc` 在下列路径上内容一致（对照 diff 为空）：

- `crates/joyword-layout/src/cjk.rs`（新）
- `crates/joyword-layout/tests/cjk.rs`（新，8 项）
- `crates/joyword-layout/src/lib.rs`（声明 `cjk` 模块）
- `crates/joyword-layout/src/slots.rs`（禁则谓词改为调用 `cjk`，字符集不变）
- `crates/joyword-layout/Cargo.toml`（描述补上 T207）
- `docs/joyword/TASKS.md`、`docs/joyword/IMPLEMENTATION.md`（T207 保持 `in_progress`）

未改：`document.rs` 分页、DOCX、engine、shell、app、CI、`experiments/docx-robustness`、依赖与 `Cargo.lock`。未推送。

支持子集：

- R7：调用方必须自己确定是等线。12 pt 且配对出现在 `word_punct_table.txt`／`word_punct2_table.txt` 中时，证据为 `Measured("R7")`；引号 advance 为 431/1200 em（12 pt 下 4.31 pt）。其他字号按同一比例缩放，证据为 `Scaled`。汉字跟随、但该对字符未逐字出现在表里的全宽结果为 `Inferred("R7")`。行尾、空格、ASCII 和未测标点返回 `UnverifiedPunctuation`。句号后空格不支持；逗号后「乙」只按汉字类外推，不把 11.88 pt 当成精确全宽实测。
- R4：`1.3 × win 跨度 × 字号`，再按 312 twips 向上吸附。没有字体身份，证据一律 `Inferred("R4")`。测试把四个 snap-off 点放进规则目录带宽，并把吸附结果核对到 15.6 pt 或 31.2 pt。选用 win，不用 hhea／typo。
- R3：非末行把余量按 EMU 均分到字间（余数给前面的间隙），使总和等于版心；末行间隙为 0。证据 `Inferred("R3")`，因为实测只看到行右缘，没有逐间隙。超过 65536 个 advance 返回 `BudgetExceeded`，不分配。
- 字符／行单位：只有调用方给出单位长度才做百分之一换算，证据 `SuppliedBasis`。缺单位返回 `UnverifiedCharUnit`／`UnverifiedLineUnit`。
- 中西文相邻可识别，宽度和 `adjustRightInd` 固定返回未验证，不给猜测长度。
- 六种家族只做同名匹配（宋体、仿宋、楷体、黑体、等线、微软雅黑及列出的英文别名）。缺 face 返回 `FontMatch::Missing`，不改用另一家族。`default_east_asia_family` 返回等线（R1），不加载字体。

## 验证

2026-10-18 在上述 worktree 复跑，`CARGO_TARGET_DIR` 指向该目录的 `target/`。

| 完整命令或人工步骤 | 实际结果 | 环境/输入/日志 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | passed | 工作区已格式化，无输出 |
| `cargo clippy -p joyword-layout --all-targets --locked -- -D warnings` | passed | rustc 1.98.1，macOS arm64 |
| `cargo test -p joyword-layout --locked` | passed，70 项，0 失败 | 单元 13、`tests/cjk.rs` 8、`document.rs` 21、`line_fonts.rs` 16、`w06_pagination.rs` 12 |
| `python3 scripts/check_docs.py` | passed | 84 需求、66 任务、无环 |
| `python3 scripts/check_gates.py` | passed | 108 个 Rust 文件 |
| Word／WPS 打开并对照页 | skipped | 本切片未接入排版入口 |

70 项里的 8 项新测试是：`r4_uses_the_win_span_and_the_measured_grid`、`r7_punctuation_widths_follow_the_measured_pairs`、`r3_justify_fills_the_line_and_leaves_the_last_line`、`unsupported_followers_and_extrapolated_evidence_are_explicit`、`arithmetic_and_allocation_fail_without_partial_results`、`relative_units_and_unmeasured_adjustments_stay_unverified`、`line_edge_table_matches_the_existing_break_predicate`、`families_match_exactly_and_theme_slots_do_not_cross`。

## 接口或依赖提案

无新依赖。后续若把结果接入 `layout_document`，应读取 `Evidence`，不能把 `Inferred`、`Scaled` 或 `SuppliedBasis` 写成已对照 Word 的分页。`punctuation_advance` 不接受字体参数，接入时由调用方绑定等线字节。

## 阻塞与下一步

没有实现阻塞。不要把 T207 标成 `done`。下一步才是接入分页；那一步会碰到仍在收尾的 T105 分页入口，应单独开切片。主目录里尚未提交的信创决策文档不属于本切片。

代码提交是 `71723be160324144edb43d91859af36d339643df`。本报告补记于该代码提交之后，由集成会话独立归档；报告不代表新增代码或人工验收通过。
