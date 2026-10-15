# W07 后续：来源绑定与原件替换（第二轮）

- 任务／负责人：实现 [W07 报告](W07.md)“接口／依赖提案与阻塞”第 1、2 项；Claude Desktop Code 会话（由协调会话派生的唯一执行会话，2026-10-15，本机）。
- 系统／架构／工具链：Apple M4／24 GiB、macOS 27.0.1（26A434）arm64；根卷为密封 APFS，数据卷与测试临时目录（`/var/folders`、`/private/tmp`）为本地 APFS；Rust 1.98.1（48a229cea 2026-09-01）、edition 2024，`unsafe_code = forbid` 不变。
- 代码基线：main `5573997a75d9297b24429f90c727dad05b7f90c3`（PR #9 合并提交，已含 W07 测试平台修正 `d6b765057d0f90b5602b3c5ae5b482d171f23829`）。开工时 PR #9 尚未合并，先在其提交上开发；合并后基线移到该合并提交，两者文件树相同。
- 实际目录／分支：`.claude/worktrees/modest-mclean-eb3020`／`claude/w07-original-replace`，未在 main 或控制目录开工。
- 状态：完成限定切片。来源绑定在所有平台生效；Unix 发布管线以真实 IO 实现并逐阶段注入故障；**只有 macOS 在可证明安全时真正替换原件**，Linux 等其他 Unix 与 Windows 在改写原件前拒绝。
- interface_ready：true，限于 `ExportPlan::originates_from`、macOS 上真实替换的 `save_original`、带刷新 stamp 的 `SaveOriginalReceipt` 与 `SaveOriginalError::Replace`／`Publication`／`ReplaceStage`。
- implementation_verified：true，限于本机 macOS／APFS 实测；Linux／Windows 只做交叉 clippy，待本 PR 的 CI 实际运行。
- acceptance_complete：false。无 GUI、无 Word／WPS 共同打开、无远程文件系统与断电测试，会话保存版本接入属 C08。

## 变更与边界

| 路径 | 内容 |
| --- | --- |
| `crates/joyword-docx/src/export.rs`、`backing.rs` | `ExportPlan` 私有字段 `origin: Arc<[u8]>`，由 `plan_export` 填入传入 backing 原始字节的 Arc 克隆（不复制包）；新增公共只读判定 `ExportPlan::originates_from(&[u8]) -> bool`，逐字节比较。`plan_export` 签名与既有访问器不变 |
| `crates/joyword-docx/tests/export_origin.rs` | 未编辑／已编辑 plan 绑定原包而非输出；他包、改一字节、截断、空切片均不匹配；同版本号不构成绑定；克隆保留绑定 |
| `crates/joyword-shell/src/storage/original.rs` | 能力矩阵按平台重写；按文件求值的阻塞项；`save_original` 编排；回执携带读回得到的 stamp |
| `crates/joyword-shell/src/storage/publish.rs`（新，仅 Unix） | 发布管线及 14 项内联测试（其他 Unix 13 项；故障注入、竞争、锁、回执来源） |
| `crates/joyword-shell/src/storage/observe.rs`（新，仅 macOS） | 经 `/bin/ls -ld -@ -e` 观察扩展属性名与 ACL 是否存在；4 项内联测试（含真实文件） |
| `crates/joyword-shell/src/storage/source.rs`、`error.rs`、`storage.rs` | 身份访问器、备份链接比对、发布后稳定读回；新错误面与再导出 |
| `crates/joyword-shell/tests/w07_save_original_rejection.rs` | 按新语义改写：跨文档／旧版本 plan、只读源、超读取预算、macOS 上已存在的元数据、非 macOS 对已绑定 plan 的拒绝 |
| `crates/joyword-shell/tests/w07_original_replacement.rs`（新，仅 macOS） | 经公共 API 的成功替换 |

未改 model／engine／OPC 公共 API、根或任何 Cargo 文件、CI、shell 的 docx.rs／lib.rs／input.rs／documents.rs、文案与 app；无新增依赖。

### 1. 来源绑定

`save_original` 要求 `plan.originates_from(stamp 读取的字节)`，否则返回 `ReplacementUnavailable { OriginBindingUnproven }`，不写任何东西。版本号相同、目标类型相同或输出相同都不能替代。C08 的接入方式即 W07 提案：用 `open_source` 的字节调用 `DocxSession::open`，导出时自然绑定。

