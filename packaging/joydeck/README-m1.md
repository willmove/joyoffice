# JoyDeck M1 · Windows x64 只读预览

双击 `joydeck.exe`，点击“打开文稿”，选择 samples 中的 `.joydeck` 或 `.pptx`。也可将完整文件路径填入路径框后点“读取路径”，或在命令行传入一个路径。

- 缩略图点击、上一/下一页、PageUp/PageDown、左右箭头、Home/End 浏览。
- +/− 或 Ctrl+=/Ctrl+− 缩放 25–400%；“适应窗口”/Ctrl+0 随窗口大小调整。
- 放大后可滚动画布。Ctrl+O 打开，Escape 取消读取，Ctrl+Q 退出。
- 只读，不保存、不编辑、不改动源文件。错误或取消保留当前文稿。
- 兼容报告说明未显示对象和简化排版；复杂内容不保证保真。

`.joydeck` 为项目独立定义的 ZIP+JSON v1 最小子集。PPTX 仅支持 Transitional 的直接坐标、实色矩形/椭圆/线和段落纯文本；图片、组、母版/主题、复杂文字/效果、图表和表格尚未完整显示。Strict、宏和加密格式未支持。samples 是本项目自建的 20 页样例，不能证明任意 Office 文件兼容。

版本 0.1.0，Rust 1.98.1，GPUI Kit 0.8.1 / GPUI pre 0.3.10。未签名开发包，含调试信息；不是安装器或正式发行。需要 Windows 图形运行环境与本机字体，不依赖 Office。系统字体不随包分发。macOS/Linux、物理键盘/输入法、多显示器/DPI、完整性能门槛仍待验收。

`--diagnostics` 可启动 M0 技术诊断。校验值见 SHA256.txt，依赖来源见 THIRD_PARTY.md / licenses，确切依赖图见 Cargo.lock。本包无自动更新和遥测。
