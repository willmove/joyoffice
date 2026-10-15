# W11：save_original 发布策略

- 负责人：TBD（建议 ZCode Desktop，接续 W07）；协调入口 C09。
- 代码基线／包 SHA：须为**已含 W10 绑定 API** 的协作或集成 tip；若首包只有占位，等 C09 公布“W11 开工 SHA”。
- 分支／目录：`codex/w11-save-original`／`joyoffice-w11`。
- 对应：T103／T106；落实 [W07 提案 2](../reports/W07.md)。先读第三轮 README／INTERFACES、W07 任务卡与报告、C03 报告。

## 依赖

- **硬依赖 W10**：无 `ExportPlan::origin_bytes`（或等价）不得开始写入路径实现。可先补充拒绝用例，但报告须标明“写入未实现／等待绑定”。
- 不依赖 W12。
- 应用菜单接入是 C09-C，不在本卡。

## 首份交付

1. 去掉（在已验证平台子集上）`OriginBindingUnproven`：冲突复查后 `plan.origin_bytes()` 与 stamp 字节精确相等才继续；不等则拒绝且零写入。
2. 实现有界发布策略（对齐 W07 提案）：临时文件 → 写 → sync → 逐字节回读 → 目标硬链接备份并回读确认等于 stamp → rename；发布前失败保留原件；发布后错误带已发布回执。
3. 更新能力矩阵：无法保证的元数据仍写入前拒绝；不引入未批准的 xattr／ACL 依赖。
4. 保留 `save_export_copy` 与 W07 冲突检测回归；硬链接源、锁文件、路径解析变化等既有拒绝不退化。
5. 若某平台仍不能安全发布：继续 `ReplacementUnavailable`，矩阵与报告写清，**不得**伪造成功。

## 独占可写路径

- `crates/joyword-shell/src/storage.rs`、`storage/**`
- `crates/joyword-shell/tests/w11_*.rs`、`tests/w11_data/**`
- `docs/coordination/reports/W11.md`

不得改 shell 的 docx／input／documents／lib／command／文案／app；不得改 ExportPlan 定义（只调用）；不得改 Cargo／CI／公共产品文档。

## 必测与验收

- 绑定成功路径：同源 plan＋stamp → 发布 → 同预算重开；源身份刷新来自读回。
- 绑定失败：异源 plan、同版本异字节、缺 origin → 零副作用。
- 故障注入：写中／sync／校验／备份／rename／清理各阶段；发布后失败回执。
- 锁文件、符号／硬链接、删除重建、超预算。

```sh
cargo test -p joyword-shell --locked
cargo test -p joyword-shell --release --locked
cargo fmt --all -- --check
cargo clippy -p joyword-shell --all-targets --locked -- -D warnings
python3 scripts/check_docs.py
git diff --check
```

`acceptance_complete=false`（无断电／远程 FS／Office 共开人工验收不标完成）。

## 不要做

- 不要在无 W10 的分支上删除 origin blocker 并写入。
- 不要自动“另存逃逸”或静默丢元数据。
- 不要创建／删除 Office `~$` 锁；不要实现恢复日志。