绑定是“包字节完全相同”，不是会话身份：两个字节完全相同的文件在本层不可区分，会话与 stamp 的配对属 C08 会话生命周期。保存成功后，原会话的 backing 仍是保存前的包，所以它的下一个 plan 不再来自磁盘上的文件，会被拒绝（测试 `the_next_save_needs_a_session_rebased_on_the_saved_bytes`）；目前只能用回执路径重新打开会话。这是正确的冲突语义（另一个窗口用同一旧包导出的 plan 同样被拒），但连续保存需要 C08 提供“保存后重新基于新字节”的会话接口。

### 2. 发布策略（Unix）

在 `check_source` 复查、扩展名与读取预算校验、来源绑定和元数据门之后执行：

| 阶段 `ReplaceStage` | 动作 | 失败后磁盘状态 |
| --- | --- | --- |
| CreateTemporary | 源目录内 `.joyword-save-*` 私有临时文件（O_EXCL，0600） | 原件不变，无残留 |
| Write | 写入 plan 字节 | 同上 |
| RestoreMetadata | 必要时 `fchown` 到源的 owner／group（不允许即拒绝 Ownership），再设权限位；macOS 设创建时间 | 同上 |
| SyncFile | `sync_all` | 同上 |
| VerifyTemporary | 复用 C03 逐字节回读；核对 owner／group／权限位／链接数，macOS 另核对创建时间、`st_flags` 与 `ls` 观察（新文件若因目录默认 ACL 等获得原件没有的元数据即拒绝）；临时路径仍指向已写对象 | 同上 |
| Recheck | 完整 `check_source`：锁、路径解析、身份含 ctime、逐字节 | 同上 |
| Backup | 把当前源对象硬链接为私有 `.joyword-backup-*` | 同上 |
| VerifyBackup | 再扫锁；备份须是 stamp 的同一 dev／inode，模式、owner、mtime、长度一致，链接数正好加一；有界读回与 stamp 字节完全相同 | 原件不变；备份已删除（删不掉则在错误中报告） |
| Publish | 再 stat 一次源路径须仍是该对象，随后 `rename` 临时文件覆盖源路径 | 同上 |
| Confirm | 对源路径做稳定有界读回：须是临时文件的同一对象、前后观察一致、字节等于 plan、元数据符合预期；回执与刷新 stamp **只由这次读回构造** | `Publication::Unconfirmed`，无回执，原件字节保留在备份 |
| RemoveBackup | 删除备份链接 | `Publication::Confirmed(回执)`，备份路径在错误中报告 |
| SyncDirectory | 目录 `sync_all` | `Publication::Confirmed(回执)`，持久化状态 FileSynced |

改名之后不回滚、不重试，也不自动删除目标或备份。从 VerifyBackup 起，即使失败，建立并删除备份链接也可能推进源的 ctime，旧 stamp 随之报 TimesChanged；字节与对象不变，重新 `open_source` 即可（测试覆盖）。

已知未关闭的竞争：最后一次对象检查与 `rename(2)` 之间另一写者替换源路径，会被覆盖（备份仍保有已验证原件）。关闭它需要 macOS `renamex_np(RENAME_SWAP)`／Linux `renameat2(RENAME_EXCHANGE)`，std 未提供。

### 3. 元数据门与能力矩阵

`MetadataCapability` 改为按文件求值：Preservable＝用 std 恢复并在发布前后核对；ObservableNotPreservable＝源上存在即拒绝、不存在不阻塞；Unobservable＝一律拒绝。另有：源无任何写权限位→`ReadOnlySource`；硬链接数大于 1→`LinkedSourceSemantics`；无写入路径的平台→`UnsupportedPlatform`。

