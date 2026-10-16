# C09 第三轮交接报告：打开会话与另存为新副本

- 任务 ID／唯一负责人（软件／主机／会话）：C09 第一个切片（C09-A，以及 C09-B 的打开、另存为、dirty／关闭）。当前 Cloud Agent 会话，仓库 willmove/joyoffice，工作目录 `/workspace`。公共负责人台账由协调者更新。
- 系统／架构／工具链：Linux 6.12.94+ x86_64；Rust 1.98.1（48a229cea 2026-09-01），edition 2024。应用二进制在本机链接时需要 `libxkbcommon`，并且 `libstdc++.so` 位于 GCC 目录，测试时设置了 `LIBRARY_PATH=/usr/lib/gcc/x86_64-linux-gnu/13`。没有打开图形窗口。
- 代码基线（PR #10 合并后 main tip）：`47747952f97d7a82fc0be7768bbea52ed909ec61`。
- 协作包起始完整 SHA：`31dcedf3cc7756a23e2c9d921a82f06f04d7fbb0`。本切片从 W14 合入后的 main `a15d2401bd1b77b53b21ecbaa890745ac435bed8` 开工。
- 实际独立目录／任务分支：`/workspace`（不是独立 worktree）／`codex/c09-open-source-session`。
- 状态：完成限定切片。只做打开接入、另存为新副本、dirty／关闭提示。没有实现 `save_original`，没有接入恢复日志。
- interface_ready：true，仅限下面写明的打开与另存子集。`save_original`、原件保存按钮、恢复日志都不在此列。
- implementation_verified：true，仅限本报告「验证」表里通过的无界面测试。不是 T106 完成。
- acceptance_complete：false。没有文件对话框、关闭提示或另存为的人工图形验收。

## 变更与边界

依赖的是已在 main 上的 W07 `open_source`／锁检测、W10 `ExportPlan::origin_bytes`、C03 `save_export_copy`。本切片不修改 `storage.rs` 与 `storage/**`。`save_original` 仍在写入前拒绝；本切片不调用它，也不登记 `file.save`。

JoyWord 锁的名字是项目约定，与 W07 检测一致：规范来源是 `crates/joyword-shell/src/storage/locks.rs` 的 `.<文件名>.joyword-lock`，不是 OOXML 条款。打开后的包字节仍由既有 DOCX 导入／导出处理。

### 支持

| 行为 | 实现 |
| --- | --- |
| 打开 | `BoundDocx::open`：`open_source` → `DocxSession::open` → `export_plan().origin_bytes()` 必须等于刚打开的字节 → 再创建锁。任一步失败都不安装会话、不留下锁 |
| 来源 | `source_path()`／`stamp().path()` 是打开文件的规范路径。`origin_bytes` 指向这次打开的包，不是后来的副本 |
| 锁 | 规范路径旁边的 `.<文件名>.joyword-lock`。内容是 `joyword-lock 1`、pid、递增序号和路径。`Drop` 只在文件内容仍是本会话令牌时删除 |
| 另存为 | `BoundDocx::save_copy` 只调用 `save_export_copy`。不覆盖已有目标，扩展名必须与计划类型一致。不改路径、戳、引擎版本、撤销栈 |
| dirty | 打开文件用 `TextInput::has_unsaved_changes`（当前版本 ≠ 导入版本）。未编辑为假；编辑后为真。另存为成功或失败都不改这个值，也不调用 `DocumentManager::mark_saved` |
| 外壳 | 文件菜单：打开（`file.open`，cmd-o／ctrl-o）、另存为新副本（`file.save_copy`，cmd-shift-s／ctrl-shift-s）。没有打开路径时另存为不可用。成功文案写明新副本路径，并说明当前打开的文件没有被替换 |
| 关闭／再打开 | 仍是取消 + 不保存，没有保存按钮。只有对当时版本明确选择丢弃（按钮序号 1）才关闭或继续挑选文件。等待期间版本变了就再问。取消留下原会话 |

`open_default` 使用 `Budget::default()`，让应用不必依赖 OPC crate。

### 拒绝与未做

- 非 DOCX 字节：导入失败，不建锁。
- 已有 Office `~$` 所有者文件：`open_source` 以 `LockKind::OfficeOwnerFile` 拒绝，不建 JoyWord 锁，不改源字节。
- 会话仍打开时再次 `open_source` 同一文件：`LockKind::JoyWordLockFile`。崩溃留下的锁不自动删除、不覆盖。
- 另存为目标已存在，或扩展名不是 `docx`／`dotx`／`docm`／`dotm`：`save_export_copy` 在准备阶段拒绝，不发布文件。
- 没有 `file.save`，没有保存成功按钮，没有静默把当前路径改成副本路径。
- 同一规范路径已经打开时，再选一次是空操作，避免被自己的锁拒绝。
- 样本内存文档没有来源路径，不能另存为。打开失败时保留原来的编辑器；新的 `BoundDocx` 被丢弃并释放自己的锁。
- C09-C、`save_original`、W14 恢复日志、最近文件列表都没有做。T103／T104／T106／T107 不标 done。

