# macOS 系统文件打开与 JoyWord R8 回归

2026-11-05，Apple M4，macOS 27.2，Rust 1.98.1。基于主线 `6adc6147`。JoyWord 注册 DOCX，JoyDeck 注册 PPTX，均声明为备用编辑器。接收 LaunchServices 文件 URL 后调用现有的文档打开流程，当前窗口仍按原有方式处理未保存修改。两个应用在一次批量事件中选择第一个受支持的文件。[Apple 文件类型声明](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html)

## 系统文件打开

两个 Release 包解包后移到仓库外。测试通过 `open -n -a APP FILE` 冷启动和 `open -a APP FILE` 运行中打开，分别核对实际进程与文档标题。冷启动文件名同时含中文、空格、`#`、`%` 和大写扩展名。重复请求保留一个窗口，截断文件保留当前文稿，随后可继续打开有效文件。实际输入或复制幻灯片后，另一个文件的打开请求保留当前未保存文稿，两份源文件散列均不变。JoyDeck 新增 macOS「文件」「编辑」菜单，实际点击「撤销」「重做」验证到当前编辑会话。另用独立构建的 `dev-runtime-shaders` App 重跑相同场景。[Word 运行中打开](gui/joyword-warm.png) · [Deck 运行中打开](gui/joydeck-warm.png) · [Deck 未保存保护](gui/joydeck-dirty-guard.png)

## JoyWord R8

- 固定 Source Sans 3 Regular、face 0、144 DPI。PNG 像素和独立 PDF 文本／几何均匹配仓库金样。将同一金样命令加入现有 macOS CI 分支。
- LLVM 覆盖引导运行 81 次探针，含 48 次变异，覆盖 DOCX 和 JWREC001/002/003。无崩溃、超时或协议失败。本机拒绝地址空间上限探针，保留文件大小预算和每次探针超时。
- 旧桌面恢复目录从 `JOYWORD_RECOVERY_DIR` 或旧配置根的 `recovery` 发现。隔离配置中实际点击恢复 JWREC001 副本，读回正文标记，旧日志与旧偏好原样保留，Release 和修复栈后的 Debug 版均通过。另用生成的 JWREC003 快照覆盖表格、图片、修订、页眉 PAGE 字段和额外 XML 部件，独立核对首次另存与快照字节相同。[恢复副本](gui/joyword-legacy-recovery.png)
- 已保存的作者名用于桌面审阅，最近文档接入文件页。成功打开和保存后更新活动配置。旧自动保存间隔的行为接入继续开放。
- 调试版发现 GPUI 打开工作线程栈溢出。桌面读入改用 8 MiB 独立工作线程，后台取消与代次守卫继续生效。

## 验证范围

工作区联合特性回归在系统文件打开改动上通过 5,782 项，3 项忽略。后续菜单和恢复改动分别执行产品检查，最后的打开线程改动另跑 Word 外壳与桌面适配器测试。详细计数、归档、二进制散列及实屏证据见 [验证清单](verification.json)。

Word 旧 CLI 兼容、旧自动保存、Word/WPS 双向往返和千页性能任务仍开放。Deck 的完整编辑对齐、旧文件和命令无回退、macOS 媒体适配、许可及正式发行验收继续开放。开发包保留原应用版本和系统默认打开方式。
