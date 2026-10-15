# 第三轮：手动派工、Git 交付（草稿）

日期：2026-10-16。本文件是 Wave 03 协作包草稿，风格对齐 [第二轮](../wave02/README.md)。**尚未派工**；不启动执行 Agent、自动队列、后台监控或 GUI。开发 Agent 协作不涉及产品内 AI 功能。

## 文档来源与前置

- 协作文档读取自 PR #10 head `526e6f85d90efa5e4b6034d5f1d6dec69fa72755`（`codex/wave02-ledger-sync`）。
- 代码 tip（Wave 02 + PR #9）：main `5573997a75d9297b24429f90c727dad05b7f90c3`。
- **硬前置（非本轮任务）**：先合并 [PR #10](https://github.com/willmove/joyoffice/pull/10)（仅同步第二轮公共台账／过时文档）。合并后再从 main tip 切 `codex/wave03-coordination`，并把 [plan.json](plan.json) 的 `code_base` 换成该合并提交完整 SHA。
- 第二轮 W05–W09 均为 `delivered_integrated` 且 `acceptance_complete=false`；第三轮不重开这些 ID。

## 推荐顺序（摘要）

| 次序 | 项 | 类型 | 依赖 |
| --- | --- | --- | --- |
| 0 | 合并 PR #10 | 前置 | — |
| 1 | [W10](../tasks/W10-EXPORT-ORIGIN.md) ExportPlan 来源绑定 | 并行可开工 | 无 |
| 1′ | [W12](../tasks/W12-FORMATTED-STRUCTURE.md) 带格式段落结构导出 | 可与 W10 并行开发 | 无；**合入顺序优先 W10 再 W12**（二者均可能改 `export.rs`） |
| 2 | [C09](../tasks/C09-WAVE03.md) `open_source`→`DocxSession` 接线 | 协调者切片 C09-A | W10 |
| 3 | C09 应用外壳：打开 + **另存为新副本** + dirty／关闭提示 | 协调者切片 C09-B | 可用现有 `save_export_copy`；**禁止伪造成功的原件覆盖** |
| 4 | [W11](../tasks/W11-SAVE-ORIGINAL.md) `save_original` 发布策略 | 执行者 | **必须 W10**（及包 SHA 已含绑定） |
| 5 | C09 接入原件保存命令（若 W11 能力矩阵允许） | 协调者切片 C09-C | W10+W11+C09-A |
| ∥ | [W13](../tasks/W13-W09-RETARGET.md) 重锚定 W09 `TARGET_SHA` | 可选并行 | 无 |
| 后 | [W14](../tasks/W14-RECOVERY-JOURNAL.md) T104 恢复日志 | 可选／延后 | **仅在打开／保存设计写清之后**；本轮倾向只出设计 |

## 明确延后（本轮不做）

- ADR-010（GPUI 来源选定与字形实测）
- 信创真机 T005
- 完整 Word／WPS 固定环境 T004
- 覆盖引导 fuzz（cargo-fuzz／nightly）
- 大 GUI／系统 IME 人工验收
- 把 W08 实验直接标成 T104 done，或在打开／另存设计未定前写生产恢复代码

## 任务与边界

| 任务 | 首份可验收交付 | 独立分支 | 建议目录 |
| --- | --- | --- | --- |
| [C09 协调](../tasks/C09-WAVE03.md) | 包维护、接口裁决、open→会话接线、另存为新副本／dirty 关闭、集成 | `codex/wave03-coordination` → 集成 `codex/wave03-integration` | `joyoffice-wave03` |
| [W10 来源绑定](../tasks/W10-EXPORT-ORIGIN.md) | `ExportPlan` 携带可验证 `origin_bytes`；既有导出回归不退化 | `codex/w10-export-origin` | `joyoffice-w10` |
| [W11 原件发布](../tasks/W11-SAVE-ORIGINAL.md) | 在绑定成立且能力矩阵允许的平台子集上实现 W07 提案的发布路径；否则继续保守拒绝并更新矩阵 | `codex/w11-save-original` | `joyoffice-w11` |
| [W12 格式结构导出](../tasks/W12-FORMATTED-STRUCTURE.md) | 完全映射、无保留片段的带格式正文段落拆分／合并（W05 提案 3(a)） | `codex/w12-formatted-structure` | `joyoffice-w12` |
| [W13 W09 重锚定](../tasks/W13-W09-RETARGET.md)（可选） | harness 固定到经核准的生产 tip；冒烟矩阵可复现 | `codex/w13-w09-retarget` | `joyoffice-w13` |
| [W14 恢复设计](../tasks/W14-RECOVERY-JOURNAL.md)（可选／延后） | 会话生命周期／持久化时机／与 dirty 版本关系的设计报告；默认不写生产代码 | `codex/w14-recovery-design` | `joyoffice-w14` |

精确可写路径见各卡与 [plan.json](plan.json) 的 `allowed_paths`。接口见 [INTERFACES.md](INTERFACES.md)。

### 路径独占（相对第二轮）

- **W10／W12**：`joyword-docx` 导出实现与各自 `w10_*`／`w12_*` 测试。二者都动 `export.rs`／`export/**` 时，**禁止同时改同一文件无人协调**；并行开发可以，合入由 C09 按 W10→W12 处理冲突。
- **W11**：仅 `joyword-shell` 的 `storage.rs`／`storage/**` 与 `w11_*` 测试；不改 app、docx 会话、ExportPlan。
- **C09**：独占 `apps/joyword/**`，shell 的 `lib.rs`／`docx.rs`／`documents.rs`／`input.rs`／`command.rs`／`locales/**`，公共 TASKS／IMPLEMENTATION／README／ADR／CI／根 Cargo，以及集成分支。
- **W13**：仅 `experiments/docx-robustness/**` 与本任务证据／报告。
- **W14**：默认只写报告（及经批准的 ADR）；不改 `experiments/recovery-journal` 生产接入，除非 C09 另批。

W10／W12 不写 shell 实现；W11 不写 docx／app。根及生产 crate 的 Cargo.toml／Cargo.lock 由 C09 裁决。新增／升级依赖先提案。

## 基线与负责人（待用户确认）

| 职责 | 建议 | 状态 |
| --- | --- | --- |
| C09 | 本草稿协调入口；派工时登记唯一会话 | 未启动 |
| W10 | 建议 Claude Code 或 Codex | `ownership_confirmed=false` |
| W11 | 建议 ZCode（接续 W07 storage） | 同上；**依赖 W10** |
| W12 | 建议 Claude Code 或 Codex | 可与 W10 并行开发 |
| W13 | 可选 | 可选 |
| W14 | 可选／延后 | 打开／保存设计之后 |

`ownership_confirmed=true` 只表示软件／主机已定，不表示进程已启动。

## 准备命令（合入仓库并填好 code_base 后）

POSIX（将 `JOYOFFICE_WAVE03_PACKAGE_SHA` 换成协调者交接的 40 位包 SHA；`CODE_BASE` 为 PR #10 合并后的 main tip）：

```sh
git clone --single-branch --branch codex/wave03-coordination git@github.com:willmove/joyoffice.git joyoffice-wave03-control
cd joyoffice-wave03-control
JOYOFFICE_WAVE03_PACKAGE_SHA='替换为协调者交接的40位协作包SHA'
CODE_BASE='替换为PR10合并后的40位main tip'
git fetch --no-tags origin codex/wave03-coordination
test "$(git rev-parse HEAD)" = "$JOYOFFICE_WAVE03_PACKAGE_SHA"
test "$(git rev-parse FETCH_HEAD)" = "$JOYOFFICE_WAVE03_PACKAGE_SHA"
git merge-base --is-ancestor "$CODE_BASE" "$JOYOFFICE_WAVE03_PACKAGE_SHA"

python3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave03/plan.json --start "$JOYOFFICE_WAVE03_PACKAGE_SHA" --task W10
# W11 仅在包 SHA 已含 W10 绑定 API 后准备；若首包不含 W11 所需 API，等 C09 发布含 W10 的集成 tip 再准备
python3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave03/plan.json --start "$JOYOFFICE_WAVE03_PACKAGE_SHA" --task W12
# 可选：
# python3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave03/plan.json --start "$JOYOFFICE_WAVE03_PACKAGE_SHA" --task W13
```

预览加 `--dry-run`。软件不支持 worktree 时加 `--mode clone`。不要在他人 checkout 上切分支或 reset。

## 交付与集成

执行者：只 `git add` 自己的明确文件 → commit → `git push -u origin <任务分支>`。不强推、不合并／推送 main。报告按 [REPORT-TEMPLATE.md](REPORT-TEMPLATE.md)；启动提示见 [START-PROMPTS.md](START-PROMPTS.md)。

C09：核对路径与冻结 API → 按建议次序合入 `codex/wave03-integration` → 复验受影响 crate 与全默认成员、应用无窗口测试／开发构建、fmt／clippy、文档检查。不推 main。

**成功标准（本轮“接口／实现切片”，不是 M1 完成）：**

1. `plan_export` 产生的 `ExportPlan` 可提供与打开时原包字节一致的 `origin_bytes()`（或明确 `None` 仅用于无 backing 场景且 `save_original` 拒绝）。
2. 经 `open_source` 读取的字节打开的 `DocxSession`，其导出计划能通过 `save_original` 的来源相等检查（在 W11 启用写入的平台上）。
3. 应用可：**打开** DOCX、**另存为新路径**（仅 `save_export_copy`）、在 dirty 时关闭给出不保存／取消（及已实现范围内的保存入口）；**默认“保存”不得在未实现原件替换时假装成功覆盖原件**。
4. W12：带格式、完全映射、无保留片段的正文段落拆分／合并有真实往返测试；不能证明安全则拒绝。
5. 全部任务 `acceptance_complete` 默认 false，除非另附 GUI／Office／平台人工证据。

## 不要做什么

- 不要在 PR #10 合并前从过时 main 文档派工第三轮。
- 不要把另存为新副本做成“保存成功并清除原件 dirty、改写来源路径却未替换原件”的假原件保存。
- 不要在 W10 未合入时实现 W11 写入路径（可先加强拒绝测试，但报告须写“等待绑定”）。
- 不要把 W13 的固定种子变异写成覆盖引导 fuzz 验收。
- 不要在打开／另存设计未定时把 W08 实验代码接入生产会话。
- 不要重开 W01–W09；不要把 T103／T104／T106 因接口就绪直接标 `done`。
- 不要新增未审批依赖；不要改 model／engine／OPC／字体／整形／PageScene 公共契约（除非 C09 书面批准的精确增量）。
