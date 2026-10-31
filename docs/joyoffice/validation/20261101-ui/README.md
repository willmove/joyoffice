# JoyOffice UI 刷新验证

2026-11-01，Apple Silicon / macOS 27.0.1，Rust 1.98.1，完整 Xcode。源码为 `codex/joyword-unified-ui`，基于主干 `d70e8cb`；本记录是本轮执行结果，不修改既有里程碑人工验收状态。

## 自动检查

| 命令 | 结果 |
| --- | --- |
| `cargo xtask check joyword` | fmt、严格 Clippy、1,273 项测试通过，0 忽略 |
| `cargo xtask check joysheet` | fmt、严格 Clippy、1,400 项测试通过，1 项原有忽略 |
| `cargo xtask check joydeck` | fmt、严格 Clippy、153 项测试通过，0 忽略 |
| `cargo xtask build joyword --release` | 默认 feature 构建成功，使用完整 Xcode 的 Metal 编译器 |
| `cargo xtask build joysheet` | 默认 feature 开发构建成功，并运行实际窗口 |
| `python3 -m unittest discover -s scripts/tests -p test_check_gates.py` | 58 项通过 |
| `python3 -m unittest discover -s tools/xtask -p 'test*.py'` | 13 项通过 |
| `python3 scripts/check_gates.py` | 通过 |

不同产品测试统计包含各自消费的共享库，不能相加当作唯一测试总数。公共锁文件仅增加内部 UI 包及两条消费者依赖边，第三方版本未变。公共配色测试覆盖 Word／Sheet 两种强调色和浅深色模式的文字／操作对比度。

逐命令日志及统计见 [engineering.json](engineering.json)。末轮查找复选框左对齐调整后额外执行应用 crate 的严格 Clippy、测试及构建，见 `word-ui-final-*` 日志。

## 原生界面验证

执行生产 GPUI 窗口，通过 macOS AX 操作和系统合成按键验证 15 项：稳定本地化按钮身份、初始撤销禁用、五个功能区切换、选区加粗状态、撤销恢复正文和按钮状态、字体弹窗打开／取消及焦点返回、查找侧栏打开／关闭及焦点、窄窗口常用格式／更多入口和溢出菜单打开。

额外使用本轮原创 [DOCX 样例](styled-fixture.docx) 检查样式卡片实际导入、键盘应用及撤销，见 [native-styles.json](native-styles.json)。

结果及实际 Release 二进制哈希见 [native-word.json](native-word.json)。截图均来自本轮实际窗口，保存后已逐张检查。启动使用独立测试配置目录，Sheet 使用独立 `JOYSHEET_CONFIG_DIR`，不使用个人文档或真实恢复目录。

| 状态 | 截图 | 检查结果 |
| --- | --- | --- |
| 改造前，1100 像素宽 | [原界面](00-word-before.png) | 当前基线构建截图，仅供前后比较 |
| Word 主窗口，1200 像素宽 | [主界面](01-word-main.png) | 标题、图标、分组、页内容和状态栏完整 |
| 字体弹窗 | [字体](02-font-dialog.png) | 预设归属、字段、焦点、提交／取消清晰 |
| 查找侧栏 | [查找](03-find-panel.png) | 字段聚焦；复选框与操作按统一尺寸排列 |
| Word 紧凑档，1100 像素宽 | [紧凑档](04-word-compact.png) | 与原界面同尺寸；操作通过更多入口保留 |
| Word 窄窗口，700 像素宽 | [窄窗口](05-word-narrow.png) | 工具区高度降为 54，正文区域扩大 |
| Word 导入真实样式 | [样式卡片](08-word-styles.png) | 三张卡片与编辑工具完整展示；键盘应用样式及撤销通过 |
| Sheet 浅色 | [浅色](06-sheet-light.png) | 共用外壳配色与尺寸，网格／图表正常呈现 |
| Sheet 深色 | [深色](07-sheet-dark.png) | 实际主题切换后检查；表格内容自身颜色保持 |

## 范围与未验证事项

- Word 首批交付浅色模式；共享深色配色与 Sheet 深色接入不代表 Word 完整深色模式已实现。
- Sheet 本轮只归并主要外壳与工具按钮，未宣称所有历史业务面板已经统一。
- AX 和合成按键验证不替代真实中文输入法、物理输入、真实读屏或多显示器 DPI 验收；这些未在本轮执行。
- Windows、Linux、信创真机未运行，不以本次 macOS 或跨产品测试替代。
- 未执行正式签名、公证和发布；本地应用包装仅供查看。

实现边界和设计来源见 [UI 规范](../../UI-SYSTEM.md)。
