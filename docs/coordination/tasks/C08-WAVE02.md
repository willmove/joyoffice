# C08：第二轮协调与集成

负责人：本次准备协作包的 Codex 聊天 `01a13d2b-acba-7473-8662-8f183393fd46`。代码基线 C07 `a88217136a570a8f224d92b808bc0b8840ab26ca`。准备分支 `codex/wave02-coordination`，业务集成分支 `codex/wave02-integration`。本次只交付包；后续根据用户手动派工及结果转交进行集成，不创建后台任务或跨聊天代发。

## 责任

1. 核对旧负责人和软件／主机、实际 cwd／分支／完整 SHA；保留 C07 原目录。核实到的历史交付与仍待用户确认的外部 Agent 状态分开记录。W05–W09 每项最多一个活跃负责人；先确认再派工，旧 W01–W04 不重启。
2. 固定代码 SHA 与协作包 SHA，维护 [本轮台账](../wave02/plan.json)、任务卡、接口、启动提示。新增依赖／公共 API 由 C08 裁决；执行者仅提交自己的报告。
3. 准备包文件范围：`docs/coordination/README.md`、`docs/coordination/wave02/**`、本任务卡与 W05/W06/W07/W08/W09 卡、`docs/coordination/reports/C08-WAVE02.md`、`scripts/prepare_agent_workspace.py` 和对应 Python 测试。保留第一轮 plan.json 与报告，不改产品状态以暗示开发已完成。
4. 后续集成时 C08 独占 `apps/joyword/**`、shell 的 `lib.rs/docx.rs/documents.rs/input.rs/command.rs/locales/**`、公共 TASKS／IMPLEMENTATION／PROVENANCE／README／ADR／CI／Cargo 文件与集成专用测试。model／engine／OPC／字体／整形／PageScene 仍冻结，新增接口先评估，不借协调权限重写。执行者可写清单内文件在交付前不同时修改。

## 集成次序与检查

三项基于同一协作包并行；W05/W07 直接使用 C07 ExportPlan，W06 用现有 snapshot。不以 W05 尚未完成为由阻塞 W07 开始。

收到用户转交后，fetch 对应任务分支，核对真实 SHA、代码／包祖先、`git diff --name-only <包SHA>...<结果SHA>` 是否仅包含任务卡允许路径，以及报告中的来源、冻结接口、依赖变化与测试。变更越界先裁决，不无审查合入；在自己的集成 checkout 处理冲突，保留交付原分支。按 W05→W07→W06 建议次序逐项合入并针对性复验，禁止推 main 或强推。

后续 C08 接入真实打开／受限导出命令和文件选择、错误诊断、版本绑定的保存结果／关闭提示；未实现原件替换时只提供明确的新副本流程，不制造保存成功按钮。导出拒绝结构变化时显示限制并保留会话；晚到导出／保存不得清除新编辑 dirty。产品新空白文档建包、恢复日志、完整格式／字体回退等仍是后续缺口。

## 验证与交付

本次准备包只运行文档／工具检查、独立本地 Git remote 的准备脚本测试、所有已登记项的 dry-run 和 diff 检查；不把 C07 Rust 成绩当作本轮重测。

业务集成阶段运行相关 crate／release、全默认成员、应用无窗口测试与开发构建、fmt、完整 clippy、文档与 Python 工具检查，明确字体环境。端到端验证 importer→engine/C07 输入→export→storage→reopen；同 snapshot 的布局／CPU／PDF 不变量；失败原文件／会话保留。至少覆盖原件冲突、保存期间编辑／关闭、导出拒绝和已发布后错误。远程 CI 必须绑定集成 SHA。

GUI 与用户另约时段；Office/IME/跨系统/元数据/真实掉电证据独立记录。T002 已人工验收；T100/T101 不重复实现；未完成完整验收的任务不标 done。

交付本轮包分支、完整提交 SHA、测试和待确认负责人；自身报告 [C08-WAVE02](../reports/C08-WAVE02.md) 不自引用结果 SHA。完成后等待用户手动派工／交接，无自动跟踪服务。

## 扩容裁决（本轮补充）

当前主机 Claude→W05、独立 Codex 执行聊天→W06、ZCode→W07；另一主机 Claude→W08、Codex→W09 可选。软件／主机分配与会话启动分开记账。本协调聊天不同时作为 W06 执行者。远端两项范围仅为独立工程；允许其自身 manifest／锁文件及卡内列明的现有精确依赖，不授权改根 workspace。W08/W09 分三批连续交付，不是只写方案；生产接入与新增 fuzz 工具链仍由 C08 审核。任务文件边界覆盖全部五位执行者，禁止另建后台服务自动派工。
