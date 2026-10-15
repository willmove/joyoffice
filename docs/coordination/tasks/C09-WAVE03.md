# C09：第三轮协调与集成

负责人：派工时登记的唯一协调聊天（本草稿作者不自动成为已确认会话）。代码基线：PR #10 合并后的 main tip（草稿期间文档读自 `526e6f85`，代码 tip `5573997`）。准备分支 `codex/wave03-coordination`，业务集成分支 `codex/wave03-integration`。本次先交付包；后续根据用户手动派工及结果转交集成，不创建后台任务或跨聊天代发。

## 责任

1. 确认 PR #10 已合入；固定 `code_base` 与协作包 SHA；维护 [plan.json](../wave03/plan.json)、本卡、W10–W14 卡、接口、启动提示。
2. 核对各任务负责人／主机／cwd／分支／完整 SHA；每项最多一个活跃执行者。不重开 W01–W09。
3. 准备包文件范围：`docs/coordination/README.md`、`docs/coordination/wave03/**`、本任务卡与 W10–W14 卡、`docs/coordination/reports/C09-WAVE03.md`（若需）、准备脚本若有第三轮增量测试。保留第一／二轮 plan 与报告。
4. 集成时 C09 独占：`apps/joyword/**`；shell 的 `lib.rs`／`docx.rs`／`documents.rs`／`input.rs`／`command.rs`／`locales/**`；公共 TASKS／IMPLEMENTATION／PROVENANCE／README／ADR／CI／Cargo；集成专用测试。model／engine／OPC／字体／整形／PageScene 仍冻结，新增接口先评估。

## 协调者切片（有序）

### C09-A：来源贯通（依赖 W10）

- 打开：`storage::open_source` → `DocxSession::open(bytes, budget)`；失败不装会话。
- 保证随后 `plan_export` 计划带有可验证 `origin_bytes`，供 W11 使用。
- 登记／释放 JoyWord 锁的策略写进报告；与 W07 检测名一致。

### C09-B：应用外壳打开 + 另存为新副本 + dirty／关闭（T106）

- 文件打开对话框与错误文案；最近文件若做，须绑定真实打开路径。
- **另存为／导出副本**只走 `save_export_copy`；不覆盖已存在目标。
- dirty／关闭：保留 C07 不保存／取消；副本成功默认不清 dirty、不假装原件已存。
- **禁止**：在 `save_original` 仍拒绝时提供“保存”成功按钮或静默改写原件路径语义。
- 可在 W11 之前交付本切片（用户已有明确新副本退路）。

### C09-C：原件保存接入（依赖 W10+W11+C09-A）

- 仅当 W11 在目标平台去掉写入 blocker 后，才把“保存”接到 `save_original`。
- 会话身份＋请求身份＋文档版本绑定；迟到回执规则同第二轮 INTERFACES。

## 集成次序与检查

建议合入：协作包 → W10 → W12 → W11 → W13（可选）→ 再做 C09-A／B／C。W10 与 W12 都改导出时先合 W10。W11 不得在无绑定 API 的 SHA 上启动写入实现。

收到交接后：fetch 任务分支；核祖先、`git diff --name-only` 仅允许路径；报告中的冻结接口／依赖／测试。越界先裁决。在自己的集成 checkout 处理冲突；不推 main、不强推。

## 验证与交付

准备包：文档／工具检查、准备脚本 dry-run（若适用）、diff 检查。

业务集成：相关 crate／release、全默认成员、应用无窗口测试与开发构建、fmt、clippy、文档与 Python 工具检查。端到端：`open_source`→会话→编辑→`plan_export`→`save_export_copy` 及（若启用）`save_original`→reopen。至少覆盖：错误源计划、原件冲突、保存期间编辑／关闭、导出拒绝、已发布后错误、另存为不覆盖。

GUI 与用户另约。T103／T104／T106／T107 不因接口就绪标 done。

## 不要做

- 不代执行者改其独占文件；不自动派工；不把延后项（ADR-010、信创真机、覆盖 fuzz、大 IME）塞进本轮。
- 不把 W14 生产代码接入当作本轮必达。
