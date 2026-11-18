# 桌面产品图标修复

2026-11-18，分支 `fix/desktop-product-icons`，基线 `1bc74140e15970855928a373d86330dab127b5f5`。本轮只处理三产品桌面图标、开发运行和便携包桌面注册，不改变编辑模型或历史平台验收。

同日用户复测仍见通用任务栏图标。本记录验证了窗口属性和无真实用户注册项时的任务栏，遗漏已注册 `.desktop` 优先覆盖窗口图标、且图标名称无法解析的场景；因此不能据此声称所有启动方式都已解决。根因复现、补充修复和使用现有启动器的结果见 [UOS 任务栏复验](../20261118-uos-registered-icons/README.md)。以下保留前轮实际范围与证据。

## 原因和修复

- 既有 SVG、ICO、ICNS 资源及 Windows EXE 资源编号 `1` 均存在；macOS 发布 `.app` 已引用各自 ICNS。图标资源存在不能证明运行窗口已经使用它。
- 三产品原有六个 `WindowOptions` 入口只设置 `app_id`，未设置 `icon`。固定 GPUI Pre 0.3.12 的 X11 后端只在收到 `WindowOptions.icon` 时写入 `_NET_WM_ICON`，不会从 `app_id` 自动加载包内 SVG。修复前本机 JoyWord 的该属性确实不存在，见 [baseline.json](baseline.json)。[EWMH 图标规范](https://specifications.freedesktop.org/wm/latest/ar01s05.html)与[桌面项规范](https://specifications.freedesktop.org/desktop-entry/latest/recognized-keys.html)分别定义运行窗口图标、启动器图标和窗口类匹配。
- `joyoffice-ui` 复用入库 ICO，按产品缓存解码后的 256 × 256 RGBA；Word、Sheet、Deck 编辑器/示例/放映/演示者窗口统一选用所属品牌的窗口默认项。应用内品牌位置复用内嵌 SVG，移除 Deck 示例的临时 `JD` 字样。没有重画图标或引入系统字体。
- Linux 便携包增加 `install-desktop.py`；用户显式运行后安装自己的 SVG/桌面项，使用正确引用和经过转义的绝对路径。支持 AppDir、仓库开发构建、中文/空格/引号/反斜杠/美元符/百分号路径。发行检查拒绝旧版安装脚本，源码指纹包含脚本及共享 UI 实际内嵌的全部品牌资产。Wayland 需要先注册匹配 `app_id` 的桌面项。
- macOS `cargo xtask run` 现在从 Cargo 实际返回的产物创建所属产品的开发 `.app`，写入 ICNS/Info.plist 并运行包内可执行文件。原 JoyWord 开发包装脚本也修复为读取统一工作区版本。Windows 继续使用已有 EXE/安装器图标链路。

## 自动验证

| 检查 | 本轮结果 |
| --- | --- |
| 三产品 `cargo xtask build` | GUI 和 CLI 均通过 |
| 三 UI crate 与公共 UI，含 Deck validation/Sheet automation | 78 项通过，含内嵌图标尺寸、透明度、品牌差异及缓存测试 |
| `cargo xtask check joydeck` | 989 项通过；fmt、all-targets 严格 Clippy 通过 |
| `cargo xtask check joysheet` | 1,440 项通过、1 项既有忽略；fmt、all-targets 严格 Clippy 通过 |
| `cargo xtask check joyword` | fmt、严格 Clippy 通过；2,397 项通过、1 失败、2 忽略后停止。失败为 `odt::tests::pictures_over_the_budget_are_not_inflated` |
| Word 失败的基线复现 | 在独立、未修改的上述 main worktree 重跑该 ODT 测试，同样失败，均为 `left: 1000 / right: 0`；本轮未修改该模块或断言。见 [baseline-odt.log](baseline-odt.log) |
| 工程/分发 Python | 产品路由 18 项、图标/打包/归档/发布/源码指纹 36 项、Sheet 打包 7 项通过 |
| 工程门禁 | `check_gates.py`、291 项 Deck 来源指纹、`check_docs.py` 和 `git diff --check` 通过 |

Python 覆盖实际 GIO 启动含特殊字符路径的程序，独立于自身的字符串比较；macOS 开发 bundle 的产品/图标/可执行权限和运行参数路由有文件级自动验证。Cargo.lock 只为 `joyoffice-ui` 增加已有 `image` 依赖，第三方版本、来源及校验和未变。

## 本机运行验证与边界

UOS Desktop 25 Professional、x86_64、X11/DDE。三个新构建均从仓库外 `/tmp` 启动，逐窗口读取实际 `_NET_WM_ICON`：尺寸均为 256 × 256，每个 ARGB 像素与独立解码的所属产品 ICO 一致，`WM_CLASS` 与桌面项一致。见 [native-icons.json](native-icons.json)。另将三个实际新二进制放入中文/空格便携目录，在隔离 XDG 数据目录注册桌面项并通过 GIO 启动，均核对进程路径、256 × 256 窗口图标和已安装 SVG，通过记录见 [desktop-launchers.json](desktop-launchers.json)。验证完成后只终止本轮创建的进程，未修改用户文档或安装到真实用户配置目录。

初轮在锁屏时只验证窗口属性。用户随后解锁，已完成本机目视补验，见 [visual-icons.json](visual-icons.json)：

- [任务栏](gui/taskbar.png)：三产品分别显示蓝色 JoyWord、绿色 JoySheet 和橙色 JoyDeck 图标，无通用程序图标。
- [Alt-Tab](gui/alt-tab.png)：三产品预览各自显示正确图标；该裁剪图顺序为 Deck、Sheet、Word。
- 应用内品牌位置：[Word](gui/joyword-header.png)、[Deck](gui/joydeck-header.png)、[Sheet](gui/joysheet-header.png) 均实屏显示所属 SVG。
- 从实际 Deck 窗口按 F5 打开放映，再从「幻灯片放映 → 演示者视图」打开观众/演示者窗口，均有 256 × 256 图标；后两窗口的完整 ARGB 属性与已独立核对的 Deck 图标一致。[放映记录](slideshow-icons.json) · [演示者记录](presenter-icons.json)

目视补验使用宿主 systemd 用户临时服务运行当前构建，再通过 X11 操作系统合成激活、鼠标点击和 Alt-Tab 检查。Codex 所处玲珑环境的 PID 命名空间与宿主应用分组不同，不能把其中直接派生进程的任务栏分组当作宿主桌面结果。本轮未向真实用户应用目录注册启动器；验证后停止所有测试服务，用户原有窗口及文档保持。

Wayland、Windows、macOS、ARM64、正式安装/升级/签名及信创硬件仍未补验。Windows/macOS 的既有链路检查与新开发包自动测试不计为真机通过；完整阶段状态保持独立。

本机完整日志暂存于 `target/icon-validation-20261118/` 和 `/tmp/joyoffice-icons-*.log`；仓库保留精简证据及失败基线，未保存锁屏截图。新图标需使用本轮重新构建的程序或后续更新包，历史已发布包不改写。
