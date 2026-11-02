# Linux 系统文件选择器（2026-11-02）

## 变更

用户要求打开 Word 文档时遵循 Ubuntu／其他桌面系统的选取习惯。Linux 的“打开”与 Ctrl+O 现在优先调用 GPUI 的系统 `prompt_for_paths`，通过 [XDG FileChooser](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html) 由桌面后端绘制选择窗口，不在 JoyWord 内另造目录导航界面。本机为 UOS Desktop 25 Professional／DDE，实际显示 `dde-file-dialog`，窗口的 `WM_TRANSIENT_FOR` 与发起请求的 JoyWord 窗口一致。Ubuntu 的实际外观取决于其桌面后端，本轮没有 Ubuntu 真机。

用户取消系统窗口后直接返回文档；重复执行打开不叠加请求。系统接口返回错误时，保留原路径输入和 zenity／qarma／kdialog 辅助选择退路；“浏览”始终允许重试系统接口。请求身份和窗口生命周期校验防止过期回调操作新的路径窗口，返回原文档或备用路径框时恢复对应焦点。

使用既有 GPUI API，没有新增依赖、修改锁文件、调整文档格式支持或改变导入保护契约。另存副本和导出沿用原有流程。此变更没有对无响应门户加入超时，不宣称修复所有旧 UKUI 后端挂起问题。

## 自动检查

基线：远端 `main` 的 `95f632dee778d53bee67d9391c6c1b402861bca3`；工作分支 `fix/joyword-native-file-chooser`；Rust 1.98.1。

实际运行并通过：

```sh
cargo xtask check joyword
cargo xtask build joyword
python3 scripts/check_docs.py
python3 scripts/check_gates.py
```

产品工程检查包含 fmt、全目标 Clippy（`-D warnings`）和 **1,317 项测试**，其中桌面 99 项。没有把重复执行累计为新增覆盖。开发二进制 `target/debug/joyword` 的 SHA-256 为 `65469004e17ca0fb0c86dd36f9b35e639cd626f499945685537a4e2c1099c151`。

本机编译器和 X11 工具经 `/run/host/rootfs` 访问，临时命令转接位于 `/tmp/joyword-tools`；没有更改系统软件安装或输入法配置。

## 原生窗口验证

同一最终二进制在本机 X11／125% 缩放下完成六项 OS 鼠标／键盘事件检查，截图经目视核对：

1. Ctrl+O 弹出系统文件选择窗口，侧栏、图标列表和选择按钮来自 DDE。
2. 点击系统侧栏、进入文件夹并双击原创 DOCX 副本，JoyWord 标题切换到对应文档。
3. 再次调用打开能够重新显示系统窗口。
4. 请求在途再次执行打开，只有一个关联的系统窗口。
5. 系统“取消”返回原文档；随后正文键入和撤销均可用。
6. 两份测试源文件的 SHA-256 保持不变。

测试使用独立配置和临时文件，保留用户正在使用的窗口。临时主目录测试文件夹已删除。原始记录和截图在本机忽略目录 `artifacts/native-file-chooser-20261102/native-final/`，机器结果为 `report.json`；截图 `01-system-file-chooser.png`、`02-opened.png`、`03-cancelled.png`。默认 Fcitx 环境未覆盖或关闭；这组键盘事件不等于物理中文 IME／候选框验收。

另对独立进程设置不存在的 `DBUS_SESSION_BUS_ADDRESS` socket，验证连接失败时显示备用路径框并保留“浏览”，Escape 可取消且原文档不变；未改系统 D-Bus 服务或当前用户会话。结果与截图在 `artifacts/native-file-chooser-20261102/fallback/`，与原生检查使用同一二进制散列。

未覆盖：Ubuntu 实机、Wayland、macOS／Windows 本轮运行、其他架构、系统读屏器、物理中文 IME、门户无响应／崩溃的全部组合。没有把完整平台任务或里程碑标为 done。