### 变更路径

- `crates/joyword-shell/src/docx.rs`：`BoundDocx`、`BoundOpenError`、`BoundSaveError`
- `crates/joyword-shell/src/input.rs`：`InputBackend for BoundDocx`
- `crates/joyword-shell/src/command.rs`、`locales/zh-CN.lang`、`locales/en.lang`
- `crates/joyword-shell/tests/c09_open_session.rs`
- `apps/joyword/src/main.rs`、`apps/joyword/src/input_ui.rs`
- 本报告

没有改 model／engine／OPC／字体／整形／PageScene，没有改 `storage`，没有改 Cargo 依赖，没有改公共 TASKS。退出命令仍在应用菜单。

## 验证

| 完整命令／步骤 | passed/failed/skipped/blocked | 主机／输入／日志与实际结果 |
| --- | --- | --- |
| `cargo test -p joyword-shell --locked --offline --test c09_open_session --test docx_copy --test docx_session --test docx_export --test text_input --test w05_text_input_export --test w10_origin --test w12_text_input_export` | passed | Linux x86_64。c09 6、docx_copy 8、docx_export 17、docx_session 8、text_input 20、w05 7、w10 4、w12 1，全部通过 |
| `cargo test -p joyword-shell --release --locked --offline --test c09_open_session` | passed | 同上 6 项通过 |
| `cargo test -p joyword-shell --locked --offline --lib` | failed | 25 项里 24 通过。失败的是既有 `storage::source::tests::identity_change_between_stat_and_open_is_not_a_baseline`：本机同一时间戳内不更新 mtime／ctime。未改 `storage` 测试 |
| `LIBRARY_PATH=/usr/lib/gcc/x86_64-linux-gnu/13 cargo test -p joyword --locked` | passed | 11 项通过，含注册表动作与关闭判定。无窗口 |
| `cargo fmt --all -- --check` | passed | |
| `cargo clippy -p joyword-shell -p joyword --all-targets --locked -- -D warnings` | passed | |
| `python3 scripts/check_docs.py` | passed | OK：82 requirements，58 tasks，9 cases，12 Markdown files |
| `git diff --check` | passed | |
| 文件对话框、关闭提示、另存为的图形操作 | skipped | 没有显示服务器上的人工点击。GPUI `prompt_for_paths`／`prompt_for_new_path` 未在真窗口里执行 |

远程 CI 绑定本分支实际 SHA。交叉编译与无窗口测试不等于真机或 Office 打开。

## 接口／依赖提案与阻塞

W11 在实现 `save_original` 时必须放过**本会话自己创建的** JoyWord 锁。现在 `check_source` 把 `.<文件名>.joyword-lock` 一律当成冲突。会话开着时，原件保存的复查会看到这把锁并拒绝，即使令牌就是当前进程写的。建议按锁文件内容里的令牌识别所有者，不要在 `storage` 里删除或覆盖别人的锁。本切片不改 `storage`，所以没有做这个豁免。

没有新的 Cargo 依赖。`BoundDocx` 放在 `joyword_shell::docx`，应用通过 `open_default` 使用默认 OPC 预算。

## 交接

分支 `codex/c09-open-source-session`。代码基线 `47747952f97d7a82fc0be7768bbea52ed909ec61`，协作包 `31dcedf3cc7756a23e2c9d921a82f06f04d7fbb0`，开工 main `a15d2401bd1b77b53b21ecbaa890745ac435bed8`。报告正文不写本提交 SHA。用户转交后续集成；本切片不合并。C09-C 仍等 W11。

## 切片 2：按令牌放过自己的 JoyWord 锁

- 分支：`codex/c09-own-lock-token`。基线是切片 1 的合并提交 `fedea4341b6e8769daec378cec7181c8bc59cef1`。
- 状态：完成限定切片。`check_source_with_lock_token` 与 `save_original_with_lock_token` 在锁文件全文等于调用者令牌时跳过这一把 JoyWord 锁。不删除、不改写锁文件。
- `open_source` 和没有令牌的 `check_source`／`save_original` 仍把任何 JoyWord 锁当成冲突。别的令牌、空令牌、目录形式的锁名、Office `~$` 仍是冲突。
- 没有登记 `file.save`，应用外壳不调用 `save_original`。令牌豁免之后，当前平台仍因无法保留的元数据拒绝原件替换。
- interface_ready：true，仅限这个锁豁免。原件保存成功和保存按钮仍为 false。
- implementation_verified：true，仅限 `c09_own_lock` 与既有 W07／W11 锁测试。acceptance_complete：false。T106 不标 done。
