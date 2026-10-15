# W07：源文件冲突检测与安全保存

- 负责人：当前主机的 ZCode Desktop；已分配、未启动。启动时登记唯一会话；协调入口 C08。
- 代码基线：C07 `a88217136a570a8f224d92b808bc0b8840ab26ca`；另记录 C08 协作包起始完整 SHA。
- 分支／目录：`codex/w07-safe-save`／`joyoffice-w07`。
- 对应：T103／T106、FORMAT-004、QUALITY 的保存失败不变量；T104 日志恢复另派。先读 AGENTS.md、第二轮 README／[接口](../wave02/INTERFACES.md)、SPEC、ARCHITECTURE §5、QUALITY 及 C03/C07 报告。

## 首份交付

C03 已有同目录临时文件、写入／sync／逐字节校验、无覆盖 hard-link 发布的新副本 API 与故障测试。保留它，增量实现：

1. 有界读取源文件并捕获与该批字节对应的私有 SourceStamp；检查文件类型／链接／Office 所有者文件和 JoyWord 锁冲突，支持之后检测源替换或内容变化。读取前后变化不能接受为稳定基线。
2. 明确的冲突预检和平台能力诊断；同大小同时间内容变化、删除／重建、目标指向改变不能漏报。保留用户文件／链接和锁，不自动删除其他应用的锁或强行解除权限。
3. 仅在能够证明来源绑定、目标竞争处理及元数据保留的已验证本机子集实现原件替换；临时文件、验证、持久化、发布前后错误／回执语义必须真实执行。不能把“先 stat 再 rename”说成无竞争安全保存，不能用同版本不同来源的 ExportPlan 覆盖目标。

**首份最低可验收成果**是 1／2 的真实实现、3 的能力矩阵与保守拒绝，以及实际故障回归。若原件替换需要根依赖／平台 API 或 ExportPlan 增量来源绑定，报告精确提案及阻塞，先交付独立成果；本轮不得为了呈现完整保存而降低标准。save_original 若仅拒绝，报告必须写“原件替换未实现／未验证”，不得写保存完成。已验证的 save_export_copy 可由 C08 作为用户明确选择的退路接入。

## 独占可写路径

- `crates/joyword-shell/src/storage.rs`
- `crates/joyword-shell/src/storage/**`（内部／平台模块，由 storage.rs 声明）
- `crates/joyword-shell/tests/w07_*.rs`、`crates/joyword-shell/tests/w07_data/**`
- `docs/coordination/reports/W07.md`

不得改 shell/docx.rs、input.rs、documents.rs、lib.rs、command.rs、文案或 app；不得改 ExportPlan／model／engine／OPC、Cargo 文件、CI 和公共文档。新 API 位于 storage 模块，具体边界见 INTERFACES。不清会话 dirty，不重置 backing／undo，不在此实现恢复日志或文件对话框。

## 必测与验收

- 真实临时目录／文件验证读取及冲突：同长度内容改动、大小／mtime 变化、替换／删除／重建、路径别名、符号／硬链接、非普通文件、Office／JoyWord 锁存在或状态不明、超过源读取预算。
- 每个写前／写中／sync／校验／发布／清理阶段故障注入；短写、磁盘满、权限变化、目标竞争、读回损坏、取消。发布前失败保留原件；发布后失败回执必须承认已发布，不能盲目删目标或重试。
- metadata 能力矩阵逐项记录权限／ACL、链接、扩展属性、创建时间、只读位、MOTW／quarantine；无法保证则改写前拒绝。远程文件系统与真实掉电未经测试不得宣称支持。
- 原 save_export_copy 所有回归；原件分支若支持，需真实 plan→落盘→同预算重新导入检查，并拒绝错误源计划／旧 stamp；慢保存期间新编辑、会话已关闭的回执接入由 C08 测试，W07 返回精确版本和身份信息。
- 使用既有依赖和 Rust safe API；无目标 OS 仅交叉编译不是实测，明确报告未覆盖。

```sh
cargo test -p joyword-shell --locked
cargo test -p joyword-shell --release --locked
cargo fmt --all -- --check
cargo clippy -p joyword-shell --all-targets --locked -- -D warnings
python3 scripts/check_docs.py
git diff --check
```

按模板记录实际平台、文件系统、测试与阻塞；interface_ready／implementation_verified／acceptance_complete 只对应明确子集。只提交独占路径并推送任务分支；用户转交 C08 决定后续平台依赖／接口接力。
