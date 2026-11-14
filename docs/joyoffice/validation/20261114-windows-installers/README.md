# v0.1.4 Windows 安装器补齐

2026-11-14。用户要求 JoyWord、JoyDeck 与 JoySheet 一样提供 Setup EXE。本轮仅补充分发，不改变原 v0.1.4 应用字节或产品标签。原应用源码为 `fee5a463c740c6f37f09476678f2848fa136ff0b`；安装器实现和来源说明见[三产品 Windows 安装器](../../WINDOWS-INSTALLERS.md)。

## 本机验证

- Word、Deck 的 Windows x64 ZIP 从公开 Release 下载，匹配原 SHA256。安装到仓库 target 下带中文/空格的隔离路径，逐文件比较安装载荷与 ZIP，全部一致；安装后的 CLI `--help` 成功。
- 当前用户真实安装登记和开始菜单入口通过；测试在不存在产品注册项/快捷方式时执行，未替换既有安装。
- Word/Deck 卸载后当前用户登记和快捷方式移除，额外 `user-owned.txt` 保留。Sheet 检测到本机既有安装，系统登记测试按契约拒绝覆盖，改用 `/TEST` 隔离回归。
- Computer Use/sky 实屏验证安装目录中的普通生产应用：Word 打开随包混合中英文、图片、表格 DOCX；Deck 普通编辑窗口显示正常。证据：[Word](gui/word-installed.png)、[Deck](gui/deck-installed.png)。
- Python 安装器与发布门禁回归初轮 194 项，7 项既有环境跳过，无失败。最终工程和原生 runner 结果以同目录证据及工作流为准。

## 验证边界与交付

本机为 Windows x64。ARM64 通过原生 runner 自动安装/CLI/卸载验证，不声称人工实机验收。默认 smoke 的 `/TEST` 跳过注册表和快捷方式；本机单列的当前用户登记测试覆盖这两项。可选“打开方式”的 Explorer 行为、完整升级/回滚、签名、系统 IME/读屏/打印/多显示器不在本轮验收范围。原产品功能差距继续保留。

公开安装器补充以 GitHub 实时 Release 为准。原始 ZIP、其他平台资产、公开标签均保留；新增安装器携带原 ZIP 摘要和安装器源码指纹，以区分应用源码与分发实现。
