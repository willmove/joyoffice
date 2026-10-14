# C03：已验证 DOCX 新副本落盘（2026-10-14）

- 执行者：当前 Codex 协调者，单人执行；没有跨聊天派工或后台监控。
- 任务卡：[C03](../tasks/C03-DOCX-COPY.md)。
- 完整基线：4dd3870a609509ec225a7f85cb8509fbb602dfae（C02）。
- 工作目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice。
- 分支：codex/coordination-docx-storage；从干净基线创建。
- 环境：Apple M4／24 GiB，macOS 27.0.1 arm64，Rust 1.98.1。
- interface_ready：true，仅限已验证 ExportPlan 创建新文件副本，不是原件覆盖保存。
- implementation_verified：true，12 项新增落盘／故障测试、本机全默认成员与 release 回归通过。
- acceptance_complete：false，原件覆盖、元数据保留、Office 锁、GUI、Word/WPS 与其他目标系统人工验收未执行；T103/T106 不标 done。

## 实际实现

shell::storage::save_export_copy 只接收 C02 的不可变 ExportPlan；目标扩展名须与计划类型一致。规范化已存在的父目录，检查目标后在同目录创建私有临时文件，完整写入、sync_all，并按 64 KiB 分块逐字节回读（包括长度与多余尾部检查）。再以 std::fs::hard_link 发布新路径；OS 操作本身拒绝既有目标，初始检查与发布间出现新文件也不覆盖。文件系统不支持硬链接时明确失败，不降级为可覆盖或部分写入。

发布前出错自动清理临时文件，不产生目标。发布完成后清理临时文件并在 Unix 请求目录 sync。若这两步出错，SaveCopyError::published 携带实际已发布路径、版本、长度和 FileSynced 收据；不回删已发布目标，不把失败伪装成未写入。Unix 成功返回 FileAndDirectorySynced，Windows／非 Unix 当前只声明 FileSynced，不声称目录 sync 或断电持久性已验证。

收据绑定真正写出的计划版本；会话此后继续编辑不会使旧计划被误报成新版本。此接口不改 DocxSession 的 backing、版本、撤销或 dirty，也不调用通用 DocumentManager::mark_saved。原件和现有目标不修改；新副本的元数据不是原文件元数据复制，Unix 默认权限为 0600。文件替换、ACL／扩展属性／来源标记和链接保留仍需后续独立实现。

示例 docx_text_export 现在接受可选的新输出路径，直接使用此接口；原 stdout 二进制模式仍可作实验，但产品安全落盘路径不依赖 shell 重定向。

## 依赖与协作说明

协调者裁决增加 tempfile =3.27.0 直接依赖，精确复用锁文件已存在的版本。Cargo.lock 只有 shell 的一行 tempfile 依赖边变化；无外部包增加或版本升级。临时文件名和私有创建使用通用基础库，DOCX 与发布策略由项目独立实现。现有 model／engine／OPC／字体／场景公共 API 未改，unsafe_code = forbid 不变。来源与许可见 [PROVENANCE](../../joyword/PROVENANCE.md)。

C02 提交前主工作目录曾被切回 main；协调者提交前漏查导致自己的一个提交短暂落在本地 main。已把该提交快进到 C02 分支，并用精确旧／新 SHA 恢复本地 main 到 b48ca9b426b80c220f0b2518d1e0ad363e07afea；远程 main 始终未变更。C02 真正成果已推送协调分支。本任务因此使用附加的独立 worktree 和新协调分支，不切换主目录、不接管其他未提交文件。此记录解释环境隔离，不改变 C02 的测试结果。

## 本机验证

所有 Cargo 命令在上述独立 worktree 执行，并设置 CARGO_TARGET_DIR=/Users/willm/Coding/joyoffice/target 复用编译缓存；源码读取来自本 worktree。

| 完整命令（除已说明的 CARGO_TARGET_DIR 外） | 实际结果 |
| --- | --- |
| cargo test -p joyword-shell --locked | 49 项通过：原有 37 项 + 新增 4 项故障单元测试 + 8 项落盘集成测试 |
| JOYWORD_REQUIRE_TEST_FONT=1 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:0' cargo test --locked | 全默认成员 172 项通过，0 failed／ignored；真实字体路径执行 |
| cargo test -p joyword-shell -p joyword-docx --release --locked | 76 项通过，0 failed／ignored |
| cargo check -p joyword-shell --target x86_64-unknown-linux-gnu --locked | passed；仅交叉检查，不是 Linux 运行 |
| cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings | passed |
| cargo build -p joyword --features dev-runtime-shaders --locked | passed；未开窗口，仅开发着色器构建 |
| cargo fmt --all -- --check | passed |
| python3 scripts/check_docs.py | passed：82 个需求、58 项任务、9 个登记用例 |
| python3 -m unittest discover -s scripts/tests -v | 9 passed |
| git diff --check | passed |

测试覆盖：真实 importer→engine→ExportPlan→磁盘→重开；五个语料的精确副本；现有文件／目录／硬链接／符号链接（包括悬空链接）保护；八个并发写入者仅一个成功，目标字节完整且无遗留临时文件；写入／sync／回读／发布各阶段错误；半写入、同长度损坏、截断和初检后另一写入者创建目标；发布后清理／目录 sync 失败收据；旧计划版本与继续编辑／撤销；扩展名、坏路径和 Unix 私有权限。

独立 CLI 冒烟：在 Python TemporaryDirectory 中运行 `cargo run -p joyword-shell --example docx_text_export --locked -- tests/fixtures/docx/DOCX-MIN-001.docx '安全副本🙂 &<> ' <新目录>/copy.docx`；实际返回 FileAndDirectorySynced 收据。macOS `textutil -convert txt -stdout` 读出完整前缀与原始正文。再次执行同命令实际失败（目标存在），逐字节确认原输出未变，目录只有最终文件。临时测试目录结束时清理，没有写入用户原件。

上游 block／proc-macro-error2 的既有 future-incompat 提示仍在。没有系统字体入库。该测试不是 Word/WPS、GUI、真实断电或目标发行版验收。

## 下一步

1. 保留当前不覆盖目标的接口；既有文件保存需要文件身份／指纹、Office 锁、平台原子替换，以及链接／权限／ACL／扩展属性／来源标记保留。能力未齐前不开放原件覆盖。
2. 扩展 C02 的结构与格式安全导出；T103 的覆盖引导 fuzz 仍缺失。
3. T105 分页、命中与 T102 输入随后接入界面；M0 的正式 GPUI 来源、GL／CPU、IME／DPI 与信创真机门槛保持未完成。
4. 新源码远程 CI 需绑定其提交记录；不把 C02 运行结果当作本任务通过。

## 后续独立验证与远程 CI

在相同 shell 源码上完成 `cargo check -p joyword-shell --all-targets --target x86_64-pc-windows-msvc --locked`，passed；为既定 Rust 1.98.1 增加 Windows 标准库目标，没有安装或切换其他工具链。这是交叉检查，不是 Windows 人工运行。

源码 574c44f3f9e4b9a93766ce7fa7f3dd8aa8ecb05e 的 [38549344307](https://github.com/willmove/joyoffice/actions/runs/38549344307) 最终 8/8 作业 success：macOS／Windows／Linux x86_64 与原生 ARM64 构建、ARM64／LoongArch64 QEMU、依赖和统一 PDF。C03 的新存储路径进入默认成员测试，Windows 的 Unix 专属权限／符号链接测试不编译，不能据此声称这些平台语义已全量验收。远程 CI 不替代真实断电、GUI 或 Office。
