# JoyOffice 开发约定

适用于整个仓库，用户明确指令优先。

## 工作方式

- 可由当前会话直接完成任务，也可由协调者按任务边界并行派工多个 Agent；是否单会话或并发以加快交付为准，不强制某一种。
- 并发时：每个 Agent 独占一条任务分支与明确文件／模块范围，互不改对方工作区；共享文档（如 `TASKS.md`、`IMPLEMENTATION.md`）只追加本任务证据、改本任务状态与进度表中与本任务相关的计数，合并前对最新 `main` 做 merge（禁止强推、禁止 rebase 已推送历史）。
- 开始前检查工作目录、git status、分支和 HEAD；按用户指定分支工作，未指定时从最新 `main` 开任务分支；独立 worktree 可选。
- 按需阅读 docs/joyword/ 下的 SPEC.md、ARCHITECTURE.md、QUALITY.md、TASKS.md 和 IMPLEMENTATION.md；docs/coordination/ 仅供历史查证，其中旧的固定派工／文件独占表不再生效，由当次任务说明划定范围。
- 本仓库为 Word／Deck／未来 Sheet 的 Monorepo。开始 Deck 工作先读 `docs/joyoffice/MONOREPO-MIGRATION.md`、迁移结果记录与 `docs/joydeck/STATUS.md`；在本仓库开发，不回到旧独立 JoyDeck 目录。应用位于 `apps/`，产品模块位于 `crates/joyword-*`／`crates/joydeck-*`，跨产品模块位于 `crates/joyoffice-*`，禁止产品应用互相依赖。
- 只修改任务相关内容，保留已有未提交修改；必要时同步相关文档。提交、推送按用户或协调者要求执行，不强推。
- 提交信息一律用英文书写（2026-10-18 用户明确）。

## 工程要求

- 固定 Rust 1.98.1 或以上、edition 2024，遵守 unsafe_code = forbid；依赖精确固定，不顺带升级，不创建空壳成功接口。
- engine 负责编辑，layout 只读，OPC 不含 Word 语义；屏幕/PDF 共用页面场景，字体按实际字节与 face 标识；公共 API 优先增量扩展。
- 保持不可信输入预算、原始内容保留和只读/拒绝契约；不静默丢弃未知 XML。
- 从公开规范独立实现并记录来源；不搬运 Word/WPS/Zed 编辑器实现。通用库须符合许可，系统字体不提交。

## 验证与交付

- Rust 修改运行相关 crate 测试、fmt、clippy；文档/语料修改运行 python3 scripts/check_docs.py。
- 使用产品工程命令选择 Word／Deck 的构建、测试和打包；修改共享库须测试其使用方。公共锁文件、resolver、构建规则变更须运行跨产品回归。产品有独立版本与发布标签，不能为发布单一产品无条件提高整个 workspace 版本；原有 v* 仅兼容 JoyWord，后续使用 joyword-v*／joydeck-v*。
- 完成后说明变更、测试和未覆盖项；区分实现、自动验证与人工验收，有证据才标 done。缺环境写明未验证，CI/QEMU 不替代真机验收；GUI 验证可直接执行，无需提前约定时段或再次确认（2026-10-30 用户明确）。
- 信创真机验证留待有条件时补验，不阻塞开发、任务依赖、阶段验收或发布；保留相关构建与自动测试要求，未实测的平台明确标未验证。
- 并行任务各自开非草稿 PR；合并前须合入最新 main、CI 全绿且可合并；由协调者或用户合并，执行 Agent 不自行合并，除非用户明确授权。
