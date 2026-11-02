# JoyDeck 实施状态

2026-11-02：当前为 JoyDeck 0.2.0 / R1 真实文稿只读查看切片。Rust 1.98.1、GPUI Kit 0.8.1、GPUI pre 0.3.10 与 JoyOffice 保持一致，未引入 egui/epaint。

## 已实现

- 迁入 DeckCraft 的 color/geom/model/metafile/fonts/text/anim/render/format/pptx，共十个无 GPUI 核心库，保留来源、许可和可运行测试；engine 依赖的历史集成测试保留在 tests-upstream，等 R2 接回，未计为通过。
- 复用 JoyOffice 公共 UI、OPC 严格 ZIP 读取器与现有 JoyDeck 的异步只读浏览器。只保留一份 Presentation，CLI 与桌面都通过 shell 文稿会话读取并使用同一 CPU 渲染器。
- 真实 PPTX / .deckcraft v1 读取，包/对象/像素预算，坏文件恢复、取消和过期结果丢弃；保留原始字节与未知内容，R1 不写回文件。
- 实际页画布、虚拟缩略图、32 张缩略图缓存、切页、适应窗口、缩放、明暗配色、只读备注和兼容提示。当前画布保留一张页面栅格；可见缩略图每批最多 12 张后台生成。
- 无窗口 CLI inspect/render，PNG 与 JSON 输出；新目录要求防止覆盖既有结果。包含原创 native/PPTX 和独立 python-pptx 样本。

## 验证与交付

当次命令、测试、截图、包路径及 SHA-256 见 [R1 验证记录](evidence/R1/README.md)。R0 开发包和原有证据保留。当前验证环境是 Windows x86_64/MSVC；macOS/Linux 和远端 CI 未执行本轮原生验证。CPU 参考栅格通过 GPUI 显示，原生对象场景加速仍属于 R4，不将本切片称为性能优化已完成。

## 后续范围

R2 接入 engine 的统一命令/撤销、编辑与保存重开，并恢复 engine 相关格式集成测试、对齐 329 条编辑命令 registry。R3 继续精确字体替代/缺字报告及原位文字系统 IME。R1 的格式兼容报告和只读备注不代表编辑或 PowerPoint 全保真；外链媒体不下载，3D/SmartArt/OLE/动画及复杂效果可能使用 fallback。

取消在读取、包检查、解析前后与光栅任务边界检查；已进入的单次解析/光栅不能立即抢占，旧结果由 generation 校验拦截。字体由许可明确的 Ubuntu fallback 和本机字体提供，不分发系统字体。正式签名、安装/更新及完整再分发许可审计仍待后续阶段。
