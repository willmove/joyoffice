# JoyDeck R1 命令对照与查看包复验

2026-11-05，在 macOS 27.2／Apple M4 上构建并移位 ARM64 Release 验收包。归档与包内 1,805 项校验一致。[构建与样本指纹](verification.json)

## JD-107 静态清单与运行时注册表

`verify-r1-registry.py` 用实际包内程序读取 CLI `commands`、JSON control 和 MCP 的 `engine.commands`。三者公布相同的 28 个 schema 2 类型化编辑入口，两个协议公布相同的 23 个方法和 11 个旧 JoyDeck 编辑子集。逐项与 [329 条 DeckCraft 静态清单](../../COMMANDS.csv) 求交集。唯一同名的是控制方法 `document.inspect`。

在已打开的三页 PPTX 上，分别把 329 个上游 ID 送入旧字符串编辑与 schema 2 编辑预检。658 次请求均明确拒绝未注册的别名。对真实 `insert-rect` 的预检则返回可编辑且无副作用。输入散列、活动文稿、修订、选择、撤销能力和检查结果保持一致。[实际注册表及交集](registry.json)

| 上游查看语义 | 当前入口和范围 |
| --- | --- |
| `commands.list` | CLI `commands` 列出类型化入口，控制端 `engine.commands` 另列协议方法和迁移子集。 |
| `document.inspect`、`file.info` | CLI `inspect` 或控制方法 `document.inspect` 读取文稿与各页语义。 |
| `file.open` | 桌面文件选择器与受控 `document.open` 使用相同的文稿解析核心。 |
| `file.render` | CLI `render` 从真实文稿输出单页或全页 PNG。 |
| `slide.inspect` | `document.inspect` 返回每页对象、标题及备注，调用方按页索引取值。 |
| `slide.first`、`slide.go`、`slide.last`、`slide.next`、`slide.previous` | 桌面导航和缩略图操作，类型化 `select` 提供绝对页选择。 |
| `file.openBytes`、`slide.colorAt` | 保留为待覆盖项，现有查看器接口面向文件和页面预览。 |

逐命令的 UI、参数及行为对齐继续由 JD-509 跟踪。名称重合与否独立于功能覆盖。[命令矩阵](../../COMMANDS.csv)

包内 CLI 通过 R1 的 28 项回归。独立 PPTX 的公开 XML 给出三页，第二页含图片和表格，CLI 语义读回一致。用 AppKit 独立解码第二页的实际输出 PNG，确认 960 × 540，并核对图形和图片内部的两个 RGBA 采样值。[结构及像素证据](verification.json)

## JD-108 当次查看包

- 冷启动打开中文空格路径的大写 `.PPTX`。实点缩略图按 1 → 2 → 3 → 1 → 2 翻页，包内 CLI 与窗口绑定控制端返回相同的三页对象、标题和备注。
- 第二页从 82% 缩小到 57%，再点「适应窗口」。独立解码截图测得纸面白色区域横向宽度为 775 → 539 → 775 pt。缩放前后的页面选中状态一致，复位画面散列与原图相同。[适应窗口](01-page2-fit.png) · [缩小](02-page2-zoom-out.png)
- 取消原生选择器及取消一份 18,015 对象文稿的实际后台读取，都保留第 2 页、文稿 ID 和修订。画布焦点下先用右方向键翻页，再按 Esc 取消同一压力场景，结果一致。随后打开坏 PPTX，窗口报告错误并保留当前页。[坏文件实屏](04-corrupt-file-retains-page2.png)
- 通过原生打开面板选择另一份 DeckCraft 文稿，窗口和 CLI 都读回三页。所有测试输入的散列保持一致。[打开 DeckCraft](05-deckcraft-open.png)

## JD-200 引擎与 IO 边界

运行时的内存编辑、选择与撤销在 `joydeck-engine::Session`。桌面编辑器和 CLI 都通过该事务入口。锁定的引擎 normal/build 工作区依赖闭包仅含引擎自身、`joydeck-model`、`joydeck-geom` 和 `joydeck-color`。文件保存与恢复由 shell 的 `Editor`、`SaveJob`、`storage` 编排，平台文件发布、打印和导出服务留在 shell 边界。依赖隔离检查与文件级 IO 门禁通过。

同一包内 CLI 的 R2 验收脚本通过 20 次调用，覆盖编辑、保存重开、保留读取、拒绝覆盖、输入不变和强制终止后恢复。产品检查 899 项通过，并通过格式和严格 Clippy。R2 的多窗口视图、手势和完整交付另由后续任务验收。[本次检查记录](verification.json)