| 项目 | macOS（本机实测） | Linux／其他 Unix（未在目标机运行） | Windows（未运行） |
| --- | --- | --- | --- |
| 写入路径 | 有 | 有，但下列不可观察项使公共 API 总是拒绝 | 无，`UnsupportedPlatform` |
| 权限位 | 恢复并核对 | 恢复并核对 | 只读属性可观察；只读源被拒绝 |
| 所有者／组 | `fchown` 恢复并核对，不允许即拒绝 | 同左 | 不可观察，拒绝 |
| ACL | `ls -e` 观察，存在即拒绝 | 不可观察（POSIX ACL 是扩展属性），拒绝 | 不可观察，拒绝 |
| 扩展属性 | `ls -@` 观察，存在即拒绝；内核管理的 `com.apple.provenance` 除外 | 不可观察，拒绝 | 不可观察，拒绝 |
| 来源标记 | `com.apple.quarantine`、`kMDItemWhereFroms` 存在即拒绝 | 浏览器写入的 `user.xdg.origin.url` 等不可观察，拒绝 | MOTW 不可观察，拒绝 |
| 创建时间 | `FileTimesExt::set_created` 恢复并核对 | 无法设置；文件系统报告出生时间时拒绝 | 可观察，拒绝 |
| 文件标志 | `st_flags` 非零即拒绝 | 不可观察，拒绝 | 不可观察，拒绝 |
| 符号链接 | 写到解析后的目标，链接本身不变（测试覆盖） | 同左（仅管线单元测试） | — |
| 硬链接 | 链接数大于 1 即拒绝 | 同左 | 拒绝 |

`com.apple.provenance` 的处理依据本机实测：本进程新建的每个文件都会被内核加上该属性，用户写入的值会被内核改成本进程的值，因此任何保存都无法沿用上一写者的值；替换后文件带保存进程的来源记录。真实用户文档常带 `com.apple.macl`（TCC 授权）、`com.apple.quarantine` 或 `com.apple.lastuseddate#PS`，这些文件按本矩阵**会被拒绝**；本切片能真正替换的是没有这类元数据的文件（例如由脚本或本程序新建的文档）。扩大到常见文档需要下文的依赖决定。

`/bin/ls` 观察只判断“是否存在”，不比较值；解析严格（模式串后的 `@`／`+` 标记须与明细行一致，任何异常即视为不可观察并拒绝），路径须为绝对路径且不含控制字符，环境清空并设 `LC_ALL=C`。依据 macOS ls(1) 对 `-@`、`-e` 与 `@`／`+` 标记的说明，本机与 CI 的 macOS 运行器验证格式；沙箱化应用能否执行 `/bin/ls` 未验证。

### 4. 为什么不用 `std::fs::copy` 携带元数据

Rust 1.98.1 在 Apple 平台的 `fs::copy` 先试 `fclonefileat`，失败（目标已存在、非 APFS、跨设备）再用 `fcopyfile(COPYFILE_METADATA | COPYFILE_DATA)`（源码 `library/std/src/sys/fs/unix.rs`）。本机实测：clone 路径保留扩展属性和创建时间，但**没有带上 ACL**；fcopyfile 路径带上 ACL，但**改写了 quarantine 的值（21 字节变 15 字节）**，创建时间也成了新值。两条路径都不忠实，且无法逐次验证，所以不作为保留手段。

## 验证

所有 Cargo 命令在上述 worktree 执行，独立 `target/`，`CARGO_BUILD_JOBS` 4–6。未开窗口、未占用 GUI。

| 完整命令／步骤 | passed/failed/skipped/blocked | 主机／输入／日志与实际结果 |
| --- | --- | --- |
| cargo test -p joyword-docx -p joyword-shell --locked | passed | 本机 macOS／APFS；joyword-docx 51 项（lib 11＋export_origin 3＋import 20＋w05_structure 17）；joyword-shell 130 项（lib 43＋docx_copy 8＋docx_export 17＋docx_session 8＋text_input 20＋w05_text_input_export 7＋w07_original_replacement 4＋w07_save_original_rejection 9＋w07_source_conflicts 14）；doctest 0；0 failed／ignored |
| cargo test -p joyword-docx -p joyword-shell --locked --release | passed | 同上源码与项数 |
| cargo fmt --all -- --check | passed | 无输出 |
| cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings | passed | 上游 block 0.1.6／proc-macro-error2 2.0.1 既有 future-incompat 提示（C03／C07／W07 已记录），非本次引入 |
| cargo clippy -p joyword-shell -p joyword-docx --all-targets --locked --target x86_64-unknown-linux-gnu／aarch64-unknown-linux-gnu／loongarch64-unknown-linux-gnu／x86_64-pc-windows-msvc -- -D warnings | passed | 仅交叉编译与 lint，不是在目标系统运行 |
| python3 scripts/check_docs.py | passed | 82 需求、58 任务、9 用例，7 workflow 按 SHA 固定 |
| git diff --check | passed | 无问题 |

