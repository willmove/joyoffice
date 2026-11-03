# R1 真实文稿查看切片验证

2026-11-02，JoyDeck 0.2.0，Windows x86_64 / MSVC，Rust 1.98.1，GPUI Kit 0.8.1 / GPUI pre 0.3.10。本轮在此前三个核心库底座上接入真实文稿，保留 R0 开发包与历史提交。

## 三个可编译运行的切片

1. **格式与 CPU 底座**：迁入 metafile/fonts/text/anim/render/format/pptx，适配 JoyOffice 锁定的 image/ZIP/字体/XML API。保留十个核心库；新接入库及 OPC 的 649 项测试通过（其中 render 191、PPTX 114、native 40）。首次 API 适配过程和通过日志见 logs/import-check.txt、logs/import-tests.txt；最终更严格预算的通过结果以 logs/product-check.txt 为准。
2. **有界文稿会话与 CLI**：复用 JoyOffice 严格 OPC ZIP 读取器，接入 shell Document、原始字节保留、格式识别、兼容/字体提示与同一 CPU 渲染器。真实 CLI 在仓库外中文空格路径完成 28 项检查：inspect、全页 PNG、坏文件、未来 native 版本、非法页码/缩放、非零错误码、不覆盖既有输出和源文件不变。独立 python-pptx 样本另有形状、图片、表格、缩略图像素断言；不以私有模型 sidecar 证明 PPTX 互通。
3. **原生查看器与开发包**：复用现有 JoyDeck 的异步加载、generation 校验、取消、虚拟页列表和 32 张缩略图缓存。画布和缩略图都来自同一模型/CPU 栅格，再以 GPUI 显示。接入真实文件路径/系统选择器、翻页、缩放/适应窗口、JoyOffice 明暗 Palette、只读备注及兼容报告。最终 Windows ZIP 与原生验证的哈希/截图保存在本目录。

## 检查结果

- `python tools/xtask/product.py check joydeck`：824 项 Rust 测试、5 项 Python 工程测试，文档/来源/精确依赖、fmt、严格 Clippy 全部通过。见 logs/product-check.txt。
- `python tools/xtask/product.py check joydeck --headless`：823 项 Rust 测试及相同工程门禁通过；CLI、shell、model、formats、render 闭包无 GPUI、egui/epaint 或其它产品。见 logs/headless-check.txt。
- 源来源记录在 licenses/deckcraft/file-map.json、licenses/joyoffice/source-map.json；独立字体资产及完整许可在 licenses/fonts。源码检出 JoyOffice 保持不变。
- 独立生产器为 python-pptx 1.0.2；仅用于生成原创测试文稿，不是运行时依赖。默认 OOXML 模板的 MIT 许可保留在 licenses/python-pptx-MIT.txt。

## 范围与未验收项

本轮提供读取/只读浏览/PNG 输出，未接入编辑或保存。十个核心库测试并不代表 PowerPoint 全保真。母版、主题、文字、形状、图片、表格等走源 CPU 渲染路径；复杂效果、动画、SmartArt/OLE/3D 可近似或使用 fallback，字体依赖本机，缺少 family 会提示替代。外链媒体不下载；未知内容及原包精确字节在会话中保留。

文件输入 64 MiB，4096 ZIP 项，单项 8 MiB、解压总量 128 MiB、XML 深度 128/每项 100 万事件、1–1000 页/2 万对象。输出 8192 边长/1600 万像素；解码图片 RGBA 上限 64 MiB、图片缓存 128 MiB。收紧后的原图片预算测试同步验证 4096² 可接受、8192² 被拒绝，没有放宽回归阈值。

取消在包检查、解析前后及光栅边界生效，单次解析/光栅无法抢占。取消 token 有单元验证；本轮原生压力样本在点击取消前已读取完成，未把该尝试计作持续运行中取消成功。迟到请求竞争专项仍待验收。engine 依赖的历史格式/CFB 集成测试保存在 tests-upstream，R2 接回，未计入上述通过数量。CLI commands 与 329 条编辑命令运行时 registry 对照也留在 R2。

本轮只验证 Windows；macOS/Linux 原生构建和远端 CI 未执行。当前为未签名开发包，正式安装/更新和再分发许可审计仍在后续阶段。GPUI 原生对象场景与性能优化仍为 R4，不把 CPU 栅格查看器称为已完成性能加速。


## 最终 Windows 包与桌面证据

- [打包记录](package.json)：ZIP `joydeck-0.2.0-r1-windows-x86_64-release-fdd20f30eb.zip`，18,794,122 字节（约 17.9 MiB），SHA-256 `ceecf4e4aee3337aa20eaec45ec24d8072c3bf128ef7fde2338c4f5dbe4bfc0b`。
- [仓库外启动与校验](standalone-launch.json)：中文空格路径解压，746 个文件 SHA-256 一致；GUI SHA-256 `2768e333ff4a8550ca99e6ce1ccfe2f5135a4f981feddda648aea97d9b2dd1c6`。包内源码指纹仍与当前工作区匹配。
- [最终包 CLI 的 28 项实际检查](cli-release-verification.json)，[调试 CLI 检查](cli-verification.json)。两者分开记录，不混用二进制哈希。
- [980 像素 native 文稿](01-release-native-980.png)、[真实 PPTX 图片/表格页](02-release-pptx-page2.png)、[演讲备注](03-release-speaker-notes.png)、[深色主题](04-release-dark-pptx.png)、[兼容报告](05-release-compatibility.png)、[坏文件保留当前页](06-release-broken-preserves-document.png)、[96 页虚拟列表末页](07-release-96-pages-virtual-rail.png)、[缩放](08-release-zoom.png)。均来自上述最终解压包。

系统文件选择器可打开；本次 Computer Use helper 未能把其 owned modal 绑定为独立目标（报错：point is over joydeck.exe empty title, not target root window）。因此未把自动选择文件计为完成；最终验收改用启动路径参数与应用路径框，覆盖仓库外中文路径的真实文稿读取。取消/迟到专项边界同前述记录。

SBOM 包含 439 个实际 normal/build 依赖；21 个注册表 crate 未在发布归档内附完整许可文本，具体列表在 package.json。已登记许可证表达式和仓库来源；正式对外分发前由 R8 补齐，不把当前未签名开发包称为正式发布。独立 Ubuntu/Hack 及 python-pptx 模板许可已随包保留。
