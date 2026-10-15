# W08：恢复日志、检查点与崩溃重放内核

- 预留负责人：Windows 11 x64 主机的 Claude Desktop Code；环境已确认，尚未启动。
- 分支／目录：`codex/w08-recovery-journal`／`joyoffice-w08`。
- 代码基线：C07 `a88217136a570a8f224d92b808bc0b8840ab26ca`；另记录新版协作包完整 SHA。
- 对应：T104、EDIT-005／FORMAT-004／QUALITY-004；先读 AGENTS.md、第二轮 README／INTERFACES、TASKS T104、ARCHITECTURE §4、QUALITY 保存恢复要求。

这是一个可分三批交付的大任务，目标是可运行、可恢复真实文本事务的独立内核，不只交设计文档。三批属于同一负责人，不另拆给其他 Agent；完成边界内工作可连续推进，遇到冻结接口才提案。正式应用接入由 C08 后续完成。

## 交付批次

1. **有界日志与故障语义。** 在独立 Rust crate 中实现带 magic／schema_version／长度／序号／会话身份／校验和的记录格式，明确最大记录／总文件／操作数。真实追加、flush／sync、短写和磁盘满诊断；扫描只返回连续有效前缀，不跳过中间损坏继续拼接；未知 schema 拒绝，不修改原日志，尾部诊断可定位。校验和仅作损坏检测，不宣称抗恶意认证。
2. **真实事务重放。** 使用现有 engine／DocxSession，在有明确定义的纯文本子集内支持插入、删除、拆段、undo／redo 的恢复记录和重放；经 engine 提交，不创建第二套撤销栈。原始 DOCX 字节及身份作为初始恢复基线，保护和 locked 规则仍有效。不要假定重新导入后的 NodeId 与旧进程一致：定义逻辑定位、前置内容校验及版本映射；重复、乱序、来源错误、非法位置、预算失败均拒绝。无法表达的对象／格式／事务明确拒绝，不能默默漏记。预编辑不记成已提交事务。
3. **检查点与恢复工具。** 周期检查点采用已验证基线＋可重放事件的版本化格式；有界压缩／轮转必须保证旧检查点仍可恢复，在新检查点持久化完成前不得删除唯一恢复链。编写 inspect/recover 的无窗口 CLI，恢复产物是独立会话／用户选择的新副本，绝不覆盖正式源文件。使用子进程强制结束，在真实文件系统上验证最后完整记录恢复，并对比独立预期文本、撤销／重做结果与未改源文件。C07 导出不支持的结构修改可恢复到会话，不能假称其已可保存为 DOCX。

## 独占可写路径

- `experiments/recovery-journal/**`（代码、独立 Cargo.toml/Cargo.lock、README、测试、原创小样例、CLI）
- `docs/coordination/reports/W08.md`

其余全部只读，尤其 shell/storage.rs、docx.rs、input.rs、engine、model、根 Cargo／CI／公共文档。独立 manifest 声明自己的 `[workspace]`、edition 2024、`rust-version = "1.98"`、`unsafe_code = "forbid"`，实际工具链固定 1.98.1。由 C08 在本卡授权使用 std、现有本地 crates 的 path 依赖，以及精确 `crc32fast = "=1.5.2"`、`sha2 = "=0.10.9"`、`tempfile = "=3.27.0"`（按需）；自己的锁文件必须提交。不得升级现有版本；其他依赖先提案，不修改根 workspace。首次生成独立锁文件后，验证均用 --locked。

## 验证与完成标准

空文件／空事务、Unicode、每字节截断、记录头／载荷损坏、伪造超长长度、序号重复／缺口、错误源／schema、追加失败、checkpoint 切换中断、重放期间取消／预算、保存后继续编辑、关闭后迟到结果；版本推进与 dirty 记账只能给出精确结果，正式会话接入由 C08 测试。限时／限量读写，不将不可信长度直接用于分配。

```sh
cargo test --manifest-path experiments/recovery-journal/Cargo.toml --locked
cargo test --manifest-path experiments/recovery-journal/Cargo.toml --release --locked
cargo fmt --manifest-path experiments/recovery-journal/Cargo.toml --all -- --check
cargo clippy --manifest-path experiments/recovery-journal/Cargo.toml --all-targets --locked -- -D warnings
python3 scripts/check_docs.py
git diff --check
```

报告另列实际 CLI 命令、故障种子、子进程退出方式、文件系统／OS、有效前缀长度和恢复语义比较。杀进程不等于断电；独立内核不等于 T104 产品接入完成。只提交上述文件并推送自己的分支，按第二轮模板交接。三批分别给真实 commit／测试，不把工具缺失写通过，不因第一批通过就停止后续已授权工作。

## Windows 11 x64 验证补充

原生 MSVC／Rust 1.98.1 为本任务首个目标；报告记录 Windows build、实际文件系统、工具版本和路径，不能预先假设为 NTFS。Python 文档检查用 `py -3 scripts/check_docs.py`。测试覆盖文件仍被打开时的检查点轮转／重命名失败、权限拒绝、包含空格／中文的路径、子进程终止后的句柄释放与日志重新打开；没有可靠目录持久化证据时不能承诺掉电安全。子进程只终止本任务创建的测试进程，不操作桌面应用。macOS／Linux 未运行则列未验证，WSL 单列。
