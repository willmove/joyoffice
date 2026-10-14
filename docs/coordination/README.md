# 第一轮协作：启动与交付

日期：2026-10-14。总协调者为发出本轮任务的 Codex 聊天。由用户在各软件中启动执行者，并把最终交接转回本聊天。目前没有自动调度、自动跨软件发消息或后台监控。

第一轮成果已由 PR #1 合入 main（2b9d58c2866703f4cd10b7f5e0fc56c961c23c57）。后续由协调者在 codex/coordination-docx-session 分支执行 [C01：DOCX 会话安全接入](tasks/C01-DOCX-SESSION.md)，交付见 [C01 报告](reports/C01-DOCX-SESSION.md)；W01–W04 不重复启动。下文保留第一轮派工与交接记录。

## 基线与分配

代码基线 c3fabea449ea25a876301b35305105e00cafba06（main；T002 已人工验收）。任务从包含本协作包的 codex/multi-agent-coordination 分支启动，记录启动时完整 HEAD。机器清单取得后可重新映射机器，任务 ID 保持不变。

| 任务 | 执行者 | 第一份交付 | 环境 |
| --- | --- | --- | --- |
| [W01](tasks/W01-DOCX.md) | Claude Desktop 的 Code 会话 | DOCX 最小只读导入与原始包保留 | 任一 Rust 主机 |
| [W02](tasks/W02-GPUI.md) | ZCode Desktop | 独立 GPUI 字体/字形绘制探针 | Mac 优先 |
| [W03](tasks/W03-LAYOUT.md) | DSH Desktop | 单字体横向段落断行切片 | 任一 Rust 主机 |
| [W04](tasks/W04-VALIDATION.md) | 空闲主机上的一个 Agent，可选 | 平台或 Word/WPS 参考证据 | 有对应环境的主机 |

每项只启动一个负责人。W04 可以暂不启动。应用外壳、完整 IME、编辑保存和整体分页留到下一轮。

## 用户操作

1. 每台主机准备一个控制 clone。已有干净 clone 可复用；新 clone 使用以下两种方式之一，目标目录不得已存在：

   ```sh
   git clone git@github.com:willmove/joyoffice.git joyoffice-control
   ```

   未配置 SSH 时可使用已授权的 HTTPS 方式：

   ```sh
   git clone https://github.com/willmove/joyoffice.git joyoffice-control
   ```

2. 在控制 clone 内执行：

   ```sh
   git fetch origin codex/multi-agent-coordination
   git switch --track origin/codex/multi-agent-coordination
   ```

   已有该本地分支时用 git switch codex/multi-agent-coordination，再 git pull --ff-only。不要在有未提交代码的目录切换。

3. 在控制 clone 为对应任务执行一条命令（Windows 把 python3 换成 py -3）：

   ```sh
   python3 scripts/prepare_agent_workspace.py --task W01
   ```

   W02/W03 替换任务 ID。脚本在控制 clone 的同级创建 joyoffice-w01 等独立 worktree 和任务分支，输出绝对路径和任务卡。发现目录/分支已存在时停止，不覆盖。软件不支持 worktree 时加 --mode clone 创建完整独立 clone；仅显示步骤用 --dry-run。无需给桌面 Agent 安装 CLI。

4. 在对应桌面软件打开输出的目录，新建编程会话。Claude 使用 Code 会话。选择在这个目录执行；如果软件自动另建 worktree，先核对实际基线和任务分支。
5. 发下面的启动提示。完成或阻塞后，把最终报告转回总协调聊天，包含提交 SHA；不要只转“完成了”。

## 复制给执行者的启动提示

```text
你是 JoyOffice 第一轮协作执行者，任务 W01。
先检查 git status、git branch --show-current、git rev-parse HEAD，阅读 AGENTS.md、docs/coordination/README.md、docs/coordination/tasks/W01-DOCX.md。
确认独立目录、任务分支与基线正确后，直接完成任务卡的第一份交付，不扩展到整个 M1，不修改其他执行者的文件。
把环境、变更、完整测试命令与真实结果、未覆盖项和阻塞写到 docs/coordination/reports/W01.md，格式参照 TEMPLATE.md。
提交自己的明确文件，有权限时推送自己的任务分支；不强推、不合并或推送 main。
最终消息附任务 ID、分支、基线 SHA、git rev-parse HEAD 的结果提交 SHA、测试与阻塞。报告正文不写自己的最终提交 SHA，以免自引用。
缺环境或冻结接口时，先完成独立部分，提交准确缺口；不猜测通过，不为任务卡内常规工作反复询问是否开工。
```

W02 将 ID/报告改为 W02，任务卡为 docs/coordination/tasks/W02-GPUI.md。
W03 改为 W03，任务卡为 docs/coordination/tasks/W03-LAYOUT.md。
W04 改为 W04，任务卡为 docs/coordination/tasks/W04-VALIDATION.md。

## 边界和接力

- W01 独占新 crates/joyword-docx/**，可更新根 Cargo 文件以接入真实实现。
- W02 独占 experiments/gpui-font-probe/**，自己的 workspace/锁文件；不改正式应用、根依赖或现有 layout。
- W03 独占 layout 新 line.rs、新测试/示例和增量模块导出；不改 font/shape/scene/PDF 公共接口。
- W04 只新增自己的证据；不重生成语料、不改清单或比较器。
- 各人只写自己的报告；协调者维护公共状态、接口提案、产品文档、ADR、CI 和集成。
- W01/W03 可使用已验证的现有接口，不等待 W02。W03 的切片不等于 T105 全量验收。
- plan.json 是初始分配快照，不是在线原子队列；执行者不自行认领第二项。掉线先确认原进程停止、保存工作，再安排接手。
- interface_ready、implementation_verified、acceptance_complete 是交接字段，各附证据，不改变产品 TASKS 的状态枚举。T100/T101 完整人工验收依赖后续界面，不能阻止使用其已验证接口，也不能据此提前标 done。

## 用户负责什么

选择正确目录、启动会话；提供主机清单（电脑名/系统/Agent/Word-WPS 及版本）；系统授权、安装组件与 GUI 人工操作按实际需要处理；一次只允许一个执行者控制同一桌面；将交接或阻塞转回本聊天。第一轮不需要你开发任务服务。

## 交付与集成

按[报告模板](reports/TEMPLATE.md)交付。推送命令为 git push -u origin <自己的任务分支>。首轮不要求执行者自己建 PR。协调者核对 diff、路径边界、规范来源和测试，再合并到集成分支并复验。

W02 探针通过不等于批准正式 GPUI 升级；W01 读取不等于保存可用；W03 限定断行不等于 Word 排版保真。
