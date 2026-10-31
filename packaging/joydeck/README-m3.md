# JoyDeck M3 文字编辑开发预览

未签名的本机构建（debug），仅供开发验证，尚未达到完整 M3 退出门槛。macOS M0–M3 原生 adapter 回归和系统拼音输入法集成测试已通过；系统测试使用 OS 键盘事件，未冒充物理键盘或其他平台测试。

运行 `./joydeck`（Windows：`joydeck.exe`；macOS 可双击 Launch.command）。插入文本框后双击，或选择后点“编辑文字”。支持输入中英/emoji、换行、选择、字素删除、纯文字剪贴板、Ctrl/Cmd+B/I 和局部字体/颜色。文字格式按钮保持选区并返回画布。Esc 取消预编辑或结束编辑。保存、切页、关闭、窗口失活时取消未提交预编辑，保留已提交内容。

原生文件升级为 schema 1.2 / minReader 3 / rich-text-v1。可读旧 1.0/1.1；旧程序会拒绝新文件。上一阶段源码快照保留用于回退。PPTX 仍只读。

限制：编辑态 GPUI 排版与 CPU 静态预览尚未统一；未实现自动换行、段落列表/缩进/段距、混合字号编辑、旋转文本输入、完整 bidi affinity。逐字缺字诊断、Windows/Linux IME、跨屏/DPI、读屏验收尚待完成。不能据输入桥自动 smoke 声称物理输入法通过。

诊断 `./joydeck --smoke`；只读 `./joydeck --viewer`；原生验证 `./joydeck --text-smoke`；其他阶段 `--viewer-smoke`、`--editor-smoke`。源码另提供 `python3 scripts/smoke-macos-ime.py`，在测试窗口中切换系统拼音、验证提交/取消/一次撤销及保存重开，并恢复原输入源。验证只读样例位于 fixtures，可用 JOYDECK_FIXTURES 指定另一路径。自动脚本由源码 scripts/smoke-native.py 提供，默认超时 40 秒，要求可交互桌面。

支持状态、测试证据与遗留任务见源码 docs/23-m3-implementation-report.md。系统字体来自 OS，不打包分发。SHA256.json 记录文件摘要。
