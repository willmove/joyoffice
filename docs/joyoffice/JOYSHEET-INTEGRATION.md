# JoySheet Monorepo 整合

2026-10-31，用户批准实施。任务分支为 `codex/integrate-joysheet`，主仓库起点为 `d47f036340f5aed6b2ad114c434184cf3b7436a2`。原 JoySheet 本地仓库没有提交历史，本次通过可核对的文件快照导入。

## 范围与边界

- 一个 Git 仓库、主 Cargo workspace、根 Cargo.lock 和 Rust 1.98.1。
- 两个应用入口、15 个产品库和一个专用工程工具；产品包名为 `joysheet-*`。
- 工作簿模型、计算、引擎、UI、协议、用户配置与恢复数据继续保持原有职责及兼容性。
- Sheet 独立版本、构建、测试、分发和 `joysheet-v*` 发布路由；迁移开发包不等于正式发布或 M8 完成。
- OPC/DrawingML、字体和平台服务的进一步共享按后续切片实施。

## 来源与恢复

原目录为 `D:\Coding\EditorProjects\joysheet`，迁移过程中不修改其源码。完整源码和历史包/日志存放于本机 `backups/joysheet-migration-20261031/`，构建不依赖该归档。

[原始文件清单](validation/joysheet-source-inventory-20261031.json)、[路径映射](validation/joysheet-migration-map.json)和[测试模块迁移](validation/joysheet-test-module-migration.json)分别记录来源、去向及测试体拆分。GridCraft 原始来源哈希与原有适配记录保留在产品文档中，不改写为迁移后结果。

## 已运行的迁移前基线

独立副本的原 `cargo xtask ci` 已通过：1,390 项测试通过、1 项忽略，同时通过 fmt、严格 Clippy 和原分层检查。原 M8-preview6 解压包 Windows GPUI 串联回归 52 项通过。应用 SHA256 为 `80c862341e77cb60c731ff5d95bf3dba8f6ecfe6f02b694a0377c12492642169`。这些是迁移前基线，不是新包验证。

## 实施内容

- [x] 保存来源清单、原始源码和历史产物归档。
- [x] 重跑迁移前工程与 Windows 桌面基线。
- [x] 导入产品模块及包名；将内联测试体迁入独立测试文件，保留生产扫描强度。
- [x] 接入根 workspace，保留加密库版本线并适配 zip 9 的可失败名称解码 API。
- [x] 接入产品工程、分发、CI 和发布路由，增加独立源码包与实际依赖许可清单。
- [x] 三个产品共同使用根工程门禁，独立选择构建与验证功能。
- [x] 新旧文件、桌面、性能及安装/卸载脚本保留可重复入口。

实际执行结果、包哈希、CI 和 PR 状态单列于 JoyOffice 的 `docs/joyoffice/JOYSHEET-RESULT.md`；上述实施内容不代替相应运行证据或人工验收。

## 依赖与门禁

Sheet 原 serde 1.0.229 对齐主仓库 1.0.228；serde_json 的 float_roundtrip 在 Sheet 各调用方显式保留。zip 3.0.0 接入主仓库 9.0.0，保留工作簿预算、未知内容和失败契约。SHA-2 0.11.1、getrandom 0.4.3 按 Sheet 实际需求独立精确固定，主产品既有版本线保持。

Windows 剪贴板沿用 arboard 3.7.0；clipboard-win 5.4.3 与 error-code 3.4.0 的 BSL-1.0 接受限定于这两个精确包，原文进入产品实际依赖清单。cargo-deny 0.20.2 已通过 advisories/bans/licenses/sources。本次没有新增安全公告忽略。

逐文件副作用登记见 [门禁审阅](JOYSHEET-GATE-AUDIT.md)。输入法、GPU 呈现、真实读屏、物理打印、macOS/Linux 原生、正式签名和信创真机等既有未验证事项继续开放；CI 和合成输入不能替代相应验收。

Sheet 的开发构建保留原 opt-level=1，通过精确包覆盖接入；Word/Deck 与公共依赖的 profile 不变。主仓库默认 opt-level=0 曾使三个递归栈预算回归提前返回 #NUM，故本轮保持原编译配置，而不放宽预算或改变断言。

原 zip 3.0.0 实际依赖审查命中 RUSTSEC-2026-0367、0368、0369，因此保留升级到 9.0.0，不添加公告忽略。zip 9 会优先猜测 UTF-8；Sheet 明确按语言编码标志解码，保留无标志 CP437 名称，拒绝无效的声明 UTF-8，避免旧部件名称碰撞。独立 XLSX crate 显式选择根 flate2 Rust 后端，不再依赖 image/OPC 的 feature 合并。
