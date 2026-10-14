# C03：已验证 DOCX 导出副本的安全落盘

- 执行者：当前 Codex 协调者，单人执行。
- 基线：C02 的 4dd3870a609509ec225a7f85cb8509fbb602dfae。
- 分支：codex/coordination-docx-storage。
- 独立目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice。
- 对应：T103/T106，FORMAT-003、FORMAT-004、QUALITY-004。

## 交付边界

提供仅创建新路径的 DOCX 导出副本落盘接口，输入必须是已验证、版本绑定的 ExportPlan。在同目录私有临时文件完整写入、sync 和逐字节回读后，以不覆盖目标的硬链接发布；已存在文件、目录与符号链接均拒绝。发布前失败不创建目标；发布后目录 sync／清理失败必须明确说明目标已发布，不能假报未写入或删除目标。返回收据绑定实际写出的版本；不改变 DocxSession 或通用 DocumentManager 的保存状态。

只使用固定工具链与 safe Rust。协调者裁决：为安全临时文件增加直接 tempfile =3.27.0，使用锁文件已有精确版本；不升级或新增其他外部包。root Cargo.toml／lock 仅此依赖声明与 shell 边。允许 shell 新 storage 模块、导出、集成／故障测试、示例和本任务报告，以及协调者公共文档。

不实现既有文件覆盖、原文件元数据复制、Office 锁文件或原文件修改检测，不把“导出副本”称为完整保存。硬链接不支持时明确失败，不回退成可覆盖复制。

## 验证

真实导入→编辑→计划→落盘→重开；既有目标保护、坏路径、符号链接、并发同目标只有一个成功；写入／sync／回读／发布故障注入，发布后失败分类；会话版本和撤销保持。运行 shell 与默认成员测试、release、fmt／clippy、文档工具检查。真实其他系统、断电及固定 Office 验收独立记录。
