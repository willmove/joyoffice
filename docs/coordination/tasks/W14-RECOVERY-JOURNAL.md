# W14（可选／延后）：T104 恢复日志 — 设计先行

- 负责人：TBD；**默认本轮不派工实现**。
- 分支／目录：`codex/w14-recovery-design`／`joyoffice-w14`（若启用）。
- 背景：W08 已在 `experiments/recovery-journal` 交付有界日志／重放／检查点 CLI；**未接入生产**。T104 仍 planned。

## 依赖

- **硬前置**：C09-B（或等价文档）已写清打开、另存为新副本、dirty／保存版本、关闭提示的产品语义。
- 在此之前只允许阅读 W08 报告与实验代码，**不得**把实验代码迁入 shell／app。

## 首份交付（本轮若启用）

1. 设计报告 `docs/coordination/reports/W14.md`：会话生命周期、日志落盘时机、与 DocumentVersion／dirty／`save_export_copy`／`save_original` 的关系、崩溃恢复选择 UI 草图、与 W11 文件锁的职责划分、平台（含 Windows 原生）验证缺口。
2. 可选：经 C09 批准的 ADR 草稿于 `docs/adr/**`。
3. **默认不写生产代码**；若用户明示授权实现切片，另开任务卡修订 allowed_paths，且仍不得改 W11 storage 为“恢复后端”。

## 独占可写路径（设计阶段）

- `docs/coordination/reports/W14.md`
- `docs/adr/**`（仅经批准的新 ADR）

## 不要做

- 不要在打开／另存语义未定前接入生产。
- 不要把实验通过写成 T104 done。
- 不要重实现 engine 撤销。
