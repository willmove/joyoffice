# JoyDeck M5 图片创作开发预览

macOS ARM64 本机 release 开发包，未签名/公证。运行 `./joydeck` 或 Launch.command。

图片路径栏输入完整 PNG/JPEG 路径后导入；也可拖入一个文件、从系统剪贴板粘贴图片。按内容识别类型，应用 EXIF 方向，转换为内嵌 PNG；无外部文件依赖。单文件≤8 MiB、边≤4096、像素≤4M；资源总编码≤32 MiB，模型/历史还有更保守的预算，超限整次拒绝。

选中图片，设置归一化 x/y/宽/高 (0–1)，应用或重置裁剪；支持替换和既有旋转/透明度。裁剪不改变规范化源 PNG，未实现交互式裁剪预览/取消或图片修正。复制对象、粘贴或画布焦点 Cmd/Ctrl+C/V 使用版本化内部快照，包含资源与样式；普通应用支持文字和 PNG/JPEG fallback。内部元数据的跨应用保留取决于 OS。对齐≥2对象，分布≥3对象，均单次撤销。

默认 Scene/GPUI；底部切 CPU 对照。图片目前走有界对象局部 raster fallback，并非直接源图片纹理路径。组/连接符/锁定、完整选择面板、SVG/metafile、自由路径/布尔合并尚未实现；完整 M5 及 M0–M4 的所有退出条件均未通过。Windows/Linux 剪贴板与物理拖放尚未验收。

schema 1.3 / minReader 4，requiredFeatures 增加 image-resource-v1，旧版本会拒绝读取。可读取旧 1.0–1.2；升级前请保留源文件副本。PPTX 仍只读。

验证：`python3 scripts/smoke-native.py M0 M1 M2 M3 M4 --binary ./joydeck`；macOS 带 Pillow/numpy 的 Python 运行 `scripts/verify-m5.py --binary ./joydeck`，做系统图片/对象剪贴板、裁剪历史、保存重开及真实 CPU/GPUI 窗口像素对照。测试会暂时占用并恢复剪贴板；意外中断时可能无法恢复。系统拼音用 `scripts/smoke-macos-ime.py --binary ./joydeck`。这些是 OS/原生 adapter 测试，不能替代物理键盘、三平台与完整阶段验收。SHA256.json 提供文件校验值。
