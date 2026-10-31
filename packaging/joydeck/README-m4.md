# JoyDeck M4 Scene 画布开发包

本机构建，默认 release。未签名/公证。已有 M3 文件读取和编辑功能保留，完整 M3 及其他平台/设备门槛仍有未完成项。

运行 `./joydeck`（Windows：`joydeck.exe`；macOS 可双击 Launch.command）。默认画布使用 GPUI 路径与逐 glyph mask；底部可切到 CPU 对照，或重建绘制缓存。CPU 基线可用 `./joydeck --cpu-canvas`。静态正文保持与 CPU 同一 SVG glyph 轮廓；活动文字输入仍使用原输入桥，其完整布局收尾仍属 M3。

透明度、彩色 glyph 或其他不支持的组合按隔离对象局部栅格化，只在原 z-order 合成一次。路径/纹理缓存和后台最新任务合并避免平移时重画整页。界面中的上传/缓存计数来自适配器，不等同于驱动 VRAM 或 GPU profiler 的统计。重建缓存会移除旧 atlas image；它不能冒充真实 device-lost 恢复。

图片/复杂效果在 Scene 基准中有 backend 覆盖，但文档图片导入、持久化和组等仍按后续阶段实施；不据基准宣布 M5 完成。

验证入口：

- `./joydeck --scene-smoke`：M4 原生编辑器平移缓存、CPU 切换、undo/redo、缓存重建验证。
- `python3 scripts/smoke-native.py M0 M1 M2 M3 M4 --binary ./joydeck`：需可交互桌面；Windows 使用 joydeck.exe。
- macOS `python3 scripts/smoke-macos-ime.py --binary ./joydeck`：真实系统拼音，由 OS 键盘事件驱动，恢复原输入源。
- `./joydeck --m4-bench`：运行固定 BENCH-EDIT/EFFECT CPU/GPUI 原生绘制对照，默认输出 logs/m4-native-benchmark。
- macOS `python3 scripts/benchmark-m4.py --binary ./joydeck`：同时截取真实窗口做像素比较，Python 需要 Pillow/numpy。可通过 JOYDECK_M4_OUTPUT 指定应用单独运行的基准输出目录。

原生基准记录的是程序变换提交到 GPUI post-render callback 的反馈，不能称为物理输入到屏幕扫描输出时间。当前完整 M4 不因本机绘制门槛通过而自动验收；具体支持范围、数据、截图、未测设备及旧阶段缺口见源码 docs/25-m4-scene-validation.md、docs/26-m4-implementation-report.md。

原生格式仍为 schema 1.2 / minReader 3；PPTX 仍只读。系统字体不随包分发。SHA256.json 包含分发文件校验值。旧阶段包和开发前源码快照保留。
