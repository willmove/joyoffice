# C04：受预算约束的自动 Unicode 断点

- 执行者：当前 Codex 协调者，单人执行。
- 基线：574c44f3f9e4b9a93766ce7fa7f3dd8aa8ecb05e（C03）。
- 分支：codex/coordination-unicode-wrap。
- 目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice。
- 对应：T105、LAYOUT-002、LAYOUT-003、LAYOUT-009。

## 交付

在现有单字体 LTR 行拟合之上增量提供自动 Unicode 断点入口；现有显式断点 API 和错误契约不变。Unicode 候选断点与整段 shaping cluster 边界求交，再使用原有候选行重整形、剩余文本 cluster 复验与资源预算。不得用重复调用 fitter 复位预算；不得为断行切开整形簇或丢弃空白。

协调者裁决精确复用锁文件已有 unicode-linebreak =0.1.5（Unicode 15.0.0）；只增加 layout 依赖边，不升级外部包。其 UAX #14 默认规则不等同于固定 Word 的中文兼容规则。允许 layout/line.rs 的内部复用重构、增量导出、tests/line_fonts.rs 的新增集成测试、line_scene 示例和本报告，以及协调者公共文档。不改字体／整形／页面场景公共 API。

## 验证和限制

真实字体覆盖中文标点、拉丁空格／NBSP／Word Joiner、组合字符、emoji ZWJ、fi 连字、空段落及所有输入字节／标量连续覆盖；预算拒绝、超宽诊断及显式入口回归。运行 layout／全默认成员测试、fmt／clippy、文档检查。

完整中文 Word 规则、混合样式、字体回退、分页、命中、GUI 和 Office 排版验收仍待后续实现。
