# JoyOffice 开发约定

适用于整个仓库，用户明确指令优先。

## 工作方式

- 默认由当前会话直接完成用户任务，不启用多 Agent 派工、协调者、交接报告或后台监控。
- 开始前检查工作目录、git status、分支和 HEAD；按用户指定分支工作，未指定时沿用当前分支，不强制创建 worktree。
- 按需阅读 docs/joyword/ 下的 SPEC.md、ARCHITECTURE.md、QUALITY.md、TASKS.md 和 IMPLEMENTATION.md；docs/coordination/ 仅供历史查证，其任务分配与文件独占规则不再生效。
- 只修改任务相关内容，保留已有未提交修改；必要时直接同步相关文档。提交、推送按用户要求执行，不强推。

## 工程要求

- 固定 Rust 1.98.1、edition 2024，遵守 unsafe_code = forbid；依赖精确固定，不顺带升级，不创建空壳成功接口。
- engine 负责编辑，layout 只读，OPC 不含 Word 语义；屏幕/PDF 共用页面场景，字体按实际字节与 face 标识；公共 API 优先增量扩展。
- 保持不可信输入预算、原始内容保留和只读/拒绝契约；不静默丢弃未知 XML。
- 从公开规范独立实现并记录来源；不搬运 Word/WPS/Zed 编辑器实现。通用库须符合许可，系统字体不提交。

## 验证与交付

- Rust 修改运行相关 crate 测试、fmt、clippy；文档/语料修改运行 python3 scripts/check_docs.py。
- 完成后说明变更、测试和未覆盖项；区分实现、自动验证与人工验收，有证据才标 done。缺环境写明未验证，CI/QEMU 不替代真机验收；GUI 验证先与用户约定时段。
