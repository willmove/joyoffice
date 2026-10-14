# W02 集成审查（2026-10-14）

- 交付提交：3a682a8be41d2933320600e4e66ad96dfeed6201。
- 执行者：ZCode Desktop/GLM-5.3，以原交接为准；工作目录干净，已推送。
- 接收范围：独立 macOS 字体/外部字形探针及原始证据；不批准正式 GPUI 来源升级，不改根 Cargo 或生产应用。
- 状态：reviewed_experiment；interface_ready 仅限实验，acceptance_complete=false。T003、T005 与 ADR-010 保持未完成。

## 范围与来源

任务路径符合 W02 边界。交付中约 96,000 行主要是 3.1 MiB 的 GPUI 第三方源码；协调者逐文件与本机 registry gpui-0.2.2 比较，保留的源码文件仅 text_system.rs 有差异，且精确等于 GlyphId 字段由 pub(crate) 改为 pub 的一行替换。examples/docs/tests 的删减及许可见 vendor/PATCH.md；不把大段上游源码当成原创实现。

根 workspace 和独立实验的 Cargo.lock 各自保持原状，不能据本机 macOS 通过声称其 Linux 锁依赖可用。

## 协调者修正

实验把 unsafe_code 从 forbid 放宽为 deny 并在截图模块局部 allow。现有 core-graphics 0.24.0 已提供 create_window_list 和返回类型化字典数组的 create_description_from_array，可直接安全迭代。已换用该路径，删除自有 unsafe 转换并恢复 forbid；没有扩大补丁范围或新增依赖。

另修正文档：glyph-crosscheck 实际会瞬时创建一个测量窗口，不属于无窗口验证。协调者本轮只执行真正无 GUI 的测试和已有截图测量，没有重跑会创建窗口的命令。

## 本机独立复验

| 检查/命令 | 结果与限制 |
| --- | --- |
| JOY_PROBE_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:6' cargo test --manifest-path experiments/gpui-font-probe/Cargo.toml --features runtime-shaders --locked -- --nocapture | 4/4 实际执行并通过，使用存在的 Songti face 6；包括字体抽取/改名不改变字形与场景、PDF 确定性和投影测试 |
| cargo fmt --manifest-path experiments/gpui-font-probe/Cargo.toml --all -- --check | passed |
| cargo clippy --manifest-path experiments/gpui-font-probe/Cargo.toml --features runtime-shaders --all-targets --locked -- -D warnings | 自有代码通过；vendored gpui 仍有上游 future-incompat 警告，不声称整个依赖树零警告 |
| measure.py window（原始 z100/z200 PNG + 对应 JSON） | 两档重测通过：z100 89/89 标尺，最大 dx=0.50/dy=0.18px；z200 可视 82/82，最大 dx=0.48/dy=0.36px，7 个在可视区外 |
| measure.py scale | 89 项期望位置满足 2×；这是计划数据检查，不是两个物理 DPI 的测量 |
| measure.py pdf（已有 PDF 栅格 PNG） | 重新提取两行墨迹范围，与原结果一致；该子命令只报告墨迹，不自动执行精确几何验收 |
| pypdf 6.10.0 读取已有 probe.pdf | 1 页、两行文字与探针原创样例一致；只验文字，不冒充屏幕/PDF 逐字形坐标验收 |
| 协调者查看原始 z100 PNG | 中英文字和标尺清晰，未发现样例范围内的缺字/异常裁切；没有由本次复验重新捕获窗口 |

原始截图/JSON/PDF 保持交付时内容。重测日志在本聊天 target/coordination-review/w02，关键结果由本报告记录。截图来自执行者，协调者将静态复验与重新运行 GUI 区分。

## 正式接入前仍需满足

1. ADR-010 明确要求 Linux GL 或 CPU 渲染。0.2.2+GlyphId 补丁没有提供这项能力，因此不能把该实验推荐直接提升为长期跨平台来源；候选仍需固定源码/构建/目标机证据。
2. 正式字体适配需使用许可明确的 OFL 字体复测，验证实际注册字体身份与 fallback，不以少量字形或短散列名称证明全部字体字节绑定。抽取/改名后的字节须同时用于两出口；生产映射需要保留源身份。
3. name 表处理仅证明受控系统字体样例可用；生产复用前需补整数边界、name 表 u16 容量、坏字体/错误 face、TTC 共享表和变体测试，不直接把实验解析器暴露给任意文档字体。
4. 像素标尺验证不等于全部字形墨迹/位置通过。Retina/DPI=2、非零字形 y 偏移、IME 和 Linux/信创真机均未验收。

因此接收实验成果、保留生产门槛。W01/W02/W03 第一轮切片均已审查，W04 的 WPS GUI 与固定 Word/其他平台参考环境另行安排；不将开发切片整体标记为 M0/M1 完成。
