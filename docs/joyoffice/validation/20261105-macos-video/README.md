# JoyDeck R9 macOS 静态 MP4 验证

2026-11-05 在 Apple M4、macOS 27.2 上验证 JD-705 的静态 MP4。编码器读取 JoyDeck 已生成的 PNG，使用 AVFoundation 生成 H.264 MP4。每页至少有一帧，短于帧间隔的停留时间仍能保留该页。输出不含音频、动画、备注或嵌入媒体。媒体探测、播放和打印的 macOS 验收仍开放。[Apple AVFoundation 像素缓冲接口](https://developer.apple.com/documentation/avfoundation/avassetwriterinputpixelbufferadaptor/append%28_%3Awithpresentationtime%3A%29)

## 开发包和读回

- `cargo xtask package joydeck --release --name joydeck-0.8.0-macos-video` 生成 ARM64 开发包，归档 SHA-256 为 `5dd43f11ff76ef7a175ff1f4eed285420b906106ce27ecdd5cc1770e968f39e6`，源码快照为 `e6f2a20cf30cf1dc1093e410f9c0ff997162a393b3b04886d7e927b5a4ee3495`。归档及包内 1,763 项校验通过。
- 将包解压至仓库外的中文空格路径，使用 `JoyDeck.app/Contents/MacOS/joydeck-cli` 执行 `scripts/joydeck/verify-r6-r7.py --cli PATH --evidence REPORT`，33 次 CLI 调用和 39 次 MCP 响应通过。重复导出拒绝覆盖，语料 SHA-256 一致。
- 对 `tests/fixtures/joydeck/r6-r7/delivery-media.pptx` 使用 `--seconds .5 --fps 4 --scale .5`。AVAssetReader 独立读回 360 × 202、6 帧、1.5 秒、1 个视频轨和 0 个音轨。将 6 帧逐一与同尺寸参考 PNG 比较，三页各有两帧且顺序一致。输出 SHA-256 为 `13544c480e23cf823979721282ee477a86c6030e97e81ae562bc444e43ba598a`。
- 原生 helper 拒绝目录穿越、符号链接、假 PNG、超限帧率、超限总时长、空幻灯片列表与已有目标；0.1 秒的单帧 MP4 可独立读回。开发入口 `cargo xtask run joydeck --headless --release --target aarch64-apple-darwin -- export …` 也已实际导出。

## 回归范围

三个产品的 `cargo xtask check` 均通过格式检查、Clippy 和测试。JoyDeck 894 项通过，JoyWord 3,494 项通过、2 项忽略，JoySheet 1,400 项通过、1 项忽略。工程路由单测 15 项通过。Windows 和 Intel Mac 的新包尚未在对应机器复验，桌面导出入口尚未实屏复验。
