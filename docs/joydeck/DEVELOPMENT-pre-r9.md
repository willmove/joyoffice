# 在 JoyOffice 中开发 JoyDeck

所有开发从 JoyOffice 仓库根目录开始；不使用旧独立 JoyDeck 目录的源码、target、脚本或环境默认路径。macOS 本机目录通常为 `/Users/willm/Coding/joyoffice`，CI 和其它开发机使用实际检出路径。

先读根 AGENTS.md、[迁移计划](../joyoffice/MONOREPO-MIGRATION.md)、[模块边界](../joyoffice/ARCHITECTURE.md)、[当前状态](STATUS.md)和实施任务。迁移验证结果与未完成项以[迁移结果](../joyoffice/MONOREPO-RESULT.md)为准。

## 工程命令

```sh
cargo build -p joydeck --locked
cargo build -p joydeck-cli --locked
cargo run -p joydeck --locked
cargo run -p joydeck-cli --locked -- diagnostics
cargo xtask check joydeck
cargo xtask test joydeck
cargo xtask package joydeck --release
```

CLI 没有 GPUI 依赖。媒体、打印和视频的可用能力取决于平台及打包的原生 helper；不能将无 GUI 构建成功记为三端实际运行通过。桌面包和 helper 必须从新位置重新构建，不能沿用旧目录的 target 文件冒充迁移验证。

## macOS 播放挂起诊断

手动运行 GitHub Actions 的 `JoyDeck playback diagnostic`（`.github/workflows/diagnose-joydeck-playback.yml`），可通过 `cargo xtask check joydeck` 复现原生播放测试挂起，并在超时后采样所启动进程及其子进程的栈和打开文件。日志与报告保存在 `joydeck-playback-diagnostic-entrypoint` artifact 中；超时或测试失败仍返回失败，不视为修复或验收通过。

该诊断来自原 `codex/diagnose-joydeck-playback-20261031` 分支，合入时迁为独立的手动 workflow，保留 `ci-joydeck.yml` 的自动触发、headless 和跨平台桌面检查。脚本依赖 macOS 的 `sample`、`lsof` 与 POSIX 进程组，不在 Windows 上直接执行；本轮仅在 Windows 验证整合和工程检查，未重新执行 macOS 挂起采样。

## 目录与证据

- `apps/joydeck`：GPUI 应用；`apps/joydeck-cli`：命令行入口。
- `crates/joydeck-*`：实际产品模块；`crates/joyoffice-*`：跨产品公共模块。
- `tests/fixtures/joydeck`：原创测试样本，见其 README；系统字体不入库。
- `scripts/joydeck`、`packaging/joydeck`、`licenses/joydeck`：产品验证、打包材料与许可证。
- `docs/joydeck/evidence`：关键历史证据。历史 JSON 仍含旧路径/旧构建 hash，参照迁移路径映射读取，不修改历史文件来宣称新构建通过。
- 本次新验证使用新名字的输出目录，不覆盖历史开发包。完整旧快照及开发包在本机 `backups/joydeck-migration-20261031`，不参与构建、不属于 Git 检出依赖。

## 下一步请求示例

> 请在 ~/Coding/joyoffice 中继续 JoyDeck。先阅读 AGENTS.md、docs/joyoffice/MONOREPO-MIGRATION.md、迁移结果记录和 docs/joydeck/STATUS.md，检查当前 Git 状态。本轮完成【任务编号或用户可见功能】，保持产品边界并运行相关测试；涉及公共模块同时验证 Word 使用方。不回到旧 JoyDeck 目录。
