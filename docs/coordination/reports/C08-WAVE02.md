# C08：第二轮协作包准备（2026-10-15）

- 执行者：本轮 Codex 协调聊天 `01a13d2b-acba-7473-8662-8f183393fd46`；没有派发开发 Agent 或启动后台服务。
- 代码候选基线：C07 `a88217136a570a8f224d92b808bc0b8840ab26ca`。
- 准备分支：`codex/wave02-coordination`。
- 独立目录：`/Users/willm/.codex/worktrees/joyoffice-wave02/joyoffice`；从固定基线创建，未切换或修改 C02 主目录与原 C07 worktree。
- 交付范围：协作包和准备工具。业务实现、人工验收与派工状态不混用；W05／W06／W07 尚未启动。
- interface_ready：true，任务文件边界、现有接口及拟新增存储契约已写入包；不代表新增业务 API 已实现。
- implementation_verified：true，仅指本次准备工具与文档检查。
- acceptance_complete：false；负责人仍待用户确认，产品验收未执行。

## 核查结论

检查主目录为干净的 `codex/coordination-docx-export`／`4dd3870a609509ec225a7f85cb8509fbb602dfae`。原独立目录 C07 干净，实际 HEAD、本地分支与远程 ls-remote 一致为上述代码基线；C07 原负责人已交付 [PR #7](https://github.com/willmove/joyoffice/pull/7)，聊天查询时 idle。GitHub CLI 查询到 [38654508321](https://github.com/willmove/joyoffice/actions/runs/38654508321)／[38654515174](https://github.com/willmove/joyoffice/actions/runs/38654515174) 均 completed/success 且绑定同一 SHA；本轮未重跑 C07 Rust 测试。

W01–W04 历史交付和负责人已从原 plan／报告核对，W03 按实际 Codex 交付记录，W04 为原协调者接手。未将“未加载／空闲”当作所有外部软件已停止的证明。已询问用户在途负责人和三位新执行者，当前无确认记录，plan 保持 owner=null、ownership_confirmed=false、awaiting_manual_assignment。包可交付，实际启动要先完成负责人确认。

## 文件与选择

[第二轮入口](../wave02/README.md) 包含基线、负责人核查、1＋3 分工、独立 worktree／clone 准备命令、手动交接与集成顺序。[启动提示](../wave02/START-PROMPTS.md) 可分别复制给三位执行者及协调者；[接口](../wave02/INTERFACES.md) 与各任务卡明确单文件／子目录所有权、公共 API 冻结、独立开发条件、受控保存与源绑定契约。

首份交付分别选择保守正文拆并段导出、单字体分页 keep-next／keep-lines 约束、源身份与冲突检测／有条件原件替换。安全保存对平台元数据、竞争或源绑定未解决的情况允许交付真实预检及明确阻塞，不虚构原件保存完成。更广格式导出、多字体／中文排版、日志恢复仍未派工。

旧 plan.json 原样保留。准备器增加显式 --plan 与 --start；第二轮必须指定完整包 SHA，拒绝远程包漂移、配置不同、路径逃逸；保持旧轮默认方式以及目录／分支存在时的拒绝保护。不安装依赖，不修改 Rust、产品 TASKS／IMPLEMENTATION、根 Cargo 或 CI。

## 验证

| 命令／检查 | 实际结果 |
| --- | --- |
| python3 scripts/check_docs.py | passed；82 需求、58 任务、9 用例；元数据检查不代表产品验收 |
| PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests -v | 19 passed；包括真实临时本地 Git remote 上的 worktree／clone、已存在目录／远程分支拒绝、固定 SHA、分支漂移拒绝、包配置不一致、完整 SHA 要求、第二轮独立配置与目录逃逸拒绝 |
| 本轮协调文档本地链接与 plan/card 路径交叉核对（Python 只读检查） | passed；10 份 Markdown、3 个唯一任务／分支／目录、所有允许路径与卡片一致 |
| git diff --check | passed |

包分支及结果完整 SHA 在提交后的最终交接提供，报告不自引用自身提交。未创建执行者工作目录、未更新执行者报告、未操作桌面；三位负责人、原 C07 后续职责接力仍由用户手动确认。
