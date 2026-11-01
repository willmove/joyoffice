# UOS 25 三产品测试与缺陷修复

2026-11-01。分支 `codex/uos-product-validation`，起点 `6500e3a50f6abf4b83b09581def1344a7995015c`。本轮完成三产品自动回归和下述原生 GUI 场景，修复五项代码／测试缺陷及一项文档错误。**不代表所有产品任务或阶段已 done。** Word 原任务仍为 **17 done／38 in_progress／11 planned**；完整任务逐项见 [task-ledger.json](task-ledger.json)，不能用测试总数代替任务验收。

## 实际环境与证据

宿主为 **UOS Desktop 25 Professional，x86_64**；AMD Ryzen AI 9 HX 370／Radeon 890M，实际窗口日志选中 RADV GFX1150 Vulkan；X11，原生缩放 125%。不是国产 CPU／GPU 验收。Codex 位于玲珑运行环境，其 `/etc/os-release` 显示 Deepin 25；宿主标识来自 `/run/host/rootfs/etc/os-release`，两份原文均记录在 [verification.json](verification.json)。Rust 1.98.1，Cargo.lock 和依赖版本未改。

默认运行环境缺少 Git／cc／截图程序路径。本轮使用宿主工具及临时 GCC `--sysroot=/run/host/rootfs` 包装恢复编译和 X11 截图；包装位于本机 `target/uos-validation/bin/`，不作为仓库构建依赖。默认 umask 为 `000`，真实暴露下述权限问题。`git fetch origin main` 因 SSH 主机密钥校验失败未完成；开始时本地 main 与 origin/main 均为上述提交，未宣称已核对远端最新提交。原有未跟踪 `Joysheet.pdf` 保留。

验证使用测试副本／隔离配置。X11 窗口截图和 GPUI／OS 合成事件与物理键盘、系统输入法、读屏、GPU 呈现延迟分别记录。测试配置、token、系统字体、二进制和恢复日志不纳入本证据目录。构建与各 GUI 场景的二进制散列及范围见 verification.json 和对应报告；最终源文件／证据散列见 [SHA256SUMS.json](SHA256SUMS.json)。

## 已修复与复测

| 编号 | 触发、修复与验收 | 状态 |
| --- | --- | --- |
| UOS-001 | Word 原件保存的权限漂移用例把临时文件设为 0666，但当前源文件也可能是 0666，未注入变化。改为显式 0600／0640／0666 三种源权限并翻转一个权限位，保留拒绝、源字节、源权限和临时清理断言。不是放宽生产保存契约 | done |
| UOS-002 | Sheet 新建工作簿继承 OpenOptions 默认权限，umask 000 下得到 0666。Unix 创建临时文件时明确使用新文件 0600，已有文件沿用原模式；测试加强为组／其他用户无权限且无遗留临时文件 | done |
| UOS-003 | Sheet Linux automation 构建宣称支持 GPU 截图，实际返回 `render_to_image not implemented for this platform`。能力声明按固定 GPUI 实现限制为 macOS／Windows，Linux 立即明确拒绝且不落文件；原生回归改用注明来源的 X11 窗口像素，未假装补齐 GPUI GPU 读回 | done（能力声明／验证入口） |
| UOS-004 | Word 长路径的最小内容宽度挤出确认／取消按钮。路径行与输入区域限制最小宽度和溢出，按钮不收缩，对话框限制最大宽度。[修复前](word-long-path-before.png)、[修复后](word-long-path-after.png) | done（本机所测布局） |
| UOS-005 | Word 成功提交路径后删除输入框，却未恢复正文焦点。打开／另存／导出提交时恢复文档焦点；同一原生回归在修复前二进制上因后续键盘导出超时而失败，在最终二进制通过，见 [失败报告](word-before-focus-fix-report.json)、[通过报告](word-native-report.json) | done |
| DOC-001 | Sheet 控制文档的 `hello` 认证示例与实际协议不符。改为 `auth` 方法及 `params.token`；新桌面回归实际执行该握手 | done |

[initial-failures.json](initial-failures.json) 保留首轮 Word／Sheet 权限失败、Linux GPU 截图失败，以及新增测试脚本首轮未正确处理多个窗口的失败。脚本现按测试进程的实际前台窗口截图；未以放宽图像或引擎断言通过。panic baseline 只增加 Word 测试模块中的三个新增断言／准备操作，没有新增生产 panic 许可。

## 自动验证

| 检查 | 实际结果 | 证据 |
| --- | --- | --- |
| Word 产品 check | 1298 通过，0 失败；fmt、严格 Clippy 通过 | verification.json 中的原始日志散列，本机 target/uos-validation/joyword-final-check.log |
| Deck 产品 check，validation | 194 通过，1 忽略；fmt、严格 Clippy 通过 | verification.json 中的原始日志散列 |
| Sheet 产品 check，automation | 1400 通过，1 忽略；fmt、严格 Clippy 通过 | verification.json 中的原始日志散列 |
| 联合 workspace，automation + validation | **2880 通过，0 失败，2 忽略** | [完整日志](workspace-tests.log)、[严格 Clippy](workspace-clippy.log)、[fmt](workspace-fmt.log) |
| Python | 根脚本 146、产品路由 13、Deck 工具 11、Sheet 打包 7 项通过 | [根脚本](python-root.log)、[路由](python-products.log)、[Deck](python-deck.log)、[Sheet](python-sheet.log) |
| 文档／架构／IO／panic 门禁 | 通过 | [文档](docs.log)、[门禁](gates.log) |
| C05 页面场景／独立 Poppler 读回 | 2 页，0 几何／文字错误 | [读回](pdf-readback.log) |
| Word CLI PDF／PNG | PDF 1 页；PNG 1191×1684，144 dpi | [PDF 收据](word-cli-pdf.json)、[PNG 收据](word-cli-png.json) |
| Sheet M8 引擎冒烟 | Unicode、公式、重算、历史、格式和 XLSX 往返通过；工具明确报告 milestoneComplete=false | [日志](sheet-m8-engine.log) |

