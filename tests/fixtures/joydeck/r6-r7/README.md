# R6/R7 语料

`delivery-media.pptx` 由本仓库 `scripts/joydeck/generate-r6-r7-pptx.py` 使用 python-pptx 1.0.2 独立生成：三页、多语文字、公开超链接、演讲者备注、隐藏页、真实内嵌原创 H.264/AAC、poster 和循环播放标记。`vendor/retain-r6.xml` 用于未知部件保留验证。文稿布局/内容按本仓库 MIT 许可；模板许可见 licenses/python-pptx-MIT.txt，无系统字体分发。

另四份语料逐字节取自固定 JoyOffice MIT 基线，路径、来源 SHA-256 和当前指纹见 `licenses/joyoffice/source-map.json`：

- legacy-1.0：原创 20 页格式样例。
- legacy-1.11：旧 JoyDeck preset 几何实际保存结果，1 页。
- powerpoint-authored-motion：原项目登记的 PowerPoint 动画样例；作为公开读取/状态采样语料，不把其存在当作本轮 Office 互操作已通过。
- original-h264-aac：原项目原创 1920×1080、30fps、5 秒动态色块和 440 Hz 音频。源仓库 fixtures README 登记 Swift/系统 AAC 生成来源，无第三方视频/录音/音乐。默认静音验证。

生成器需已准备 python-pptx/Pillow 验证工具目录；它们不进入应用运行时依赖。旧文件用于只读转换验证，禁止覆盖原件。
