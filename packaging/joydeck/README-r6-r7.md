# JoyDeck 0.7.0 · R6/R7 Windows 开发包

解压整个目录，运行 `joydeck.exe`。建议打开 `samples/r6-r7/delivery-media.pptx`，在“输出”导出 PDF、备注/讲义、PNG/JPEG、GIF、静态 MP4、提纲或标题 VTT；“放映”可打开预览、全屏和演示者视图。系统打印先打开 Windows 对话框，可取消。

保留三个 `joydeck-*-windows.exe` 和主程序在同一目录。打印使用 Windows WinForms；媒体使用 WPF MediaPlayer；静态 MP4 使用 Windows MediaComposition。依赖本机 .NET Framework 4.x、Windows 媒体组件与可用编解码器；不分发系统字体或独立 codec。Windows N 等缺失媒体组件的系统未资格化。包未签名。

旧 `samples/r6-r7/legacy-1.0.joydeck`、`legacy-1.11.joydeck` 可导入并另存新的 `.joydeck` v2，原件与 ID/转换报告保存在迁移附件中。旧媒体/审阅/放映链接状态、未知或未来必需特性会明确拒绝，避免静默迁移。旧主题/字段经公开 OOXML 解析为结果，未重建完整继承关系；请保留原件。

```powershell
.\joydeck-cli.exe inspect .\samples\r6-r7\delivery-media.pptx
.\joydeck-cli.exe migrate .\samples\r6-r7\legacy-1.0.joydeck --output C:\Work\converted.joydeck
.\joydeck-cli.exe exchange .\samples\r6-r7\delivery-media.pptx --mode preserve --output C:\Work\copy.pptx
.\joydeck-cli.exe exchange .\samples\r6-r7\delivery-media.pptx --mode regenerate --output C:\Work\new.potx
.\joydeck-cli.exe export .\samples\r6-r7\delivery-media.pptx --format pdf --layout notes --output C:\Work\notes.pdf
.\joydeck-cli.exe export .\samples\r6-r7\delivery-media.pptx --format mp4 --seconds 1 --fps 10 --output C:\Work\slides.mp4
.\joydeck-cli.exe print .\samples\r6-r7\delivery-media.pptx
.\joydeck-cli.exe media-inspect .\samples\r6-r7\original-h264-aac.mp4
.\joydeck-cli.exe control --root C:\Work
.\joydeck-cli.exe mcp --root C:\Work
```

输出必须为新路径且父目录已存在。`export` 支持 `--pages 1,3`、`--dpi 36..300`、`--scale .05..8`、`--layout slides|notes|handouts-3|handouts-6`、`--seconds .1..30`、`--fps 1..30`，PNG 可加 `--transparent`。输出默认包含隐藏页；放映默认跳过隐藏页。PDF 采用 CPU 图形加准确定位的可检索字形子集/链接/书签，非全矢量、非 PDF/A 或标记 PDF。备注超出页面会报错。打印目前仅幻灯片；备注/讲义请导出 PDF。已验证打印对话框取消，未向实体打印机提交作业。

GIF 采样已有动画；MP4 仅静态页，不含对象动画、内嵌媒体、音频或备注。VTT 仅标题。媒体默认静音，支持每页一个顶层本地内嵌片段；旋转、翻转、跨页、全屏片段、设备变化与完整音画同步仍待完善。系统可能不报告总时长，界面不会编造时长。

放映：Space/右箭头下一步，左箭头上一步，P 暂停，B/W 黑白，L 激光，I 墨迹，E 清除，M 媒体，V 静音，J/K 前后 seek 5 秒，F11 全屏，Esc 退出。临时墨迹不写回正文。演示者窗口独占备注，关闭任一放映窗口会关闭其配对窗口。当前放映沿共享 CPU 状态渲染；真实多屏、反向转场、全部动画/排练持久化未资格化。

控制为独立无界面会话，未连接正在运行的编辑器；没有 TCP 监听。MCP 明确实现 `2025-11-25` stdio 生命周期与 `joydeck.control` 工具，不宣称新修订或 MCP Tasks 扩展。命令协议 `joydeck-control-1`，1 MiB JSON 行；所有路径须绝对且位于启动 root 内，新文件输出；文稿命令带 documentId/expectedRevision。选择也递增控制修订。相同 requestId/参数重放返回同一结果，128 次变更后需新会话。导出 jobs 可轮询/取消，断连会取消并等待自有任务退出；提交已胜出的并发取消会报告完成。应显式保存无界面编辑，未实现自动恢复检查点。

这是 R6/R7 可运行纵向切片；完整任务、Windows 当前验证与剩余平台工作记录于源码仓库 `docs/joydeck/evidence/R6-R7`。R2/R3/R4/R5 基础编辑和场景能力继续保留。`SHA256SUMS`、`source.json`、`native-helpers.json`、`sbom.json` 提供来源与产物指纹；实际 SBOM 中缺许可全文的既有依赖仍需正式发行审计。