各产品集合包含共享库，不能把三个产品测试数直接相加。联合测试的两项忽略为 Deck 原生启动测试和 Sheet timing-sensitive 范围公式性能测试。Deck 原生启动随后以 `--ignored --exact` 单独运行通过，覆盖 1280／980 两种逻辑宽度、合计 96 个真实 draw 场景，见 [原生启动测试](deck-native-startup-test.log)；另保留独立 48 场景日志。未将 debug CPU 数据冒充 Release 性能预算。三个产品的桌面／CLI debug 构建均通过；本轮未构建发布包、签名或提交远端 CI。

## 原生 GUI 验证

| 产品 | 本轮实际操作与观察 | 证据 |
| --- | --- | --- |
| Word | 最小中文 DOCX 显示；Sheet 系统剪贴板粘贴中文与 emoji；撤销／重做；保存风险提示取消／确认；磁盘 CLI 读回 Unicode；受保护副本拒绝键入且原件字节不变 | [保存提示](word-save-confirmation.png)、[读回](word-saved-text.txt)；保护判断由 CLI read_only=true 复核，未宣称其保存菜单 disabled |
| Word 最终自动桌面回归 | 真实键盘编辑／undo／redo、长路径另存副本；不再点击页面即以快捷键导出 PDF，Poppler 读回与 DOCX 一致；导出后快捷键再次打开对话框；取消后原件字节不变 | [3 组结果](word-native-report.json)、[快捷键导出截图](word-keyboard-export.png)、[PDF 文字](word-native-pdf-text.txt) |
| Deck | M0–M4 窗口、UTF-16 组合态提交／取消、编辑／历史、文本缓存与场景缓存冒烟；48 个工具栏／属性面板真实绘制场景 | [M0](deck-m0.log)、[M1](deck-m1.log)、[M2](deck-m2.log)、[M3](deck-m3.log)、[M4](deck-m4.log)、[48 场景](deck-startup.log) |
| Deck 实际界面操作 | 打开 20 页中文原生文稿；最终二进制点击新增／保存→21 页、撤销／保存→20 页、重做／保存→21 页；每步独立 ZIP 断言，最终 CLI 读回 21 页 | [报告](deck-native-report.json)、[编辑器](deck-editor.png)、[撤销](deck-undo.png)、[保存](deck-saved.png) |
| Sheet | 10 组基础原生回归：Linux 截图能力拒绝契约；样例／图表与窗口截图；中文 emoji、公式依赖重算、键盘历史、合成组合态保护、系统剪贴板相对公式、函数补全、冻结缩放／格式表单、取消关闭、中文空格路径 XLSX 保存关闭重开 | [报告](sheet-native-report.json)、[样例](sheet-sales.png)、[重开](sheet-roundtrip.png) |

复跑入口（普通 UOS 开发终端；需要解锁桌面、xdotool、ImageMagick import、Poppler）：

```sh
cargo xtask check joyword
cargo xtask check joydeck --validation
cargo xtask check joysheet --validation
cargo xtask build joyword
cargo xtask build joysheet --validation
python3 scripts/smoke_word_linux.py --binary target/debug/joyword --output target/word-native-new-run
python3 scripts/joysheet/smoke-native.py --binary target/debug/joysheet --output target/sheet-native-new-run
```

输出目录须不存在；失败报告也保留。两个入口终止的只是其自己启动的测试进程。Deck 既有入口为 `python3 scripts/joydeck/smoke-native.py M0 M1 M2 M3 M4 --binary target/debug/joydeck`；启动绘制入口为 validation 二进制的 `--startup-smoke`。Word 新回归测试验证了焦点缺陷的修复前失败／修复后成功，不只是截图。

## 未关闭事项及任务口径

- **UOS-IME-001 仍未修复**：Word 在当前系统搜狗／Fcitx5（`com.sogou.ime.ng.fcitx5.uos`）中输入 `zhongwen`，观察到拼音直通，未见中文候选；见 [失败截图](word-ime-failed.png)。XIM 连接日志存在，但不足以证明组合态正常，根因和生产修复尚未完成。Sheet／Deck 本轮只验证合成组合态，不能据此认证系统 IME。
- **Word 字体覆盖仍有缺口**：选定 Noto CJK face 2 不含所测 emoji，GUI 显示缺字提示；DOCX 保留 emoji 码点，但该字形的屏幕／PDF 覆盖未通过。最终 PDF 对照用不含缺字的规范样例，不隐藏此前的缺字观察。
- Wayland、读屏、物理键盘／触控板、多显示器／DPI 切换、实体打印、安装／升级／签名、其他系统和 ARM64／LoongArch64、国产 CPU／GPU 均未在本轮认证。UOS x86_64 的 RADV 成功不能代替这些环境。
- 完整 Office 格式互操作、故障／性能认证以及任务清单中尚未实现的功能继续开放。本轮没有对每个 planned 任务补实现，也没有把自动回归成功改写为其完整验收。

Word 的 T003／T005／T103／T104／T109／T110／T203／T204／T205／T700／T701 得到本轮对应的增量证据，父任务状态不变；其他任务仅记录产品自动回归。Sheet 的 M3／M4 编辑生命周期及 M8 Linux 基础子场景获得新证据，不等于 Windows M4–M8 全矩阵或 M8 完成。Deck M0–M4 冒烟及本机桌面操作不关闭完整三平台里程碑。后续应优先处理真实系统输入法，再按 task-ledger.json 的未满足条件逐项补验／开发。
