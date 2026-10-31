# JoyOffice Monorepo 迁移与后续工作

2026-10-31，用户已批准实施。原 JoyDeck 独立目录将停止作为开发入口。

## 已批准目标

- 一个 Git 仓库、一个主 Cargo workspace、产品共享根 Cargo.lock 和 Rust 1.98.1。
- 桌面与 CLI 分别位于 apps/joydeck、apps/joydeck-cli；真实实现拆入 crates/joydeck-model、engine、formats、scene、export、platform、shell。
- Word、Deck、未来 Sheet 保持独立模型、构建、测试、版本与发布；应用不能依赖另一产品应用。
- 公共能力使用 joyoffice-*；已有 OPC/DrawingML 的替换须先对齐预算与保留契约，以行为回归逐步推进。
- 直接采用目标目录；不引入 products/joydeck 或永久杂物 core。
- 生产错误路径可恢复；逐文件审阅 IO/进程、panic，工具也受门禁约束。
- 产品独立 CI/发布路由，未来标签 joyword-v*、joydeck-v*、joysheet-v*；历史 v* 保留 Word 兼容。
- 本次不发布正式版本、不改既有功能验收状态、不创建空的 JoySheet 包。

## 实施顺序与验收

1. 记录原文件 hash 与新旧路径映射；保留完整源代码、证据与既有开发包的本机归档。
2. 拆开纯数据、编辑、格式、场景、输出、平台及服务边界；保持原生 schema 与命令语义。
3. 迁移测试与脚本，统一根 workspace、依赖、工具链、工程命令。
4. 接入架构/IO/panic、文档、依赖与 provenance 检查；不以目录级豁免通过检查。
5. 分产品构建/测试/打包，并联合验证 feature 组合；macOS 验证新包和移位启动。
6. 提交并创建 PR，CI 通过后合入 main；记录结果与未验证平台。

## 本机存档

原始文件清单见 validation/joydeck-source-inventory-20261031.json。完整旧源码、历史证据和日志保存在仓库本机 backups/joydeck-migration-20261031/，不进入 Git。开发与构建不得依赖该存档。Git 中保留报告引用的关键证据，旧证据的路径和 hash 不重写为新验收结果。

## 进度

- [x] 已批准目标及后续行动说明迁入新仓库。
- [ ] 完成目标模块拆分及 workspace 接入。
- [ ] 完成工程工具、CI 与独立发布机制。
- [ ] 完成全部迁移验证并记录实际结果。
- [ ] PR 合入 main，原 JoyDeck 目录退出开发使用。

## 在新目录继续

以后在 Codex 中选择 /Users/willm/Coding/joyoffice 项目，开始工作时先阅读本文件、JOYDECK-INTEGRATION.md、docs/joydeck/STATUS.md，以及迁移结果记录；检查分支与工作区，不回到原独立目录开发。

可使用的提示词：

> 请在 ~/Coding/joyoffice 中继续 JoyDeck 开发。先阅读 AGENTS.md、docs/joyoffice/MONOREPO-MIGRATION.md、迁移结果记录和 docs/joydeck/STATUS.md，检查当前 Git 状态。保持 Monorepo 产品边界，本轮完成【具体功能或任务编号】，运行 Deck 相关测试；涉及公共模块时同时验证其使用方。不要回到旧 ~/Coding/joydeck 目录。

如果只是继续尚未完成的迁移：

> 请继续 ~/Coding/joyoffice 的 Monorepo 迁移，先读取 docs/joyoffice/MONOREPO-MIGRATION.md 与迁移结果/交接记录，从第一个未完成项继续，保留已有工作，不重复导入旧快照。
