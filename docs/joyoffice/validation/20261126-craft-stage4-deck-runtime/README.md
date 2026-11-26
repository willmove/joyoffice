# JoyDeck 阶段 4 文本与放映运行时验收

使用 2026-11-24 的冻结提交 `14cbb29616d7c0ababbf40a8f6c0218b996e7ae8` 和当时保存的 Windows CLI。阶段 4 从 `c172e907` 开始。冻结源码和候选源码运行同一份只读探针，使用同一台 Windows 机器的字体。转场互读使用冻结与候选 CLI，放映窗口使用原生桌面构建。[回归数据](verification.json) · [冻结记录](../20261124-craft-pilots/README.md)

## 生产切换

- 文本按 Unicode 断行机会、整形簇和字素边界换行。绘制、光标移动和命中使用同一份定位字形及字素范围。自适应布局保留文件中保存的比例，实时布局按容器求可用比例。字体选择按家族、样式和完整字素覆盖排序，保留字体文件散列、集合索引和变体坐标。[换行](../../../../crates/joydeck-text/src/breaking.rs) · [命中](../../../../crates/joydeck-text/src/caret.rs) · [字体](../../../../crates/joydeck-fonts/src/fontdb.rs)
- 放映按点击、同击和随后播放编译主序列及对象触发序列。重叠的触发轨道按实际启动顺序应用。效果求值使用独立的缓动、显示与隐藏、强调、路径和终态逻辑。桌面和 CLI 通过同一个播放时钟取样。[时间表](../../../../crates/joydeck-anim/src/timeline.rs) · [效果](../../../../crates/joydeck-anim/src/effects.rs) · [播放器](../../../../crates/joydeck-shell/src/show.rs)
- Morph 优先配对显式 `!!` 名称，再按名称、文字或空形状的预设轮廓匹配。重复对象和词字按出现顺序一对一匹配。轮廓在共享绕向下对齐，两端直接绘制原页与目标页。中间帧继续使用按模式计算的轮廓变形及字形位置。[对象配对](../../../../crates/joydeck-anim/src/morph.rs) · [轮廓对齐](../../../../crates/joydeck-geom/src/morph.rs) · [文字 Morph](../../../../crates/joydeck-render/src/morph_text.rs)

断行与双向文字规则参考 Unicode UAX #14、UAX #9。整形簇参考 HarfBuzz。放映与 Morph 语义对照 Microsoft 的启动方式和 `!!` 命名说明。具体数值及颜色通过冻结实现的样本核验。[Unicode Line Breaking Algorithm](https://www.unicode.org/reports/tr14/) · [Unicode Bidirectional Algorithm](https://www.unicode.org/reports/tr9/) · [HarfBuzz Clusters](https://harfbuzz.github.io/clusters.html) · [PowerPoint 对象动画](https://support.microsoft.com/en-us/powerpoint/training/animate-text-or-objects) · [PowerPoint Morph 说明](https://support.microsoft.com/en-us/powerpoint/morph-transition-tips-and-tricks)

## Windows 回归

| 路径 | 覆盖与验收 |
| --- | --- |
| 排版和命中 | 60 组布局包含双向文字、CJK、组合字符、emoji、制表位、竖排、列表和自动缩小。372 行、4,904 个光标位置、4,904 次命中、2,038 个定位字形及 436 处字体身份与冻结源码逐条一致。 |
| 对象动画 | 198 种效果与选项组合的 1,188 个取样状态与冻结版一致。合成演示验证了主序列顺序、重复、自动反向、两个重叠的对象触发、暂停后画面保持、退出后返回至已完成状态。两份 PowerPoint 原生样本的时序与像素也一致。 |
| Morph | Objects、Words、Characters 同时验证了重复词字、双向文字、有名轮廓变形和对象淡入淡出。冻结版在零进度重绘轮廓，三个首帧与原页有像素差异。候选版三种模式均与原页精确相等，最终帧与目标页精确相等，三种模式的其他采样帧与冻结版一致。 |
| 原生放映 | Windows 观众窗口复验 PowerPoint 16 文件的起始隐藏、主序列完成位置、点击触发、结束黑屏及 Esc 退出。词语 Morph 在真实放映中到达第二页，蓝色星形和旋转的双向文字正确显示。 |
| 转场合成及交换 | Windows Intel Arc Vulkan 在 64×36 和 193×109 对照 1,850 帧，最大通道差 1。两种尺寸的 CPU 清单和冻结版一致。既有 17 类、48 个选项完成 192 组冻结与候选交叉读写。 |
| 工程门禁 | `cargo xtask check joydeck` 的格式、Clippy 和 1,063 项 Rust 测试通过。桌面与 CLI 构建通过，R1–R7 的 126 次 CLI 检查通过。 |

自动探针分别为 `cargo test --locked -p joydeck-text --test layout_reference -- --nocapture` 和 `cargo test --locked -p joydeck-shell --test playback_reference -- --nocapture`。冻结工作树和候选使用相同探针，分别采集 `REF` 及 `PLAY` 记录。放映状态按 JSON 字段值比较，像素按 RGBA 字节散列比较。放映探针默认使用仓库中的 PowerPoint 样本，可通过 `JOYDECK_STAGE4_OFFICE` 载入阶段 3 的 PowerPoint 16 样本。设置 `JOYDECK_STAGE4_OUTPUT` 可写出三个用于桌面放映的合成 Morph 样本。[排版探针](../../../../crates/joydeck-text/tests/layout_reference.rs) · [放映探针](../../../../crates/joydeck-shell/tests/playback_reference.rs) · [转场互读](../20261123-uos-refactor/scripts/transition-exchange.py)

## 余量

阶段 4 余量为 10–18 人周。CPU 合成和相关转场路径估 3–5 人周，WMF／EMF 解释和绘制估 2–4 人周，定位字形、字体嵌入及 PDF／打印估 3–5 人周，功能区呈现估 2–4 人周。macOS Metal 与 UOS Vulkan 保留各自的冻结门槛，留到实机复跑。阶段 5 的三个产品共用来源核查及回退演练沿用原估。[替换计划](../../CRAFT-REPLACEMENT-PLAN.md)
