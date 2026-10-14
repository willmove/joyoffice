# C01：DOCX 导入到编辑会话的安全接入

- 执行者：当前 Codex 协调者，单人执行；不重新启动 W01–W04。
- 基线：main 的 2b9d58c2866703f4cd10b7f5e0fc56c961c23c57。
- 分支：codex/coordination-docx-session。
- 对应任务：T103、T106；需求 FORMAT-002、FORMAT-003、QUALITY-005。
- 优先级：后续打开／编辑入口前必须兑现导入限制；可独立于正式 GPUI 来源、GUI 和 Word/WPS 环境验证。M0 尚未通过，不解除阶段门槛。

## 本轮交付

在 joyword-shell 增量提供持有 DOCX backing、diagnostics、editability 和 engine Session 的适配器。全篇只读或 locked 非空时，在第一笔事务前设置 engine 全篇只读；局部锁未实现时不允许编辑其他段落。调用者不能通过适配器取得可变 engine、修改权限或替换模型。

未受限文档通过 engine 执行内存事务与撤销／重做；AI 写入默认关闭。精确原包复制只供没有提交过编辑的会话，提交后即拒绝，全部撤销仍保守拒绝。导入拒绝和资源超预算不能产生空白成功会话。

## 文件边界

- crates/joyword-shell/src/docx.rs、增量模块导出和 tests/docx_session.rs。
- shell Cargo.toml 接入 workspace 已有的 joyword-docx 与 joyoffice-opc；Cargo.lock 仅更新 shell 依赖边，不升级或引入外部库。
- 协调者更新公共文档与 C01 报告；不修改 Wxx 执行者报告。
- 不改 model、engine、OPC、font/shape/scene/PDF 公共 API。

## 验证与限制

使用五个原创 DOCX fixture 和独立保护／强制修订／写保护设置验证真实 importer→engine 链路；覆盖未知节点锁、不同写入来源、失败原子性、无编辑字节一致、编辑／撤销／重做后拒绝陈旧原包、坏包与预算拒绝。运行 shell 与全默认 workspace 测试、fmt、clippy、文档检查。

本轮不实现编辑后 DOCX 导出、文件系统写入、锁文件、元数据保留、文件对话框或画布输入；旧的通用 DocumentManager::open_file 不接收 DOCX，后续 DOCX 打开入口必须持有该适配器，不能只取模型。完整 T103/T106 与 M1 验收保持未完成。
