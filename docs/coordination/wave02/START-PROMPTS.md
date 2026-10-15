# 可复制的第二轮启动提示

本机已分配三项，远端两项可选预留；由用户手动把对应提示交给对应软件，不自动创建聊天或代发。把目录、协作包 SHA 和唯一会话标识补齐后启动。全部新启动任务采用本轮更新后的同一个包 SHA，不使用旧版 fbea51080a46752e4532544731748b95a951c851。若已经有人从旧包开工，先交接其真实 HEAD 和状态，保留工作，不能强制重建或 reset。

本聊天 C08 保持协调；本机 W06 使用独立执行聊天和目录。远端 W08/W09 的 Windows 11 x64 环境已确认，由用户启用时登记唯一会话，不影响本机三项开始。

## W05：当前主机 · Claude Desktop / Code · DOCX 导出

```text
你负责 JoyOffice 第二轮 W05（DOCX 导出），由用户手动派工。
指定负责人：当前主机的 Claude Desktop / Code。本执行会话标识：启动时记录，保持每任务只有一个活跃执行者。
已准备的独立绝对目录：填入准备器输出路径。
任务分支：codex/w05-docx-export。
代码基线：a88217136a570a8f224d92b808bc0b8840ab26ca。
协作包起始完整 SHA：填入 C08 最新交接的40位SHA（不是旧版 fbea510，也不是 C07 代码SHA）。
先运行 pwd、git status --short、git branch --show-current、git rev-parse HEAD、git merge-base --is-ancestor a88217136a570a8f224d92b808bc0b8840ab26ca HEAD。
首次启动 HEAD 必须等于所分配包 SHA；检查本项是否已有分支／目录／其他执行进程，冲突时报告，不 reset、不重复开工。续做先确认自己的历史与未提交修改归属。软件另建 worktree 时核对实际 cwd 与分支，不从 main 或 C02 重做。
阅读 AGENTS.md、docs/coordination/README.md、docs/coordination/wave02/README.md、docs/coordination/wave02/INTERFACES.md、docs/coordination/tasks/W05-DOCX-EXPORT.md 和卡中规格。
按任务卡完成全部已授权交付；分批提交和测试，不在第一批后自动停止。只写 allowed_paths，依赖仅按任务卡已批准范围使用，不改冻结 API、不重写 T100/T101。需跨界时先完成独立部分并在报告提出精确方案，由用户交给 C08。
把实际环境、变更、支持／拒绝范围、完整测试命令与结果、未覆盖项及阻塞写到 docs/coordination/reports/W05.md，参照 docs/coordination/wave02/REPORT-TEMPLATE.md。
只提交自己的明确文件，有权限时 git push -u origin codex/w05-docx-export；不强推、不合并或推送 main、不处理他人目录冲突。
最终输出任务 ID、软件／主机／唯一会话、分支、代码基线、包起始 SHA、git rev-parse HEAD 的真实结果 SHA、推送状态、测试及阻塞；报告正文不自引用结果 SHA。
缺字体／工具／目标系统写未验证，不伪造验收。GUI 先约时段，同一桌面只有一位执行者；后台编译采用独立缓存并控制并发。
由用户把交接转给 C08，不自行联系其他聊天／软件，不启动其他执行者或自动服务。完成全部任务卡范围后等待下一次手动分配。
```

## W06：当前主机 · Codex Desktop（独立执行聊天，非 C08） · 排版

