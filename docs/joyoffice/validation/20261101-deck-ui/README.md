# JoyDeck UI 统一验证

2026-11-01，macOS 27.0.1 / Apple Silicon、Rust 1.98.1。任务分支 `codex/joydeck-unified-ui`，起点 `2fe4796`。本轮使用已有 GPUI Kit 0.8.1 与 `joyoffice-ui`，第三方依赖版本不变；公共锁文件仅增加 Deck → 公共 UI 的内部依赖。独立默认/无界面构建边界保留。

## 实现与架构

- 标题与保存状态、九个常驻标签、工具分组、缩略图栏、工作台、备注、状态栏采用统一配色与尺寸；新文稿不会被误标为已保存到本机。
- 媒体、输出、比较和维护保留在“更多”；表格/图表/图示上下文入口按对象出现。紧凑开始工具区保留常用操作，并提供更多工具。
- 字体显示真实当前值，通用 `sans-serif` 显示为“默认字体”；颜色菜单提供色板与当前文稿主题颜色，属性允许精确值输入。主题色草稿需显式应用。
- 主题卡片读取实际文稿主题；版式卡片读取真实母版/版式占位符几何并发布当前页选中边框，点击调用原有版式事务。没有设计时提供创建入口，不生成虚构预览。
- 属性和高级任务面板共享有界容器与关闭入口，表格插入弹窗有具名字段和统一操作页脚。
- 主外壳、标题导航、功能区、属性、文稿表面分别位于 `chrome.rs`、`header.rs`、`ribbon.rs`、`inspector.rs`、`stage.rs`；`ui.rs` 绑定公共呈现原语。`workspace_layout.rs` 统一栏宽、工具区/备注高度与画布 fit，文稿坐标仍由真实绘制边界转换。
- 普通文件参数默认使用编辑器；`--viewer` 显式进入旧的只读浏览器。命令、保存、未知内容保留与拒绝契约沿用产品服务。

## 自动验证

| 检查 | 结果 |
| --- | --- |
| `cargo xtask check joydeck --validation` | fmt、严格 Clippy、155 项测试通过 |
| `cargo xtask check joydeck --headless` | 无界面闭包检查、fmt、严格 Clippy、146 项测试通过 |
| `cargo xtask check joyword` | fmt、严格 Clippy、1,273 项测试通过 |
| `cargo xtask check joysheet` | fmt、严格 Clippy、1,400 项通过、1 项原有忽略 |
| 分层门禁及 Python 回归 | 门禁通过，58 项测试通过 |
| 工程工具 Python 回归 | 13 项通过 |
| 文档检查 | 通过；仅元数据验证 |
| Release 构建与生产功能打包 | 通过；未启用 validation feature |

逐项结果与日志哈希见 [engineering.json](engineering.json)。三个产品统计均包含各自消费的公共包，不能相加当作唯一测试数。

几何回归覆盖 800/960/1200/1600 宽度、属性开关、备注开关、完整/紧凑工具区，以及横向/纵向/大尺寸页面的 fit。现有 M2 拖拽检查原先使用浮点精确相等，新 fit 比例暴露约 0.000046 pt 差异；改为 0.001 pt 精度比较，生产几何没有增加人为取整。

## 原生界面与文件回读

使用生产 GPUI 控件，macOS AX 定向到本次启动的 PID，配合系统合成键盘事件；状态与恢复目录独立，使用原创测试语料副本，不修改原件。可复现命令：

```text
cargo xtask build joydeck --validation
python3 scripts/joydeck/verify-unified-ui.py --output target/deck-ui-native-acceptance
python3 scripts/joydeck/smoke-native.py M1 M2 M3 M4
```

开发构建结果见 [native-result.json](native-result.json)，最终生产包结果见 [packaged-result.json](packaged-result.json)。本页改造后截图均来自最终包内的 Release 程序。菜单通过焦点与 Enter 验证，不能将其描述为 AXPress 已验证所有弹出控件。既有 M2/M3/M4 检查覆盖拖动/缩放/旋转、撤销、保存/关闭/恢复、富文本/组合态适配和缓存重建；这些仍是合成操作。

| 状态 | 截图 |
| --- | --- |
| 原始主界面，1200×800 | [改造前](00-before-main.png) |
| 原始设计面板，1200×800 | [改造前设计](00-before-design.png) |
| 新主界面，1200×800 | [主界面](01-main.png) |
| 插入工具区 | [插入](02-insert.png) |
| 表格插入弹窗 | [弹窗](03-table-dialog.png) |
| 真实主题与版式 | [设计](04-design.png) |
| 系统字体菜单 | [字体](05-font-menu.png) |
| 高级工作流入口 | [更多](06-workflows.png) |
| 宽窗口（请求 1600，本机系统实际限制为 1560） | [1560](07-width-1600.png) |
| 紧凑窗口 | [960](07-width-960.png) |
| 小窗口保留操作与画布 | [800](07-width-800.png) |
| 原创表格样例与属性面板 | [表格格式](08-table-properties.png) |

窗口验证以 CoreGraphics 的实际边界为准。宽度 1600 的请求在本机被系统限制为 1560；800 和 960 均为实际逻辑宽度。1600 另有纯几何自动回归，不能冒充原生 1600 截图。

改造前截图来自本次任务开始前同一基线的实际窗口；其文稿为空白。新截图使用原创测试文稿，不能把文稿本身的视觉差异归因于 UI 改造。

## 构建与边界

Release 已通过实际包内程序的全部原生控件操作、保存与独立回读，结果见 [packaged-result.json](packaged-result.json)。已核对包内 1,930 项文件哈希，App 内程序与分发程序逐字节一致。产物信息、二进制哈希和包验证记录于 engineering.json。仅为本机查看用构建，不是正式发布。没有正式签名、公证或更新发布。

- Deck 首轮为浅色；公共深色 token 不代表 Deck 完整深色模式。
- 高级业务表单本轮统一容器与主要呈现，未宣称所有历史字段和本地化细节已全部重做。
- 未执行 Windows/Linux/信创真机、物理键盘与真实系统 IME、读屏语音、多显示器 DPI、实体打印或双屏放映验收。
- 没有改记已有 M0–M14 父阶段或功能池为完成。

设计来源与公共 API 边界见 [UI 规范](../../UI-SYSTEM.md)。
