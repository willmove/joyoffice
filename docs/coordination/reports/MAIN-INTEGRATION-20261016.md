# main 汇总、复验与清理（2026-10-16）

用户明确授权“所有的都合并到 main 分支，测试完成后，可删除的（包括 worktree）就删除”。因此本次在协调分支 `codex/integrate-cleanup-20261016` 汇总，测试后快进并推送 main，随后清理已完全合并的任务引用和干净 worktree。此授权覆盖 AGENTS.md 的常规执行者 main 禁令；没有重新派工、跨聊天发消息或 GUI 操作。

## 集成范围与裁决

- 开始复验时本地 main 为 `ee4f57d3cd6361ff666170e3d40b1f68373b78d4`；拉取后生产基线为 main `dcac97e193fbe800787a18a2e7c2724abca97dc7`（PR #19）。W05–W09、W10–W14、C09 交付均已在该基线历史中，不重复开发。
- PR #11 原件替换候选 `f0a8a6105b9cbfc51dad96cd074299f1ed147ee1` 的提交历史合入协调分支；后续协调期间发现本地候选分支新增合并提交 `659fc8bc3ce17e4026806bed2358766d2fe0f4a3`，也纳入历史，其生产文件树已与基线一致。
- **保留当前 W10／W11／C09 生产实现**：来源字节绑定、自有锁令牌、元数据无法保证时拒绝原件写入。旧候选承认检查至 rename 存在竞争窗口，且回执／错误接口已与当前实现分歧，故不启用旧 `/bin/ls` 观察与真正原件覆盖路径。候选报告保留为[带裁决说明的历史记录](W07-ORIGINAL-REPLACE.md)，完整候选代码仍可从 Git 历史恢复。合并历史不表示该候选的原件覆盖已通过验收。
- W09 三处冻结常量及 README 重锚定到生产基线 `dcac97e193fbe800787a18a2e7c2724abca97dc7`，编译／运行仍检查生产 crates 未漂移，不移除守卫。
- CI 的 macOS、Windows Server 2022、Linux x86_64／ARM64 四个原生构建作业增加 W08／W09 独立 workspace 的测试、fmt、clippy 和 W09 Python 测试。构建作业完整拉取历史，确保冻结基线可供核验；未升级依赖。

## 本机复验

主机：Apple M4、macOS 27.0.1 arm64、Rust 1.98.1、Python 3.9.6。根工作区测试设置 `JOYWORD_REQUIRE_TEST_FONT=1`，使用系统字体 `/System/Library/Fonts/Supplemental/Songti.ttc:6`，字体 SHA-256 为 `dd2f0628d302f322a2e9c3cf3253d5e73f418275e1f1421c84a7c65ce5a87777`；不提交字体。

根工作区命令：

```sh
cargo test --locked
cargo test --release --locked
cargo test -p joyword --features dev-runtime-shaders --locked
cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings
cargo build -p joyword --features dev-runtime-shaders --locked
cargo fmt --all -- --check
python3 scripts/check_docs.py
python3 -m unittest discover -s scripts/tests -v
git diff --check
```

初次默认测试因旧构建缓存嵌入已删除 checkout 的 fixture 绝对路径失败。清除本仓库内部包的构建产物后重新完整运行；没有修改测试断言或 fixture，也没有清除依赖源码。最终默认成员 debug／release 各 352 passed，应用 11 passed；fmt、clippy、开发构建、文档检查通过，见[验证汇总](../evidence/MAIN-20261016/local-validation.json)。依赖 `block 0.1.6` 与 `proc-macro-error2 2.0.1` 有 Rust future-incompatibility 提示，未在本轮升级。

独立实验分别使用自身锁文件运行 `cargo test --locked`、`cargo test --release --locked`、`cargo fmt --all -- --check` 与 `cargo clippy --all-targets --locked -- -D warnings`：W08 debug／release 各 15 passed；W09 各 3 passed。W09 Python 11 passed；仓库脚本 Python 19 passed。

