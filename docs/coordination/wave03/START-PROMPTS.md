# 可复制的第三轮启动提示（草稿）

由用户手动把对应提示交给对应软件；不自动创建聊天或代发。先合并 PR #10，填好协作包 SHA 与 `code_base`，再准备目录。全部新任务使用同一已审核包 SHA。

## W10：ExportPlan 来源绑定

```text
你负责 JoyOffice 第三轮 W10（ExportPlan 来源绑定），由用户手动派工。
指定负责人：填入软件／主机。本执行会话标识：启动时记录，保持每任务只有一个活跃执行者。
已准备的独立绝对目录：填入准备器输出路径。
任务分支：codex/w10-export-origin。
代码基线：填入 PR #10 合并后的 40 位 main tip。
协作包起始完整 SHA：填入 C09 交接的 40 位包 SHA。
先运行 pwd、git status --short、git branch --show-current、git rev-parse HEAD，并确认 merge-base 祖先含代码基线。
首次启动 HEAD 必须等于所分配包 SHA；冲突时报告，不 reset、不重复开工。
阅读 AGENTS.md、docs/coordination/README.md、docs/coordination/wave03/README.md、docs/coordination/wave03/INTERFACES.md、docs/coordination/tasks/W10-EXPORT-ORIGIN.md。
按任务卡完成 ExportPlan origin 绑定与测试；只写 allowed_paths；不实现 save_original 写入，不改 app。
报告写到 docs/coordination/reports/W10.md，参照 docs/coordination/wave03/REPORT-TEMPLATE.md。
只提交自己的明确文件；有权限时 git push -u origin codex/w10-export-origin；不强推、不合并或推送 main。
最终输出任务 ID、软件／主机／会话、分支、代码基线、包 SHA、结果 SHA、推送状态、测试及阻塞；报告正文不自引用结果 SHA。
由用户把交接转给 C09。完成后等待下一次手动分配。
```

## W11：save_original 发布策略（须 W10 已在开工 SHA）

```text
你负责 JoyOffice 第三轮 W11（save_original 发布策略），由用户手动派工。
硬依赖：开工 SHA 必须已含 W10 的 origin_bytes（或等价）API；若没有，停止并报告，只允许加强拒绝测试。
指定负责人：填入软件／主机（建议 ZCode）。任务分支：codex/w11-save-original。
代码基线／包或集成开工 SHA：填入 C09 公布的含 W10 的 40 位 SHA。
阅读 wave03 README／INTERFACES 与 docs/coordination/tasks/W11-SAVE-ORIGINAL.md、W07 报告提案 2。
只写 storage 与 w11 测试／报告；不改 docx／app／Cargo。不能证明安全则继续拒绝，不伪造成功保存。
报告 docs/coordination/reports/W11.md。推送 codex/w11-save-original。用户转交 C09。
```

## W12：带格式段落结构导出

```text
你负责 JoyOffice 第三轮 W12（带格式段落结构导出），由用户手动派工。
任务分支：codex/w12-formatted-structure。可与 W10 并行开发，但勿无协调覆盖同一 export hunk；合入由 C09 处理。
阅读 W12 任务卡与 W05 报告提案 3(a)。只扩展可证明安全的带格式结构导出；节边界／对象／Word 验收不做。
报告 docs/coordination/reports/W12.md。推送自己的分支。用户转交 C09。
```

## W13（可选）：W09 重锚定

```text
你负责 JoyOffice 第三轮可选任务 W13（W09 TARGET_SHA 重锚定）。
仅改 experiments/docx-robustness 与证据／W13 报告。TARGET_SHA 必须是 C09／用户核准的生产 tip。
不改生产代码，不安装 cargo-fuzz／nightly，不把变异冒烟写成覆盖引导 fuzz。
```

## W14（可选／延后）：恢复设计

```text
你负责 JoyOffice 第三轮可选 W14（恢复日志设计）。仅在 C09 确认打开／另存／dirty 语义已写清后开始。
默认只写 docs/coordination/reports/W14.md（及批准的 ADR）；不把 experiments/recovery-journal 接入生产。
```

## C09：用户转回执行结果后

```text
你是 JoyOffice 第三轮唯一协调者 C09。继续使用现有协调聊天，不额外启动第二位协调者。
阅读 docs/coordination/tasks/C09-WAVE03.md、wave03/README.md、INTERFACES.md、plan.json。
先确认 PR #10 已合入；核实交接的任务 ID、分支、完整结果 SHA 与路径边界。
在 codex/wave03-integration 按建议次序集成（W10→W12→W11→可选 W13），再做 C09-A／B／C 切片。
另存为只许 save_export_copy；无 W11 成功路径时禁止伪原件保存。不推 main。GUI 另约。
采用手动派工、Git 交付；不代发、不后台监控。
```
