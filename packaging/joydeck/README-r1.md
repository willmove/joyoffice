# JoyDeck 0.2.0 · R1 Windows 开发包

解压后运行 `joydeck.exe`，默认打开内置真实 `.deckcraft` 三页样例。使用“打开文稿”或路径框读取本地 `.pptx` / `.deckcraft`，也可将文件路径作为启动参数。支持缩略图、翻页、适应窗口、缩放、明暗配色、只读备注与兼容提示。`samples` 包含原创 native/PPTX 和独立工具生成的 PPTX。

```powershell
.\joydeck.exe ".\samples\independent.pptx"
.\joydeck-cli.exe inspect ".\samples\independent.pptx"
.\joydeck-cli.exe render ".\samples\independent.pptx" --output ".\rendered" --all --scale 1
.\joydeck-cli.exe render ".\samples\r1-preview.deckcraft" --output ".\page2" --page 2 --scale 2
.\joydeck-cli.exe diagnostics
```

页码从 1 开始；`--all` 与 `--page` 二选一，默认渲染第一页。输出目录必须尚不存在；生成 PNG 与 inspect.json。文件路径可包含中文与空格。`inspect` 包含演讲备注，请按原文件的访问范围保管输出。

本包提供只读查看，不编辑或保存文稿。原始字节被保留；不会下载外链媒体。字体来自本机及许可明确的 fallback，缺失字体会提示替代；复杂效果、OLE/SmartArt、3D、动画等可能近似或显示 fallback，不保证 PowerPoint 像素一致。独立的 R0 UI 样例可用 `--sample-workspace` 打开，临时输入不写入文稿。

当前预算：输入 64 MiB、4096 ZIP 成员、单成员 8 MiB、解压总量 128 MiB、XML 深度 128/每成员 100 万事件、最多 1000 页/2 万对象。单张渲染最多 8192 边长和 1600 万像素。取消在读取、包校验、解析前后与渲染任务边界检查；已开始的单次解析或光栅不能立即中断。

本包是未签名 Windows x86_64/MSVC 开发构建；macOS/Linux 尚未进行本轮原生验证。`source.json`、`sbom.json`、`SHA256SUMS` 和 `licenses` 可核对来源与内容。正式再分发许可审计及安装/更新流程仍在后续阶段。
