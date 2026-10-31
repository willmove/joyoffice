# JoyOffice Monorepo 迁移结果

2026-10-31。代码整合和本机验收已完成；远端 CI 与最终主分支合入状态以 [PR #115](https://github.com/willmove/joyoffice/pull/115) 为准。**从 main 检出本文件后直接开展产品开发，不要重新导入旧 JoyDeck。**

## 新的开发入口

日常工作目录为 `~/Coding/joyoffice`。先读根 `AGENTS.md`、[迁移计划](MONOREPO-MIGRATION.md)、[模块约定](ARCHITECTURE.md)、[Deck 开发指南](../joydeck/DEVELOPMENT.md)及 [Deck 状态](../joydeck/STATUS.md)。

- GUI：`apps/joydeck`；CLI：`apps/joydeck-cli`。
- 真实实现：`crates/joydeck-model`、`engine`、`formats`、`scene`、`export`、`platform`、`shell`。
- 两个产品共享根 workspace、Cargo.lock、Rust 1.98.1 与门禁，应用与产品模型保持独立。CLI 的实际依赖图不含 GPUI 或 Word。
- `cargo xtask check|test|build|package joyword|joydeck` 按产品选择；无界面使用 `--headless`，构建测试可指定 `--target`，原生验收包增加 `--validation`。
- 应用版本分别在应用 manifest 维护，CLI 与所属应用版本一致；发布使用 `joyword-v*`／`joydeck-v*`，历史 `v*` 只映射 Word。

## 实际整合与必要修复

源快照导入提交为 `1a09f6a`，JoyOffice 起点为 `eef903e0d37024109539ded520ad7070de607f37`。依赖对齐后的 GPUI Kit 0.8.1、GPUI pre 0.3.10、image 0.25.9 保持不变；根锁将 serde 统一为 Deck 已精确固定的 1.0.228。Word 与 Deck 各自的 PDF 版本线保留。

为通过依赖门禁，将 zip 从0.6.6升级为精确9.0.0，使用已有 flate2 Rust 后端，不添加安全公告忽略。Deck 格式层实际共享 `joyoffice-opc` 的原始 ZIP 结构及预算预检，拒绝重复原始成员后再进入名字索引，保留取消、路径及符号链接拒绝行为。完整文稿维持64MiB压缩、4096成员、单成员8MiB、总128MiB；嵌入工作簿保持独立4MiB压缩／128成员／8MiB总预算。新增回归防止把工作簿限制误用于完整文稿。完整 OPC／DrawingML 语义替换仍按切片推进。

普通 GUI／CLI／核心错误路径改为可恢复错误或停止操作；会话锁中毒时保留快照、保持 dirty 并拒绝编辑保存。显式原生验收代码使用 validation feature，并继续受同一门禁扫描，[逐项审阅](JOYDECK-GATE-AUDIT.md)没有给生产目录添加通配豁免。

Windows 首轮 CI 揭示工程工具用系统编码解码 UTF-8 Cargo 元数据的问题，已用显式 UTF-8 与子进程 UTF-8 模式修复，并增加真实非ASCII子进程回归。GUI 截图前明确确认前台可绘制窗口，不放宽原像素阈值。打包前后核对源码快照；源码变动即拒绝生成已验证包。

## 本机验证

实际日志、hash和范围见[验证索引](validation/20261031-monorepo/verification.json)。

| 检查 | 实际结果 |
| --- | --- |
| Word 分产品 fmt／严格 Clippy／测试 | 1272项Rust测试通过 |
| Deck 分产品 validation 检查 | 131项Deck +12项共享OPC测试通过 |
| 默认功能联合workspace严格Clippy／测试 | 1403项Rust测试通过 |
| 工程门禁及Python | 56项门禁测试；原有及门禁总139项；新增交付工具20项通过 |
| 独立release构建 | Word、Word CLI、Deck、Deck CLI均通过 |
| 原有测试迁移 | 123个原测试函数全部保留；新增9项，静态共132项，其中1项为Windows专属 |
| CLI／SBOM隔离 | CLI无GPUI／Word；GUI+CLI包444项依赖，包含共享OPC／zip9，排除Word |
| 新包一致性 | 1925项文件hash、167项源码hash、构建前后快照一致 |
| macOS真实窗口 | 比较接受／undo／redo、偏好恢复、24次开关窗及120次编辑撤销通过 |
| MP4／标题字幕 | 实际编码播放、独立解码270帧960×540／18秒／无音轨；标题VTT隐私检查通过 |
| 移出仓库运行 | 自包含App、中文空格文件事件、重复打开激活、原生菜单撤销通过 |
| 系统拼音 | 普通文字与表格的提交／取消／撤销／重开通过；OS合成键盘事件，非物理键盘认证 |

本机在线 RustSec 拉取曾超时；最终本机依赖检查使用缓存数据库 `2295466c4e9df26647fd67d8fa1c705ac2fc7f81`（2026-10-30），advisories／bans／licenses／sources均通过。远端工作流继续在线检查，合入须其通过。

验证包为本机 `dist/joydeck/joydeck-monorepo-final-utf8-debug.zip`，SHA-256为 `b9f24a6ad3475e52f4185b2f5f9c1c43d2981ff9bc02fd89c53d4b41e50447e7`；它包含明确的validation入口，不是正式发布。测试、CI不等于Windows/Linux GUI、干净机安装、信创硬件、双屏、正式签名或完整Beta验收；这些原有开放项保留。

## 原目录退出开发与恢复材料

[4752项原文件清单](validation/joydeck-source-inventory-20261031.json)、[新旧路径／拆分映射](validation/joydeck-migration-map.json)、[测试迁移](validation/joydeck-test-migration.json)与[源目录最终复核](validation/joydeck-source-final-recheck.json)均已保存。最终复核没有发现原目录文件遗漏、变化或新增；本次没有修改原目录。

完整旧源码／全部历史证据／日志和开发包保存在日常新目录 `~/Coding/joyoffice/backups/joydeck-migration-20261031/`，22351个文件的清单与大小已核对，原始源码归档hash已验证。该本机存档不进入Git，也不是构建依赖。[106项顶层历史产物索引](validation/joydeck-legacy-artifacts.json)覆盖22350个旧dist文件，旧包和历史证据hash保持原义。Git保留报告直接引用的关键证据，另有完整原始快照可供本机审计。

## 下一步怎么发起

在 `~/Coding/joyoffice` 工作时可以直接说：

> 请在 ~/Coding/joyoffice 中继续 JoyDeck 开发。先阅读 AGENTS.md、docs/joyoffice/MONOREPO-MIGRATION.md、docs/joyoffice/MONOREPO-RESULT.md 和 docs/joydeck/STATUS.md，检查 Git 状态。本轮完成【任务编号或具体功能】；保持产品模块边界，运行 Deck 相关检查，修改共享模块时同时验证 Word 使用方。不再从旧 JoyDeck 目录重新导入。

若需要我选择后续任务，可以把最后一句换成“先结合当前状态给出下一项任务建议与范围，再执行我已授权的工作”。后续公共能力优先考虑OPC剩余读写接入与DrawingML契约对齐；产品功能按既有任务清单推进，不把本次迁移当成原有阶段验收关闭。
