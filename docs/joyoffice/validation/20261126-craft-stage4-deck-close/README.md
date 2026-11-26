# JoyDeck 阶段 4 功能区与收尾验收

接续 `3c24fd2` 的合成、图元、PDF 与打印替换。冻结参照仍为 `14cbb296`。本轮将功能区标签从界面文字分派切换为 JoyDeck 命令标识，按形状、图片、表格、图表的实际选区计算上下文标签。文件页保留进入前选中的标签。共享 GPUI 组件负责控件焦点、启用状态和动作执行。[标签状态](../../../../crates/joydeck-ui-gpui/src/ribbon_tabs.rs) · [窗口呈现](../../../../crates/joydeck-ui-gpui/src/chrome.rs) · [输出基线](../20261126-craft-stage4-deck-output/README.md)

| 验证面 | Windows 结果 |
| --- | --- |
| 宽窗、浅色 | 默认 1440×900 会话核对冻结版常驻标签、开始页及已禁用的重置。点选「新建幻灯片」后出现第二页，Ctrl+Z 恢复一页。 |
| 窄窗、深色 | 990×720 会话打开冻结语料中的三页 PPTX，左右滚动入口可见。向右滚动露出右侧命令。选中形状显示「形状格式」。 |
| 折叠与焦点 | Ctrl+F1 收起命令区，标签仍可达。Tab 将真实键盘焦点移至「插入」，Enter 显示插入命令组。焦点在该标签时再次折叠，按 Enter 重新展开。 |
| 工程回归 | `cargo xtask check joydeck`、`cargo xtask build joydeck` 通过。1,066 项 Rust 测试和 R1–R7 的 126 次 CLI 检查通过。 |

阶段 4 开发余量为 0 人周。Windows 已验证本阶段替换。macOS Metal、UOS Vulkan 同配置实机对照，以及独立 Windows 打印对话框的交互检查，继续登记为平台待验。Windows 打印分页已有原生虚拟驱动产物，含三页的页序读回。阶段 5 的生产路径来源复查和回退演练沿用三个产品共用估算。[替换计划](../../CRAFT-REPLACEMENT-PLAN.md) · [验收数据](verification.json)