```text
你负责 JoyOffice 第二轮 W06（排版），由用户手动派工。
指定负责人：当前主机的 Codex Desktop（独立执行聊天，非 C08）。本执行会话标识：启动时记录，保持每任务只有一个活跃执行者。
已准备的独立绝对目录：填入准备器输出路径。
任务分支：codex/w06-layout。
代码基线：a88217136a570a8f224d92b808bc0b8840ab26ca。
协作包起始完整 SHA：填入 C08 最新交接的40位SHA（不是旧版 fbea510，也不是 C07 代码SHA）。
先运行 pwd、git status --short、git branch --show-current、git rev-parse HEAD、git merge-base --is-ancestor a88217136a570a8f224d92b808bc0b8840ab26ca HEAD。
首次启动 HEAD 必须等于所分配包 SHA；检查本项是否已有分支／目录／其他执行进程，冲突时报告，不 reset、不重复开工。续做先确认自己的历史与未提交修改归属。软件另建 worktree 时核对实际 cwd 与分支，不从 main 或 C02 重做。
阅读 AGENTS.md、docs/coordination/README.md、docs/coordination/wave02/README.md、docs/coordination/wave02/INTERFACES.md、docs/coordination/tasks/W06-LAYOUT.md 和卡中规格。
按任务卡完成全部已授权交付；分批提交和测试，不在第一批后自动停止。只写 allowed_paths，依赖仅按任务卡已批准范围使用，不改冻结 API、不重写 T100/T101。需跨界时先完成独立部分并在报告提出精确方案，由用户交给 C08。
把实际环境、变更、支持／拒绝范围、完整测试命令与结果、未覆盖项及阻塞写到 docs/coordination/reports/W06.md，参照 docs/coordination/wave02/REPORT-TEMPLATE.md。
只提交自己的明确文件，有权限时 git push -u origin codex/w06-layout；不强推、不合并或推送 main、不处理他人目录冲突。
最终输出任务 ID、软件／主机／唯一会话、分支、代码基线、包起始 SHA、git rev-parse HEAD 的真实结果 SHA、推送状态、测试及阻塞；报告正文不自引用结果 SHA。
缺字体／工具／目标系统写未验证，不伪造验收。GUI 先约时段，同一桌面只有一位执行者；后台编译采用独立缓存并控制并发。
由用户把交接转给 C08，不自行联系其他聊天／软件，不启动其他执行者或自动服务。完成全部任务卡范围后等待下一次手动分配。
```

## W07：当前主机 · ZCode Desktop · 安全保存

```text
你负责 JoyOffice 第二轮 W07（安全保存），由用户手动派工。
指定负责人：当前主机的 ZCode Desktop。本执行会话标识：启动时记录，保持每任务只有一个活跃执行者。
已准备的独立绝对目录：填入准备器输出路径。
任务分支：codex/w07-safe-save。
代码基线：a88217136a570a8f224d92b808bc0b8840ab26ca。
协作包起始完整 SHA：填入 C08 最新交接的40位SHA（不是旧版 fbea510，也不是 C07 代码SHA）。
先运行 pwd、git status --short、git branch --show-current、git rev-parse HEAD、git merge-base --is-ancestor a88217136a570a8f224d92b808bc0b8840ab26ca HEAD。
首次启动 HEAD 必须等于所分配包 SHA；检查本项是否已有分支／目录／其他执行进程，冲突时报告，不 reset、不重复开工。续做先确认自己的历史与未提交修改归属。软件另建 worktree 时核对实际 cwd 与分支，不从 main 或 C02 重做。
阅读 AGENTS.md、docs/coordination/README.md、docs/coordination/wave02/README.md、docs/coordination/wave02/INTERFACES.md、docs/coordination/tasks/W07-SAFE-SAVE.md 和卡中规格。
按任务卡完成全部已授权交付；分批提交和测试，不在第一批后自动停止。只写 allowed_paths，依赖仅按任务卡已批准范围使用，不改冻结 API、不重写 T100/T101。需跨界时先完成独立部分并在报告提出精确方案，由用户交给 C08。
把实际环境、变更、支持／拒绝范围、完整测试命令与结果、未覆盖项及阻塞写到 docs/coordination/reports/W07.md，参照 docs/coordination/wave02/REPORT-TEMPLATE.md。
只提交自己的明确文件，有权限时 git push -u origin codex/w07-safe-save；不强推、不合并或推送 main、不处理他人目录冲突。
最终输出任务 ID、软件／主机／唯一会话、分支、代码基线、包起始 SHA、git rev-parse HEAD 的真实结果 SHA、推送状态、测试及阻塞；报告正文不自引用结果 SHA。
缺字体／工具／目标系统写未验证，不伪造验收。GUI 先约时段，同一桌面只有一位执行者；后台编译采用独立缓存并控制并发。
由用户把交接转给 C08，不自行联系其他聊天／软件，不启动其他执行者或自动服务。完成全部任务卡范围后等待下一次手动分配。
```

