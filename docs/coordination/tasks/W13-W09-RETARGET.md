# W13（可选）：W09 TARGET_SHA 重锚定

- 负责人：TBD；可选任务；协调入口 C09。
- 背景：W09 harness 曾固定 C07／集成 tip；PR #9 修改生产 crate 后固定目标已漂移，见第二轮 README／W09 报告。
- 分支／目录：`codex/w13-w09-retarget`／`joyoffice-w13`。

## 依赖

- 无硬依赖。建议锚定 **PR #10 合并后的 main tip**，或 C09 书面指定的集成 SHA。
- 不依赖 W10–W12 完成；若锚定集成 tip 则等该 tip 存在。

## 首份交付

1. 更新 `experiments/docx-robustness` 内 build.rs／driver／README／配置中的 `TARGET_SHA`（及二进制哈希记录流程）到核准 tip。
2. 复跑约定冒烟／小规模 campaign；证据写入 `docs/coordination/evidence/W09/**` 或本任务约定目录，并在 `docs/coordination/reports/W13.md` 说明 ok／rejected／异常计数。
3. **不**修改生产 crate；发现异常分类后交 C09，不越界修补。
4. **不**安装 nightly／cargo-fuzz；不把本任务写成覆盖引导 fuzz 完成。

## 独占可写路径

- `experiments/docx-robustness/**`
- `docs/coordination/evidence/W09/**`（或 C09 批准的 W13 证据路径）
- `docs/coordination/reports/W13.md`

## 不要做

- 不要改根 workspace 依赖。
- 不要用旧 binary 证据冒充新 TARGET_SHA 验证。
