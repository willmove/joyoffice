# 第二轮：手动派工、Git 交付

日期：2026-10-15。此次只准备协作包，不启动执行 Agent、自动队列、后台监控或 GUI。开发 Agent 协作不涉及产品内 AI 功能。基本配置为 **C08 协调者＋W05 DOCX 导出＋W06 排版＋W07 安全保存**；按用户补充可扩为 **1＋5**，另一主机预留 W08 恢复日志与 W09 安全测试工程；旧 W01–W04 不重新启动。

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
| W05 | 当前主机 Claude Desktop Code | 已分配、未启动；启动时登记唯一会话 |
| W06 | 当前主机 Codex Desktop 的独立执行聊天 | 已分配、未启动；与本 C08 协调聊天分开 |
| W07 | 当前主机 ZCode Desktop | 已分配、未启动；启动时登记唯一会话 |
| W08 | Windows 11 x64 主机 Claude Desktop Code | 可选预留：恢复日志与崩溃重放；环境已确认，待启用与唯一会话登记 |
| W09 | Windows 11 x64 主机 Codex Desktop | 可选预留：DOCX 安全回归与 fuzz 工程；环境已确认，待启用与唯一会话登记 |

以上核查为 2026-10-15 的观察快照。用户随后先回复“未分配”，再指定当前主机 Codex／Claude／ZCode，并允许增加另一台主机的 Claude／Codex。C08 据此登记本地三项分配与远端两项预留；角色映射是本轮安排，不是各软件能力排名。用户已确认另一台主机为 Windows 11 x64；环境确认不等于该平台的测试已通过。

本聊天只协调；本机 W06 应在其独立任务目录的执行聊天完成。用户把启动提示交给对应软件时登记唯一会话，并检查是否已存在同任务分支／进程；不要重新启动原 C07 聊天同时承担这些范围。`ownership_confirmed=true` 表示软件／主机分配已确定，不表示执行进程已启动或外部软件已检查。远端两项由用户启用后登记主机／会话，不需要重新规划任务；若暂不启用，不阻塞 W05–W07。该 JSON 仍是人工台账，不是原子认领服务。离线接手先确认原执行停止并保存工作。

## 任务与边界

| 任务 | 首份可验收交付 | 独立分支 | 建议独立目录 |
| --- | --- | --- | --- |
| [C08 协调](../tasks/C08-WAVE02.md) | 负责人／接口裁决、审查集成；执行阶段接续真实打开与导出流程 | `codex/wave02-coordination` | 本次 `/Users/willm/.codex/worktrees/joyoffice-wave02/joyoffice` |
| [W05 DOCX](../tasks/W05-DOCX-EXPORT.md) | 保守正文段落拆分／合并导出与拒绝回归 | `codex/w05-docx-export` | `joyoffice-w05` |
| [W06 排版](../tasks/W06-LAYOUT.md) | 单节单字体 keep-next／keep-lines 分页约束与几何 | `codex/w06-layout` | `joyoffice-w06` |
| [W07 安全保存](../tasks/W07-SAFE-SAVE.md) | 打开源身份、冲突检测与受控原件保存切片 | `codex/w07-safe-save` | `joyoffice-w07` |
| [W08 恢复内核](../tasks/W08-RECOVERY.md) | 日志／真实事务重放／检查点与崩溃恢复工具，分三批 | `codex/w08-recovery-journal` | `joyoffice-w08` |
| [W09 安全测试](../tasks/W09-DOCX-ROBUSTNESS.md) | harness／结构化变异与最小化／往返证据，分三批 | `codex/w09-docx-robustness` | `joyoffice-w09` |

精确可写路径在各卡及 plan.json 的 `allowed_paths`。W05 不写 shell，W06 不写字体／整形／PageScene，W07 不写 DOCX 会话或 app。C08 独占公共导出入口、app、命令／文案、会话保存版本、根依赖、CI、公共台账；不在执行者在途时改动其独占文件。根及生产 crate 的 Cargo.toml/Cargo.lock 均由 C08 裁决。W08/W09 仅在各自实验目录获准创建独立 manifest／锁文件，按任务卡使用已列精确版本；其他新增／升级依赖先提案。

W05/W07 可用现有 ExportPlan 和 save_export_copy 独立开发，W06 使用现有 snapshot／PageScene；不互相等待未交付的新 API。接口约定见 [接口与集成](INTERFACES.md)。W08/W09 只写各自独立实验目录及报告／证据，生产接口只读，互不修改对方代码；不依赖 W05/W07 的在途分支。每人只更新自己的 Wxx 报告；首次创建报告按 [第二轮报告模板](REPORT-TEMPLATE.md)。

## 准备命令

以下为 POSIX shell（macOS／Linux／Windows Git Bash）。原生 Windows 可把 Python 命令中的 `python3` 换成 `py -3`，其余变量语法需使用 Git Bash。每台机器可准备一个全新控制 clone；目录已存在时先检查，不删除、不 reset、不在他人 checkout 切分支：

```sh
git clone --single-branch --branch codex/wave02-coordination git@github.com:willmove/joyoffice.git joyoffice-wave02-control
cd joyoffice-wave02-control
git status --short
git branch --show-current
git rev-parse HEAD
```

已有本人维护、干净且位于 `codex/wave02-coordination` 的控制 clone，可先 `git pull --ff-only origin codex/wave02-coordination` 更新协作包，再核对新版完整 SHA。不要在执行者的任务 checkout 运行此更新步骤，也不覆盖未提交修改。

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

另一台主机启用后，在该机自己的控制 clone 中运行（不要在本机为远端任务重复开执行者）：