本机覆盖的场景：成功替换（新对象、owner／组、0640 权限、创建时间到纳秒、链接数 1、目录无残留、刷新 stamp 通过 `check_source`、旧 stamp 冲突）、未编辑会话重写相同字节、经符号链接写穿、二次保存需重新基于新字节；跨文档 plan、旧版本 plan、旧 stamp、只读源、plan 超过 stamp 读取预算、硬链接源、扩展名与类型不符；macOS 上扩展属性／quarantine／ACL／`hidden` 标志均在改写前拒绝且零副作用。管线单元测试（全部 Unix）：9 个发布前阶段逐一注入（含写入阶段的部分字节）均保留原件、无临时文件或备份残留；临时文件同长度损坏与截断；临时文件权限漂移；macOS 临时文件新增扩展属性；无法给出的所有者；复查前改写、备份前替换（同字节不同对象）、备份后追加、改名前替换均报冲突且他人文件原样保留；复查与备份核对阶段出现 `~$`／JoyWord 锁即拒绝且锁不被删除；Confirm 注入、同对象改字节、以 plan 字节换成另一对象均不产生回执并保留备份；RemoveBackup／SyncDirectory 失败携带已确认回执。

跳过／未覆盖：Linux（ext4／tmpfs）与 Windows NTFS 上的实际运行待 CI；Linux 上公共 API 只验证拒绝，管线只经单元测试；非本人所有者的成功替换（需 root）；FAT／exFAT（无硬链接时在 Backup 阶段失败、保留原件，属推断未实测）、SMB／NFS、真实掉电；Word／WPS 共同打开；沙箱化应用调用 `/bin/ls`。

## 接口／依赖提案与阻塞

1. 公共接口增量（请 C08 审）：joyword-docx 新增 `ExportPlan::originates_from`。joyword-shell storage 新增 `ReplaceStage`、`Publication`、`SaveOriginalError::Replace` 及 `replace_stage()`／`publication()`／`published()`／`retained_backup()`；`ReplacementBlocker::{ReadOnlySource, UnsupportedPlatform}`；`MetadataItem::FileFlags`；`ConflictDetail::OwnershipChanged`；`OriginalReplacementCapabilities::{write_path, file_flags}`；回执新增 `bytes()`／`stamp()`／`into_stamp()`。会影响穷尽匹配与结构体字面量；`SaveOriginalReceipt` 去掉了 `PartialEq`／`Eq`（W07 时为从未构造的占位类型，现含 stamp）。`OriginBindingUnproven` 的含义从“ExportPlan 无绑定字段”改为“plan 的原包与 stamp 字节不同”。
2. 依赖提案（扩大到常见文档与 Linux 所需，属根 Cargo 决策，本次未申请）：`rustix 1.1.5` 已在 Cargo.lock（经 tempfile 引入），其安全 API `fs::flistxattr`／`fgetxattr`／`fsetxattr` 可在 Linux 与 macOS 列出并复制扩展属性（Linux 的 POSIX ACL 即 `system.posix_acl_*`），替代 `/bin/ls`；`fs::renameat_with(…, RenameFlags::EXCHANGE)`（Linux `RENAME_EXCHANGE`、Apple `RENAME_SWAP`）可关闭改名前的竞争窗口，并让被换下的对象可核对。macOS ACL 与受保护属性（如 `com.apple.macl`）仍无安全封装，需另行评估。
3. 待 C08 裁决：是否接受用 `/bin/ls` 观察 macOS 扩展属性与 ACL。若不接受，删除 `observe.rs` 并把 macOS 矩阵相应项改为 Unobservable，macOS 即与其他平台一样在改写前拒绝。
4. C08 接入：用 `open_source` 的字节打开会话；成功后以 `receipt.stamp()` 取代旧 stamp，并让会话重新基于保存后的字节（目前只能重新打开，撤销历史随之丢失）；只有回执与仍打开的对应会话相符才更新保存版本。`save_export_copy` 仍是被拒绝时的已验证退路。

## 交接

最终消息附代码基线 `5573997a75d9297b24429f90c727dad05b7f90c3`、分支 `claude/w07-original-replace`、结果提交 SHA、推送与 PR 状态和测试摘要；报告正文不写自身提交 SHA。macOS 以外平台的原件替换未实现／未验证；Linux／Windows 行为以本 PR 的 CI 实际结果为准。
