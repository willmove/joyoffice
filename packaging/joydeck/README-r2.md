# JoyDeck 0.3.0 · R2 Windows 开发包

解压运行 `joydeck.exe`，或传入本地 `.pptx`、`.joydeck` v2、旧 `.deckcraft` 文稿路径。支持基础对象和页面编辑、保存重开、撤销重做及恢复。依赖与 JoyOffice 对齐：Rust 1.98.1，GPUI Kit 0.8.1 / GPUI pre 0.3.10。

点击画布选择顶层对象，Shift 点击多选，拖动移动；一次拖动只产生一个撤销步骤，Escape 取消尚未提交的手势。工具栏可插入矩形/文字、复制/删除、调整大小/旋转/层次，新建/复制/删除页面。“下一对象”也可选择对象。文字在独立输入框修改后点击“应用文字”；整段替换采用首个文字样式，原位富文本和 IME 编辑留在 R3。

保存路径框默认是当前 PPTX / JoyDeck 路径；输入一个新 `.pptx` 或 `.joydeck` 文件名即可另存。Ctrl+S 保存；画布聚焦时 Ctrl+Z 撤销、Ctrl+Y / Ctrl+Shift+Z 重做。输入框内的撤销只作用于输入草稿。未保存修改会阻止打开/新建替换；关闭时可保存或取消。保存期间允许继续编辑，新修改仍显示未保存。

PPTX 保存保留原包，更新受支持的页面和对象节点；未触及的成员字节保留。复制范围是普通形状、图片和表格；分组/3D/不透明对象、带节或自定义放映的页面结构、删除带动画/连接引用的对象及签名文稿等不支持的写回会拒绝，原文件不变，可另存为 JoyDeck 项目。不保证 PowerPoint 全保真。未保存恢复/撤销不等于跨次启动保留 undo 历史。

`.joydeck` 是版本 2 项目格式，具有专用 MIME、必需能力声明、模型载荷、原始包和源对象映射；旧 `.deckcraft` 读取器会拒绝它。未知必需能力/未来主版本被拒绝；未知项目成员、清单额外字段和原始包被保留。旧 `.deckcraft` 仅导入，编辑后另存为 `.joydeck` 或 `.pptx`。

修改后约 250 ms 安排后台恢复快照；大文件还需等待编码/落盘。“恢复”列出快照，活动会话受文件租约保护，损坏和未来版本记录隔离，恢复后须选择新路径保存。Windows 恢复目录为 `%LOCALAPPDATA%\joyoffice\joydeck\recovery-v2`。恢复内容包含文稿与原始包，应按原文件的访问范围保管。

```powershell
.\joydeck.exe ".\samples\independent.pptx"
.\joydeck-cli.exe inspect ".\samples\independent.pptx"
.\joydeck-cli.exe commands
.\joydeck-cli.exe edit ".\samples\independent.pptx" --script ".\samples\r2-edit.json" --output ".\edited.pptx"
.\joydeck-cli.exe render ".\edited.pptx" --output ".\rendered" --all
.\joydeck-cli.exe recovery-list "C:\path\to\recovery-v2"
.\joydeck-cli.exe recover "C:\path\to\recovery-v2" SESSION --output ".\recovered.joydeck"
```

CLI 页码从 1 开始，编辑脚本中的对象 ID 来自 inspect。批量 edit 失败不发布输出，新输出不覆盖既有文件。原位保存使用读取时的大小/SHA-256 指纹，检测外部修改后拒绝覆盖。临时文件与目标在同目录，sync 后原子提交；不承诺跨进程文件竞争的操作系统级 compare-and-swap 或断电事务。

预算延续 R1：输入/输出 64 MiB、ZIP 项 4096、解压总量 128 MiB、最多 1000 页/2 万对象，图片/像素预算不变。项目会同时保留模型媒体和原始包，大源文件可能更早触及项目/恢复大小预算。撤销最多 128 步和约 64 MiB 增量估算；单个最新步骤可超过估算预算。

本包为未签名 Windows x86_64/MSVC 开发构建；macOS/Linux 未进行本轮原生验收。source.json、sbom.json、SHA256SUMS 和 licenses 可核对来源，正式再分发、安装更新及完整许可文本审计仍属后续阶段。