## W08：另一台主机（Windows 11 x64） · Claude Desktop / Code · 恢复日志与崩溃重放

```text
你负责 JoyOffice 第二轮 W08（恢复日志与崩溃重放），由用户手动派工。
环境：Windows 11 x64，优先原生 x86_64-pc-windows-msvc／Rust 1.98.1。PowerShell 中用 Get-Location 代替 pwd、py -3 代替 python3；记录实际工具版本与文件系统，WSL 结果单列，不算原生 Windows 验证。
指定负责人：另一台主机（Windows 11 x64）的 Claude Desktop / Code。本执行会话标识：启动时记录，保持每任务只有一个活跃执行者。
已准备的独立绝对目录：填入准备器输出路径。
任务分支：codex/w08-recovery-journal。
代码基线：a88217136a570a8f224d92b808bc0b8840ab26ca。
协作包起始完整 SHA：填入 C08 最新交接的40位SHA（不是旧版 fbea510，也不是 C07 代码SHA）。
先运行 pwd、git status --short、git branch --show-current、git rev-parse HEAD、git merge-base --is-ancestor a88217136a570a8f224d92b808bc0b8840ab26ca HEAD。
首次启动 HEAD 必须等于所分配包 SHA；检查本项是否已有分支／目录／其他执行进程，冲突时报告，不 reset、不重复开工。续做先确认自己的历史与未提交修改归属。软件另建 worktree 时核对实际 cwd 与分支，不从 main 或 C02 重做。
阅读 AGENTS.md、docs/coordination/README.md、docs/coordination/wave02/README.md、docs/coordination/wave02/INTERFACES.md、docs/coordination/tasks/W08-RECOVERY.md 和卡中规格。
按任务卡完成全部已授权交付；分批提交和测试，不在第一批后自动停止。只写 allowed_paths，依赖仅按任务卡已批准范围使用，不改冻结 API、不重写 T100/T101。需跨界时先完成独立部分并在报告提出精确方案，由用户交给 C08。
把实际环境、变更、支持／拒绝范围、完整测试命令与结果、未覆盖项及阻塞写到 docs/coordination/reports/W08.md，参照 docs/coordination/wave02/REPORT-TEMPLATE.md。
只提交自己的明确文件，有权限时 git push -u origin codex/w08-recovery-journal；不强推、不合并或推送 main、不处理他人目录冲突。
最终输出任务 ID、软件／主机／唯一会话、分支、代码基线、包起始 SHA、git rev-parse HEAD 的真实结果 SHA、推送状态、测试及阻塞；报告正文不自引用结果 SHA。
缺字体／工具／目标系统写未验证，不伪造验收。GUI 先约时段，同一桌面只有一位执行者；后台编译采用独立缓存并控制并发。
由用户把交接转给 C08，不自行联系其他聊天／软件，不启动其他执行者或自动服务。完成全部任务卡范围后等待下一次手动分配。
```

## W09：另一台主机（Windows 11 x64） · Codex Desktop · DOCX 安全回归与 fuzz 工程

