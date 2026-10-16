# W07 后续增量：在 W11 之上补全读回确认与元数据恢复

- 任务／负责人：[W07 报告](W07.md)第 1、2 项的后续切片；Claude Desktop Code 会话（由协调会话派生的唯一执行会话，2026-10-15 起，本机）。前一轮候选为 [PR #11](https://github.com/willmove/joyoffice/pull/11)，已按[集成裁决](MAIN-INTEGRATION-20261016.md)只合入历史，候选报告保留为[历史记录](W07-ORIGINAL-REPLACE.md)。
- 系统／架构／工具链：Apple M4／24 GiB、macOS 27.0.1（26A434）arm64；根卷为密封 APFS，数据卷与测试临时目录（`/var/folders`、`/private/tmp`）为本地 APFS；Rust 1.98.1（48a229cea 2026-09-01）、edition 2024，`unsafe_code = forbid` 不变。
- 代码基线：main `fea6358b6d92c1ef23bf354f8f4a4779534c3636`（含 W10、W11、W12、C09 锁令牌与 2026-10-16 集成）。
- 实际目录／分支：`.claude/worktrees/modest-mclean-eb3020`／`claude/w11-confirmed-publish`，未在 main 或控制目录开工。
- 状态：有限交付。W11 合入后，用户在本会话裁决“以 W11 为底重做，并保持严格”。本切片只在 W11 的公共接口上做增量，不恢复旧候选的 `/bin/ls` 观察，也不启用真正的原件覆写。公开 `save_original` 在所有平台仍于改写前拒绝（ACL、扩展属性、来源标记不可观察）。
- interface_ready：true，限于“增量”一节列出的接口；原件替换在任何平台都没有经公开 API 成功。
- implementation_verified：true，限于本机 macOS／APFS 的自动测试；Linux／Windows 只做交叉 clippy，以本 PR 的 CI 为准。
- acceptance_complete：false。无 GUI、无 Word／WPS 共同打开、无远程文件系统与断电测试。

## 增量（相对 W11）

全部位于 `crates/joyword-shell/src/storage/**` 与测试；未改 joyword-docx、model／engine／OPC、shell 的 docx.rs／lib.rs 等 C09 文件、Cargo、CI、app；无新增依赖。

1. **读回确认与刷新 stamp**。INTERFACES 要求“SourceStamp 刷新须来自成功落盘并读回的实际对象”，W11 未做。新增 `SourceStage::Confirm`：改名后对目标做稳定有界读回，要求它就是刚改名的临时文件（同 dev／inode）、前后观察一致、字节等于 plan、元数据符合预期。回执只由这次读回构造；`SaveOriginalReceipt` 新增 `bytes()`、`stamp()`、`into_stamp()`，其中 stamp 的身份与字节来自读回，不来自 plan。`SourceStamp` 增加 `PartialEq`／`Eq`，因为回执原本就派生了这两者。
2. **未确认的发布保留备份**。新增 `SaveOriginalError::PublishedUnconfirmed { path, source, backup }` 与 `retained_backup()`：改名已发生但读回不符时不发回执，原件字节留在备份，不自动删除、不重试。`stage()` 对它返回 `Confirm`。
3. **owner／组与创建时间的恢复和核对**。`RestorePermissions` 阶段移到 sync 之前：W11 在 sync 之后才改权限，这次改动不在 sync 范围内。该阶段依次恢复 owner／组（仅在不同时调用 `fchown`；无权给出即以 `UnpreservableMetadata(Ownership)` 拒绝）、权限位和 macOS 创建时间（`FileTimesExt::set_created`）。sync 与逐字节回读之后，再核对 owner／组、权限位、链接数和（macOS）创建时间，任一不符即拒绝。能力矩阵相应改为：Unix 上所有权为 Preservable，macOS 上创建时间为 Preservable，其他平台的创建时间仍为 ObservableNotPreservable。
4. **新的写前拒绝**：`ReplacementBlocker::ReadOnlySource`（源文件没有任何写权限位）；plan 大于 stamp 的读取上限时，在 `Validate` 阶段以 `FileTooLarge` 拒绝，否则无法在同一上限下读回。
5. **缩小竞争窗口**：核对备份时重新扫描 Office／JoyWord 锁（沿用 C09 的所有者令牌）；改名前再确认一次目标仍是同一对象。最后一次检查与 `rename(2)` 之间的窗口仍未关闭，关闭它需要 `renamex_np(RENAME_SWAP)` 或 `renameat2(RENAME_EXCHANGE)`。
6. 回执内部把 stamp 装箱，避免错误类型变大而触发 C09 `docx.rs` 的 `result_large_err`。

为与 W11 保持一致，以下均未改动：W11 的阶段名、`OriginMismatch`、`Io { published }`、`AtomicReplaceUnverified`，以及“ObservableNotPreservable 一律阻塞”的语义。

### 改动的他人测试断言

- `tests/w07_save_original_rejection.rs` 与 `tests/w11_save_original.rs` 原先断言创建时间和所有权一定在阻塞项中。按新矩阵改为：创建时间只在非 macOS 上阻塞，所有权只在非 Unix 上为 Unobservable。其余断言（ACL、扩展属性、来源标记、零写入）不变，公共 API 仍拒绝。
- `tests/w07_save_original_rejection.rs` 新增三项：旧版本 plan（旧 stamp 报冲突，新 stamp 报 `OriginMismatch`）、只读源、plan 超过读取上限。

## 为什么保持严格（实测依据）

std 没有 listxattr 或 acl 接口，所以扩展属性、ACL 和来源标记在现有依赖下都不可观察。本机实测表明，Rust 1.98.1 的 `std::fs::copy` 也不能用来携带元数据；它在 Apple 平台先试 `fclonefileat`，失败后改用 `fcopyfile(COPYFILE_METADATA | COPYFILE_DATA)`：

- clone 路径没有带上 ACL；
- fcopyfile 路径改写了 `com.apple.quarantine` 的值（从 21 字节变为 15 字节），创建时间也变成了新值。

此外，本机进程新建的每个文件都会被内核加上 `com.apple.provenance`，而真实文档常带 `com.apple.macl`、quarantine 或 `com.apple.lastuseddate#PS`。

## 验证

| 完整命令／步骤 | passed/failed/skipped/blocked | 主机／输入／日志与实际结果 |
| --- | --- | --- |
| cargo test -p joyword-docx -p joyword-shell --locked | passed | 本机 macOS／APFS；joyword-docx 64 项（lib 12＋import 20＋w05_structure 17＋w10_origin 4＋w12_structure 11）；joyword-shell 140 项（lib 36＋c09_open_session 6＋c09_own_lock 4＋docx_copy 8＋docx_export 17＋docx_session 8＋text_input 20＋w05_text_input_export 7＋w07_save_original_rejection 9＋w07_source_conflicts 14＋w10_origin 4＋w11_save_original 6＋w12_text_input_export 1）；doctest 0；0 failed／ignored |
| cargo test -p joyword-docx -p joyword-shell --locked --release | passed | 同上源码与项数 |
| cargo fmt --all -- --check | passed | 无输出 |
| cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings | passed | 上游 block 0.1.6／proc-macro-error2 2.0.1 有既存的 future-incompat 提示，非本次引入 |
| cargo clippy -p joyword-shell -p joyword-docx --all-targets --locked --target x86_64／aarch64／loongarch64 Linux、x86_64-pc-windows-msvc、aarch64-apple-darwin -- -D warnings | passed | Linux 与 Windows 只是交叉编译加 lint，没有在目标系统上运行 |
| python3 scripts/check_docs.py | passed | 82 需求、58 任务、9 用例，7 个 workflow 按 SHA 固定 |
| git diff --check | passed | 无问题 |

以上 Rust 结果在 `dcac97e` 加本增量的文件树上取得。之后 main 前进到 `fea6358`，其间改动只涉及 CI、文档与独立实验 workspace，不涉及上述两个 crate。

下列单元测试直接调用发布步骤并绕过能力矩阵，只证明步骤本身，不代表公开保存成功：

- 成功发布：保留 0640 权限、owner／组与（macOS）创建时间；目标是新对象；目录无残留；回执里的 stamp 通过 `check_source`。
- W11 原有用例：发布前各阶段的故障注入与发布后的回执用例都保留，并增加了 stamp 核对。
- 竞争：建立备份链接后被替换（字节相同、对象不同）、备份后被追加、改名前被替换，三种情况都报冲突，他人文件原样保留。
- 锁：核对备份阶段出现 `~$` 或 JoyWord 锁时拒绝，且不删除锁。
- 元数据：临时文件权限被改动、进程无权给出 owner，两种情况都在发布前拒绝。
- 读回：在 Confirm 注入失败、同一对象被改字节、目标被换成内容等于 plan 的另一对象，三种情况都不产生回执，并保留备份。

跳过／未覆盖：Linux（ext4／tmpfs）与 Windows NTFS 的实际运行待 CI；非本人所有者的成功替换（需要 root）；FAT／exFAT、SMB／NFS 与真实掉电；Word／WPS 共同打开。

## 接口／依赖提案与阻塞

1. **公共接口增量（请 C09 审）**：
   - 新增 `SourceStage::Confirm`、`SaveOriginalError::PublishedUnconfirmed` 及其 `retained_backup()`、`ReplacementBlocker::ReadOnlySource`、`SaveOriginalReceipt::{bytes, stamp, into_stamp}`，并为 `SourceStamp` 派生 `PartialEq + Eq`。
   - `SourceStage::RestorePermissions` 的含义扩大到 owner／组与创建时间，显示文本改为 “metadata restore”。
   - 新变体会影响对这些枚举的穷尽匹配；当前仓库内没有这类匹配。
2. **依赖提案**（放开原件替换所需，属于根 Cargo 决策）：
   - `rustix 1.1.5` 已在 Cargo.lock 中（经 tempfile 引入）。它的安全 API `fs::flistxattr`／`fgetxattr`／`fsetxattr` 可以在 Linux 与 macOS 上观察并复制扩展属性；Linux 的 POSIX ACL 就是 `system.posix_acl_*` 属性。
   - `fs::renameat_with(…, RenameFlags::EXCHANGE)`（Linux 上为 `RENAME_EXCHANGE`，Apple 上为 `RENAME_SWAP`）可以关闭改名前的竞争窗口。
   - macOS 的 ACL 与受保护属性（如 `com.apple.macl`）仍无安全封装，需要另行评估。
3. **会话接入（C09）**：保存成功后，用 `receipt.stamp()` 取代旧 stamp。会话的 backing 仍是保存前的包，下一个 plan 会报 `OriginMismatch`，因此需要一个“保存后基于新字节”的会话接口；目前只能重新打开。
4. **归属**：本增量的范围与 W11 重合，测试和报告沿用 W07 命名。是否并入 W11 的记录或改名，由 C09 决定。

## 交接

最终消息附代码基线、分支 `claude/w11-confirmed-publish`、结果提交 SHA、推送与 PR 状态以及测试摘要；报告正文不写自身的提交 SHA。原件替换在任何平台上都没有经公开 API 成功；Linux 与 Windows 的行为以本 PR 的 CI 实际结果为准。
