# 第二轮：手动派工、Git 交付

日期：2026-10-15。此次只准备协作包，不启动执行 Agent、自动队列、后台监控或 GUI。开发 Agent 协作不涉及产品内 AI 功能。人数固定为 **C08 协调者＋W05 DOCX 导出＋W06 排版＋W07 安全保存**；旧 W01–W04 不重新启动。

## 基线与负责人核查

代码候选基线：`a88217136a570a8f224d92b808bc0b8840ab26ca`，分支 `codex/coordination-text-input`。本地分支、原协调 worktree HEAD、`git ls-remote origin refs/heads/codex/coordination-text-input` 三者一致。当前常用目录 `/Users/willm/Coding/joyoffice` 仍在 C02 `4dd3870a609509ec225a7f85cb8509fbb602dfae`；main 为 `b48ca9b426b80c220f0b2518d1e0ad363e07afea`，都不能作为本轮起点。

C07 已提交 [草稿 PR #7](https://github.com/willmove/joyoffice/pull/7)。本次通过 GitHub CLI 重新查询，[CI 38654508321](https://github.com/willmove/joyoffice/actions/runs/38654508321) 和 [38654515174](https://github.com/willmove/joyoffice/actions/runs/38654515174) 均 completed/success 且 headSha 等于以上 SHA。此为已有源码的远程证据，不是本轮重新执行 Rust 测试，也不代表 C07 已合入 main、GUI 或 Office 验收完成。

| 已有职责 | 可核对的负责人／证据 | 本次结论 |
| --- | --- | --- |
| C02–C07 | 原 Codex 协调聊天 `01a139d9-5699-72d2-87d9-e794103e45c1`；C07 报告与最终交接 | 查询时 idle，最终交接为上述 SHA／PR #7；原目录干净。idle 不等于永久放弃任务，后续与其重叠工作仍须由用户转交接力 |
| W01 | Claude Desktop Code；第一轮 plan 与报告 | 已评审交付，不重开；无法从 Codex 核实 Claude 当前进程 |
| W02 | ZCode Desktop；第一轮 plan 与报告 | 已评审实验，不重开；无法从 Codex 核实 ZCode 当前进程 |
| W03 | 最初分配 DSH，实际交付为 Codex Desktop；第一轮报告 | 已评审交付，不按最初软件名重复启动 |
| W04 | 第一轮协调者接手；报告 | 背景验证已交付，GUI 输入受阻；不是等待重新派开发工作的空白任务 |
| C08 | 本聊天 `01a13d2b-acba-7473-8662-8f183393fd46` | 负责本轮包准备；后续集成沿用这一协调入口，由用户转交结果 |
| W05／W06／W07 | 尚未指定软件、主机、唯一会话 | **待派工**；不把历史软件映射自动当成第二轮负责人 |

以上是 2026-10-15 的观察快照。已向用户请求核实其他软件的在途任务和新负责人；未收到回复时保留待派工状态。可继续准备目录，但不能把“未发现运行中”写成“所有负责人均已确认”。

用户派工前，将每项的 `owner`（软件／主机／唯一会话标识）和 `ownership_confirmed` 转交 C08，由 C08 更新 [plan.json](plan.json) 并交付新的包 SHA；确认原 C07 负责人不再同时开发本轮三个范围及 app 接入。负责人未确认的项不发启动提示。离线／掉线交接先确认原执行停止并保存工作，再安排接手。该 JSON 是人工台账，不是原子认领服务。

## 任务与边界

| 任务 | 首份可验收交付 | 独立分支 | 建议独立目录 |
| --- | --- | --- | --- |
| [C08 协调](../tasks/C08-WAVE02.md) | 负责人／接口裁决、审查集成；执行阶段接续真实打开与导出流程 | `codex/wave02-coordination` | 本次 `/Users/willm/.codex/worktrees/joyoffice-wave02/joyoffice` |
| [W05 DOCX](../tasks/W05-DOCX-EXPORT.md) | 保守正文段落拆分／合并导出与拒绝回归 | `codex/w05-docx-export` | `joyoffice-w05` |
| [W06 排版](../tasks/W06-LAYOUT.md) | 单节单字体 keep-next／keep-lines 分页约束与几何 | `codex/w06-layout` | `joyoffice-w06` |
| [W07 安全保存](../tasks/W07-SAFE-SAVE.md) | 打开源身份、冲突检测与受控原件保存切片 | `codex/w07-safe-save` | `joyoffice-w07` |

精确可写路径在各卡及 plan.json 的 `allowed_paths`。W05 不写 shell，W06 不写字体／整形／PageScene，W07 不写 DOCX 会话或 app。C08 独占公共导出入口、app、命令／文案、会话保存版本、根依赖、CI、公共台账；不在执行者在途时改动其独占文件。所有 Cargo.toml/Cargo.lock 均由 C08 裁决，本轮执行者无新增／升级依赖授权。

W05/W07 可用现有 ExportPlan 和 save_export_copy 独立开发，W06 使用现有 snapshot／PageScene；不互相等待未交付的新 API。接口约定见 [接口与集成](INTERFACES.md)。每人只更新自己的 Wxx 报告；首次创建报告按 [第二轮报告模板](REPORT-TEMPLATE.md)。

## 准备命令

以下为 POSIX shell（macOS／Linux／Windows Git Bash）。原生 Windows 可把 Python 命令中的 `python3` 换成 `py -3`，其余变量语法需使用 Git Bash。每台机器可准备一个全新控制 clone；目录已存在时先检查，不删除、不 reset、不在他人 checkout 切分支：

```sh
git clone --single-branch --branch codex/wave02-coordination git@github.com:willmove/joyoffice.git joyoffice-wave02-control
cd joyoffice-wave02-control
git status --short
git branch --show-current
git rev-parse HEAD
```

无 SSH 权限时将 URL 换成 `https://github.com/willmove/joyoffice.git`。包分支须已推送；无远程权限时由协调者转交 Git bundle 并另给导入命令，不假设远程已发布。

将协调者最终交接的 **40 位协作包提交 SHA** 填入下列变量；它不是上面的 C07 代码 SHA。不能直接把当前远程 tip 当作已审核包。包提交应只在 C07 上增加协作文档／准备工具，不含业务实现；执行开始后不要继续在发布包分支追加业务代码，业务集成使用 `codex/wave02-integration`。

```sh
JOYOFFICE_WAVE02_PACKAGE_SHA='替换为协调者交接的40位协作包SHA'
git fetch --no-tags origin codex/wave02-coordination
test "$(git rev-parse HEAD)" = "$JOYOFFICE_WAVE02_PACKAGE_SHA"
test "$(git rev-parse FETCH_HEAD)" = "$JOYOFFICE_WAVE02_PACKAGE_SHA"
git merge-base --is-ancestor a88217136a570a8f224d92b808bc0b8840ab26ca "$JOYOFFICE_WAVE02_PACKAGE_SHA"
```

以上三项核对任一失败，停止并让 C08 核对包版本，不继续后面的命令。准备器也会拒绝包 SHA 漂移、配置不一致、目录已存在、任务分支本地／远程已存在，避免覆盖工作。它不验证其他软件是否仍运行，负责人核对仍由用户和 C08 完成。

按分配每台机器只运行对应项；同机三位执行者可分别运行这三条，创建三个互不共享 checkout 的目录：

```sh
python3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave02/plan.json --start "$JOYOFFICE_WAVE02_PACKAGE_SHA" --task W05
python3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave02/plan.json --start "$JOYOFFICE_WAVE02_PACKAGE_SHA" --task W06
python3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave02/plan.json --start "$JOYOFFICE_WAVE02_PACKAGE_SHA" --task W07
```

预览加 `--dry-run`（不访问远程，不表示负责人已确认或所有前置已通过）。软件不支持 worktree 时加 `--mode clone`，创建独立 Git 元数据的完整任务 clone。可用 `--destination /绝对/新目录` 改位置。不要在另一机器用同一个任务 ID 同时开第二份。

打开准备器输出的绝对目录，再复制 [对应启动提示](START-PROMPTS.md)。如软件另建 worktree，核对实际 cwd／分支／完整 HEAD，错误时回到已准备目录；不要重复创建任务。Rust 编译缓存按任务独立，未协调前不共用 CARGO_TARGET_DIR。

## 交付与集成

执行者先写自己的报告，然后 `git add <自己的明确文件>`、commit，再 `git push -u origin <自己的任务分支>`。不使用 `git add .`，不强推、不合并或推送 main，不处理他人 checkout 冲突。无远程权限保留本地提交，由用户转交完整 SHA、补丁／bundle 需求。

最终交接必须有：任务 ID、软件／主机、分支、C07 代码基线、协作包起始 SHA、结果完整 SHA、测试命令和真实结果、未覆盖项、阻塞与接口提案。报告正文不自引用最终提交 SHA。用户复制交接回 C08，不由 Agent 跨聊天／软件代发。

C08 先检查路径与冻结 API，再逐个集成到 `codex/wave02-integration`。建议顺序 W05→W07→W06；这只是集成顺序，三项开发可并行。最后才接入 app、保存版本／关闭提示并复验真实 importer→engine→export→storage→reopen 及布局链路。T100/T101 不重写，T002 保留人工验收；T103/T105/T106/T107 不因接口就绪或本轮测试通过直接标 done。GUI 使用时段另约；系统 IME、固定 Word/WPS、平台原子性与元数据实测不能由无窗口测试替代。
