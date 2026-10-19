# 修订

## 使用者

审阅功能区：

| 命令 | 界面文字 |
| --- | --- |
| `review.track` | 跟踪修订 |
| `review.author` | 修订作者 |
| `review.accept` | 接受 |
| `review.reject` | 拒绝 |
| `review.accept_all` | 全部接受 |
| `review.reject_all` | 全部拒绝 |
| `review.view.markup` | 最终：显示标记 |
| `review.view.final` | 最终 |
| `review.view.original` | 原始 |

已支持的子集：

- 打开跟踪修订后，键入是插入，删除是删除，作者来自首选项。只开跟踪、没有强制修订时，文档仍可编辑。
- 三种视图会改变页面上的文字。最终：显示标记在改过的行左边画作者色条。
- 接受或拒绝一处是一步。切换修订模式会封存当前的连续键入，使撤销不跨模式。
- 全部接受落到最终文字，全部拒绝落到原始文字，各是一步。
- 成对的移动：接受或拒绝任意一端会同时处理两端。交叉或不完整的移动对整篇不改。
- 已类型化单元格里的文字插入和删除可以往返，并带上作者。
- 页眉和页脚里的文字修订同样画下划线、删除线和页边色条。最终和原始视图不画这些装饰。

## 开发者

文字修订的写出按 ECMA-376 §17.13.5（[structure.rs](../../../crates/joyword-docx/src/export/structure.rs)）。引擎测试 [engine.rs](../../../crates/joyword-engine/tests/engine.rs) 的 `user_tracking_accept_and_reject_undo_and_a_mode_switch_seals_typing`，以及 [revisions.rs](../../../crates/joyword-engine/tests/revisions.rs) 的 `accept_all_and_reject_all_restore_the_move_projections`、`accepting_either_move_end_resolves_the_pair_and_leaves_other_revisions`、`a_move_run_that_crosses_its_marker_is_refused`。

排版 [document.rs](../../../crates/joyword-layout/tests/document.rs)：`revision_views_change_pagination_text_and_match_the_incremental_cache`、`markup_paints_a_margin_bar_beside_a_changed_line`、`move_markup_paints_both_ends_and_the_projections_do_not`。页眉页脚在 [header_revisions.rs](../../../crates/joyword-layout/tests/header_revisions.rs)。命令在 [review.rs](../../../crates/joyword-shell/tests/review.rs) 的 `review_commands_use_the_registry_and_the_author_preference`。

作者色是按作者名散列的 8 色。

## 限制

- 任务原文里的四种视图，目前只有标记、最终、原始三种命令。
- `rPrChange` 和表格结构修订仍使对应范围只读。那是 T301，还没开始。
- 在已有修订内部继续编辑没有专门测试。
- 带图片的页眉或页脚段落不画修订装饰。
- 色板不是 Word 的修订色。
- 与 Word 或 WPS 交换修订、逐项和批量接受拒绝的人工验收未在本环境执行。

## JoyWord 假定，没有 Word 观察

- 页边色条用 8 色作者散列。实施记录写明这不是 Word 的修订色。
