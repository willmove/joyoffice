# W10：ExportPlan 来源绑定

- 负责人：TBD（建议 Claude Desktop／Code 或 Codex Desktop）；`ownership_confirmed=false` 直至用户指定。协调入口 C09。
- 代码基线：PR #10 合并后 main tip；启动 HEAD 为 C09 协作包完整 SHA。
- 分支／目录：`codex/w10-export-origin`／`joyoffice-w10`。
- 对应：T103／T106；落实 [W07 提案 1](../reports/W07.md)。先读 AGENTS.md、[第三轮包](../wave03/README.md)、[接口](../wave03/INTERFACES.md)、SPEC、ARCHITECTURE §5、QUALITY、C02／W07 报告。

## 依赖

- **无代码依赖**；可立即开工（包发布后）。
- **被依赖方**：W11、C09-A／C09-C 需要本任务的 `origin_bytes` API。

## 首份交付

`ExportPlan` 必须能证明“该计划从哪一份原包字节导出”：

1. `plan_export` 公开签名不变；从 `PackageBacking::original_bytes()` 填入私有 origin（建议 `Option<Arc<[u8]>>` 共享）。
2. 公共只读访问器（如 `origin_bytes() -> Option<&[u8]>`）。无公共 API 允许调用方塞入任意 origin。
3. 对真实导入→编辑→导出路径，`origin_bytes()` 与打开时原包字节精确一致；无 backing／无法证明时须使 `save_original` 侧可拒绝（本任务至少保证 plan 侧不提供假绑定）。
4. 既有文字／W05 结构导出、警告、预算、不可变计划语义不退化。
5. 测试：同源计划匹配；故意不同包／截断／同版本异内容的不相等；序列化／克隆后仍保持相等语义（若有）；不影响 `save_export_copy` 既有测试（shell 侧只读调用访问器的集成测试可放 `w10_*.rs`）。

不在此实现 `save_original` 写入，不改 app／storage 发布逻辑。

## 独占可写路径

- `crates/joyword-docx/src/export.rs`、`export/**`
- `crates/joyword-docx/src/result.rs`（若 ExportPlan 定义于此或需重导出）
- `crates/joyword-docx/src/lib.rs`、`backing.rs`（**仅**文档注释与必要 re-export；不改导入语义）
- `crates/joyword-docx/tests/w10_*.rs`、`tests/w10_data/**`
- `crates/joyword-shell/tests/w10_*.rs`（只测访问器与真实链路，不改 shell 实现）
- `docs/coordination/reports/W10.md`

除此之外只读。不改 Cargo 文件、importer、model／engine／OPC。

## 必测与验收

```sh
cargo test -p joyword-docx -p joyword-shell --locked
cargo test -p joyword-docx -p joyword-shell --release --locked
cargo fmt --all -- --check
cargo clippy -p joyword-docx -p joyword-shell --all-targets --locked -- -D warnings
python3 scripts/check_docs.py
git diff --check
```

`interface_ready`／`implementation_verified` 可对绑定子集为 true；`acceptance_complete=false`（无 GUI／原件保存人工验收）。完成后只推送本分支，经用户交 C09。

## 不要做

- 不要放宽“版本号即同源”。
- 不要为了让 W11 好看而在 plan 里写入 stamp 以外的“摘要替代”（必须是可精确比较的原包字节或同等证明）。
- 不要改 W12 范围的格式结构逻辑（可与 W12 并行，但勿抢做 3(a)）。
