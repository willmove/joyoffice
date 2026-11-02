# JoyDeck 实施状态

2026-11-02：JoyDeck 0.3.0 / R2 基础编辑闭环切片。应用、CLI 和所有 crate 使用 joydeck 命名。Rust 1.98.1、GPUI Kit 0.8.1 / pre 0.3.10 与 JoyOffice 保持一致。

## 本轮实现

- 建立无文件/窗口/导出服务依赖的 joydeck-engine，统一 UI/CLI 命令和不可变快照；迁出 DeckCraft 的 copy-on-write 历史内存估算。
- 选择、Shift 多选、基础框选/拖动、取消手势；真实 Presentation 的对象/文字/页面基础编辑。一次拖动一次事务；失败不消耗 ID/历史，undo/redo 恢复选择和保存点 dirty。
- 复用 JoyOffice 文件指纹、同目录 prepare/sync/提交、租约和 revision 恢复服务；另存不覆盖既有文件，外部修改冲突拒绝，保存期间新编辑仍 dirty。关闭可保存或取消，未保存内容阻止打开/新建替换。
- .joydeck v2 明确 MIME、版本和必需能力，持有模型、原始包及对象映射；拒绝未来主版本/未知必需能力，保留未知成员/清单字段和原始载荷。旧 .deckcraft 仅导入。
- PPTX 基础保留式 writer：保留未触及成员，修改受支持的页面/对象 XML；出版前在内存中重开并核对页面、对象数量、文字、变换。不能稳定读回的写回拒绝，可保存为 JoyDeck 项目。
- CLI commands/edit/recovery-list/recover 与原 inspect/render；实际强制终止探针进程后，从新进程恢复编辑结果。

## 验证与交付

本轮 [R2 验证记录](evidence/R2/README.md) 包含产品/无界面门禁、CLI 独立检查、Windows 包、源码指纹、截图与适用范围。R0/R1 包保留。没有修改 JoyOffice 和 DeckCraft 源检出。

## 后续与限制

当前迁移的是基础命令子集，未把上游 329 条编辑命令称为已全部迁移；原 engine 依赖的历史格式集成测试仍在 tests-upstream，等待完整兼容 API 接回。跨次启动不持久化 undo 历史；恢复保存当前模型和原始包。多窗口共享、复杂指针/路径命中和完整编辑器菜单仍需继续验收。

整段文字替换采用首个文字样式，原位富文本和系统 IME 属 R3。画布仍使用共享 CPU 参考渲染器，GPUI 原生对象场景和性能优化属 R4。PPTX 的签名、复杂对象、动画/连接引用和节/自定义放映结构等有明确受限写回；原始内容在项目中保留，不保证 PowerPoint 全保真。

Windows x86_64/MSVC 为本轮实际环境；macOS/Linux 原生构建和桌面未执行。开发包未签名；正式安装更新与完整许可文本审计仍属后续阶段。