```text
你负责 JoyOffice 第二轮 W09（DOCX 安全回归与 fuzz 工程），由用户手动派工。
环境：Windows 11 x64，优先原生 x86_64-pc-windows-msvc／Rust 1.98.1。PowerShell 中用 Get-Location 代替 pwd、py -3 代替 python3；记录实际工具版本与文件系统，WSL 结果单列，不算原生 Windows 验证。
指定负责人：另一台主机（Windows 11 x64）的 Codex Desktop。本执行会话标识：启动时记录，保持每任务只有一个活跃执行者。
已准备的独立绝对目录：填入准备器输出路径。
任务分支：codex/w09-docx-robustness。
代码基线：a88217136a570a8f224d92b808bc0b8840ab26ca。
协作包起始完整 SHA：填入 C08 最新交接的40位SHA（不是旧版 fbea510，也不是 C07 代码SHA）。
先运行 pwd、git status --short、git branch --show-current、git rev-parse HEAD、git merge-base --is-ancestor a88217136a570a8f224d92b808bc0b8840ab26ca HEAD。
首次启动 HEAD 必须等于所分配包 SHA；检查本项是否已有分支／目录／其他执行进程，冲突时报告，不 reset、不重复开工。续做先确认自己的历史与未提交修改归属。软件另建 worktree 时核对实际 cwd 与分支，不从 main 或 C02 重做。
阅读 AGENTS.md、docs/coordination/README.md、docs/coordination/wave02/README.md、docs/coordination/wave02/INTERFACES.md、docs/coordination/tasks/W09-DOCX-ROBUSTNESS.md 和卡中规格。
按任务卡完成全部已授权交付；分批提交和测试，不在第一批后自动停止。只写 allowed_paths，依赖仅按任务卡已批准范围使用，不改冻结 API、不重写 T100/T101。需跨界时先完成独立部分并在报告提出精确方案，由用户交给 C08。
把实际环境、变更、支持／拒绝范围、完整测试命令与结果、未覆盖项及阻塞写到 docs/coordination/reports/W09.md，参照 docs/coordination/wave02/REPORT-TEMPLATE.md。
只提交自己的明确文件，有权限时 git push -u origin codex/w09-docx-robustness；不强推、不合并或推送 main、不处理他人目录冲突。
最终输出任务 ID、软件／主机／唯一会话、分支、代码基线、包起始 SHA、git rev-parse HEAD 的真实结果 SHA、推送状态、测试及阻塞；报告正文不自引用结果 SHA。
缺字体／工具／目标系统写未验证，不伪造验收。GUI 先约时段，同一桌面只有一位执行者；后台编译采用独立缓存并控制并发。
由用户把交接转给 C08，不自行联系其他聊天／软件，不启动其他执行者或自动服务。完成全部任务卡范围后等待下一次手动分配。
```

## C08：用户转回执行结果后

```text
你是 JoyOffice 第二轮唯一协调者 C08。继续使用现有协调聊天，不额外启动第二位协调者或执行 Agent。
阅读 docs/coordination/tasks/C08-WAVE02.md、docs/coordination/wave02/README.md、INTERFACES.md 和 plan.json。
先核实用户转交的负责人、任务 ID、分支与完整结果 SHA；检查既有活跃负责人，避免与原 C07 会话重复开发。
本次收到的任务及结果：填入用户转交的完整交接。
协作包起始完整 SHA：填入已派工包 SHA。
在自己的独立集成目录／codex/wave02-integration 分支核查祖先、diff 文件边界、来源、冻结接口及测试，再依卡审查集成。不要向发布包分支加入业务代码。
需要准备集成目录时，在控制 clone 执行 git worktree add -b codex/wave02-integration ../joyoffice-wave02-integration <协作包完整SHA>；目录／分支已存在时核查并复用自己的工作，不重复创建，不 reset。
公共文档、app 接入、保存版本跟踪、根依赖／CI 由你维护；执行者的在途文件不同时改动。完成相关与集成测试后交付分支／真实 SHA／实际结果／未覆盖项，不推 main，不把接口通过写成完整人工验收。
采用手动派工、Git 交付，用户转交消息；不代发、不后台监控。GUI 时段另约。完成后等待下一次分配。
```
