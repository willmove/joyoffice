# JoyOffice macOS ARM64 本机验证

2026-11-04。Mac mini、Apple M4、24 GB，macOS 27.2（26B123），Rust 1.98.1，Xcode 27.2（27B52）。源码为 `6adc61472e8b5dd65410c76c4d0e3c85b1b4fe85`。Xcode 原来缺少可执行的 Metal 组件，安装 Metal Toolchain 27B47 后用默认特性构建三个 Release 开发包。样例均来自仓库，读写在临时副本和隔离配置内进行。

## 自动验证

| 产品 | 无界面测试通过／失败／忽略 | 产品检查 |
| --- | ---: | --- |
| JoyWord | 3,483／0／2 | fmt、生产 Clippy、3,492／0／2 |
| JoyDeck | 891／0／0 | 来源、依赖、文档、fmt、严格 Clippy、894／0／0 |
| JoySheet | 1,383／0／1 | fmt、严格 Clippy、1,400／0／1 |

各产品测试集合可能包含公共模块，计数分别使用。根工程边界门禁通过。Python 工具测试执行 152 项，150 项通过、2 项跳过。JoyDeck 单独构建的 `dev-runtime-shaders` 开发版也创建了原生窗口，并通过相同的桌面 stdio 保存和重开检查。开发版与普通 Release 的二进制指纹分别登记。

## 开发包与包内 CLI

| 产品 | 实际开发包 | 包内验证 |
| --- | --- | --- |
| JoyWord 0.1.1 | `dist/joyword/macos-validation-20261104/joyword-0.1.1-macos-aarch64.zip` | `verify-r8.py` 通过混合媒体 DOCX 编辑、另存和重开。独立检查表格、媒体与原件散列。PNG、普通 PDF 和拒绝覆盖通过。用 pypdf 严格解析 PDF 并用 Poppler 读回 Unicode 标记。64 个确定性损坏变体中 21 个解析、43 个干净拒绝。 |
| JoyDeck 0.8.0 | `dist/joydeck/joydeck-0.8.0-r9-macos-aarch64-release-20261104.zip` | R1–R5 原脚本通过。R6–R7 原脚本在 MP4 导出收到 Windows 适配器限制。将两处 Windows 媒体断言换为明确拒绝后，32 次 CLI 调用、60 次 MCP 响应通过，覆盖旧文稿迁移、OOXML 交换、PDF／PNG／JPEG／GIF 等输出、修订／重放和异步取消。MP4 拒绝后无部分输出。 |
| JoySheet 0.1.0 | `dist/joysheet/macos-validation-20261104.dmg` 与同名 ZIP、源码 ZIP | DMG 校验、只读挂载、卸载及 App 的临时签名校验通过。便携 ZIP 校验和与两个 ZIP 的完整性检查通过。包内 CLI 对 XLSX 副本写入 C2 的 `=SUM(800,8)`，重开读回 808，独立 OOXML 检查公式与缓存值，源件散列一致。 |

三个解包或挂载后的 GUI、CLI 均为原生 ARM64 Mach-O。归档与二进制 SHA-256、测试计数和原始日志指纹见 [验证清单](verification.json)。JoyDeck 打包清单有 49 条依赖记录未自动收录许可正文，继续由 JD-807/808 跟踪。发行者签名和公证待验。

## 原生窗口复验

锁屏期间保留了三套移位 App 的进程与窗口证据。Word 的桌面打开作业对有效 DOCX 初始化了 Reader 恢复目录，截断输入未初始化。Deck 的桌面 stdio 对同一编辑会话完成复制、过期修订拒绝、撤销／重做、保存和重启重开。Sheet 首次截图因等待当前帧超时。

解锁后使用同一批 Release 二进制。鼠标和键盘由 macOS 事件注入，操作测试副本。用于文字和公式的输入源临时切换为 ABC，测试后恢复。

| 产品 | 实屏与交互结果 | 截图 |
| --- | --- | --- |
| JoyWord | 首屏显示混排正文、表格和三色图片。鼠标定位、键盘输入、撤销和重做通过。普通保存显示替换确认，取消时副本散列不变，确认后正常关窗并重启。DOCX XML 和包内 CLI 读回编辑标记，表格与媒体保留。 | [替换确认](gui/joyword-replace-confirmation.png) · [重开](gui/joyword-reopen.png) |
| JoyDeck | 鼠标选中并复制第 2 张幻灯片，键盘撤销、重做和保存通过，正常关窗并重启。PPTX 关系与页面 XML 确认第 2、3 页为复制对，包内 CLI 读回 4 页。 | [重开](gui/joydeck-reopen.png) |
| JoySheet | DMG 中的 App 接收 macOS 文件事件并显示中文空格路径的 XLSX。用鼠标定位 C2，键盘写入公式、保存和正常关窗，重启后界面显示 808，OOXML 和 CLI 独立读回一致。automation 开发版的完整 9 组合成输入回归及三张画面均通过。 | [重开](gui/joysheet-reopen.png) · [自动化格式表单](gui/joysheet-automation-format.png) |

从 LaunchServices 启动后，给 Word 与 Deck 发送系统文件打开事件，二者仍停留在原文档，包也未声明文档类型。Word 原生打开面板能显示，选择路径与另存的完整往返未确认。Deck 的 JD-801 继续跟踪系统文件事件和菜单覆盖。系统 IME 的完整提交／取消、读屏、显示器与 DPI、物理打印及 GPU 呈现性能仍待验。

## 复跑入口

```sh
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_OPT_LEVEL=1 RUST_TEST_THREADS=2
cargo xtask test joyword --headless
cargo xtask test joydeck --headless
cargo xtask test joysheet --headless
cargo xtask check joyword
cargo xtask check joydeck
cargo xtask check joysheet
cargo xtask package joyword --release --out dist/joyword/macos-validation-rerun
cargo xtask package joydeck --release --name joydeck-macos-validation-rerun
cargo xtask package joysheet --release --output dist/joysheet/macos-validation-rerun
cargo xtask build joydeck --features joydeck/dev-runtime-shaders
cargo xtask smoke joysheet --stage M8
```

详细运行日志和临时探针留在本机 `target/macos-validation-20261104/`。锁屏冒烟的原始失败也保留。
