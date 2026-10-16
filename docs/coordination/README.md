# 当前协作入口

2026-10-16：W05–W09、W10–W14 与 C09 的已交付切片均已集成；本轮按用户指令完成 main 汇总、测试与已合并分支／worktree 清理，见 [本轮集成报告](reports/MAIN-INTEGRATION-20261016.md)。下述第三轮“草稿／未派工”是协作包编写时的历史状态；当前能力与剩余人工验收以本轮报告及各任务交付报告为准，不重新启动这些任务。

第三轮协作包见 [第三轮协作包](wave03/README.md)（草稿／派工状态以该包 `plan.json` 的 `dispatch_status` 为准）。第二轮（2026-10-15）已交付并合入 main；台账同步见 [PR #10](https://github.com/willmove/joyoffice/pull/10)。

## 第二轮摘要（已完成集成）

- 实际执行与报告：见 [第二轮协作包](wave02/README.md)交付与集成结果。
- 集成结果 main：`ee4f57d3cd6361ff666170e3d40b1f68373b78d4`；W07 平台测试修正 [PR #9](https://github.com/willmove/joyoffice/pull/9) → `5573997a75d9297b24429f90c727dad05b7f90c3`。
- W05–W09：`delivered_integrated`，`acceptance_complete=false`。第三轮不重开这些任务 ID。

## 第三轮（草案要点）

- 前置：合并 PR #10。
- 执行者任务：W10 来源绑定 →（并行）W12 带格式结构导出；W11 `save_original` 发布（依赖 W10）；可选 W13 重锚定 W09；可选／延后 W14 恢复设计。
- 协调者 C09：`open_source`→`DocxSession`、打开＋另存为新副本（`save_export_copy`）＋dirty／关闭；原件保存仅在 W11 可用后接入。
- 延后：ADR-010、信创真机 T005、完整 Word/WPS T004、覆盖引导 fuzz、大 GUI／IME 验收。

第一轮 `plan.json` 与第二轮 [plan.json](wave02/plan.json) 保留为历史。下面是第一轮与 C01–C07 历史记录，其中当前协调者等称谓属于当时记录，不作为后续派工授权。


# 第一轮协作：启动与交付

日期：2026-10-14。总协调者为发出本轮任务的 Codex 聊天。由用户在各软件中启动执行者，并把最终交接转回本聊天。目前没有自动调度、自动跨软件发消息或后台监控。

第一轮成果已由 PR #1 合入 main（2b9d58c2866703f4cd10b7f5e0fc56c961c23c57）。后续由协调者在 codex/coordination-docx-session 分支执行 [C01：DOCX 会话安全接入](tasks/C01-DOCX-SESSION.md)，交付见 [C01 报告](reports/C01-DOCX-SESSION.md)；W01–W04 不重复启动。C01 已合入 main；当前协调者根据用户继续推进项目的指令，在 codex/coordination-docx-export 执行 [C02：保守 DOCX 导出](tasks/C02-DOCX-EXPORT.md)，见 [C02 报告](reports/C02-DOCX-EXPORT.md)。后续独立 worktree 在 codex/coordination-docx-storage 执行 [C03：DOCX 新副本落盘](tasks/C03-DOCX-COPY.md)，见 [C03 报告](reports/C03-DOCX-COPY.md)。C04 后台在 codex/coordination-unicode-wrap 推进 [C04：自动 Unicode 断点](tasks/C04-UNICODE-WRAP.md)，见 [C04 报告](reports/C04-UNICODE-WRAP.md)。C05 由协调者在 codex/coordination-layout-snapshot 执行 [C05：文档排版快照、分页与命中几何](tasks/C05-LAYOUT-SNAPSHOT.md)，见 [C05 报告](reports/C05-LAYOUT-SNAPSHOT.md)；用户已选择继续后台开发，GUI 验证暂缓。当前协调者在 codex/coordination-cpu-canvas 执行 [C06：CPU 页面与真实只读画布](tasks/C06-CPU-CANVAS.md)，见 [C06 报告](reports/C06-CPU-CANVAS.md)。当前输入切片由同一协调者在 codex/coordination-text-input 执行 [C07：真实输入、选择与 IME 组合态](tasks/C07-TEXT-INPUT.md)，见 [C07 报告](reports/C07-TEXT-INPUT.md)；GUI 按用户指示暂缓，仅进行后台开发和无窗口验证。下文保留第一轮派工与交接记录。

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
