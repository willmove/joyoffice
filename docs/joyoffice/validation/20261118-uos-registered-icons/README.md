# UOS 已注册启动器覆盖窗口图标复验

2026-11-18。基线 `f04d8aa5`，分支 `fix/uos-desktop-icon-identity`。用户反馈前轮修复后，从终端直接运行 `target/debug`／`target/release` 仍显示通用图标。本轮在同一台 UOS 25 x86_64 / X11 真机复现并修复，没有把旧包或错误分支当作原因。

## 根因和前轮遗漏

1. 当前新构建的 JoyWord 已有正确的 256 × 256 `_NET_WM_ICON`，全部 ARGB 像素与所属 ICO 一致，但真实任务栏仍显示[通用图标](gui/before.png)。因此窗口属性正确不能单独证明任务栏正确。
2. 用户目录已有三份 `~/.local/share/applications/joy*.desktop`，含 `Icon=joyword`／`joydeck`／`joysheet`，以及各自的 `StartupWMClass`。SVG 文件存在。DDE 2.0.52 的 [`AppItem::icon()`](https://github.com/linuxdeepin/dde-shell/blob/2.0.52/panels/dock/taskmanager/appitem.cpp#L53-L59) 优先使用有效桌面项的图标，覆盖窗口图标；[匹配逻辑](https://github.com/linuxdeepin/dde-shell/blob/2.0.52/panels/dock/taskmanager/taskmanager.cpp) 也适用于从终端直接启动的窗口。
3. 当前任务栏对这些图标名称查询失败。磁盘 `hicolor/icon-theme.cache` 初始早于新 SVG；重建磁盘缓存后，已运行任务栏仍显示通用图标。只把桌面项改为 SVG 的绝对路径，重开同一个二进制，图标即恢复；没有重启 shell 或重新编译。这个对照把故障定位到桌面图标名称解析／会话内缓存链路，而非内嵌 ICO 解码。
4. 前轮实屏验证时没有真实用户注册项，隔离 GIO 验证也只检查窗口图标和文件存在，遗漏了已注册桌面项优先覆盖的情况。前轮“任务栏正常”的结论只适用于当时验证条件，已在[原记录](../20261118-desktop-icons/README.md)明确修正。

## 修复

- 三产品便携／开发安装脚本使用安装后的 SVG 绝对路径，按 [Desktop Entry 的 iconstring 规则](https://specifications.freedesktop.org/desktop-entry/latest/value-types.html)转义，避免依赖当前桌面是否已经缓存图标名称。可执行文件保留符号链接调用名，不破坏按名称分派的包装脚本。
- 新增 `--refresh-icon`，只修改既有主桌面项的 Icon，保留 Exec、本地化名称、文件关联、注释和桌面动作。已对本机三份用户桌面项执行；原有 glibc 兼容包装器及其路径保持不变。安装脚本可选通知已运行的 DDE 应用管理器重读注册项，提示重开产品窗口，不启动或重启桌面服务。
- JoySheet deb 直接引用安装后的 `/usr/share/icons/hicolor/scalable/apps/joysheet.svg`。可移位 AppDir 保留名称引用，由注册脚本生成实际路径。发布检查拒绝 deb 暂存资源中旧的名称引用和旧版注册脚本。
- Rust、依赖和产品版本未改。本轮针对桌面注册问题，使用现有新构建完成运行复验。

## 验证与边界

| 检查 | 结果 |
| --- | --- |
| Python 图标／注册 | 9 项通过；实际 GIO 解析为绝对文件图标，覆盖无主题缓存和中文／特殊字符路径；实际 GIO 启动保留符号链接调用名 |
| 发布资产 | 10 项通过，含拒绝旧注册脚本、错误品牌资源和 deb 名称引用 |
| Sheet 打包 | 9 项通过，含 deb 绝对路径与 AppDir 可移位布局 |
| 工程路由与源码 | 18 + 2 项通过；本轮相关 Python 共 48 项 |
| 三产品当前构建 | 原生宿主启动、从原生终端直接运行均核对实际窗口；各自 256 × 256 ARGB 像素与 ICO 一致，任务栏属性引用正确的绝对 SVG |
| 已安装程序 | 通过现有真实桌面项 Launch 运行，保留 glibc 包装器；三产品任务栏属性和实屏图标正确。已安装的是此前发布程序，其 `_NET_WM_ICON` 仍为空，不能据此声称旧二进制已包含前轮 Rust 修复 |

运行属性、程序指纹、启动分组及修复前后引用见 [report.json](report.json)。从原生终端启动的三个新二进制分别为脚本记录的实际 PID，处于原生 `deepin-terminal` 分组；没有用独立 systemd 应用分组代替这个场景。

用户退出 Snipaste 全屏截图模式后补拍实时桌面，三产品图标由左至右为 Word、Sheet、Deck：[终端直接运行](gui/terminal-after.png) · [已有安装启动器](gui/installed-after.png)。截图模式冻结的中间画面不作为本轮图标验收证据。仅保留任务相关的任务栏裁剪；验证后退出本轮创建的进程与终端，不关闭用户原有窗口，不编辑文档。

本轮工程门禁为 `check_docs.py`、`check_gates.py`、291 项来源指纹和 `git diff --check`。未重新生成正式发布包或执行 deb 安装／升级；没有改动 Rust，未重复全量 Rust 测试。Wayland、其他系统和架构仍未补验；此处不扩展前轮的平台验收。
