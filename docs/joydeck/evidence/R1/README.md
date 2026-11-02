# R1 首批核心底座迁移

2026-11-02。已迁入 DeckCraft 本机固定快照的颜色、几何和文稿模型，统一命名为 `joydeck-color`、`joydeck-geom`、`joydeck-model`。31 个文件的原始及适配后 SHA-256 在 `licenses/deckcraft/file-map.json`；MIT 选择、原始版权和许可证已保留。

## 实现和验证

- 保留完整的颜色变换、几何/预设/形变、Presentation/对象/主题/表格/文字语义模型与原测试。
- 采用宿主已有的 kurbo 0.13.1、thiserror 2.0.21、unicode-segmentation 1.13.3；serde/serde_json 版本不升级。启用所需 kurbo serde 与 serde_json float_roundtrip，并将共同 feature 变化纳入产品检查。
- 颜色 7、几何 62、模型 98，共 167 项核心测试通过；连同外壳和公共 UI 为 170 项 Rust 测试，另有 5 项 Python 工程回归。
- fmt、严格 Clippy、来源指纹、精确依赖与 CLI/核心无 GUI 依赖检查通过。原始日志在 `logs/`。

适配包括命名、LF/rustfmt、测试模块的断言 lint 范围和少量保留原 API 的样式标注。测试中的 unwrap/expect/panic 只在 `cfg(test)` 模块允许；生产代码的禁止 panic/unwrap/unsafe 要求保持。保留原始值枚举与动画 tuple API，未以迁移名义改变分配和序列化表示。

## 范围

本切片完成底层代码迁入及回归，JD-100 继续进行。字体、文字布局、CPU 渲染、格式/OPC、异步打开、缩略图、真实文稿 CLI 和完整查看器仍待接入。当前桌面预览仍运行 R0；这 167 项测试不计作 PPTX、输入法或桌面编辑功能验收。