W09 在实验源码干净的 `ede89a1820fe1a760f7943b1af730ec5f0d3d9f2` 上运行：81 次冒烟（43 ok／38 拒绝）、种子 20261016 的 600 次变异（93 ok／507 拒绝），均无崩溃、超时或非预期分类。[证据目录](../evidence/MAIN-20261016/)保留摘要和完整逐例日志的 SHA-256／路径；逐例日志留在主工作区忽略的 target 目录。macOS 地址空间限制探针返回 3，未实施 OS 内存硬限制；该实验仍非覆盖引导 fuzz。

## 完成边界与清理规则

最终 main 推送后以该提交的 GitHub Actions 状态核验跨平台门禁；CI 是自动化构建／无窗口测试证据，不能替代 Windows 11、信创真机、Office 往返或 GUI／IME 人工验收。本次未将产品 TASKS 标成 done；W08 仍为恢复实验，W14 仍为生产恢复设计，原件保存仍受 W11 安全门禁限制。

仅删除当前 main 可达、无未提交／未跟踪源码的任务 worktree 与分支；重新检查实际 HEAD，避免删除协调期间的新提交。候选删除目录为 `joyoffice-cifix`、`joyoffice-ledger`、`.claude/worktrees/modest-mclean-eb3020`，忽略文件仅为可重建 target 产物。主工作区、历史交付报告和 Git 提交历史保留。最终执行结果以 main、远程引用、worktree 列表及交接消息为准。

## 复验期间追加：PR #20

首次 main 集成到 `fea6358b6d92c1ef23bf354f8f4a4779534c3636` 后，清理复查发现 Claude worktree 新增未提交修改，因此暂缓删除。该执行者随后在 `claude/w11-confirmed-publish` 提交 `5dec164f75417e00e5532d187fcea722134cd266` 并发布 [PR #20](https://github.com/willmove/joyoffice/pull/20)。按用户汇总全部交付的授权，本协调分支审查后纳入这份已提交增量，没有接管其未提交文件。

协调裁决：批准[增量报告](W07-REPLACE-INCREMENT.md)提出的读回阶段、未确认发布错误、只读阻塞与回执来源 stamp 接口；保留 W11 的来源绑定、锁令牌与全部尚未消除的元数据阻塞。Unix owner／组和 macOS 创建时间改为可恢复，元数据恢复先于文件 sync，发布前后均核对；公开原件保存仍在所有平台改写前拒绝。不存在“原件覆盖验收通过”的结论。

审查发现 `confirm_published` 在确认读取预算前按外部可变的文件长度分配容量。若同一 inode 在确认前被扩展，这会绕过读取预算对内存分配的约束。本协调者增加分配前长度／预算核验和读取后上限核验，并以 8 字节预算下同一文件扩展至 9 字节的回归测试验证 `FileTooLarge` 拒绝，文件不被改写。

修复后的生产基线为 `23b3901c54598ccf7c2195bcf6eda2a3df02043d`；W09 再次重锚定到该固定提交，而非移除守卫。前述首次验证证据保留，追加复验独立记录。清理仍以最终检查时 main 祖先关系和 worktree 实际状态为准；新在途修改不得删除。

追加复验全部通过：默认成员 debug／release 各 364 项，应用 11 项；W08 各 15 项，W09 各 3 项与 Python 11 项；root／独立 workspace 的 fmt、clippy、应用开发构建、文档及 diff 检查通过。W09 在新基线上重跑 81＋600 次，分类与首次相同且无异常；详见[追加验证汇总](../evidence/MAIN-20261016/pr20-validation.json)、[冒烟摘要](../evidence/MAIN-20261016/pr20-w09-smoke-summary.json)与[变异摘要](../evidence/MAIN-20261016/pr20-w09-campaign-summary.json)。首次 main 的[八项 CI 门禁](https://github.com/willmove/joyoffice/actions/runs/38831307604)已全绿；追加提交仍以其自身的 main CI 作为最终远程验证。