```sh
python3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave02/plan.json --start "$JOYOFFICE_WAVE02_PACKAGE_SHA" --task W08 --mode clone
python3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave02/plan.json --start "$JOYOFFICE_WAVE02_PACKAGE_SHA" --task W09 --mode clone
```

### Windows 11 x64 的原生 PowerShell 准备

W08／W09 优先在原生 Windows 的 MSVC Rust 环境运行，以取得该目标的真实文件系统与进程证据；若使用 WSL，报告必须单列为 Linux 环境。先确认 Git、Python 3（`py -3 --version`）和固定 Rust（`rustc -vV`，1.98.1／x86_64-pc-windows-msvc）；缺少工具时记录阻塞，不把环境确认当作已安装。目录路径可带空格，以下命令保留引号。

在计划存放仓库的父目录中执行；新控制 clone 目录必须不存在。填入本轮最新交接的包 SHA，执行失败即停止：

```powershell
$ErrorActionPreference = 'Stop'
$JoyofficeWave02PackageSha = '替换为协调者交接的40位协作包SHA'
if ($JoyofficeWave02PackageSha -cnotmatch '^[0-9a-f]{40}$') { throw '请填写完整协作包 SHA' }
function Invoke-JoyGit {
    & git @args
    if ($LASTEXITCODE -ne 0) { throw 'Git 命令失败，保留现场后核对' }
}
Invoke-JoyGit clone --single-branch --branch codex/wave02-coordination 'https://github.com/willmove/joyoffice.git' 'joyoffice-wave02-control'
Set-Location -LiteralPath 'joyoffice-wave02-control' -ErrorAction Stop
Invoke-JoyGit status --short
Invoke-JoyGit fetch --no-tags origin codex/wave02-coordination
$JoyofficeLocalHead = Invoke-JoyGit rev-parse HEAD
$JoyofficeRemoteHead = Invoke-JoyGit rev-parse FETCH_HEAD
if ($JoyofficeLocalHead -cne $JoyofficeWave02PackageSha -or $JoyofficeRemoteHead -cne $JoyofficeWave02PackageSha) { throw '协作包 SHA 不一致，停止准备' }
Invoke-JoyGit merge-base --is-ancestor a88217136a570a8f224d92b808bc0b8840ab26ca $JoyofficeWave02PackageSha
py -3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave02/plan.json --start $JoyofficeWave02PackageSha --task W08 --mode clone
if ($LASTEXITCODE -ne 0) { throw 'W08 准备失败，不重建已有目录' }
py -3 scripts/prepare_agent_workspace.py --plan docs/coordination/wave02/plan.json --start $JoyofficeWave02PackageSha --task W09 --mode clone
if ($LASTEXITCODE -ne 0) { throw 'W09 准备失败，不重建已有目录' }
```

只启用一项时仅执行相应准备命令。控制 clone 若已存在，使用上文的干净目录更新流程后，从 status／fetch／SHA 核对开始，跳过 clone／Set-Location。准备器会输出两个独立绝对目录；Claude 打开 W08，Codex 打开 W09。这些 PowerShell 命令是待远端实测的操作步骤，本机 macOS 文档检查不能证明 Windows 执行成功。

本机三个编译任务可设置各自 `CARGO_BUILD_JOBS=2` 作为初始资源上限，并交错执行 release／全量 clippy；视实际内存再调整，不宣称未经测量的吞吐。GUI 仍一次只有一位执行者。远端长 campaign 必须有结束条件，不常驻监控。

预览加 `--dry-run`（不访问远程，不表示负责人已确认或所有前置已通过）。软件不支持 worktree 时加 `--mode clone`，创建独立 Git 元数据的完整任务 clone。可用 `--destination /绝对/新目录` 改位置。不要在另一机器用同一个任务 ID 同时开第二份。

打开准备器输出的绝对目录，再复制 [对应启动提示](START-PROMPTS.md)。如软件另建 worktree，核对实际 cwd／分支／完整 HEAD，错误时回到已准备目录；不要重复创建任务。Rust 编译缓存按任务独立，未协调前不共用 CARGO_TARGET_DIR。

## 交付与集成

执行者先写自己的报告，然后 `git add <自己的明确文件>`、commit，再 `git push -u origin <自己的任务分支>`。不使用 `git add .`，不强推、不合并或推送 main，不处理他人 checkout 冲突。无远程权限保留本地提交，由用户转交完整 SHA、补丁／bundle 需求。

最终交接必须有：任务 ID、软件／主机、分支、C07 代码基线、协作包起始 SHA、结果完整 SHA、测试命令和真实结果、未覆盖项、阻塞与接口提案。报告正文不自引用最终提交 SHA。用户复制交接回 C08，不由 Agent 跨聊天／软件代发。

C08 先检查路径与冻结 API，再逐个集成到 `codex/wave02-integration`。建议顺序 W05→W07→W06；这只是集成顺序，三项开发可并行。最后才接入 app、保存版本／关闭提示并复验真实 importer→engine→export→storage→reopen 及布局链路。T100/T101 不重写，T002 保留人工验收；T103/T105/T106/T107 不因接口就绪或本轮测试通过直接标 done。GUI 使用时段另约；系统 IME、固定 Word/WPS、平台原子性与元数据实测不能由无窗口测试替代。

W08/W09 分批交付真实代码／测试，先审查独立工程再安排生产接入。W09 发现 bug 由 C08 分回生产文件负责人，不越界修补；W08 正式日志接入与 W07 文件保存职责分开。二者不会自动使 T104／T107 变为 done。
