# C01：DOCX 会话安全接入（2026-10-14）

- 执行者：当前 Codex 协调者；单人执行，不启动其他桌面 Agent。
- 主机：macOS 27.0.1（26A434），arm64；Rust 1.98.1。
- 任务卡：[C01](../tasks/C01-DOCX-SESSION.md)。
- 启动时分支 main，工作区干净；完整基线 2b9d58c2866703f4cd10b7f5e0fc56c961c23c57。
- 交付分支：codex/coordination-docx-session；在 /Users/willm/Coding/joyoffice 执行，没有接管其他 worktree。
- interface_ready：true，仅限有界导入→engine 会话及未编辑原包字节接口。
- implementation_verified：true，新增 8 项真实 importer/engine 集成测试和完整本地回归通过。
- acceptance_complete：false，未执行 GUI、固定 Word/WPS 或保存工作流人工验收；T103/T106 仍为 in_progress。

## 实际变更

shell 新增 DocxSession，保留原包、所有未解释部件／片段、导入诊断和原始可编辑边界。全篇只读或任意局部锁均映射为 engine 的整篇只读，直到局部锁支持到位；即使命令针对其他支持段落，也拒绝。适配器只允许只读访问 engine，不能取出可变 session 清除保护或替换模型。

不受限文档通过原有事务入口执行内存编辑、撤销与重做，所有来源遵守只读检查，AI 默认关闭。unchanged_package_bytes 只允许当前版本等于导入版本；任何提交编辑后返回包含两个版本的 EditedPackage，撤销仍保守拒绝。失败／空事务不使原包失效。此功能没有文件写入，也不是 ExportPlan 或保存可用性声明。

只新增 shell 对 workspace 现有 joyword-docx、joyoffice-opc 的依赖边；Cargo.lock 仅两行依赖变化，没有新增外部依赖或版本升级。model、engine、OPC、字体／整形／场景／PDF 接口保持原状。无新格式标准实现，使用既有导入器及其公开规范来源。

## 验证

| 完整命令 | 实际结果 |
| --- | --- |
| cargo test -p joyword-shell --locked | 12 项既有单元测试 + 8 项新增集成测试通过；0 failed／ignored |
| JOYWORD_REQUIRE_TEST_FONT=1 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:0' cargo test --locked | 全默认 workspace 143 项通过，字体必需路径实际执行 |
| cargo clippy -p joyword-shell -p joyword-docx -p joyword-engine --all-targets --locked -- -D warnings | passed |
| cargo fmt --all -- --check | passed |
| cargo build -p joyword --features dev-runtime-shaders --locked | passed；仅构建，未创建窗口；已有 block 0.1.6、proc-macro-error2 2.0.1 future-incompat 警告仍在，不称发行构建通过 |
| python3 scripts/check_docs.py | passed：82 个需求、58 项任务、9 个登记用例 |
| python3 -m unittest discover -s scripts/tests -v | 9 passed，0 failed |
| git diff --check | passed |

## 公开仓库后的远程 CI（2026-10-14）

用户将仓库设为公开后，协调者确认 visibility=PUBLIC，并对源码提交 a67db293d3f397eaf76c7d1c429c9479c18ca0b3 重跑 [38531619157 第 2 次执行](https://github.com/willmove/joyoffice/actions/runs/38531619157/attempts/2)。8/8 作业全部 success：macos-15、windows-2022、ubuntu-24.04 和 ubuntu-24.04-arm 各自通过文档／工具检查、fmt、核心测试、clippy 和预编译着色器应用构建；ARM64 与 LoongArch64 的 QEMU 无界面测试、cargo-deny、统一 PDF 作业通过。Linux 与 LoongArch64 日志分别确认 143 项测试通过，含全部 8 项新增 DOCX 会话测试。

统一 PDF 使用 NotoSansCJK-Regular.ttc face 2，SHA-256 b76b0433203017ca80401b2ee0dd69350349871c4b19d504c34dbdd80541690a；5 行、0 缺字，PDF 17,374 bytes。Poppler 独立读回的行首与行尾最大偏差均为 0.0000 pt；行／页断点比较作业同时通过。依赖检查结果仍按既有停止维护公告例外解释，不声明所有传递依赖无公告。

之前因付款／消费额度而未启动的第 1 次执行保留为历史失败；第 2 次执行证明当前公开仓库可正常跑 CI。CI/QEMU 不替代 GUI、信创真机、IME 或 Word/WPS 人工验收，acceptance_complete 仍为 false。本段证据绑定上述源码 SHA；后续记录提交仅修改文档。

新增测试分别覆盖：五个 fixture 原包逐字节保留和模型等价；独立保护、强制修订、写保护及关闭设置；各写入来源和撤销／重做拒绝；未知节点使其他可编辑段落也只读；内存编辑与全部撤销后拒绝陈旧原包；失败和空事务保持版本／原包；坏包及超预算拒绝。

字体：Songti.ttc face 0，SHA-256 dd2f0628d302f322a2e9c3cf3253d5e73f418275e1f1421c84a7c65ce5a87777，仅本地使用；拉丁固定 OFL fixture joyoffice-test-serif.ttf face 0，SHA-256 cbc12abc87271f04aa0cf107214a49bad3b865f2dd01a4dec80fed818c032ec3。没有提交系统字体。

## 优先级与阻塞

1. CI 运行条件已恢复：公开仓库后，C01 源码提交的第 2 次执行 8/8 全通过，见上文。main 的历史运行 38529091352 仍记录为未启动失败，不能据 C01 重跑改写其结果；详情见 [CI 报告](CI-WINDOWS-FONT.md)。
2. M0 的 T003/T005 与 ADR-010 继续作为阶段门槛：正式 GPUI 来源须支持 GL 或 CPU，实际字体身份、IME、DPI 和信创目标机仍待验证；本轮未控制桌面。
3. T103 后续先实现保守 ExportPlan 与编辑后安全写回，再处理文件锁及保留元数据的原子保存和 fuzz。新的 DOCX 打开入口必须持有 DocxSession；通用 DocumentManager::open_file 尚未成为 DOCX 入口，不能只传模型绕过限制。
4. T105 完整断点生成、分页与命中数据就绪后，再推进 T102 画布输入及 T106 多文档／打开保存界面。M0/M1 未整体通过，未修改任务为 done。
