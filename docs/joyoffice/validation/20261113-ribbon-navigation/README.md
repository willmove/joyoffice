# Ribbon 折叠与 JoyDeck 方向键翻页

2026-11-13。功能分支 `feat/ribbon-collapse-deck-arrow-navigation` 从 main `55b15071` 建立，并合入已有的 JoySheet Ribbon 分支 `72bcffb3`；整合提交为 `1d0835c3`，保留两侧的主题与 Sheet 对象功能。

## 实现范围

- JoyWord、JoyDeck、JoySheet：双击当前选项卡标签隐藏工具区，再次双击恢复。标签栏保持可见，切换到其他选项卡时展开对应工具区。
- “文件”同样支持双击收起和展开。文件后台展开时保留顶部标签入口；收起后显示文档工作区，文件标签仍保持选中，文档可以继续编辑。
- JoySheet 不再为折叠的工具区保留内边距或边框；F6 的功能区焦点位于始终可见的标签栏。
- JoyDeck 编辑模式：上键复用左键的上一页动作，下键复用右键的下一页动作，输入框及原位文本编辑继续接收自身的方向键。
- JoyDeck 放映窗口：上键复用已有后退操作，下键复用已有前进操作，继续使用既有动画步骤、媒体清理和放映结束逻辑。

没有改动文档模型、格式保存、依赖版本或页面排版。来源清单保留原始来源指纹，只更新修改文件的适配指纹。

## 验证

验证日志位于本机 `target/ribbon-navigation-checks/`。产品工程入口分别执行 fmt、严格 Clippy 和产品测试：

```text
cargo xtask check joyword
cargo xtask check joydeck
cargo xtask check joysheet
cargo test --locked -p joyword-ui-gpui -p joyword-shell
cargo xtask build joyword
cargo xtask build joydeck
cargo xtask build joysheet
python scripts/joydeck/check_sources.py
python scripts/check_gates.py
python -m unittest scripts.tests.test_check_gates scripts.tests.test_dependency_gates
python scripts/check_docs.py
```

| 检查 | 结果 |
| --- | --- |
| JoyWord 产品检查 | fmt、生产库/二进制严格 Clippy 通过；测试累计 2,978 通过、1 失败、2 忽略，停在既有分页断言，不能记为全量通过 |
| JoyWord UI/Shell 专项 | 123 通过、0 失败，含 GPUI 适配器测试 |
| JoyDeck 产品检查 | fmt、all-targets 严格 Clippy、来源/依赖/文档门禁通过；974 测试通过、0 失败 |
| JoySheet 产品检查 | fmt、all-targets 严格 Clippy 通过；1,420 测试通过、0 失败、1 项既有计时测试忽略 |
| 普通构建 | 三个产品及各自 CLI 的 `cargo xtask build` 全部通过 |
| 工程及文档 | 291 项来源指纹、工程门禁、61 项 Python 门禁回归、文档检查通过 |

结构化结果、源码与可执行文件指纹见 [checks.json](checks.json) 和 [source-inputs.json](source-inputs.json)。完整原始日志保留于上述本机目录。

### JoyWord 基线分页失败

`tests::pages::a_page_ends_when_a_multiple_spaced_lines_text_no_longer_fits` 在 `joyword-layout/src/tests/pages.rs:939` 失败：`1.15 shift 0: the paragraph should run onto a second page`。在独立、未修改的 main `55b15071` worktree 中用同一工具链单独运行该测试，得到同一断言失败，见 [main 复现日志](word-main-layout-baseline.log)。本轮未修改排版代码或放宽断言；完整产品检查与相关专项通过分别记录。

### 实屏与平台边界

首轮抓图返回 Windows 系统锁屏，已停止实屏输入。用户随后明确回复“已解锁”，本轮从相同二进制继续验证，以下交互在实际 Windows 原生窗口中通过。使用 Computer Use 的 SendInput 和窗口截图，不将合成输入表述为物理键盘/鼠标认证。未保存或提交锁屏画面。

| 实屏场景 | 观察结果与截图 |
| --- | --- |
| JoyWord 开始标签 | 双击隐藏工具区，文档顶部上移 88 逻辑像素；再次双击恢复。[展开](gui/word-home-expanded.jpg) · [收起](gui/word-home-collapsed.jpg) · [恢复](gui/word-home-restored.jpg) |
| JoyWord 文件标签 | 文件页顶部标签始终可见，双击后显示文档，文件仍为活动标签；再次双击恢复后台。收起时可输入 `RibbonProbe` 并撤销，未保存测试副本。[收起](gui/word-file-collapsed.jpg) · [输入](gui/word-file-collapsed-edit.jpg) · [恢复](gui/word-file-restored.jpg) |
| JoyDeck 开始标签 | 双击隐藏 75 逻辑像素的工具区，缩略图与画布上移；再次双击恢复。[展开](gui/deck-home-expanded.jpg) · [收起](gui/deck-home-collapsed.jpg) · [恢复](gui/deck-home-restored.jpg) |
| JoyDeck 文件标签 | 双击收起文件后台，再次双击恢复；切回开始后正常显示编辑工具。[收起](gui/deck-file-collapsed.jpg) · [恢复](gui/deck-file-restored.jpg) |
| JoyDeck 编辑模式 | 在 20 页 PPTX 中按下键由第 1 页（标题标识 20）切到第 2 页（标识 19），上键返回第 1 页。备注输入框聚焦时，下键保留输入焦点且不切页。[下键](gui/deck-editor-down.jpg) · [上键](gui/deck-editor-up.jpg) · [输入框保护](gui/deck-notes-arrow-guard.jpg) |
| JoyDeck 放映模式 | 下键第 1→2 页，上键第 2→1 页；首页再次按上键保持第 1 页，Esc 正常结束放映。[下键](gui/deck-show-down.jpg) · [上键](gui/deck-show-up.jpg) · [首页边界](gui/deck-show-first-boundary.jpg) |
| JoySheet 开始标签 | 双击隐藏工具区，公式栏和网格上移 101 逻辑像素；再次双击恢复。[展开](gui/sheet-home-expanded.jpg) · [收起](gui/sheet-home-collapsed.jpg) · [恢复](gui/sheet-home-restored.jpg) |
| JoySheet 文件标签 | 双击显示网格，再次双击恢复文件页；收起期间可向 A1 输入 `RibbonProbe`，公式栏和可访问性值同步。[收起](gui/sheet-file-collapsed.jpg) · [输入](gui/sheet-file-collapsed-edit.jpg) · [恢复](gui/sheet-file-restored.jpg) |
| JoySheet F6 | 折叠状态下由网格依次切到公式栏、地址框和始终可见的 Ribbon 标签栏。[功能区焦点](gui/sheet-collapsed-f6.jpg) |

29 份截图及相应可访问性树索引见 [gui-checks.json](gui-checks.json)。放映原生日志见 [deck-ui.log](deck-ui.log)。Word 与 Deck 均使用仓库语料的独立副本，验证后磁盘散列与原语料一致；Sheet 使用独立配置下的新空白工作簿，没有写入用户文档。此表只验收列出的常规窗口和键鼠交互，不替代全部上下文选项卡、复杂动画/媒体、字体像素一致性或完整辅助功能验收。

macOS/Linux、实际 DPI/读屏、信创真机均未进行本轮实测，不据此关闭其他 UI 对齐或平台验收任务。

整合时发现原 Sheet 分支已使用共享 `joyoffice-cfb` 写入嵌入对象，但依赖层规则未登记；本轮只为 `joysheet-xlsx` 补齐该项允许依赖，未放宽其他产品层边界。工程门禁与 61 项相关 Python 回归通过。
