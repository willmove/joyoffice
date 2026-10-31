# 公开依据与项目证据

资料复核日期：2026-10-29。公开标准支持格式语义，应用能力由本项目的构建、测试与桌面运行证据证明。

| 来源 | 用途 |
|---|---|
| [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/) | OOXML 标记、OPC、兼容性与扩展 |
| [PresentationML 结构](https://learn.microsoft.com/en-us/office/open-xml/presentation/structure-of-a-presentationml-document) | presentation、slides、layouts、masters 和关系 |
| [Office 实现补充](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376) | 实施具体扩展时逐项核对 |
| [GPUI Kit](https://gpui-kit.com/docs/installation/) | 锁定依赖的窗口、任务、输入和绘制 API |
| [resvg](https://github.com/linebender/resvg) | 通用 SVG CPU 栅格化依赖；不定义文稿模型 |
| [roxmltree](https://github.com/RazrFalcon/roxmltree) | 受限 XML 读取依赖 |
| [zip](https://github.com/zip-rs/zip) | 受限 ZIP 读取依赖 |

## 项目实施证据

- [M0 实施记录](17-m0-implementation-report.md) 与 evidence/m0/：保留既有构建、诊断窗口和未测平台状态。
- [M1 规格](18-m1-viewer-spec.md) 与 [M1 实施记录](19-m1-implementation-report.md)：独立模型、合成样例、后台只读浏览与故障行为。
- Cargo.lock、THIRD_PARTY.md：实际依赖图和直接依赖来源；完整传递依赖发布审核仍是发布门槛。
- 样例由 scripts/make-fixtures.py 在本项目生成，生成器与文稿一起保留，未验证的外部兼容性不写成通过。

文档中的性能数字是预算；只有实施记录中的测量值是结果。未运行的平台、物理输入法、多 DPI 和真实 Office 交换保持未验证。

## 2026-10-30 M3 开发预览证据

当前交付边界与日志入口：[23 M3 实施记录](23-m3-implementation-report.md)。实际本机 macOS ARM64 build/check/stage，34 项测试通过。早期 M3 输入桥 native smoke 有绘制记录，最终包 native 验证为 no_active_window，真实 IME/截图/跨屏/读屏仍未通过。新增 Unicode segmentation 来源及许可证见 THIRD_PARTY.md；API 依据锁定 GPUI pre 0.3.10 的 EntityInputHandler、ElementInputHandler、ShapedLine 官方依赖源码。

## 解锁后 macOS 原生/IME 验证

更新证据见 [24](24-m3-macos-verification.md)：35 项测试及最终包 M0–M3 adapter 回归通过，实际系统拼音候选/提交/取消/单次撤销/保存重开有日志及截图。旧 no_active_window 状态已解除；其余平台、输入法、跨屏/DPI 与完整 M3 软件缺口仍保留。


## 2026-10-30 M4 Scene/GPUI 证据

[25 验证方法](25-m4-scene-validation.md)、[26 实施与本机验收](26-m4-implementation-report.md)：独立 Scene、路径/image/glyph mask、局部 fallback、有界缓存、45 项测试；最终 release 包的 EDIT/EFFECT CPU/GPUI 原生反馈、四组像素差分、RSS、M0–M4 与系统拼音回归均有原始记录。20 个图像是独立生成资源，未复用同一 bitmap 冒充资源集。数据生成器和固定 seed/轨迹在 scene_bench.rs，未使用外部文稿素材。

静态 glyph 桥和图片采样依据锁定依赖的 usvg/resvg、ttf-parser、GPUI pre 源码；来源和许可证见 THIRD_PARTY.md。真实 device-lost、driver/硬件 present trace、Windows/Linux 和完整 M3 前置条件仍未关闭；阶段状态以26的开放矩阵为准。


## 2026-10-30 M8 表格与工作区

[30实施记录](30-m8-implementation-report.md)、[验证索引](evidence/m8/verification.json)：79项测试，M0–M8最终包回归、普通文字/表格系统拼音、CPU/GPUI截图与真实PowerPoint编辑重读。新固定fixture为本项目原创表格经PowerPoint16.114.1修改的文件；公开合并语义依据微软MS-OE376，未使用外部用户文稿作为语料。未验证平台/物理键盘/其他DPI和格式子集按30保留。
